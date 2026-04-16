use super::protocol::RemoteControlTarget;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
#[cfg(not(test))]
use codex_keyring_store::DefaultKeyringStore;
use codex_keyring_store::KeyringStore;
use ed25519_dalek::Signer;
use ed25519_dalek::SigningKey;
use sha2::Digest;
use sha2::Sha256;
use std::io;
use std::sync::Arc;

const KEYRING_SERVICE: &str = "Codex Remote Control Approval";
pub(super) const SIGNATURE_ALGORITHM: &str = "ed25519";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RemoteControlApprovalSignature {
    pub(crate) signature: String,
    pub(crate) signature_algorithm: &'static str,
    pub(crate) approval_key_id: String,
}

#[derive(Clone)]
pub(super) struct RemoteControlApprovalKey {
    signing_key: SigningKey,
    key_id: String,
    public_key: String,
}

impl RemoteControlApprovalKey {
    pub(super) fn load_or_generate(
        remote_control_target: &RemoteControlTarget,
        account_id: &str,
        app_server_client_name: Option<&str>,
    ) -> io::Result<Self> {
        #[cfg(test)]
        {
            let _ = (remote_control_target, account_id, app_server_client_name);
            Ok(Self::new(SigningKey::from_bytes(&[42; 32])))
        }
        #[cfg(not(test))]
        {
            Self::load_or_generate_with_store(
                remote_control_target,
                account_id,
                app_server_client_name,
                Arc::new(DefaultKeyringStore),
            )
        }
    }

    fn load_or_generate_with_store(
        remote_control_target: &RemoteControlTarget,
        account_id: &str,
        app_server_client_name: Option<&str>,
        keyring_store: Arc<dyn KeyringStore>,
    ) -> io::Result<Self> {
        let account = keyring_account(remote_control_target, account_id, app_server_client_name);
        match keyring_store
            .load(KEYRING_SERVICE, &account)
            .map_err(|err| {
                io::Error::other(format!(
                    "failed to load remote control approval key from keyring: {}",
                    err.message()
                ))
            })? {
            Some(serialized) => signing_key_from_serialized_seed(&serialized).map(Self::new),
            None => {
                let seed: [u8; 32] = rand::random();
                let serialized = URL_SAFE_NO_PAD.encode(seed);
                keyring_store
                    .save(KEYRING_SERVICE, &account, &serialized)
                    .map_err(|err| {
                        io::Error::other(format!(
                            "failed to save remote control approval key to keyring: {}",
                            err.message()
                        ))
                    })?;
                Ok(Self::new(SigningKey::from_bytes(&seed)))
            }
        }
    }

    fn new(signing_key: SigningKey) -> Self {
        let public_key_bytes = signing_key.verifying_key().to_bytes();
        let key_digest = Sha256::digest(public_key_bytes);
        Self {
            signing_key,
            key_id: format!(
                "{SIGNATURE_ALGORITHM}:{}",
                URL_SAFE_NO_PAD.encode(key_digest)
            ),
            public_key: URL_SAFE_NO_PAD.encode(public_key_bytes),
        }
    }

    pub(super) fn public_key(&self) -> &str {
        &self.public_key
    }

    pub(super) fn key_id(&self) -> &str {
        &self.key_id
    }

    pub(super) fn sign(&self, challenge: &str) -> RemoteControlApprovalSignature {
        let signature = self.signing_key.sign(challenge.as_bytes());
        RemoteControlApprovalSignature {
            signature: URL_SAFE_NO_PAD.encode(signature.to_bytes()),
            signature_algorithm: SIGNATURE_ALGORITHM,
            approval_key_id: self.key_id.clone(),
        }
    }
}

fn signing_key_from_serialized_seed(serialized: &str) -> io::Result<SigningKey> {
    let seed = URL_SAFE_NO_PAD
        .decode(serialized.as_bytes())
        .map_err(|err| {
            io::Error::other(format!(
                "failed to decode remote control approval key from keyring: {err}"
            ))
        })?;
    let seed: [u8; 32] = seed.try_into().map_err(|_| {
        io::Error::other("remote control approval key in keyring has invalid length")
    })?;
    Ok(SigningKey::from_bytes(&seed))
}

fn keyring_account(
    remote_control_target: &RemoteControlTarget,
    account_id: &str,
    app_server_client_name: Option<&str>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(remote_control_target.websocket_url.as_bytes());
    hasher.update(b"\n");
    hasher.update(account_id.as_bytes());
    hasher.update(b"\n");
    if let Some(app_server_client_name) = app_server_client_name {
        hasher.update(app_server_client_name.as_bytes());
    }
    let digest = hasher.finalize();
    let encoded = URL_SAFE_NO_PAD.encode(digest);
    format!("remote-control-approval|{encoded}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_keyring_store::tests::MockKeyringStore;
    use pretty_assertions::assert_eq;

    fn target() -> RemoteControlTarget {
        RemoteControlTarget {
            websocket_url: "wss://chatgpt.com/backend-api/wham/remote/control/server".to_string(),
            enroll_url: "https://chatgpt.com/backend-api/wham/remote/control/server/enroll"
                .to_string(),
        }
    }

    #[test]
    fn load_or_generate_reuses_keyring_seed() {
        let keyring_store = Arc::new(MockKeyringStore::default());
        let first = RemoteControlApprovalKey::load_or_generate_with_store(
            &target(),
            "account-id",
            Some("desktop"),
            keyring_store.clone(),
        )
        .expect("key should generate");
        let second = RemoteControlApprovalKey::load_or_generate_with_store(
            &target(),
            "account-id",
            Some("desktop"),
            keyring_store,
        )
        .expect("key should load");

        assert_eq!(first.key_id(), second.key_id());
        assert_eq!(first.public_key(), second.public_key());
    }

    #[test]
    fn sign_returns_key_bound_signature_metadata() {
        let key = RemoteControlApprovalKey::new(SigningKey::from_bytes(&[7; 32]));

        assert_eq!(
            key.sign("challenge").signature_algorithm,
            SIGNATURE_ALGORITHM
        );
        assert_eq!(key.sign("challenge").approval_key_id, key.key_id());
    }
}
