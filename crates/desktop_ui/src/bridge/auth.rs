use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
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

#[cfg(windows)]
mod dpapi {
    use std::ptr;

    #[repr(C)]
    struct DataBlob {
        cb_data: u32,
        pb_data: *mut u8,
    }

    #[link(name = "crypt32")]
    extern "system" {
        fn CryptProtectData(
            pDataIn: *const DataBlob,
            szDataDescr: *const u16,
            pOptionalEntropy: *const DataBlob,
            pvReserved: *mut std::ffi::c_void,
            pPromptStruct: *mut std::ffi::c_void,
            dwFlags: u32,
            pDataOut: *mut DataBlob,
        ) -> i32;

        fn CryptUnprotectData(
            pDataIn: *const DataBlob,
            ppszDataDescr: *mut *mut u16,
            pOptionalEntropy: *const DataBlob,
            pvReserved: *mut std::ffi::c_void,
            pPromptStruct: *mut std::ffi::c_void,
            dwFlags: u32,
            pDataOut: *mut DataBlob,
        ) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn LocalFree(hMem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    }

    const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x1;

    pub fn protect(data: &[u8]) -> Result<Vec<u8>, String> {
        if data.is_empty() {
            return Ok(Vec::new());
        }
        let in_blob = DataBlob {
            cb_data: data.len() as u32,
            pb_data: data.as_ptr() as *mut u8,
        };
        let mut out_blob = DataBlob {
            cb_data: 0,
            pb_data: ptr::null_mut(),
        };

        unsafe {
            let res = CryptProtectData(
                &in_blob,
                ptr::null(),
                ptr::null(),
                ptr::null_mut(),
                ptr::null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out_blob,
            );
            if res == 0 {
                return Err(format!(
                    "CryptProtectData failed: {}",
                    std::io::Error::last_os_error()
                ));
            }

            let slice = std::slice::from_raw_parts(out_blob.pb_data, out_blob.cb_data as usize);
            let protected = slice.to_vec();
            LocalFree(out_blob.pb_data as *mut std::ffi::c_void);
            Ok(protected)
        }
    }

    pub fn unprotect(data: &[u8]) -> Result<Vec<u8>, String> {
        if data.is_empty() {
            return Ok(Vec::new());
        }
        let in_blob = DataBlob {
            cb_data: data.len() as u32,
            pb_data: data.as_ptr() as *mut u8,
        };
        let mut out_blob = DataBlob {
            cb_data: 0,
            pb_data: ptr::null_mut(),
        };

        unsafe {
            let res = CryptUnprotectData(
                &in_blob,
                ptr::null_mut(),
                ptr::null(),
                ptr::null_mut(),
                ptr::null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out_blob,
            );
            if res == 0 {
                return Err(format!(
                    "CryptUnprotectData failed: {}",
                    std::io::Error::last_os_error()
                ));
            }

            let slice = std::slice::from_raw_parts(out_blob.pb_data, out_blob.cb_data as usize);
            let unprotected = slice.to_vec();
            LocalFree(out_blob.pb_data as *mut std::ffi::c_void);
            Ok(unprotected)
        }
    }
}

#[cfg(not(windows))]
mod dpapi {
    pub fn protect(data: &[u8]) -> Result<Vec<u8>, String> {
        Ok(data.to_vec())
    }
    pub fn unprotect(data: &[u8]) -> Result<Vec<u8>, String> {
        Ok(data.to_vec())
    }
}

pub struct AuthBridge {
    router: Arc<ApiRouter>,
    session: Arc<RwLock<Option<AuthSession>>>,
    session_file: PathBuf,
}

impl AuthBridge {
    pub fn new(router: Arc<ApiRouter>) -> Self {
        let session_file = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("voxy")
            .join("session.json");
        Self::with_session_path(router, session_file)
    }

    pub fn with_session_path(router: Arc<ApiRouter>, session_file: PathBuf) -> Self {
        let loaded_session = if session_file.exists() {
            match std::fs::read(&session_file) {
                Ok(raw_bytes) => {
                    // Try DPAPI decryption first, fall back to plaintext for legacy sessions
                    let decrypted = dpapi::unprotect(&raw_bytes).unwrap_or(raw_bytes);
                    match serde_json::from_slice::<AuthSession>(&decrypted) {
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
                    }
                }
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
                match dpapi::protect(json.as_bytes()) {
                    Ok(encrypted_bytes) => {
                        let _ = std::fs::write(&self.session_file, encrypted_bytes);
                    }
                    Err(e) => {
                        tracing::warn!("DPAPI encryption unavailable: {e}; writing fallback");
                        let _ = std::fs::write(&self.session_file, json);
                    }
                }
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

    pub async fn google_auth(
        &self,
        id_token: &str,
        access_token: Option<String>,
    ) -> Result<AuthSession, String> {
        let req_body = voxy_api_server::GoogleAuthRequest {
            id_token: id_token.to_string(),
            access_token,
        };

        let req = ApiRequest::new("POST", "/auth/google", "127.0.0.1").with_json(&req_body);
        let res = self.router.dispatch(req).await;

        if res.status_code == 200 {
            #[derive(Deserialize)]
            struct Envelope {
                data: Option<LoginResponse>,
                error: Option<serde_json::Value>,
            }

            let env: Envelope = serde_json::from_slice(&res.body)
                .map_err(|e| format!("Failed to parse Google login response: {e}"))?;

            if let Some(data) = env.data {
                let tier = self
                    .fetch_active_tier(&data.token)
                    .await
                    .unwrap_or_else(|_| "free".to_string());
                let sess = AuthSession {
                    user_id: data.user_id,
                    email: "google-authenticated".to_string(),
                    token: data.token,
                    active_tier: tier,
                    expires_at: data.expires_at,
                };
                self.set_session(Some(sess.clone()));
                Ok(sess)
            } else {
                Err(format!("Google authentication error: {:?}", env.error))
            }
        } else {
            let err_msg: serde_json::Value = serde_json::from_slice(&res.body)
                .unwrap_or_else(|_| serde_json::json!({"error": "Unknown error"}));
            Err(format!(
                "Google sign-in failed ({}): {}",
                res.status_code, err_msg
            ))
        }
    }

    pub async fn request_otp(&self, email: &str, purpose: &str) -> Result<String, String> {
        let req_body = voxy_api_server::RequestOtpRequest {
            email: email.to_string(),
            purpose: purpose.to_string(),
        };

        let req = ApiRequest::new("POST", "/auth/otp/request", "127.0.0.1").with_json(&req_body);
        let res = self.router.dispatch(req).await;

        if res.status_code == 200 {
            #[derive(Deserialize)]
            struct Envelope {
                data: Option<voxy_api_server::RequestOtpResponse>,
                error: Option<serde_json::Value>,
            }
            let env: Envelope = serde_json::from_slice(&res.body)
                .map_err(|e| format!("Failed to parse OTP response: {e}"))?;
            if let Some(data) = env.data {
                Ok(data.message)
            } else {
                Err(format!("OTP error: {:?}", env.error))
            }
        } else {
            let err_msg: serde_json::Value = serde_json::from_slice(&res.body)
                .unwrap_or_else(|_| serde_json::json!({"error": "Unknown error"}));
            Err(format!(
                "OTP request failed ({}): {}",
                res.status_code, err_msg
            ))
        }
    }

    pub async fn verify_otp(
        &self,
        email: &str,
        code: &str,
        purpose: &str,
    ) -> Result<Option<AuthSession>, String> {
        let req_body = voxy_api_server::VerifyOtpRequest {
            email: email.to_string(),
            code: code.to_string(),
            purpose: purpose.to_string(),
        };

        let req = ApiRequest::new("POST", "/auth/otp/verify", "127.0.0.1").with_json(&req_body);
        let res = self.router.dispatch(req).await;

        if res.status_code == 200 {
            #[derive(Deserialize)]
            struct Envelope {
                data: Option<voxy_api_server::VerifyOtpResponse>,
                error: Option<serde_json::Value>,
            }
            let env: Envelope = serde_json::from_slice(&res.body)
                .map_err(|e| format!("Failed to parse verify OTP response: {e}"))?;
            if let Some(data) = env.data {
                if let Some(login_data) = data.session {
                    let tier = self
                        .fetch_active_tier(&login_data.token)
                        .await
                        .unwrap_or_else(|_| "free".to_string());
                    let sess = AuthSession {
                        user_id: login_data.user_id,
                        email: email.to_string(),
                        token: login_data.token,
                        active_tier: tier,
                        expires_at: login_data.expires_at,
                    };
                    self.set_session(Some(sess.clone()));
                    Ok(Some(sess))
                } else {
                    Ok(None)
                }
            } else {
                Err(format!("OTP verify error: {:?}", env.error))
            }
        } else {
            let err_msg: serde_json::Value = serde_json::from_slice(&res.body)
                .unwrap_or_else(|_| serde_json::json!({"error": "Unknown error"}));
            Err(format!(
                "OTP verification failed ({}): {}",
                res.status_code, err_msg
            ))
        }
    }

    pub async fn reset_password(
        &self,
        email: &str,
        code: &str,
        new_password: &str,
    ) -> Result<String, String> {
        let req_body = voxy_api_server::ResetPasswordRequest {
            email: email.to_string(),
            code: code.to_string(),
            new_password: new_password.to_string(),
        };

        let req = ApiRequest::new("POST", "/auth/password/reset", "127.0.0.1").with_json(&req_body);
        let res = self.router.dispatch(req).await;

        if res.status_code == 200 {
            #[derive(Deserialize)]
            struct Envelope {
                data: Option<voxy_api_server::ResetPasswordResponse>,
                error: Option<serde_json::Value>,
            }
            let env: Envelope = serde_json::from_slice(&res.body)
                .map_err(|e| format!("Failed to parse reset password response: {e}"))?;
            if let Some(data) = env.data {
                Ok(data.message)
            } else {
                Err(format!("Reset password error: {:?}", env.error))
            }
        } else {
            let err_msg: serde_json::Value = serde_json::from_slice(&res.body)
                .unwrap_or_else(|_| serde_json::json!({"error": "Unknown error"}));
            Err(format!(
                "Password reset failed ({}): {}",
                res.status_code, err_msg
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_session_serialization_roundtrip() {
        let session = AuthSession {
            user_id: "usr_12345".to_string(),
            email: "operator@osmoo.in".to_string(),
            token: "jwt_token_sample".to_string(),
            active_tier: "pro".to_string(),
            expires_at: 1893456000,
        };

        let serialized = serde_json::to_string(&session).expect("failed to serialize session");
        let deserialized: AuthSession =
            serde_json::from_str(&serialized).expect("failed to deserialize session");

        assert_eq!(deserialized.user_id, "usr_12345");
        assert_eq!(deserialized.email, "operator@osmoo.in");
        assert_eq!(deserialized.active_tier, "pro");
        assert_eq!(deserialized.expires_at, 1893456000);
    }

    #[test]
    fn test_dpapi_protect_and_unprotect_roundtrip() {
        let plaintext = b"{\"token\":\"secret-access-token-9876543210\"}";
        let protected = dpapi::protect(plaintext).expect("dpapi protect should succeed");

        #[cfg(windows)]
        {
            // Verify protected blob does NOT contain the raw plaintext secret token
            let protected_str = String::from_utf8_lossy(&protected);
            assert!(
                !protected_str.contains("secret-access-token-9876543210"),
                "Encrypted DPAPI blob must not contain raw token string"
            );
        }

        let unprotected = dpapi::unprotect(&protected).expect("dpapi unprotect should succeed");
        assert_eq!(unprotected, plaintext);
    }

    #[tokio::test]
    async fn test_auth_lifecycle_end_to_end() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let session_file = temp_dir.path().join("session.json");
        let db_path = temp_dir.path().join("commercial.db");

        let sqlite = Arc::new(voxy_database::SqliteDatabase::new());
        let db_cfg = voxy_database::DatabaseConfig {
            kind: voxy_database::DatabaseKind::Sqlite,
            path: Some(db_path.to_string_lossy().to_string()),
            ..Default::default()
        };
        use voxy_database::StorageProvider;
        sqlite.connect(&db_cfg).await.expect("db connect");
        let store = Arc::new(voxy_database::CommercialStore::new(sqlite));
        store.initialize_schema().await.expect("schema init");

        let auth_middleware = Arc::new(voxy_api_server::middleware::AuthMiddleware::new(
            store.clone(),
        ));
        let rate_limiter = Arc::new(voxy_api_server::middleware::RateLimitMiddleware::new(
            120,
            std::time::Duration::from_secs(60),
        ));
        let handlers = Arc::new(voxy_api_server::handlers::ApiHandlers::new(
            store, None, None,
        ));
        let router = Arc::new(voxy_api_server::router::ApiRouter::new(
            handlers,
            auth_middleware,
            rate_limiter,
        ));

        let auth_bridge = AuthBridge::with_session_path(router.clone(), session_file.clone());

        // 1. Initially unauthenticated
        assert!(!auth_bridge.is_authenticated());
        assert!(auth_bridge.current_session().is_none());

        // 2. Register new operator
        let reg_res = auth_bridge
            .register(
                "operator@osmoo.io",
                "SuperSecurePass123!",
                Some("Operator".to_string()),
            )
            .await;
        assert!(reg_res.is_ok(), "Registration must succeed: {:?}", reg_res);

        // 3. Login with wrong password fails
        let bad_login = auth_bridge
            .login("operator@osmoo.io", "WrongPassword!")
            .await;
        assert!(bad_login.is_err(), "Invalid password must fail closed");
        assert!(!auth_bridge.is_authenticated());

        // 4. Login with correct credentials succeeds and saves session
        let good_login = auth_bridge
            .login("operator@osmoo.io", "SuperSecurePass123!")
            .await;
        assert!(good_login.is_ok(), "Login must succeed: {:?}", good_login);
        assert!(auth_bridge.is_authenticated());
        assert!(
            session_file.exists(),
            "Session file must be written to disk"
        );

        // 5. Session restoration from disk
        let restored_bridge = AuthBridge::with_session_path(router.clone(), session_file.clone());
        assert!(
            restored_bridge.is_authenticated(),
            "Session must restore across restarts"
        );
        let sess = restored_bridge.current_session().unwrap();
        assert_eq!(sess.email, "operator@osmoo.io");

        // 6. Corrupt session file handling
        let corrupt_dir = tempfile::tempdir().expect("tempdir");
        let corrupt_session_file = corrupt_dir.path().join("corrupt_session.json");
        std::fs::write(
            &corrupt_session_file,
            b"NOT_A_VALID_ENCRYPTED_OR_JSON_PAYLOAD",
        )
        .unwrap();
        let corrupt_bridge = AuthBridge::with_session_path(router.clone(), corrupt_session_file);
        assert!(
            !corrupt_bridge.is_authenticated(),
            "Corrupted session file must fail closed"
        );

        // 7. Expired session handling
        let expired_dir = tempfile::tempdir().expect("tempdir");
        let expired_session_file = expired_dir.path().join("expired_session.json");
        let expired_sess = AuthSession {
            user_id: "usr_expired".to_string(),
            email: "expired@osmoo.io".to_string(),
            token: "tok_expired".to_string(),
            active_tier: "free".to_string(),
            expires_at: chrono::Utc::now().timestamp() - 3600, // 1 hour ago
        };
        let expired_json = serde_json::to_string(&expired_sess).unwrap();
        let enc = dpapi::protect(expired_json.as_bytes()).unwrap();
        std::fs::write(&expired_session_file, enc).unwrap();

        let expired_bridge =
            AuthBridge::with_session_path(router.clone(), expired_session_file.clone());
        assert!(
            !expired_bridge.is_authenticated(),
            "Expired session must not be active"
        );
        assert!(
            !expired_session_file.exists(),
            "Expired session file must be automatically removed"
        );

        // 8. Logout invalidates in-memory and removes disk file
        let logout_res = auth_bridge.logout().await;
        assert!(logout_res.is_ok());
        assert!(!auth_bridge.is_authenticated());
        assert!(
            !session_file.exists(),
            "Session file must be deleted on logout"
        );
    }
}
