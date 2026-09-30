use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Verdict for signature verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignatureVerdict {
    Valid,
    Invalid,
    Revoked,
    Expired,
    UnknownSigner,
}

/// A signed artifact (plugin, skill, model, or update).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedArtifact {
    pub id: Uuid,
    pub artifact_type: ArtifactType,
    pub name: String,
    pub version: String,
    pub content_hash: String,
    pub signature: Vec<u8>,
    pub signer_id: String,
    pub signed_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArtifactType {
    Plugin,
    Skill,
    Model,
    Update,
}

/// A registered trusted signer (developer key).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustedSigner {
    pub id: String,
    pub name: String,
    pub public_key: Vec<u8>,
    pub registered_at: DateTime<Utc>,
    pub revoked: bool,
}

/// Registry for verifying signed artifacts.
pub struct SignatureVerifier {
    trusted_signers: std::collections::HashMap<String, TrustedSigner>,
    known_hashes: std::collections::HashMap<String, SignedArtifact>,
    revoked_hashes: std::collections::HashSet<String>,
}

impl SignatureVerifier {
    pub fn new() -> Self {
        Self {
            trusted_signers: std::collections::HashMap::new(),
            known_hashes: std::collections::HashMap::new(),
            revoked_hashes: std::collections::HashSet::new(),
        }
    }

    pub fn register_signer(&mut self, signer: TrustedSigner) {
        self.trusted_signers.insert(signer.id.clone(), signer);
    }

    pub fn revoke_signer(&mut self, signer_id: &str) -> bool {
        if let Some(signer) = self.trusted_signers.get_mut(signer_id) {
            signer.revoked = true;
            true
        } else {
            false
        }
    }

    pub fn register_artifact(&mut self, artifact: SignedArtifact) {
        self.known_hashes
            .insert(artifact.content_hash.clone(), artifact);
    }

    pub fn revoke_artifact(&mut self, content_hash: &str) -> bool {
        if self.known_hashes.contains_key(content_hash) {
            self.revoked_hashes.insert(content_hash.to_string());
            true
        } else {
            false
        }
    }

    /// Compute SHA-256 hash of content bytes.
    pub fn hash_content(content: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content);
        hex::encode(hasher.finalize())
    }

    /// Generate HMAC-SHA256 signature of content using a key.
    ///
    /// HMAC accepts empty keys per RFC 2104 (key is zero-padded to block size).
    pub fn sign_content(content: &[u8], key: &[u8]) -> Vec<u8> {
        use hmac::{Hmac, Mac};
        type HmacSha256 = Hmac<Sha256>;
        let mut mac =
            HmacSha256::new_from_slice(key).expect("HMAC accepts any non-zero-length key");
        mac.update(content);
        mac.finalize().into_bytes().to_vec()
    }

    /// Verify an HMAC-SHA256 signature.
    pub fn verify_signature(content: &[u8], key: &[u8], signature: &[u8]) -> bool {
        use hmac::{Hmac, Mac};
        type HmacSha256 = Hmac<Sha256>;
        let mut mac = match HmacSha256::new_from_slice(key) {
            Ok(m) => m,
            Err(_) => return false,
        };
        mac.update(content);
        let expected = mac.finalize().into_bytes();
        expected.as_slice() == signature
    }

    /// Verify a signed artifact: checks signer trust, hash, signature, and expiry.
    pub fn verify_artifact(
        &self,
        artifact: &SignedArtifact,
        content: &[u8],
        signing_key: &[u8],
    ) -> SignatureVerdict {
        // Check revocation
        if self.revoked_hashes.contains(&artifact.content_hash) {
            return SignatureVerdict::Revoked;
        }

        // Check signer trust
        match self.trusted_signers.get(&artifact.signer_id) {
            Some(signer) if signer.revoked => return SignatureVerdict::Revoked,
            None => return SignatureVerdict::UnknownSigner,
            _ => {}
        }

        // Verify content hash
        let computed_hash = Self::hash_content(content);
        if computed_hash != artifact.content_hash {
            return SignatureVerdict::Invalid;
        }

        // Verify HMAC signature
        if !Self::verify_signature(content, signing_key, &artifact.signature) {
            return SignatureVerdict::Invalid;
        }

        // Check expiry
        if let Some(expires_at) = artifact.expires_at {
            if Utc::now() > expires_at {
                return SignatureVerdict::Expired;
            }
        }

        SignatureVerdict::Valid
    }

    pub fn get_signer(&self, id: &str) -> Option<&TrustedSigner> {
        self.trusted_signers.get(id)
    }

    pub fn all_signers(&self) -> &std::collections::HashMap<String, TrustedSigner> {
        &self.trusted_signers
    }

    pub fn is_signer_trusted(&self, signer_id: &str) -> bool {
        self.trusted_signers
            .get(signer_id)
            .map(|s| !s.revoked)
            .unwrap_or(false)
    }
}

impl Default for SignatureVerifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_signer() -> TrustedSigner {
        TrustedSigner {
            id: "dev-1".to_string(),
            name: "Test Developer".to_string(),
            public_key: b"test-key-1234567890abcdef".to_vec(),
            registered_at: Utc::now(),
            revoked: false,
        }
    }

    #[test]
    fn hash_consistency() {
        let h1 = SignatureVerifier::hash_content(b"hello world");
        let h2 = SignatureVerifier::hash_content(b"hello world");
        let h3 = SignatureVerifier::hash_content(b"tampered");
        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
    }

    #[test]
    fn sign_and_verify_roundtrip() {
        let content = b"test artifact content";
        let key = b"signing-key-12345";
        let sig = SignatureVerifier::sign_content(content, key);
        assert!(SignatureVerifier::verify_signature(content, key, &sig));
        assert!(!SignatureVerifier::verify_signature(b"wrong", key, &sig));
        assert!(!SignatureVerifier::verify_signature(
            content,
            b"wrong-key",
            &sig
        ));
    }

    #[test]
    fn sign_empty_key_produces_valid_signature() {
        let sig = SignatureVerifier::sign_content(b"data", b"");
        assert!(!sig.is_empty());
    }

    #[test]
    fn artifact_valid_signature() {
        let mut verifier = SignatureVerifier::new();
        verifier.register_signer(test_signer());

        let content = b"plugin binary content";
        let key = b"test-key-1234567890abcdef";
        let hash = SignatureVerifier::hash_content(content);
        let sig = SignatureVerifier::sign_content(content, key);

        let artifact = SignedArtifact {
            id: Uuid::new_v4(),
            artifact_type: ArtifactType::Plugin,
            name: "test-plugin".to_string(),
            version: "1.0.0".to_string(),
            content_hash: hash,
            signature: sig,
            signer_id: "dev-1".to_string(),
            signed_at: Utc::now(),
            expires_at: None,
        };

        assert_eq!(
            verifier.verify_artifact(&artifact, content, key),
            SignatureVerdict::Valid
        );
    }

    #[test]
    fn artifact_tampered_content() {
        let mut verifier = SignatureVerifier::new();
        verifier.register_signer(test_signer());

        let content = b"original content";
        let key = b"test-key-1234567890abcdef";
        let hash = SignatureVerifier::hash_content(content);
        let sig = SignatureVerifier::sign_content(content, key);

        let artifact = SignedArtifact {
            id: Uuid::new_v4(),
            artifact_type: ArtifactType::Skill,
            name: "test-skill".to_string(),
            version: "1.0.0".to_string(),
            content_hash: hash,
            signature: sig,
            signer_id: "dev-1".to_string(),
            signed_at: Utc::now(),
            expires_at: None,
        };

        assert_eq!(
            verifier.verify_artifact(&artifact, b"tampered content", key),
            SignatureVerdict::Invalid
        );
    }

    #[test]
    fn artifact_unknown_signer() {
        let verifier = SignatureVerifier::new();
        let content = b"data";
        let key = b"test-key-1234567890abcdef";
        let hash = SignatureVerifier::hash_content(content);
        let sig = SignatureVerifier::sign_content(content, key);

        let artifact = SignedArtifact {
            id: Uuid::new_v4(),
            artifact_type: ArtifactType::Model,
            name: "model".to_string(),
            version: "1.0.0".to_string(),
            content_hash: hash,
            signature: sig,
            signer_id: "unknown".to_string(),
            signed_at: Utc::now(),
            expires_at: None,
        };

        assert_eq!(
            verifier.verify_artifact(&artifact, content, key),
            SignatureVerdict::UnknownSigner
        );
    }

    #[test]
    fn artifact_revoked_signer() {
        let mut verifier = SignatureVerifier::new();
        let mut signer = test_signer();
        signer.revoked = true;
        verifier.register_signer(signer);

        let content = b"data";
        let key = b"test-key-1234567890abcdef";
        let hash = SignatureVerifier::hash_content(content);
        let sig = SignatureVerifier::sign_content(content, key);

        let artifact = SignedArtifact {
            id: Uuid::new_v4(),
            artifact_type: ArtifactType::Plugin,
            name: "plugin".to_string(),
            version: "1.0.0".to_string(),
            content_hash: hash,
            signature: sig,
            signer_id: "dev-1".to_string(),
            signed_at: Utc::now(),
            expires_at: None,
        };

        assert_eq!(
            verifier.verify_artifact(&artifact, content, key),
            SignatureVerdict::Revoked
        );
    }

    #[test]
    fn artifact_expired() {
        let mut verifier = SignatureVerifier::new();
        verifier.register_signer(test_signer());

        let content = b"data";
        let key = b"test-key-1234567890abcdef";
        let hash = SignatureVerifier::hash_content(content);
        let sig = SignatureVerifier::sign_content(content, key);

        let artifact = SignedArtifact {
            id: Uuid::new_v4(),
            artifact_type: ArtifactType::Update,
            name: "update".to_string(),
            version: "1.0.0".to_string(),
            content_hash: hash,
            signature: sig,
            signer_id: "dev-1".to_string(),
            signed_at: Utc::now(),
            expires_at: Some(Utc::now() - chrono::Duration::hours(1)),
        };

        assert_eq!(
            verifier.verify_artifact(&artifact, content, key),
            SignatureVerdict::Expired
        );
    }

    #[test]
    fn revoke_artifact() {
        let mut verifier = SignatureVerifier::new();
        let content = b"data";
        let hash = SignatureVerifier::hash_content(content);

        let artifact = SignedArtifact {
            id: Uuid::new_v4(),
            artifact_type: ArtifactType::Plugin,
            name: "plugin".to_string(),
            version: "1.0.0".to_string(),
            content_hash: hash.clone(),
            signature: vec![],
            signer_id: "dev-1".to_string(),
            signed_at: Utc::now(),
            expires_at: None,
        };
        verifier.register_artifact(artifact);

        assert!(verifier.revoked_hashes.is_empty());
        assert!(verifier.revoke_artifact(&hash));
        assert!(verifier.revoked_hashes.contains(&hash));
    }

    #[test]
    fn revoke_signer() {
        let mut verifier = SignatureVerifier::new();
        verifier.register_signer(test_signer());
        assert!(verifier.is_signer_trusted("dev-1"));
        assert!(verifier.revoke_signer("dev-1"));
        assert!(!verifier.is_signer_trusted("dev-1"));
        assert!(!verifier.revoke_signer("nonexistent"));
    }
}
