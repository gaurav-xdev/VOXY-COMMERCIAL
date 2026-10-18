//! Capability tokens and permission grants.

use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

/// A token granting specific capabilities.
/// SECURITY: All tokens must have a valid cryptographic signature to be authenticated.
/// Unsigned or tampered tokens are rejected to prevent capability spoofing.
#[derive(Debug, Clone)]
pub struct CapabilityToken {
    pub id: Uuid,
    pub subject: String,
    pub capabilities: Vec<String>,
    pub issued_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub signature: Vec<u8>,
}

impl CapabilityToken {
    /// Create a token with a pre-computed signature or explicit raw bytes.
    pub fn new(subject: impl Into<String>, capabilities: Vec<String>, signature: Vec<u8>) -> Self {
        Self {
            id: Uuid::new_v4(),
            subject: subject.into(),
            capabilities,
            issued_at: Utc::now(),
            expires_at: None,
            signature,
        }
    }

    /// Issue and cryptographically sign a token with HMAC-SHA256 using the provided secret key.
    pub fn issue_signed(
        subject: impl Into<String>,
        capabilities: Vec<String>,
        expires_at: Option<DateTime<Utc>>,
        secret: &[u8],
    ) -> Self {
        let mut token = Self {
            id: Uuid::new_v4(),
            subject: subject.into(),
            capabilities,
            issued_at: Utc::now(),
            expires_at,
            signature: Vec::new(),
        };
        token.sign(secret);
        token
    }

    /// Set expiration.
    pub fn with_expiration(mut self, expires_at: DateTime<Utc>) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    /// Compute the canonical binary representation used for signing and verification.
    pub fn signature_payload(&self) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(self.id.as_bytes());
        payload.extend_from_slice(self.subject.as_bytes());
        for cap in &self.capabilities {
            payload.extend_from_slice(cap.as_bytes());
            payload.push(0x00);
        }
        payload.extend_from_slice(&self.issued_at.timestamp().to_le_bytes());
        let exp = self.expires_at.map(|exp| exp.timestamp()).unwrap_or(-1);
        payload.extend_from_slice(&exp.to_le_bytes());
        payload
    }

    /// Compute and record an HMAC-SHA256 signature using `secret`.
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

    /// Check if the token is valid (not expired AND has a non-empty signature).
    pub fn is_valid(&self) -> bool {
        let not_expired = self.expires_at.map(|exp| Utc::now() < exp).unwrap_or(true);
        not_expired && !self.signature.is_empty()
    }

    /// Cryptographically verify token validity (not expired AND authentic HMAC-SHA256 signature).
    pub fn is_valid_with_secret(&self, secret: &[u8]) -> bool {
        self.is_valid() && self.verify_signature(secret)
    }

    /// Check if the token has a specific capability.
    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|c| {
            c == capability || (c.ends_with(":*") && capability.starts_with(&c[..c.len() - 1]))
        })
    }
}

/// A record of a granted permission.
#[derive(Debug, Clone)]
pub struct PermissionGrant {
    pub capability: String,
    pub granted: bool,
    pub granted_at: DateTime<Utc>,
    pub granted_by: String,
    pub expires_at: Option<DateTime<Utc>>,
}

impl PermissionGrant {
    pub fn new(
        capability: impl Into<String>,
        granted: bool,
        granted_by: impl Into<String>,
    ) -> Self {
        Self {
            capability: capability.into(),
            granted,
            granted_at: Utc::now(),
            granted_by: granted_by.into(),
            expires_at: None,
        }
    }

    pub fn with_expiration(mut self, expires_at: DateTime<Utc>) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    pub fn is_valid(&self) -> bool {
        self.granted && self.expires_at.map(|exp| Utc::now() < exp).unwrap_or(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_creation() {
        let token = CapabilityToken::new(
            "plugin-1",
            vec!["audio:capture".to_string()],
            vec![0xAB, 0xCD],
        );
        assert!(token.is_valid());
        assert!(token.has_capability("audio:capture"));
        assert!(!token.has_capability("file:read"));
    }

    #[test]
    fn token_wildcard() {
        let token = CapabilityToken::new("plugin-1", vec!["audio:*".to_string()], vec![0xAB]);
        assert!(token.has_capability("audio:capture"));
        assert!(token.has_capability("audio:playback"));
        assert!(!token.has_capability("file:read"));
    }

    #[test]
    fn token_unsigned_is_invalid() {
        let token = CapabilityToken::new("plugin-1", vec!["audio:capture".to_string()], vec![]);
        assert!(!token.is_valid(), "Unsigned token must be invalid");
    }

    #[test]
    fn token_hmac_sign_and_verify() {
        let secret = b"osmoo-secure-secret-key-32bytes!";
        let token = CapabilityToken::issue_signed(
            "operator",
            vec!["audio:capture".to_string(), "screen:capture".to_string()],
            Some(Utc::now() + chrono::Duration::hours(1)),
            secret,
        );

        assert!(token.is_valid());
        assert!(token.verify_signature(secret));
        assert!(token.is_valid_with_secret(secret));
        assert!(token.has_capability("audio:capture"));
        assert!(!token.has_capability("kernel:write"));
    }

    #[test]
    fn token_tampered_payload_rejected() {
        let secret = b"osmoo-secure-secret-key-32bytes!";
        let mut token = CapabilityToken::issue_signed(
            "operator",
            vec!["audio:capture".to_string()],
            Some(Utc::now() + chrono::Duration::hours(1)),
            secret,
        );

        assert!(token.verify_signature(secret));

        // Tamper with capabilities to escalate privileges
        token.capabilities.push("admin:all".to_string());
        assert!(
            !token.verify_signature(secret),
            "Tampered capabilities must fail HMAC check"
        );
        assert!(!token.is_valid_with_secret(secret));
    }

    #[test]
    fn token_tampered_signature_rejected() {
        let secret = b"osmoo-secure-secret-key-32bytes!";
        let mut token = CapabilityToken::issue_signed(
            "operator",
            vec!["audio:capture".to_string()],
            Some(Utc::now() + chrono::Duration::hours(1)),
            secret,
        );

        // Tamper with the signature bytes
        if let Some(byte) = token.signature.first_mut() {
            *byte ^= 0xFF;
        }
        assert!(
            !token.verify_signature(secret),
            "Tampered signature must fail verification"
        );
        assert!(!token.is_valid_with_secret(secret));
    }

    #[test]
    fn token_wrong_secret_rejected() {
        let secret_a = b"correct-secret-key-32bytes-osmoo!";
        let secret_b = b"wrong-secret-key-32bytes-attacker!";
        let token = CapabilityToken::issue_signed(
            "operator",
            vec!["audio:capture".to_string()],
            Some(Utc::now() + chrono::Duration::hours(1)),
            secret_a,
        );

        assert!(token.verify_signature(secret_a));
        assert!(
            !token.verify_signature(secret_b),
            "Verification with wrong secret must fail"
        );
    }

    #[test]
    fn token_expired_fails_validation() {
        let secret = b"osmoo-secure-secret-key-32bytes!";
        let past = Utc::now() - chrono::Duration::seconds(10);
        let token = CapabilityToken::issue_signed(
            "operator",
            vec!["audio:capture".to_string()],
            Some(past),
            secret,
        );

        // Cryptographic signature is valid, but token is expired
        assert!(token.verify_signature(secret));
        assert!(!token.is_valid(), "Expired token must be invalid");
        assert!(!token.is_valid_with_secret(secret));
    }

    #[test]
    fn grant_creation() {
        let grant = PermissionGrant::new("audio:capture", true, "user");
        assert!(grant.is_valid());
    }
}
