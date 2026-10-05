//! Transactional email delivery client via Resend HTTP API.
//!
//! Provides transactional email sending (welcome emails, verification codes,
//! subscription notifications, security alerts) with support for API key
//! authentication and graceful degradation when email service is unconfigured.

use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const DEFAULT_RESEND_API_URL: &str = "https://api.resend.com/emails";

#[derive(Debug, thiserror::Error)]
pub enum EmailError {
    #[error("Email service not configured: {0}")]
    NotConfigured(String),
    #[error("HTTP request error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("Resend API rejected request ({status}): {message}")]
    ApiError { status: u16, message: String },
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendEmailRequest {
    pub from: String,
    pub to: Vec<String>,
    pub subject: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub html: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendEmailResponse {
    pub id: String,
}

#[derive(Clone)]
pub struct ResendMailer {
    api_key: String,
    from_email: String,
    api_url: String,
    http_client: reqwest::Client,
}

impl ResendMailer {
    /// Creates a new `ResendMailer` instance with explicit credentials.
    pub fn new(api_key: impl Into<String>, from_email: impl Into<String>) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap_or_default();

        Self {
            api_key: api_key.into(),
            from_email: from_email.into(),
            api_url: DEFAULT_RESEND_API_URL.to_string(),
            http_client,
        }
    }

    /// Creates a `ResendMailer` from environment variables:
    /// - `RESEND_API_KEY`: Required API key from Resend (e.g. `re_...`).
    /// - `RESEND_FROM_EMAIL`: Optional sender address (default: `OSMOO <noreply@osmoo.in>`).
    pub fn from_env() -> Result<Self, EmailError> {
        let api_key = std::env::var("RESEND_API_KEY")
            .map_err(|_| EmailError::NotConfigured("RESEND_API_KEY environment variable not set".into()))?;

        if api_key.trim().is_empty() {
            return Err(EmailError::NotConfigured("RESEND_API_KEY is empty".into()));
        }

        let from_email = std::env::var("RESEND_FROM_EMAIL")
            .unwrap_or_else(|_| "OSMOO <noreply@osmoo.in>".to_string());

        Ok(Self::new(api_key, from_email))
    }

    /// Sends a transactional email.
    pub async fn send(
        &self,
        to: &[&str],
        subject: &str,
        html_body: Option<&str>,
        text_body: Option<&str>,
    ) -> Result<SendEmailResponse, EmailError> {
        let req = SendEmailRequest {
            from: self.from_email.clone(),
            to: to.iter().map(|s| s.to_string()).collect(),
            subject: subject.to_string(),
            html: html_body.map(|s| s.to_string()),
            text: text_body.map(|s| s.to_string()),
        };

        let response = self
            .http_client
            .post(&self.api_url)
            .bearer_auth(&self.api_key)
            .json(&req)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(EmailError::ApiError {
                status: status.as_u16(),
                message: error_text,
            });
        }

        let resp: SendEmailResponse = response.json().await?;
        Ok(resp)
    }

    /// Helper for sending an account verification or welcome email.
    pub async fn send_welcome(&self, recipient_email: &str, user_name: Option<&str>) -> Result<SendEmailResponse, EmailError> {
        let name = user_name.unwrap_or("Explorer");
        let subject = "Welcome to OSMOO by Osmiora";
        let html = format!(
            "<h1>Welcome to OSMOO, {}!</h1><p>Your OSMOO companion by Osmiora is ready. Open your OSMOO application on Windows to get started. Visit us at osmoo.in or follow @IamOSMOO.</p>",
            name
        );
        let text = format!(
            "Welcome to OSMOO, {}!\n\nYour OSMOO companion by Osmiora is ready. Open your OSMOO application on Windows to get started. Visit us at osmoo.in or follow @IamOSMOO.",
            name
        );
        self.send(&[recipient_email], subject, Some(&html), Some(&text)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static ENV_LOCK: parking_lot::Mutex<()> = parking_lot::Mutex::new(());

    #[test]
    fn test_from_env_missing_key() {
        let _guard = ENV_LOCK.lock();
        std::env::remove_var("RESEND_API_KEY");
        let result = ResendMailer::from_env();
        assert!(result.is_err(), "Expected error when RESEND_API_KEY is not set");
        match result {
            Err(EmailError::NotConfigured(msg)) => {
                assert!(msg.contains("RESEND_API_KEY"));
            }
            _ => panic!("Expected NotConfigured error"),
        }
    }

    #[test]
    fn test_from_env_with_key() {
        let _guard = ENV_LOCK.lock();
        std::env::set_var("RESEND_API_KEY", "re_test_dummy_key_123");
        std::env::set_var("RESEND_FROM_EMAIL", "test@voxy.ai");

        let mailer = ResendMailer::from_env().expect("Should construct mailer from env");
        assert_eq!(mailer.api_key, "re_test_dummy_key_123");
        assert_eq!(mailer.from_email, "test@voxy.ai");

        // Cleanup
        std::env::remove_var("RESEND_API_KEY");
        std::env::remove_var("RESEND_FROM_EMAIL");
    }
}
