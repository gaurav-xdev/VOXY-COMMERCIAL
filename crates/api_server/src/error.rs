use chrono::Utc;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),

    #[error("Internal server error: {0}")]
    Internal(String),
}

impl ApiError {
    pub fn status_code(&self) -> u16 {
        match self {
            Self::BadRequest(_) => 400,
            Self::Unauthorized(_) => 401,
            Self::Forbidden(_) => 403,
            Self::NotFound(_) => 404,
            Self::RateLimitExceeded(_) => 429,
            Self::Internal(_) => 500,
        }
    }

    pub fn error_code(&self) -> &'static str {
        match self {
            Self::BadRequest(_) => "BAD_REQUEST",
            Self::Unauthorized(_) => "UNAUTHORIZED",
            Self::Forbidden(_) => "FORBIDDEN",
            Self::NotFound(_) => "NOT_FOUND",
            Self::RateLimitExceeded(_) => "RATE_LIMIT_EXCEEDED",
            Self::Internal(_) => "INTERNAL_ERROR",
        }
    }

    pub fn to_error_body(&self, request_id: Option<&str>) -> ApiErrorBody {
        ApiErrorBody {
            code: self.error_code().to_string(),
            message: self.to_string(),
            request_id: request_id
                .unwrap_or(&Uuid::new_v4().to_string())
                .to_string(),
            timestamp: Utc::now().to_rfc3339(),
        }
    }
}

/// Standardized structured error response body for all commercial API endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorBody {
    pub code: String,
    pub message: String,
    pub request_id: String,
    pub timestamp: String,
}

/// Standard API response envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponseEnvelope<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiErrorBody>,
}

impl<T: Serialize> ApiResponseEnvelope<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(error: ApiErrorBody) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error),
        }
    }
}
