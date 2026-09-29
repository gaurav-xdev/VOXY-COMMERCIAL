//! Official Dodo Payments Client and Checkout API Adapter.
//!
//! Connects to Dodo Payments endpoints (production or test sandbox)
//! to generate hosted checkout sessions, query subscription status, and cancel subscriptions.

use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DodoError {
    #[error("HTTP request failed: {0}")]
    Network(#[from] reqwest::Error),

    #[error("API returned error ({status}): {message}")]
    ApiError { status: u16, message: String },

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Configuration error: {0}")]
    Config(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DodoEnvironment {
    Live,
    Test,
}

impl DodoEnvironment {
    pub fn base_url(&self) -> &'static str {
        match self {
            Self::Live => "https://api.dodopayments.com",
            Self::Test => "https://test.dodopayments.com",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BillingInterval {
    Month,
    Year,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductCartItem {
    pub product_id: String,
    pub quantity: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerPayload {
    pub email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCheckoutSessionRequest {
    pub product_cart: Vec<ProductCartItem>,
    pub customer: CustomerPayload,
    pub return_url: String,
    pub metadata: std::collections::HashMap<String, String>,
}

impl CreateCheckoutSessionRequest {
    pub fn single_product(
        product_id: String,
        customer_email: String,
        customer_name: Option<String>,
        return_url: String,
        user_id: String,
    ) -> Self {
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("user_id".to_string(), user_id);

        Self {
            product_cart: vec![ProductCartItem {
                product_id,
                quantity: 1,
            }],
            customer: CustomerPayload {
                email: customer_email,
                name: customer_name,
                customer_id: None,
            },
            return_url,
            metadata,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutSessionResponse {
    pub checkout_id: String,
    pub checkout_url: String,
    pub customer_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DodoSubscriptionResponse {
    pub subscription_id: String,
    pub customer_id: String,
    pub product_id: String,
    pub status: String,
    pub next_billing_date: Option<String>,
    pub current_period_end: Option<String>,
    pub current_period_start: Option<String>,
    pub cancel_at_period_end: Option<bool>,
    pub metadata: Option<std::collections::HashMap<String, String>>,
}

#[derive(Clone)]
pub struct DodoPaymentsClient {
    client: reqwest::Client,
    environment: DodoEnvironment,
    #[allow(dead_code)]
    api_key: String,
}

impl DodoPaymentsClient {
    pub fn new(api_key: String, environment: DodoEnvironment) -> Result<Self, DodoError> {
        if api_key.trim().is_empty() {
            return Err(DodoError::Config("Dodo API key cannot be empty".into()));
        }

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let auth_val = format!("Bearer {}", api_key.trim());
        let mut auth_header = HeaderValue::from_str(&auth_val)
            .map_err(|e| DodoError::Config(format!("Invalid auth header: {}", e)))?;
        auth_header.set_sensitive(true);
        headers.insert(AUTHORIZATION, auth_header);

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        Ok(Self {
            client,
            environment,
            api_key,
        })
    }

    /// Initializes a hosted checkout session with Dodo Payments.
    pub async fn create_checkout_session(
        &self,
        req: &CreateCheckoutSessionRequest,
    ) -> Result<CheckoutSessionResponse, DodoError> {
        let url = format!("{}/checkouts", self.environment.base_url());
        let res = self.client.post(&url).json(req).send().await?;

        if !res.status().is_success() {
            let status = res.status().as_u16();
            let text = res.text().await.unwrap_or_default();
            return Err(DodoError::ApiError {
                status,
                message: text,
            });
        }

        let data = res.json::<CheckoutSessionResponse>().await?;
        Ok(data)
    }

    /// Queries live subscription details from Dodo Payments.
    pub async fn get_subscription(
        &self,
        subscription_id: &str,
    ) -> Result<DodoSubscriptionResponse, DodoError> {
        let url = format!("{}/subscriptions/{}", self.environment.base_url(), subscription_id);
        let res = self.client.get(&url).send().await?;

        if !res.status().is_success() {
            let status = res.status().as_u16();
            let text = res.text().await.unwrap_or_default();
            return Err(DodoError::ApiError {
                status,
                message: text,
            });
        }

        let data = res.json::<DodoSubscriptionResponse>().await?;
        Ok(data)
    }

    /// Cancels a subscription via Dodo Payments API.
    pub async fn cancel_subscription(&self, subscription_id: &str) -> Result<(), DodoError> {
        let url = format!("{}/subscriptions/{}/cancel", self.environment.base_url(), subscription_id);
        let res = self.client.post(&url).send().await?;

        if !res.status().is_success() {
            let status = res.status().as_u16();
            let text = res.text().await.unwrap_or_default();
            return Err(DodoError::ApiError {
                status,
                message: text,
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dodo_client_creation_and_config() {
        let client = DodoPaymentsClient::new("test_dodo_key_12345".into(), DodoEnvironment::Test);
        assert!(client.is_ok());

        let empty = DodoPaymentsClient::new("".into(), DodoEnvironment::Live);
        assert!(empty.is_err());
    }

    #[test]
    fn test_dodo_environments() {
        assert_eq!(DodoEnvironment::Live.base_url(), "https://api.dodopayments.com");
        assert_eq!(DodoEnvironment::Test.base_url(), "https://test.dodopayments.com");
    }
}
