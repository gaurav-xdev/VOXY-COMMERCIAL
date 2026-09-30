use crate::error::{ApiError, ApiResponseEnvelope};
use crate::handlers::{
    ApiHandlers, ConsentRequest, CreateCheckoutRequest, CreateCheckoutResponse,
    EntitlementCheckRequest, LoginRequest, RegisterRequest,
};
use crate::middleware::{AuthMiddleware, RateLimitMiddleware};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;

/// Abstract HTTP-like request structure for API dispatch.
#[derive(Debug, Clone)]
pub struct ApiRequest {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub client_ip: String,
    pub request_id: String,
}

impl ApiRequest {
    pub fn new(method: &str, path: &str, client_ip: &str) -> Self {
        Self {
            method: method.to_uppercase(),
            path: path.to_string(),
            headers: HashMap::new(),
            body: Vec::new(),
            client_ip: client_ip.to_string(),
            request_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    pub fn with_header(mut self, key: &str, value: &str) -> Self {
        self.headers.insert(key.to_lowercase(), value.to_string());
        self
    }

    pub fn with_json<T: Serialize>(mut self, body: &T) -> Self {
        self.body = serde_json::to_vec(body).unwrap_or_default();
        self.headers
            .insert("content-type".to_string(), "application/json".to_string());
        self
    }
}

/// Abstract HTTP-like response structure returned by API router.
#[derive(Debug, Clone)]
pub struct ApiResponse {
    pub status_code: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl ApiResponse {
    pub fn json<T: Serialize>(status_code: u16, envelope: &ApiResponseEnvelope<T>) -> Self {
        let mut headers = HashMap::new();
        headers.insert("content-type".to_string(), "application/json".to_string());
        let body = serde_json::to_vec(envelope).unwrap_or_default();
        Self {
            status_code,
            headers,
            body,
        }
    }

    pub fn error(error: ApiError, request_id: &str) -> Self {
        let status = error.status_code();
        let body = error.to_error_body(Some(request_id));
        let envelope: ApiResponseEnvelope<()> = ApiResponseEnvelope::err(body);
        Self::json(status, &envelope)
    }
}

/// Commercial API Router that processes incoming requests through rate-limiting,
/// authentication middleware, and route dispatching.
pub struct ApiRouter {
    handlers: Arc<ApiHandlers>,
    auth_middleware: Arc<AuthMiddleware>,
    rate_limiter: Arc<RateLimitMiddleware>,
}

impl ApiRouter {
    pub fn new(
        handlers: Arc<ApiHandlers>,
        auth_middleware: Arc<AuthMiddleware>,
        rate_limiter: Arc<RateLimitMiddleware>,
    ) -> Self {
        Self {
            handlers,
            auth_middleware,
            rate_limiter,
        }
    }

    /// Dispatch an incoming API request to the appropriate route handler.
    pub async fn dispatch(&self, req: ApiRequest) -> ApiResponse {
        // 1. Rate limiting check (per IP address)
        if let Err(e) = self.rate_limiter.check(&req.client_ip) {
            return ApiResponse::error(e, &req.request_id);
        }

        let method = req.method.as_str();
        let path = req.path.as_str();

        match (method, path) {
            // Health endpoint (public)
            ("GET", "/health") => match self.handlers.health().await {
                Ok(data) => ApiResponse::json(200, &ApiResponseEnvelope::ok(data)),
                Err(e) => ApiResponse::error(e, &req.request_id),
            },

            // Registration (public)
            ("POST", "/auth/register") => {
                let reg_req: RegisterRequest = match serde_json::from_slice(&req.body) {
                    Ok(b) => b,
                    Err(e) => {
                        return ApiResponse::error(
                            ApiError::BadRequest(format!("Malformed JSON: {e}")),
                            &req.request_id,
                        );
                    }
                };
                match self.handlers.register(reg_req).await {
                    Ok(data) => ApiResponse::json(201, &ApiResponseEnvelope::ok(data)),
                    Err(e) => ApiResponse::error(e, &req.request_id),
                }
            }

            // Login (public)
            ("POST", "/auth/login") => {
                let login_req: LoginRequest = match serde_json::from_slice(&req.body) {
                    Ok(b) => b,
                    Err(e) => {
                        return ApiResponse::error(
                            ApiError::BadRequest(format!("Malformed JSON: {e}")),
                            &req.request_id,
                        );
                    }
                };
                match self.handlers.login(login_req).await {
                    Ok(data) => ApiResponse::json(200, &ApiResponseEnvelope::ok(data)),
                    Err(e) => ApiResponse::error(e, &req.request_id),
                }
            }

            // Logout (authenticated)
            ("POST", "/auth/logout") => {
                let auth_header = req.headers.get("authorization").map(|s| s.as_str());
                let auth_ctx = match self.auth_middleware.authenticate(auth_header).await {
                    Ok(ctx) => ctx,
                    Err(e) => return ApiResponse::error(e, &req.request_id),
                };
                match self.handlers.logout(&auth_ctx).await {
                    Ok(()) => ApiResponse::json(200, &ApiResponseEnvelope::ok("Logged out")),
                    Err(e) => ApiResponse::error(e, &req.request_id),
                }
            }

            // Entitlements Check (authenticated)
            ("POST", "/entitlements/check") => {
                let auth_header = req.headers.get("authorization").map(|s| s.as_str());
                let auth_ctx = match self.auth_middleware.authenticate(auth_header).await {
                    Ok(ctx) => ctx,
                    Err(e) => return ApiResponse::error(e, &req.request_id),
                };
                let check_req: EntitlementCheckRequest = match serde_json::from_slice(&req.body) {
                    Ok(b) => b,
                    Err(e) => {
                        return ApiResponse::error(
                            ApiError::BadRequest(format!("Malformed JSON: {e}")),
                            &req.request_id,
                        );
                    }
                };
                match self
                    .handlers
                    .check_entitlement(&auth_ctx, &check_req.feature)
                    .await
                {
                    Ok(data) => ApiResponse::json(200, &ApiResponseEnvelope::ok(data)),
                    Err(e) => ApiResponse::error(e, &req.request_id),
                }
            }

            // Record Consent (authenticated)
            ("POST", "/consent") => {
                let auth_header = req.headers.get("authorization").map(|s| s.as_str());
                let auth_ctx = match self.auth_middleware.authenticate(auth_header).await {
                    Ok(ctx) => ctx,
                    Err(e) => return ApiResponse::error(e, &req.request_id),
                };
                let consent_req: ConsentRequest = match serde_json::from_slice(&req.body) {
                    Ok(b) => b,
                    Err(e) => {
                        return ApiResponse::error(
                            ApiError::BadRequest(format!("Malformed JSON: {e}")),
                            &req.request_id,
                        );
                    }
                };
                match self.handlers.record_consent(&auth_ctx, consent_req).await {
                    Ok(()) => ApiResponse::json(200, &ApiResponseEnvelope::ok("Consent recorded")),
                    Err(e) => ApiResponse::error(e, &req.request_id),
                }
            }

            // Create Checkout Session (authenticated)
            ("POST", "/checkout/create-session") => {
                let auth_header = req.headers.get("authorization").map(|s| s.as_str());
                let auth_ctx = match self.auth_middleware.authenticate(auth_header).await {
                    Ok(ctx) => ctx,
                    Err(e) => return ApiResponse::error(e, &req.request_id),
                };
                let checkout_req: CreateCheckoutRequest = match serde_json::from_slice(&req.body) {
                    Ok(b) => b,
                    Err(e) => {
                        return ApiResponse::error(
                            ApiError::BadRequest(format!("Malformed JSON: {e}")),
                            &req.request_id,
                        );
                    }
                };
                match self
                    .handlers
                    .create_checkout_session(
                        &auth_ctx,
                        &checkout_req.plan_id,
                        &checkout_req.return_url,
                    )
                    .await
                {
                    Ok(url) => ApiResponse::json(
                        200,
                        &ApiResponseEnvelope::ok(CreateCheckoutResponse { checkout_url: url }),
                    ),
                    Err(e) => ApiResponse::error(e, &req.request_id),
                }
            }

            // Dodo Payments Webhook Endpoint (signed & verified)
            ("POST", "/webhooks/dodo") => {
                let sig_header = req
                    .headers
                    .get("webhook-signature")
                    .or_else(|| req.headers.get("x-dodo-signature"))
                    .map(|s| s.as_str());
                let ts_header = req
                    .headers
                    .get("webhook-timestamp")
                    .or_else(|| req.headers.get("x-dodo-timestamp"))
                    .map(|s| s.as_str());
                let id_header = req
                    .headers
                    .get("webhook-id")
                    .or_else(|| req.headers.get("x-dodo-event-id"))
                    .map(|s| s.as_str());

                match self
                    .handlers
                    .handle_webhook(sig_header, ts_header, id_header, &req.body)
                    .await
                {
                    Ok(msg) => ApiResponse::json(200, &ApiResponseEnvelope::ok(msg)),
                    Err(e) => ApiResponse::error(e, &req.request_id),
                }
            }

            _ => ApiResponse::error(
                ApiError::NotFound(format!("Route '{method} {path}' not found")),
                &req.request_id,
            ),
        }
    }
}
