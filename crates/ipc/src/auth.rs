use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityToken {
    pub id: Uuid,
    pub capabilities: Vec<CapabilityClaim>,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub issuer: String,
    pub subject: String,
    /// SECURITY: Signature is required. Tokens without a signature are invalid.
    pub signature: Vec<u8>,
}

/// Maximum allowed length for any individual string field in canonical payload (64 KB).
pub const MAX_STRING_FIELD_LEN: usize = 65535;
/// Maximum allowed capability claims per token in canonical payload.
pub const MAX_CAPABILITY_CLAIMS: usize = 1024;
/// Maximum allowed constraints per capability claim in canonical payload.
pub const MAX_CONSTRAINTS_PER_CLAIM: usize = 256;
/// Minimum HMAC-SHA256 key size (128 bits / 16 bytes).
pub const MIN_HMAC_KEY_LEN: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CapabilityClaim {
    pub capability: String,
    pub resource: Option<String>,
    pub constraints: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthRequest {
    pub method: AuthMethod,
    pub credentials: serde_json::Value,
    pub requested_capabilities: Vec<String>,
    pub client_version: String,
    pub client_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthMethod {
    Token,
    Certificate,
    ApiKey,
    OAuth2,
    MutualTls,
    Anonymous,
}

impl std::fmt::Display for AuthMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Token => write!(f, "token"),
            Self::Certificate => write!(f, "certificate"),
            Self::ApiKey => write!(f, "api_key"),
            Self::OAuth2 => write!(f, "oauth2"),
            Self::MutualTls => write!(f, "mutual_tls"),
            Self::Anonymous => write!(f, "anonymous"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    pub success: bool,
    pub token: Option<CapabilityToken>,
    pub session_id: Option<Uuid>,
    pub error: Option<String>,
    pub granted_capabilities: Vec<String>,
}

impl CapabilityToken {
    pub fn new(
        subject: impl Into<String>,
        issuer: impl Into<String>,
        capabilities: Vec<CapabilityClaim>,
        ttl_secs: u64,
        signature: Vec<u8>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            capabilities,
            issued_at: now,
            expires_at: now + chrono::Duration::seconds(ttl_secs as i64),
            issuer: issuer.into(),
            subject: subject.into(),
            signature,
        }
    }

    /// Issue and cryptographically sign a token with HMAC-SHA256 using `secret`.
    pub fn issue_signed(
        subject: impl Into<String>,
        issuer: impl Into<String>,
        capabilities: Vec<CapabilityClaim>,
        ttl_secs: u64,
        secret: &[u8],
    ) -> Self {
        let mut token = Self::new(subject, issuer, capabilities, ttl_secs, Vec::new());
        token.sign(secret);
        token
    }

    /// Compute canonical payload for cryptographic signing.
    ///
    /// Encodes all security-relevant fields unambiguously with deterministic ordering:
    /// - Token ID (UUID 16 bytes)
    /// - Issuer (length prefix + UTF-8 bytes, bounded to MAX_STRING_FIELD_LEN)
    /// - Subject (length prefix + UTF-8 bytes, bounded to MAX_STRING_FIELD_LEN)
    /// - Deterministically sorted and deduplicated capability claims (u32 LE count):
    ///   For each claim:
    ///   - Capability name (length prefix + UTF-8 bytes)
    ///   - Resource flag (0x01 if Some, 0x00 if None)
    ///   - Resource name if present (length prefix + UTF-8 bytes)
    ///   - Deterministically sorted and deduplicated constraints (u32 LE count):
    ///     - Constraint (length prefix + UTF-8 bytes)
    /// - Issued at timestamp (i64 LE)
    /// - Expires at timestamp (i64 LE)
    pub fn signature_payload(&self) -> Vec<u8> {
        let mut payload = Vec::new();
        // ID
        payload.extend_from_slice(self.id.as_bytes());

        // Issuer
        let issuer_bytes = self.issuer.as_bytes();
        let issuer_len = issuer_bytes.len().min(MAX_STRING_FIELD_LEN) as u32;
        payload.extend_from_slice(&issuer_len.to_le_bytes());
        payload.extend_from_slice(&issuer_bytes[..issuer_len as usize]);

        // Subject
        let subject_bytes = self.subject.as_bytes();
        let subject_len = subject_bytes.len().min(MAX_STRING_FIELD_LEN) as u32;
        payload.extend_from_slice(&subject_len.to_le_bytes());
        payload.extend_from_slice(&subject_bytes[..subject_len as usize]);

        // Canonical deterministic claims: sort and deduplicate
        let mut claims = self.capabilities.clone();
        for claim in &mut claims {
            claim.constraints.sort();
            claim.constraints.dedup();
        }
        claims.sort();
        claims.dedup();

        let claims_len = claims.len().min(MAX_CAPABILITY_CLAIMS) as u32;
        payload.extend_from_slice(&claims_len.to_le_bytes());

        for claim in claims.iter().take(claims_len as usize) {
            let cap_bytes = claim.capability.as_bytes();
            let cap_len = cap_bytes.len().min(MAX_STRING_FIELD_LEN) as u32;
            payload.extend_from_slice(&cap_len.to_le_bytes());
            payload.extend_from_slice(&cap_bytes[..cap_len as usize]);

            if let Some(ref r) = claim.resource {
                payload.push(0x01);
                let r_bytes = r.as_bytes();
                let r_len = r_bytes.len().min(MAX_STRING_FIELD_LEN) as u32;
                payload.extend_from_slice(&r_len.to_le_bytes());
                payload.extend_from_slice(&r_bytes[..r_len as usize]);
            } else {
                payload.push(0x00);
            }

            let constraints_len = claim.constraints.len().min(MAX_CONSTRAINTS_PER_CLAIM) as u32;
            payload.extend_from_slice(&constraints_len.to_le_bytes());
            for c in claim.constraints.iter().take(constraints_len as usize) {
                let c_bytes = c.as_bytes();
                let c_len = c_bytes.len().min(MAX_STRING_FIELD_LEN) as u32;
                payload.extend_from_slice(&c_len.to_le_bytes());
                payload.extend_from_slice(&c_bytes[..c_len as usize]);
            }
        }

        // Timestamps
        payload.extend_from_slice(&self.issued_at.timestamp().to_le_bytes());
        payload.extend_from_slice(&self.expires_at.timestamp().to_le_bytes());
        payload
    }

    /// Sign the token payload with HMAC-SHA256 using the provided secret key.
    pub fn sign(&mut self, secret: &[u8]) {
        if let Ok(mut mac) = HmacSha256::new_from_slice(secret) {
            mac.update(&self.signature_payload());
            self.signature = mac.finalize().into_bytes().to_vec();
        }
    }

    /// Cryptographically verify the token's HMAC-SHA256 signature against `secret`.
    pub fn verify_signature(&self, secret: &[u8]) -> bool {
        if self.signature.is_empty() || secret.is_empty() {
            return false;
        }
        if let Ok(mut mac) = HmacSha256::new_from_slice(secret) {
            mac.update(&self.signature_payload());
            mac.verify_slice(&self.signature).is_ok()
        } else {
            false
        }
    }

    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// SECURITY: Token is only valid if it has a signature and is not expired.
    pub fn is_valid(&self) -> bool {
        !self.is_expired() && !self.signature.is_empty()
    }

    /// Cryptographically verify token validity (not expired AND valid signature matching secret).
    pub fn is_valid_with_secret(&self, secret: &[u8]) -> bool {
        self.is_valid() && self.verify_signature(secret)
    }

    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|c| {
            if c.capability.ends_with(":*") {
                let prefix = &c.capability[..c.capability.len() - 2];
                capability.starts_with(prefix)
                    && capability.as_bytes().get(prefix.len()) == Some(&b':')
            } else {
                c.capability == capability
            }
        })
    }

    pub fn remaining_ttl_secs(&self) -> i64 {
        (self.expires_at - Utc::now()).num_seconds().max(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_token_creation() {
        let claims = vec![CapabilityClaim {
            capability: "voice:transcribe".to_string(),
            resource: None,
            constraints: vec![],
        }];
        let token = CapabilityToken::new("user-1", "system", claims, 3600, vec![0xAB]);
        assert_eq!(token.subject, "user-1");
        assert_eq!(token.issuer, "system");
        assert!(!token.is_expired());
        assert!(token.is_valid());
        assert!(token.has_capability("voice:transcribe"));
        assert!(!token.has_capability("admin:*"));
    }

    #[test]
    fn capability_token_expiry() {
        let claims = vec![CapabilityClaim {
            capability: "test".to_string(),
            resource: None,
            constraints: vec![],
        }];
        let token = CapabilityToken::new("user", "system", claims, 0, vec![0xAB]);
        assert!(token.is_expired());
        assert!(!token.is_valid());
    }

    #[test]
    fn capability_token_unsigned_invalid() {
        let claims = vec![CapabilityClaim {
            capability: "test".to_string(),
            resource: None,
            constraints: vec![],
        }];
        let token = CapabilityToken::new("user", "system", claims, 3600, vec![]);
        assert!(!token.is_valid(), "Unsigned token must be invalid");
    }

    #[test]
    fn auth_method_display() {
        assert_eq!(AuthMethod::Token.to_string(), "token");
        assert_eq!(AuthMethod::MutualTls.to_string(), "mutual_tls");
    }

    #[test]
    fn auth_response_success() {
        let response = AuthResponse {
            success: true,
            token: Some(CapabilityToken::new(
                "user",
                "system",
                vec![],
                3600,
                vec![0xAB],
            )),
            session_id: Some(Uuid::new_v4()),
            error: None,
            granted_capabilities: vec!["voice:transcribe".to_string()],
        };
        assert!(response.success);
        assert!(response.error.is_none());
        assert_eq!(response.granted_capabilities.len(), 1);
    }

    #[test]
    fn auth_response_failure() {
        let response = AuthResponse {
            success: false,
            token: None,
            session_id: None,
            error: Some("Invalid credentials".to_string()),
            granted_capabilities: vec![],
        };
        assert!(!response.success);
        assert_eq!(response.error.unwrap(), "Invalid credentials");
    }

    #[test]
    fn capability_token_remaining_ttl() {
        let claims = vec![CapabilityClaim {
            capability: "test".to_string(),
            resource: None,
            constraints: vec![],
        }];
        let token = CapabilityToken::new("user", "system", claims, 3600, vec![0xAB]);
        let remaining = token.remaining_ttl_secs();
        assert!(remaining > 3590 && remaining <= 3600);
    }

    #[test]
    fn capability_claim_constraints() {
        let claim = CapabilityClaim {
            capability: "storage:write".to_string(),
            resource: Some("namespace:user-data".to_string()),
            constraints: vec!["max_size:10MB".to_string()],
        };
        assert_eq!(claim.resource.unwrap(), "namespace:user-data");
        assert!(claim.constraints.contains(&"max_size:10MB".to_string()));
    }

    #[test]
    fn auth_request_serialization() {
        let req = AuthRequest {
            method: AuthMethod::ApiKey,
            credentials: serde_json::json!({"key": "sk-123"}),
            requested_capabilities: vec!["voice:transcribe".to_string()],
            client_version: "1.0.0".to_string(),
            client_name: "voxy-cli".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        let restored: AuthRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.method, AuthMethod::ApiKey);
        assert_eq!(restored.client_name, "voxy-cli");
    }

    #[test]
    fn capability_token_hmac_sign_and_verify() {
        let secret = b"super-secret-hmac-key-for-ipc-tokens-32b!";
        let claims = vec![CapabilityClaim {
            capability: "audio:capture".to_string(),
            resource: None,
            constraints: vec![],
        }];
        let token = CapabilityToken::issue_signed("client-a", "voxy-auth", claims, 3600, secret);

        assert!(token.is_valid());
        assert!(token.verify_signature(secret));
        assert!(token.is_valid_with_secret(secret));
        assert!(token.has_capability("audio:capture"));
        assert!(!token.has_capability("network:write"));
    }

    #[test]
    fn capability_token_tampered_claim_rejected() {
        let secret = b"super-secret-hmac-key-for-ipc-tokens-32b!";
        let claims = vec![CapabilityClaim {
            capability: "audio:capture".to_string(),
            resource: None,
            constraints: vec![],
        }];
        let mut token =
            CapabilityToken::issue_signed("client-a", "voxy-auth", claims, 3600, secret);

        // Tamper by injecting root/admin capability
        token.capabilities.push(CapabilityClaim {
            capability: "admin:*".to_string(),
            resource: None,
            constraints: vec![],
        });

        assert!(
            !token.verify_signature(secret),
            "Tampered claim must fail HMAC verification"
        );
        assert!(!token.is_valid_with_secret(secret));
    }

    #[test]
    fn capability_token_wrong_secret_rejected() {
        let secret_a = b"super-secret-hmac-key-for-ipc-tokens-32b!";
        let secret_b = b"attacker-secret-hmac-key-for-tokens-32b!";
        let claims = vec![CapabilityClaim {
            capability: "audio:capture".to_string(),
            resource: None,
            constraints: vec![],
        }];
        let token = CapabilityToken::issue_signed("client-a", "voxy-auth", claims, 3600, secret_a);

        assert!(token.verify_signature(secret_a));
        assert!(!token.verify_signature(secret_b));
    }

    #[test]
    fn capability_token_tampered_resource_rejected() {
        let secret = b"super-secret-hmac-key-for-ipc-tokens-32b!";
        let claims = vec![CapabilityClaim {
            capability: "file:read".to_string(),
            resource: Some("logs/today.txt".to_string()),
            constraints: vec![],
        }];
        let mut token =
            CapabilityToken::issue_signed("client-a", "voxy-auth", claims, 3600, secret);

        assert!(token.verify_signature(secret));

        // Tamper with resource to target sensitive system file
        token.capabilities[0].resource = Some("/etc/shadow".to_string());
        assert!(
            !token.verify_signature(secret),
            "Tampered resource must fail verification"
        );

        // Tamper by removing resource
        token.capabilities[0].resource = None;
        assert!(
            !token.verify_signature(secret),
            "Removed resource must fail verification"
        );
    }

    #[test]
    fn capability_token_tampered_constraints_rejected() {
        let secret = b"super-secret-hmac-key-for-ipc-tokens-32b!";
        let claims = vec![CapabilityClaim {
            capability: "file:write".to_string(),
            resource: Some("scratch/temp.txt".to_string()),
            constraints: vec!["max_size:1MB".to_string(), "read_only:false".to_string()],
        }];
        let mut token =
            CapabilityToken::issue_signed("client-a", "voxy-auth", claims, 3600, secret);

        assert!(token.verify_signature(secret));

        // Tamper with constraints (e.g. remove restriction)
        token.capabilities[0].constraints.remove(0);
        assert!(
            !token.verify_signature(secret),
            "Removed constraint must fail verification"
        );

        // Tamper with constraint text
        token.capabilities[0]
            .constraints
            .push("max_size:10GB".to_string());
        assert!(
            !token.verify_signature(secret),
            "Altered constraint must fail verification"
        );
    }

    #[test]
    fn capability_token_tampered_issuer_subject_or_timestamps_rejected() {
        let secret = b"super-secret-hmac-key-for-ipc-tokens-32b!";
        let claims = vec![CapabilityClaim {
            capability: "audio:capture".to_string(),
            resource: None,
            constraints: vec![],
        }];
        let token = CapabilityToken::issue_signed("client-a", "voxy-auth", claims, 3600, secret);

        assert!(token.verify_signature(secret));

        // Tamper subject
        let mut t1 = token.clone();
        t1.subject = "client-b".to_string();
        assert!(
            !t1.verify_signature(secret),
            "Tampered subject must fail verification"
        );

        // Tamper issuer
        let mut t2 = token.clone();
        t2.issuer = "rogue-auth".to_string();
        assert!(
            !t2.verify_signature(secret),
            "Tampered issuer must fail verification"
        );

        // Tamper expires_at (attempt lifetime extension)
        let mut t3 = token.clone();
        t3.expires_at = t3.expires_at + chrono::Duration::hours(24);
        assert!(
            !t3.verify_signature(secret),
            "Tampered expiry must fail verification"
        );

        // Tamper ID
        let mut t4 = token.clone();
        t4.id = uuid::Uuid::new_v4();
        assert!(
            !t4.verify_signature(secret),
            "Tampered ID must fail verification"
        );
    }

    #[test]
    fn canonical_payload_deterministic_ordering_and_deduplication() {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let expires = now + chrono::Duration::hours(1);

        // Token 1: out-of-order claims, duplicate claims, duplicate and out-of-order constraints
        let token1 = CapabilityToken {
            id,
            issuer: "voxy-auth".to_string(),
            subject: "client-a".to_string(),
            capabilities: vec![
                CapabilityClaim {
                    capability: "file:write".to_string(),
                    resource: Some("temp.txt".to_string()),
                    constraints: vec!["write".to_string(), "read".to_string(), "write".to_string()],
                },
                CapabilityClaim {
                    capability: "audio:capture".to_string(),
                    resource: None,
                    constraints: vec![],
                },
                CapabilityClaim {
                    capability: "file:write".to_string(),
                    resource: Some("temp.txt".to_string()),
                    constraints: vec!["read".to_string(), "write".to_string()],
                },
            ],
            issued_at: now,
            expires_at: expires,
            signature: vec![],
        };

        // Token 2: clean canonically ordered, deduplicated claims and constraints
        let token2 = CapabilityToken {
            id,
            issuer: "voxy-auth".to_string(),
            subject: "client-a".to_string(),
            capabilities: vec![
                CapabilityClaim {
                    capability: "audio:capture".to_string(),
                    resource: None,
                    constraints: vec![],
                },
                CapabilityClaim {
                    capability: "file:write".to_string(),
                    resource: Some("temp.txt".to_string()),
                    constraints: vec!["read".to_string(), "write".to_string()],
                },
            ],
            issued_at: now,
            expires_at: expires,
            signature: vec![],
        };

        // Both tokens MUST produce the exact same canonical signature payload
        assert_eq!(
            token1.signature_payload(),
            token2.signature_payload(),
            "Canonical serialization must be strictly deterministic across duplicates and ordering"
        );
    }

    #[test]
    fn canonical_payload_bounds_oversized_fields_safely() {
        let oversized_string = "A".repeat(100_000); // 100 KB string (exceeds 64 KB cap)
        let token = CapabilityToken {
            id: Uuid::new_v4(),
            issuer: oversized_string.clone(),
            subject: "client".to_string(),
            capabilities: vec![CapabilityClaim {
                capability: oversized_string,
                resource: None,
                constraints: vec![],
            }],
            issued_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::hours(1),
            signature: vec![],
        };

        // Must compute payload without panic or overflow, capping length prefix to MAX_STRING_FIELD_LEN
        let payload = token.signature_payload();
        assert!(!payload.is_empty());
    }
}
