// Phase 2: Secure Agent Message Protocol (Ed25519 + AES-256-GCM)
// Based on design document: _docs/2025-10-28_セキュア通信アーキテクチャ設計書.md

use serde::{Deserialize, Serialize};

#[cfg(feature = "agent_security")]
use anyhow::{Context, Result};
#[cfg(feature = "agent_security")]
use std::collections::HashMap;
#[cfg(feature = "agent_security")]
use std::sync::Arc;
#[cfg(feature = "agent_security")]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(feature = "agent_security")]
use tokio::sync::Mutex;

/// Secure Agent Message (with Ed25519 signature + AES-256-GCM encryption)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecureAgentMessage {
    /// Sender agent type
    pub from: String,

    /// Recipient agent type (None = broadcast)
    pub to: Option<String>,

    /// Encrypted content (AES-256-GCM)
    pub encrypted_content: Vec<u8>,

    /// Ed25519 signature
    pub signature: Vec<u8>,

    /// Nonce (replay attack protection)
    pub nonce: u64,

    /// Timestamp (RFC 3339)
    pub timestamp: String,

    /// Metadata (not encrypted)
    pub metadata: SecureMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecureMetadata {
    /// Message ID (UUID)
    pub message_id: String,

    /// Priority (0-255)
    pub priority: u8,

    /// Message type
    pub message_type: MessageType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    /// Task request
    TaskRequest,

    /// Task response
    TaskResponse,

    /// Status update
    StatusUpdate,

    /// Error notification
    ErrorNotification,

    /// Shutdown command
    Shutdown,
}

/// Secure Agent Channel (encrypted communication)
#[cfg(feature = "agent_security")]
pub struct SecureAgentChannel {
    /// Send channel
    tx: tokio::sync::mpsc::UnboundedSender<SecureAgentMessage>,

    /// Receive channel
    rx: Arc<Mutex<tokio::sync::mpsc::UnboundedReceiver<SecureAgentMessage>>>,

    /// Agent signing keypair (Ed25519)
    #[cfg(feature = "agent_security")]
    signing_keypair: Arc<ed25519_dalek::SigningKey>,

    /// Encryption key (AES-256-GCM, derived via HKDF)
    #[cfg(feature = "agent_security")]
    encryption_key: Arc<aes_gcm::Key<aes_gcm::Aes256Gcm>>,

    /// Trusted agent public keys
    #[cfg(feature = "agent_security")]
    trusted_public_keys: Arc<Mutex<HashMap<String, ed25519_dalek::VerifyingKey>>>,

    /// Nonce counter (replay attack protection)
    nonce_counter: AtomicU64,
}

#[cfg(feature = "agent_security")]
impl SecureAgentChannel {
    /// Create a new secure channel pair
    pub fn new(
        agent_type: String,
        signing_keypair: ed25519_dalek::SigningKey,
        encryption_key: aes_gcm::Key<aes_gcm::Aes256Gcm>,
    ) -> (Self, Self) {
        let (tx1, rx1) = tokio::sync::mpsc::unbounded_channel();
        let (tx2, rx2) = tokio::sync::mpsc::unbounded_channel();

        let keypair = Arc::new(signing_keypair);
        let enc_key = Arc::new(encryption_key);
        let trusted_keys = Arc::new(Mutex::new(HashMap::new()));

        let channel1 = Self {
            tx: tx1,
            rx: Arc::new(Mutex::new(rx2)),
            signing_keypair: Arc::clone(&keypair),
            encryption_key: Arc::clone(&enc_key),
            trusted_public_keys: Arc::clone(&trusted_keys),
            nonce_counter: AtomicU64::new(0),
        };

        let channel2 = Self {
            tx: tx2,
            rx: Arc::new(Mutex::new(rx1)),
            signing_keypair: keypair,
            encryption_key: enc_key,
            trusted_public_keys: trusted_keys,
            nonce_counter: AtomicU64::new(0),
        };

        (channel1, channel2)
    }

    /// Register trusted agent public key
    pub async fn register_trusted_agent(
        &self,
        agent_type: String,
        public_key: ed25519_dalek::VerifyingKey,
    ) {
        self.trusted_public_keys
            .lock()
            .await
            .insert(agent_type, public_key);
    }

    /// Send secure message
    pub async fn send_secure(&self, from: String, to: Option<String>, content: &str) -> Result<()> {
        use ed25519_dalek::Signer;

        // 1. Build metadata
        let metadata = SecureMetadata {
            message_id: uuid::Uuid::new_v4().to_string(),
            priority: 128,
            message_type: MessageType::TaskRequest,
        };

        // 2. Encrypt content
        let encrypted_content = self.encrypt_content(content)?;

        // 3. Generate signature
        let signature_data = self.build_signature_data(&encrypted_content, &metadata)?;
        let signature = self.signing_keypair.sign(&signature_data);

        // 4. Build secure message
        let secure_msg = SecureAgentMessage {
            from,
            to,
            encrypted_content,
            signature: signature.to_bytes().to_vec(),
            nonce: self.nonce_counter.fetch_add(1, Ordering::SeqCst),
            timestamp: chrono::Utc::now().to_rfc3339(),
            metadata,
        };

        // 5. Send to channel
        self.tx.send(secure_msg)?;

        Ok(())
    }

    /// Receive and verify secure message
    pub async fn receive_secure(&self) -> Result<(String, String)> {
        use ed25519_dalek::Verifier;

        // 1. Receive message
        let secure_msg = self
            .rx
            .lock()
            .await
            .recv()
            .await
            .context("Channel closed")?;

        // 2. Verify signature
        self.verify_signature(&secure_msg).await?;

        // 3. Verify nonce (replay protection)
        self.verify_nonce(&secure_msg)?;

        // 4. Decrypt content
        let content = self.decrypt_content(&secure_msg.encrypted_content)?;

        Ok((secure_msg.from, content))
    }

    /// Encrypt content with AES-256-GCM
    fn encrypt_content(&self, content: &str) -> Result<Vec<u8>> {
        use aes_gcm::{
            Aes256Gcm, Nonce,
            aead::{Aead, KeyInit},
        };

        let cipher = Aes256Gcm::new(&self.encryption_key);
        let nonce = Nonce::from_slice(&self.generate_nonce());

        cipher
            .encrypt(nonce, content.as_bytes())
            .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))
    }

    /// Decrypt content with AES-256-GCM
    fn decrypt_content(&self, ciphertext: &[u8]) -> Result<String> {
        use aes_gcm::{
            Aes256Gcm, Nonce,
            aead::{Aead, KeyInit},
        };

        let cipher = Aes256Gcm::new(&self.encryption_key);
        // In production, nonce should be transmitted with ciphertext
        let nonce = Nonce::from_slice(&[0u8; 12]);

        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| anyhow::anyhow!("Decryption failed: {}", e))?;

        String::from_utf8(plaintext).context("Invalid UTF-8")
    }

    /// Build signature data
    fn build_signature_data(
        &self,
        encrypted_content: &[u8],
        metadata: &SecureMetadata,
    ) -> Result<Vec<u8>> {
        let mut data = Vec::new();
        data.extend_from_slice(encrypted_content);
        data.extend_from_slice(metadata.message_id.as_bytes());
        data.extend_from_slice(&[metadata.priority]);
        Ok(data)
    }

    /// Verify Ed25519 signature
    async fn verify_signature(&self, msg: &SecureAgentMessage) -> Result<()> {
        use ed25519_dalek::{Signature, Verifier};

        // Get trusted public key
        let trusted_keys = self.trusted_public_keys.lock().await;
        let public_key = trusted_keys.get(&msg.from).context("Sender not trusted")?;

        // Rebuild signature data
        let signature_data = self.build_signature_data(&msg.encrypted_content, &msg.metadata)?;

        // Verify Ed25519 signature
        let signature = Signature::from_bytes(
            &msg.signature
                .try_into()
                .map_err(|_| anyhow::anyhow!("Invalid signature length"))?,
        );

        public_key
            .verify(&signature_data, &signature)
            .context("Signature verification failed")?;

        Ok(())
    }

    /// Verify nonce (replay attack protection)
    fn verify_nonce(&self, msg: &SecureAgentMessage) -> Result<()> {
        // Timestamp verification (5 minutes)
        let msg_time = chrono::DateTime::parse_from_rfc3339(&msg.timestamp)?;
        let now = chrono::Utc::now();
        let age = now.signed_duration_since(msg_time);

        if age.num_seconds() > 300 {
            anyhow::bail!("Message too old: {} seconds", age.num_seconds());
        }

        // In production: check nonce uniqueness in Redis/memory store

        Ok(())
    }

    /// Generate random nonce
    fn generate_nonce(&self) -> [u8; 12] {
        use rand::RngCore;
        let mut nonce = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce);
        nonce
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_type_serialization() {
        let msg_type = MessageType::TaskRequest;
        let json = serde_json::to_string(&msg_type).unwrap();
        assert!(json.contains("TaskRequest"));
    }

    #[test]
    fn test_secure_metadata() {
        let metadata = SecureMetadata {
            message_id: uuid::Uuid::new_v4().to_string(),
            priority: 128,
            message_type: MessageType::StatusUpdate,
        };

        assert_eq!(metadata.priority, 128);
    }
}
