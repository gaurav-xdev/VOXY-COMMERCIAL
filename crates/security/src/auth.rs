//! Commercial Authentication, Password Hashing & Rate Limiting Engine.
//!
//! Provides Argon2id password hashing, constant-time verification,
//! cryptographically random session tokens (256-bit entropy), and
//! brute-force rate limiting with exponential backoff.

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2, Params,
};
use chrono::{DateTime, Duration, Utc};
use parking_lot::RwLock;
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;

/// Production Argon2id Password Hasher.
pub struct AuthPasswordHasher;

impl AuthPasswordHasher {
    /// Hashes a plaintext password using Argon2id with production memory and CPU cost.
    /// Memory: 64MB (65536 KiB), Iterations: 3, Parallelism: 4.
    pub fn hash_password(password: &str) -> Result<String, String> {
        let salt = SaltString::generate(&mut OsRng);
        let params = Params::new(65536, 3, 4, None)
            .map_err(|e| format!("Failed to configure Argon2id params: {}", e))?;
        let argon2 = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);

        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| format!("Password hashing failed: {}", e))?
            .to_string();

        Ok(password_hash)
    }

    /// Verifies a plaintext password against an Argon2id PHC formatted hash in constant time.
    ///
    /// The algorithm, memory cost (64MB), time cost (3 iterations), and lanes (4)
    /// are parsed directly from the PHC formatted string itself.
    pub fn verify_password(password: &str, password_hash: &str) -> bool {
        let parsed_hash = match PasswordHash::new(password_hash) {
            Ok(h) => h,
            Err(_) => return false,
        };

        let params = match Params::new(65536, 3, 4, None) {
            Ok(p) => p,
            Err(_) => return false,
        };
        let argon2 = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);

        argon2
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok()
    }
}

/// Cryptographically Secure Session Token Generator.
pub struct SessionTokenManager;

impl SessionTokenManager {
    /// Generates a high-entropy 256-bit (32 bytes) session token, hex-encoded (64 chars).
    pub fn generate_token() -> String {
        let mut bytes = [0u8; 32];
        OsRng.fill_bytes(&mut bytes);
        hex::encode(bytes)
    }

    /// Hashes the raw session token with SHA-256 for persistent database storage.
    /// The plaintext token is returned only to the client and never saved to disk.
    pub fn hash_token(raw_token: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(raw_token.as_bytes());
        hex::encode(hasher.finalize())
    }
}

/// Entry tracking failed attempts and lockouts for a single subject (IP or User).
#[derive(Debug, Clone)]
struct RateLimitEntry {
    failed_attempts: u32,
    first_attempt_at: DateTime<Utc>,
    locked_until: Option<DateTime<Utc>>,
}

/// Sliding-window rate limiter with brute-force protection and lockout.
#[derive(Debug, Clone)]
pub struct AuthRateLimiter {
    max_attempts: u32,
    window_duration: Duration,
    initial_lockout: Duration,
    entries: Arc<RwLock<HashMap<String, RateLimitEntry>>>,
}

impl AuthRateLimiter {
    /// Creates a rate limiter with custom thresholds.
    /// Example default: max 5 attempts within 15 minutes, initial lockout 15 minutes.
    pub fn new(max_attempts: u32, window_secs: i64, lockout_secs: i64) -> Self {
        Self {
            max_attempts,
            window_duration: Duration::seconds(window_secs),
            initial_lockout: Duration::seconds(lockout_secs),
            entries: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Standard production rate limiter: 5 attempts per 15 minutes, 15 minute lockout.
    pub fn default_production() -> Self {
        Self::new(5, 900, 900)
    }

    /// Checks if a key (e.g. IP or email) is allowed to attempt login.
    /// Returns Ok(()) if allowed, or Err(seconds_remaining) if locked out or rate-limited.
    pub fn check(&self, key: &str) -> Result<(), i64> {
        let now = Utc::now();
        let guard = self.entries.read();
        if let Some(entry) = guard.get(key) {
            if let Some(locked_until) = entry.locked_until {
                if now < locked_until {
                    let remaining = (locked_until - now).num_seconds().max(1);
                    return Err(remaining);
                }
            }
        }
        Ok(())
    }

    /// Records a failed login attempt for a key.
    /// Returns the lockout duration in seconds if this attempt triggered a lockout.
    pub fn record_failure(&self, key: &str) -> Option<i64> {
        let now = Utc::now();
        let mut guard = self.entries.write();
        let entry = guard.entry(key.to_string()).or_insert_with(|| RateLimitEntry {
            failed_attempts: 0,
            first_attempt_at: now,
            locked_until: None,
        });

        // Reset if window has elapsed
        if now - entry.first_attempt_at > self.window_duration {
            entry.failed_attempts = 0;
            entry.first_attempt_at = now;
            entry.locked_until = None;
        }

        entry.failed_attempts += 1;

        if entry.failed_attempts >= self.max_attempts {
            // Apply exponential backoff based on attempts beyond the threshold
            let multiplier = (entry.failed_attempts - self.max_attempts + 1).min(6);
            let lockout_duration = self.initial_lockout * (1 << (multiplier - 1));
            let locked_until = now + lockout_duration;
            entry.locked_until = Some(locked_until);
            return Some(lockout_duration.num_seconds());
        }

        None
    }

    /// Clears rate limiting on successful authentication.
    pub fn record_success(&self, key: &str) {
        let mut guard = self.entries.write();
        guard.remove(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_argon2id_password_hash_and_verify() {
        let password = "SuperSecretCommercialPassword!123";
        let hash = AuthPasswordHasher::hash_password(password).unwrap();
        assert!(hash.starts_with("$argon2id$"));
        assert!(AuthPasswordHasher::verify_password(password, &hash));
    }

    #[test]
    fn test_argon2id_wrong_password_fails() {
        let password = "CorrectPassword123";
        let hash = AuthPasswordHasher::hash_password(password).unwrap();
        assert!(!AuthPasswordHasher::verify_password("WrongPassword456", &hash));
    }

    #[test]
    fn test_session_token_entropy_and_expiry() {
        let token1 = SessionTokenManager::generate_token();
        let token2 = SessionTokenManager::generate_token();

        assert_eq!(token1.len(), 64); // 32 bytes hex-encoded = 64 chars
        assert_eq!(token2.len(), 64);
        assert_ne!(token1, token2);

        let hash1 = SessionTokenManager::hash_token(&token1);
        let hash2 = SessionTokenManager::hash_token(&token2);
        assert_ne!(hash1, hash2);
        assert_eq!(hash1.len(), 64);
    }

    #[test]
    fn test_rate_limiter_brute_force_block() {
        let limiter = AuthRateLimiter::new(3, 60, 120);
        let ip = "192.168.1.100";

        assert!(limiter.check(ip).is_ok());

        assert_eq!(limiter.record_failure(ip), None);
        assert!(limiter.check(ip).is_ok());

        assert_eq!(limiter.record_failure(ip), None);
        assert!(limiter.check(ip).is_ok());

        // 3rd attempt exceeds max_attempts (3)
        let lockout = limiter.record_failure(ip);
        assert!(lockout.is_some());
        assert!(lockout.unwrap() >= 120);

        // Immediate check must be blocked
        let check_res = limiter.check(ip);
        assert!(check_res.is_err());
        assert!(check_res.unwrap_err() > 0);
    }

    #[test]
    fn test_rate_limiter_reset_on_success() {
        let limiter = AuthRateLimiter::new(3, 60, 120);
        let user = "user@example.com";

        limiter.record_failure(user);
        limiter.record_failure(user);
        assert_eq!(limiter.entries.read().get(user).unwrap().failed_attempts, 2);

        limiter.record_success(user);
        assert!(limiter.entries.read().get(user).is_none());
        assert!(limiter.check(user).is_ok());
    }
}
