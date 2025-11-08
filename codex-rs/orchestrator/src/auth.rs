/// HMAC-SHA256 authentication for orchestrator RPC
///
/// Provides secure local authentication using shared secret stored in .codex/secret
use anyhow::Context;
/// HMAC-SHA256 authentication for orchestrator RPC
///
/// Provides secure local authentication using shared secret stored in .codex/secret
use anyhow::Result;
/// HMAC-SHA256 authentication for orchestrator RPC
///
/// Provides secure local authentication using shared secret stored in .codex/secret
use anyhow::anyhow;
use base64::Engine;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;
use std::fs;
use std::path::Path;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

const SECRET_FILENAME: &str = "secret";
const TIME_SKEW_TOLERANCE_SECS: u64 = 300; // ±5 minutes

/// HMAC authenticator
pub struct HmacAuthenticator {
    secret: Vec<u8>,
}

/// Type alias for AuthManager (used in server.rs)
pub type AuthManager = HmacAuthenticator;

impl HmacAuthenticator {
    /// Load or generate secret from .codex/secret
    pub fn new(codex_dir: &Path) -> Result<Self> {
        let secret_path = codex_dir.join(SECRET_FILENAME);

        let secret = if secret_path.exists() {
            fs::read(&secret_path).context("Failed to read secret file")?
        } else {
            // Generate new secret
            let secret = Self::generate_secret();
            fs::write(&secret_path, &secret).context("Failed to write secret file")?;

            // Set restrictive permissions (Unix only)
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&secret_path)?.permissions();
                perms.set_mode(0o600);
                fs::set_permissions(&secret_path, perms)?;
            }

            tracing::info!("Generated new orchestrator secret");
            secret
        };

        Ok(Self { secret })
    }

    /// Generate a new random secret (32 bytes)
    fn generate_secret() -> Vec<u8> {
        use rand::Rng;
        let mut rng = rand::rng();
        (0..32).map(|_| rng.random::<u8>()).collect()
    }

    /// Sign a message with timestamp
    pub fn sign(&self, message: &[u8], timestamp: u64) -> String {
        let mut hasher = Sha256::new();
        hasher.update(&self.secret);
        hasher.update(message);
        hasher.update(&timestamp.to_le_bytes());

        let result = hasher.finalize();
        base64::engine::general_purpose::STANDARD.encode(&result[..])
    }

    /// Verify HMAC signature with time skew tolerance
    pub fn verify(&self, message: &[u8], signature: &str, claimed_timestamp: u64) -> Result<()> {
        // Check time skew
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let time_diff = if now > claimed_timestamp {
            now - claimed_timestamp
        } else {
            claimed_timestamp - now
        };

        if time_diff > TIME_SKEW_TOLERANCE_SECS {
            return Err(anyhow!(
                "Timestamp out of range: {} seconds skew (max: {} seconds)",
                time_diff,
                TIME_SKEW_TOLERANCE_SECS
            ));
        }

        // Verify signature
        let expected = self.sign(message, claimed_timestamp);
        if signature != expected {
            return Err(anyhow!("Invalid HMAC signature"));
        }

        Ok(())
    }
}

/// Authentication header for RPC requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthHeader {
    pub timestamp: u64,
    pub signature: String,
}

impl AuthHeader {
    /// Create a new auth header
    pub fn new(authenticator: &HmacAuthenticator, message: &[u8]) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let signature = authenticator.sign(message, timestamp);

        Self {
            timestamp,
            signature,
        }
    }

    /// Verify this auth header
    pub fn verify(&self, authenticator: &HmacAuthenticator, message: &[u8]) -> Result<()> {
        authenticator.verify(message, &self.signature, self.timestamp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_generate_and_load_secret() {
        let temp_dir = TempDir::new().unwrap();
        let codex_dir = temp_dir.path().join(".codex");
        fs::create_dir_all(&codex_dir).unwrap();

        // First creation should generate secret
        let auth1 = HmacAuthenticator::new(&codex_dir).unwrap();

        // Second load should read same secret
        let auth2 = HmacAuthenticator::new(&codex_dir).unwrap();

        let message = b"test message";
        let timestamp = 1234567890;

        let sig1 = auth1.sign(message, timestamp);
        let sig2 = auth2.sign(message, timestamp);

        assert_eq!(sig1, sig2, "Same secret should produce same signature");
    }

    #[test]
    fn test_sign_and_verify() {
        let temp_dir = TempDir::new().unwrap();
        let codex_dir = temp_dir.path().join(".codex");
        fs::create_dir_all(&codex_dir).unwrap();

        let auth = HmacAuthenticator::new(&codex_dir).unwrap();
        let message = b"test message";

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let signature = auth.sign(message, timestamp);

        // Should verify successfully
        auth.verify(message, &signature, timestamp).unwrap();
    }

    #[test]
    fn test_verify_fails_with_wrong_signature() {
        let temp_dir = TempDir::new().unwrap();
        let codex_dir = temp_dir.path().join(".codex");
        fs::create_dir_all(&codex_dir).unwrap();

        let auth = HmacAuthenticator::new(&codex_dir).unwrap();
        let message = b"test message";
        let timestamp = 1234567890;

        let wrong_sig = "wrong_signature";

        let result = auth.verify(message, wrong_sig, timestamp);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_fails_with_time_skew() {
        let temp_dir = TempDir::new().unwrap();
        let codex_dir = temp_dir.path().join(".codex");
        fs::create_dir_all(&codex_dir).unwrap();

        let auth = HmacAuthenticator::new(&codex_dir).unwrap();
        let message = b"test message";

        let old_timestamp = 1000000; // Very old timestamp
        let signature = auth.sign(message, old_timestamp);

        let result = auth.verify(message, &signature, old_timestamp);
        assert!(result.is_err(), "Should reject old timestamp");
    }

    #[test]
    fn test_auth_header() {
        let temp_dir = TempDir::new().unwrap();
        let codex_dir = temp_dir.path().join(".codex");
        fs::create_dir_all(&codex_dir).unwrap();

        let auth = HmacAuthenticator::new(&codex_dir).unwrap();
        let message = b"test message";

        let header = AuthHeader::new(&auth, message);

        // Should verify successfully
        header.verify(&auth, message).unwrap();
    }
}
