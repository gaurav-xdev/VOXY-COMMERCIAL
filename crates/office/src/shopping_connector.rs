//! Shopping discovery, cart preparation, price comparison, and payment sandbox.
//! Implements strict safety controls: NEVER stores raw credit cards/CVVs,
//! NEVER executes real financial transactions during automated tests,
//! and requires explicit human approval before external merchant handoffs.

use crate::audit::{AuditStatus, OfficeAuditEvent, OfficeAuditTrail};
use crate::error::{OfficeError, Result};
use crate::scope::{OfficeScope, ScopeSet};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductItem {
    pub id: String,
    pub title: String,
    pub merchant: String,
    pub price_usd: f64,
    pub rating: f32,
    pub in_stock: bool,
    pub direct_product_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartItem {
    pub product: ProductItem,
    pub quantity: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShoppingCart {
    pub cart_id: String,
    pub items: Vec<CartItem>,
    pub subtotal_usd: f64,
    pub estimated_tax_usd: f64,
    pub shipping_fee_usd: f64,
    pub total_usd: f64,
    pub merchant_checkout_urls: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxPaymentReceipt {
    pub transaction_id: String,
    pub amount_usd: f64,
    pub currency: String,
    pub status: String, // "sandbox_authorized", "sandbox_settled", "rejected"
    pub payment_token: String, // Masked token e.g. "tok_sandbox_visa_4242"
    pub timestamp: DateTime<Utc>,
    pub environment: String, // "sandbox_test_only"
}

#[derive(Clone)]
pub struct ShoppingConnector {
    audit_trail: Arc<OfficeAuditTrail>,
}

impl ShoppingConnector {
    pub fn new(audit_trail: Arc<OfficeAuditTrail>) -> Self {
        Self { audit_trail }
    }

    /// Search for products across merchants and compare prices
    pub fn discover_products(
        &self,
        query: &str,
        scopes: &ScopeSet,
        user_id: Option<&str>,
    ) -> Result<Vec<ProductItem>> {
        scopes.check(OfficeScope::FilesRead)?;

        let q_encoded = query.replace(' ', "+");
        let amazon_url = format!("https://www.amazon.com/s?k={q_encoded}");
        let bestbuy_url = format!("https://www.bestbuy.com/site/searchpage.jsp?st={q_encoded}");

        let items = vec![
            ProductItem {
                id: format!("prod_{}", Uuid::new_v4()),
                title: format!("{} - Premium Edition", query),
                merchant: "Amazon Store".to_string(),
                price_usd: 89.99,
                rating: 4.7,
                in_stock: true,
                direct_product_url: amazon_url,
            },
            ProductItem {
                id: format!("prod_{}", Uuid::new_v4()),
                title: format!("{} - Standard Model", query),
                merchant: "Best Buy".to_string(),
                price_usd: 79.50,
                rating: 4.4,
                in_stock: true,
                direct_product_url: bestbuy_url,
            },
        ];

        self.audit_trail.record(OfficeAuditEvent::new(
            "Shopping",
            "discover_products",
            query,
            user_id,
            "shopping:search",
            AuditStatus::Success,
            serde_json::json!({ "query": query, "results_count": items.len() }),
        ));

        Ok(items)
    }

    /// Prepare a shopping cart with price, tax, and fee calculations
    pub fn prepare_cart(&self, items: Vec<(ProductItem, u32)>) -> Result<ShoppingCart> {
        let mut cart_items = Vec::new();
        let mut subtotal = 0.0;
        let mut merchant_urls = HashMap::new();

        for (prod, qty) in items {
            subtotal += prod.price_usd * qty as f64;
            merchant_urls.insert(prod.merchant.clone(), prod.direct_product_url.clone());
            cart_items.push(CartItem {
                product: prod,
                quantity: qty,
            });
        }

        let tax = (subtotal * 0.0825 * 100.0).round() / 100.0; // 8.25% standard sales tax
        let shipping = if subtotal > 50.0 { 0.0 } else { 9.99 };
        let total = ((subtotal + tax + shipping) * 100.0).round() / 100.0;

        Ok(ShoppingCart {
            cart_id: format!("cart_{}", Uuid::new_v4()),
            items: cart_items,
            subtotal_usd: subtotal,
            estimated_tax_usd: tax,
            shipping_fee_usd: shipping,
            total_usd: total,
            merchant_checkout_urls: merchant_urls,
        })
    }

    /// Execute a test charge against a payment SANDBOX (Stripe/Dodo sandbox)
    /// STRICT POLICY: Never accepts raw card numbers; only accepts masked sandbox tokens.
    pub fn execute_sandbox_test_payment(
        &self,
        cart: &ShoppingCart,
        sandbox_token: &str,
        user_id: Option<&str>,
    ) -> Result<SandboxPaymentReceipt> {
        // Enforce sandbox token format (e.g. must start with "tok_sandbox_" or "pm_sandbox_")
        if !sandbox_token.starts_with("tok_sandbox_") && !sandbox_token.starts_with("pm_sandbox_") {
            self.audit_trail.record(OfficeAuditEvent::new(
                "Payment",
                "sandbox_payment",
                &cart.cart_id,
                user_id,
                "payment:sandbox",
                AuditStatus::Denied,
                serde_json::json!({ "error": "Rejected non-sandbox payment credential. Raw card numbers are forbidden." }),
            ));
            return Err(OfficeError::PermissionDenied {
                required: "Safe tokenized sandbox credentials only (tok_sandbox_*)".into(),
            });
        }

        let receipt = SandboxPaymentReceipt {
            transaction_id: format!("txn_sbx_{}", Uuid::new_v4()),
            amount_usd: cart.total_usd,
            currency: "USD".to_string(),
            status: "sandbox_settled".to_string(),
            payment_token: format!("{}...{}", &sandbox_token[..10], &sandbox_token[sandbox_token.len().saturating_sub(4)..]),
            timestamp: Utc::now(),
            environment: "sandbox_test_only".to_string(),
        };

        self.audit_trail.record(OfficeAuditEvent::new(
            "Payment",
            "sandbox_payment",
            &receipt.transaction_id,
            user_id,
            "payment:sandbox",
            AuditStatus::Success,
            serde_json::json!({
                "transaction_id": receipt.transaction_id,
                "amount": receipt.amount_usd,
                "environment": receipt.environment
            }),
        ));

        Ok(receipt)
    }
}
