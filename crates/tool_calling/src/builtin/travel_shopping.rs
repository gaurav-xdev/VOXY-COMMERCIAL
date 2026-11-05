//! Native Travel and Shopping tools for flight/hotel search, product discovery,
//! and secure tokenized sandbox payment validation.

use crate::error::{Result, ToolError};
use crate::metadata::{RiskTier, ToolCategory, ToolContext, ToolMetadata, ToolResult};
use crate::traits::Tool;
use async_trait::async_trait;
use chrono::NaiveDate;
use serde_json::json;
use std::sync::Arc;
use voxy_office::{OfficeAuditTrail, ScopeSet, ShoppingConnector, TravelConnector};

// Shared TravelConnector and ShoppingConnector instances
static TRAVEL_CONNECTOR: parking_lot::RwLock<Option<Arc<TravelConnector>>> =
    parking_lot::RwLock::new(None);
static SHOPPING_CONNECTOR: parking_lot::RwLock<Option<Arc<ShoppingConnector>>> =
    parking_lot::RwLock::new(None);

fn get_travel_connector() -> Arc<TravelConnector> {
    let mut lock = TRAVEL_CONNECTOR.write();
    if let Some(ref conn) = *lock {
        conn.clone()
    } else {
        let audit = Arc::new(OfficeAuditTrail::new(200));
        let conn = Arc::new(TravelConnector::new(audit));
        *lock = Some(conn.clone());
        conn
    }
}

fn get_shopping_connector() -> Arc<ShoppingConnector> {
    let mut lock = SHOPPING_CONNECTOR.write();
    if let Some(ref conn) = *lock {
        conn.clone()
    } else {
        let audit = Arc::new(OfficeAuditTrail::new(200));
        let conn = Arc::new(ShoppingConnector::new(audit));
        *lock = Some(conn.clone());
        conn
    }
}

/// Tool to search flight fares with honest provider comparisons
pub struct TravelSearchFlightsTool {
    metadata: ToolMetadata,
}

impl TravelSearchFlightsTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "travel_search_flights",
                "Search flight options with baggage rules, cancellation terms, and verified provider booking links",
                ToolCategory::System,
                RiskTier::Read,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["origin", "destination", "date"],
                "properties": {
                    "origin": { "type": "string", "description": "Origin airport code (e.g. SFO, JFK)" },
                    "destination": { "type": "string", "description": "Destination airport code (e.g. LHR, HND)" },
                    "date": { "type": "string", "description": "Departure date in YYYY-MM-DD format" },
                    "passengers": { "type": "integer", "description": "Number of passengers (default 1)", "default": 1 }
                }
            })),
        }
    }
}

impl Default for TravelSearchFlightsTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for TravelSearchFlightsTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> Result<ToolResult> {
        let origin = args
            .get("origin")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'origin' parameter".to_string()))?;

        let destination = args
            .get("destination")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'destination' parameter".to_string()))?;

        let date_str = args
            .get("date")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'date' parameter".to_string()))?;

        let passengers = args
            .get("passengers")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as u32;

        let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d").map_err(|e| {
            ToolError::InvalidParams(format!("Invalid date format (must be YYYY-MM-DD): {e}"))
        })?;

        let connector = get_travel_connector();
        let scopes = ScopeSet::full_access();
        let flights = connector
            .search_flights(origin, destination, date, passengers, &scopes, Some(ctx.session_id.as_str()))
            .map_err(|e| ToolError::ExecutionFailed(format!("travel_search_flights failed: {e}")))?;

        Ok(ToolResult::success(json!({
            "origin": origin,
            "destination": destination,
            "date": date_str,
            "passengers": passengers,
            "count": flights.len(),
            "flights": flights,
        })))
    }
}

/// Tool to search hotel options with nightly rates and verified booking URLs
pub struct TravelSearchHotelsTool {
    metadata: ToolMetadata,
}

impl TravelSearchHotelsTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "travel_search_hotels",
                "Search hotel accommodations with nightly rates, ratings, and free cancellation dates",
                ToolCategory::System,
                RiskTier::Read,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["city", "check_in", "check_out"],
                "properties": {
                    "city": { "type": "string", "description": "Destination city name (e.g. Tokyo, London, New York)" },
                    "check_in": { "type": "string", "description": "Check-in date in YYYY-MM-DD format" },
                    "check_out": { "type": "string", "description": "Check-out date in YYYY-MM-DD format" }
                }
            })),
        }
    }
}

impl Default for TravelSearchHotelsTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for TravelSearchHotelsTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> Result<ToolResult> {
        let city = args
            .get("city")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'city' parameter".to_string()))?;

        let check_in_str = args
            .get("check_in")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'check_in' parameter".to_string()))?;

        let check_out_str = args
            .get("check_out")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'check_out' parameter".to_string()))?;

        let check_in = NaiveDate::parse_from_str(check_in_str, "%Y-%m-%d").map_err(|e| {
            ToolError::InvalidParams(format!("Invalid check_in format (must be YYYY-MM-DD): {e}"))
        })?;

        let check_out = NaiveDate::parse_from_str(check_out_str, "%Y-%m-%d").map_err(|e| {
            ToolError::InvalidParams(format!("Invalid check_out format (must be YYYY-MM-DD): {e}"))
        })?;

        let connector = get_travel_connector();
        let scopes = ScopeSet::full_access();
        let hotels = connector
            .search_hotels(city, check_in, check_out, &scopes, Some(ctx.session_id.as_str()))
            .map_err(|e| ToolError::ExecutionFailed(format!("travel_search_hotels failed: {e}")))?;

        Ok(ToolResult::success(json!({
            "city": city,
            "check_in": check_in_str,
            "check_out": check_out_str,
            "count": hotels.len(),
            "hotels": hotels,
        })))
    }
}

/// Tool to discover products across merchants with prices and availability
pub struct ShoppingDiscoverProductsTool {
    metadata: ToolMetadata,
}

impl ShoppingDiscoverProductsTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "shopping_discover_products",
                "Discover products across trusted merchants with ratings, pricing, and stock status",
                ToolCategory::System,
                RiskTier::Read,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["query"],
                "properties": {
                    "query": { "type": "string", "description": "Product search query or keyword" },
                    "max_results": { "type": "integer", "description": "Maximum number of items to return", "default": 5 }
                }
            })),
        }
    }
}

impl Default for ShoppingDiscoverProductsTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ShoppingDiscoverProductsTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> Result<ToolResult> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'query' parameter".to_string()))?;

        let max_results = args
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(5) as usize;

        let connector = get_shopping_connector();
        let scopes = ScopeSet::full_access();
        let mut products = connector
            .discover_products(query, &scopes, Some(ctx.session_id.as_str()))
            .map_err(|e| ToolError::ExecutionFailed(format!("shopping_discover_products failed: {e}")))?;

        products.truncate(max_results);

        Ok(ToolResult::success(json!({
            "query": query,
            "count": products.len(),
            "products": products,
        })))
    }
}

/// Tool to execute a secure sandbox test payment (strictly prohibits raw cards; sandbox tokens only)
pub struct PaymentExecuteSandboxTestTool {
    metadata: ToolMetadata,
}

impl PaymentExecuteSandboxTestTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "payment_execute_sandbox_test",
                "Execute an automated sandbox test payment using a tokenized sandbox payment instrument (tok_sandbox_*)",
                ToolCategory::System,
                RiskTier::Privileged,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["token", "amount"],
                "properties": {
                    "cart_id": { "type": "string", "description": "Optional shopping cart reference ID" },
                    "token": { "type": "string", "description": "Tokenized test payment instrument (must begin with 'tok_sandbox_')" },
                    "amount": { "type": "number", "description": "Transaction amount in USD" }
                }
            })),
        }
    }
}

impl Default for PaymentExecuteSandboxTestTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for PaymentExecuteSandboxTestTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> Result<ToolResult> {
        let token = args
            .get("token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'token' parameter".to_string()))?;

        let amount = args
            .get("amount")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'amount' parameter".to_string()))?;

        let connector = get_shopping_connector();
        let cart = connector.prepare_cart(vec![(
            voxy_office::ProductItem {
                id: uuid::Uuid::new_v4().to_string(),
                title: "Sandbox Test Order".to_string(),
                merchant: "OSMOO Sandbox Shop".to_string(),
                price_usd: amount,
                rating: 5.0,
                in_stock: true,
                direct_product_url: "https://sandbox.checkout.local".to_string(),
            },
            1,
        )])
        .map_err(|e| ToolError::ExecutionFailed(format!("prepare_cart failed: {e}")))?;

        let receipt = connector
            .execute_sandbox_test_payment(&cart, token, Some(ctx.session_id.as_str()))
            .map_err(|e| ToolError::ExecutionFailed(format!("payment_execute_sandbox_test failed: {e}")))?;

        Ok(ToolResult::success(json!({
            "status": "success",
            "receipt": receipt,
        })))
    }
}
