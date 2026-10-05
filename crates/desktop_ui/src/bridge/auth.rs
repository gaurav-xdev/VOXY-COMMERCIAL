use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use voxy_api_server::handlers::{
    EntitlementCheckRequest, EntitlementCheckResponse, LoginRequest, LoginResponse,
    RegisterRequest, RegisterResponse,
};
use voxy_api_server::router::{ApiRequest, ApiRouter};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSession {
    pub user_id: String,
    pub email: String,
    pub token: String,
    pub active_tier: String,
    pub expires_at: i64,
}

#[derive(Clone)]
pub struct AuthBridge {
    router: Arc<ApiRouter>,
    session: Arc<RwLock<Option<AuthSession>>>,
    session_file: std::path::PathBuf,
}

impl AuthBridge {
    fn session_path() -> std::path::PathBuf {
        dirs::config_dir()
            .or_else(dirs::data_local_dir)
            .map(|p| p.join("voxy"))
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("auth_session.json")
    }

    pub fn new(router: Arc<ApiRouter>) -> Self {
        let session_file = Self::session_path();
        let loaded_session = if session_file.exists() {
            match std::fs::read_to_string(&session_file) {
                Ok(content) => match serde_json::from_str::<AuthSession>(&content) {
                    Ok(sess) => {
                        let now = chrono::Utc::now().timestamp();
                        if sess.expires_at > now {
                            tracing::info!("Restored active OSMOO session for {}", sess.email);
                            Some(sess)
                        } else {
                            tracing::info!("Stored OSMOO session expired, clearing");
                            let _ = std::fs::remove_file(&session_file);
                            None
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse session file: {e}");
                        None
                    }
                },
                Err(e) => {
                    tracing::warn!("Failed to read session file: {e}");
                    None
                }
            }
        } else {
            None
        };

        Self {
            router,
            session: Arc::new(RwLock::new(loaded_session)),
            session_file,
        }
    }

    pub fn current_session(&self) -> Option<AuthSession> {
        self.session.read().clone()
    }

    pub fn is_authenticated(&self) -> bool {
        if let Some(sess) = self.session.read().as_ref() {
            let now = chrono::Utc::now().timestamp();
            sess.expires_at > now
        } else {
            false
        }
    }

    pub fn set_session(&self, session: Option<AuthSession>) {
        if let Some(ref sess) = session {
            if let Some(parent) = self.session_file.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string(sess) {
                let _ = std::fs::write(&self.session_file, json);
            }
        } else {
            let _ = std::fs::remove_file(&self.session_file);
        }
        *self.session.write() = session;
    }

    pub async fn login(&self, email: &str, password: &str) -> Result<AuthSession, String> {
        let req_body = LoginRequest {
            email: email.to_string(),
            password: password.to_string(),
        };

        let req = ApiRequest::new("POST", "/auth/login", "127.0.0.1").with_json(&req_body);

        let res = self.router.dispatch(req).await;
        if res.status_code == 200 {
            #[derive(Deserialize)]
            struct Envelope {
                data: Option<LoginResponse>,
                error: Option<serde_json::Value>,
            }

            let env: Envelope = serde_json::from_slice(&res.body)
                .map_err(|e| format!("Failed to parse login response: {e}"))?;

            if let Some(data) = env.data {
                // Fetch entitlement/tier for user
                let tier = self
                    .fetch_active_tier(&data.token)
                    .await
                    .unwrap_or_else(|_| "free".to_string());

                let sess = AuthSession {
                    user_id: data.user_id,
                    email: email.to_string(),
                    token: data.token,
                    active_tier: tier,
                    expires_at: data.expires_at,
                };
                self.set_session(Some(sess.clone()));
                Ok(sess)
            } else {
                Err(format!("Login error: {:?}", env.error))
            }
        } else {
            let err_msg: serde_json::Value = serde_json::from_slice(&res.body)
                .unwrap_or_else(|_| serde_json::json!({"error": "Unknown error"}));
            Err(format!("Login failed ({}): {}", res.status_code, err_msg))
        }
    }

    pub async fn register(
        &self,
        email: &str,
        password: &str,
        name: Option<String>,
    ) -> Result<RegisterResponse, String> {
        let req_body = RegisterRequest {
            email: email.to_string(),
            password: password.to_string(),
            name,
        };

        let req = ApiRequest::new("POST", "/auth/register", "127.0.0.1").with_json(&req_body);

        let res = self.router.dispatch(req).await;
        if res.status_code == 201 {
            #[derive(Deserialize)]
            struct Envelope {
                data: Option<RegisterResponse>,
                error: Option<serde_json::Value>,
            }

            let env: Envelope = serde_json::from_slice(&res.body)
                .map_err(|e| format!("Failed to parse register response: {e}"))?;

            if let Some(data) = env.data {
                Ok(data)
            } else {
                Err(format!("Registration error: {:?}", env.error))
            }
        } else {
            let err_msg: serde_json::Value = serde_json::from_slice(&res.body)
                .unwrap_or_else(|_| serde_json::json!({"error": "Unknown error"}));
            Err(format!(
                "Registration failed ({}): {}",
                res.status_code, err_msg
            ))
        }
    }

    pub async fn logout(&self) -> Result<(), String> {
        let token = {
            let sess = self.session.read();
            sess.as_ref().map(|s| s.token.clone())
        };

        if let Some(tok) = token {
            let req = ApiRequest::new("POST", "/auth/logout", "127.0.0.1")
                .with_header("authorization", &format!("Bearer {}", tok));
            let _ = self.router.dispatch(req).await;
        }

        self.set_session(None);
        Ok(())
    }

    pub async fn check_entitlement(
        &self,
        feature: &str,
    ) -> Result<EntitlementCheckResponse, String> {
        let token = {
            let sess = self.session.read();
            sess.as_ref().map(|s| s.token.clone())
        }
        .ok_or_else(|| "Not authenticated".to_string())?;

        let req_body = EntitlementCheckRequest {
            feature: feature.to_string(),
        };

        let req = ApiRequest::new("POST", "/entitlements/check", "127.0.0.1")
            .with_header("authorization", &format!("Bearer {}", token))
            .with_json(&req_body);

        let res = self.router.dispatch(req).await;
        if res.status_code == 200 {
            #[derive(Deserialize)]
            struct Envelope {
                data: Option<EntitlementCheckResponse>,
                error: Option<serde_json::Value>,
            }

            let env: Envelope = serde_json::from_slice(&res.body)
                .map_err(|e| format!("Failed to parse entitlement check response: {e}"))?;

            if let Some(data) = env.data {
                // Update cached tier in session
                if let Some(mut s) = self.session.read().clone() {
                    s.active_tier = data.active_tier.clone();
                    *self.session.write() = Some(s);
                }
                Ok(data)
            } else {
                Err(format!("Entitlement check error: {:?}", env.error))
            }
        } else {
            Err(format!(
                "Entitlement check failed: status {}",
                res.status_code
            ))
        }
    }

    pub async fn fetch_active_tier(&self, token: &str) -> Result<String, String> {
        let req_body = EntitlementCheckRequest {
            feature: "coding_harness".to_string(),
        };

        let req = ApiRequest::new("POST", "/entitlements/check", "127.0.0.1")
            .with_header("authorization", &format!("Bearer {}", token))
            .with_json(&req_body);

        let res = self.router.dispatch(req).await;
        if res.status_code == 200 {
            #[derive(Deserialize)]
            struct Envelope {
                data: Option<EntitlementCheckResponse>,
            }
            let env: Envelope = serde_json::from_slice(&res.body).map_err(|e| e.to_string())?;
            if let Some(data) = env.data {
                Ok(data.active_tier)
            } else {
                Ok("free".to_string())
            }
        } else {
            Ok("free".to_string())
        }
    }
}
