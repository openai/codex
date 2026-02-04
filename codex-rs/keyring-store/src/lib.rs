use keyring::Entry;
use keyring::Error as KeyringError;
use std::error::Error;
use std::fmt;
use std::fmt::Debug;
use tracing::trace;

#[derive(Debug)]
pub enum CredentialStoreError {
    Other(KeyringError),
}

impl CredentialStoreError {
    pub fn new(error: KeyringError) -> Self {
        Self::Other(error)
    }

    pub fn message(&self) -> String {
        match self {
            Self::Other(error) => error.to_string(),
        }
    }

    pub fn into_error(self) -> KeyringError {
        match self {
            Self::Other(error) => error,
        }
    }
}

impl fmt::Display for CredentialStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Other(error) => write!(f, "{error}"),
        }
    }
}

impl Error for CredentialStoreError {}

/// Shared credential store abstraction for keyring-backed implementations.
pub trait KeyringStore: Debug + Send + Sync {
    fn load(&self, service: &str, account: &str) -> Result<Option<String>, CredentialStoreError>;
    fn save(&self, service: &str, account: &str, value: &str) -> Result<(), CredentialStoreError>;
    fn delete(&self, service: &str, account: &str) -> Result<bool, CredentialStoreError>;
}

#[derive(Debug)]
pub struct DefaultKeyringStore;

impl KeyringStore for DefaultKeyringStore {
    fn load(&self, service: &str, account: &str) -> Result<Option<String>, CredentialStoreError> {
        trace!("keyring.load start, service={service}, account={account}");
        let entry = Entry::new(service, account).map_err(CredentialStoreError::new)?;
        let main_value = match entry.get_password() {
            Ok(password) => password,
            Err(keyring::Error::NoEntry) => {
                trace!("keyring.load no entry, service={service}, account={account}");
                return Ok(None);
            }
            Err(error) => {
                trace!("keyring.load error, service={service}, account={account}, error={error}");
                return Err(CredentialStoreError::new(error));
            }
        };

        if let Some(rest) = main_value.strip_prefix(CHUNKED_HEADER_PREFIX) {
            let Some((count_str, first_chunk)) = rest.split_once(':') else {
                return Ok(Some(main_value));
            };
            let count: usize = count_str.parse().map_err(|_| {
                CredentialStoreError::Other(KeyringError::Invalid(
                    "failed to parse chunk count".into(),
                    "load".into(),
                ))
            })?;

            let mut full_value = first_chunk.to_string();
            for i in 1..count {
                let chunk_account = get_chunk_key(account, i);
                let chunk_entry =
                    Entry::new(service, &chunk_account).map_err(CredentialStoreError::new)?;
                match chunk_entry.get_password() {
                    Ok(chunk_data) => full_value.push_str(&chunk_data),
                    Err(error) => {
                        return Err(CredentialStoreError::new(error));
                    }
                }
            }
            trace!(
                "keyring.load success (chunked), service={service}, account={account}, chunks={count}"
            );
            Ok(Some(full_value))
        } else {
            trace!("keyring.load success, service={service}, account={account}");
            Ok(Some(main_value))
        }
    }

    fn save(&self, service: &str, account: &str, value: &str) -> Result<(), CredentialStoreError> {
        trace!(
            "keyring.save start, service={service}, account={account}, value_len={}",
            value.len()
        );

        if value.len() > MAX_KEYRING_VALUE_LEN {
            let chunks: Vec<String> = value
                .chars()
                .collect::<Vec<char>>()
                .chunks(MAX_KEYRING_VALUE_LEN)
                .map(|chunk| chunk.iter().collect::<String>())
                .collect();

            let count = chunks.len();
            let header_chunk = format!("{CHUNKED_HEADER_PREFIX}{count}:{}", chunks[0]);

            let main_entry = Entry::new(service, account).map_err(CredentialStoreError::new)?;
            main_entry
                .set_password(&header_chunk)
                .map_err(CredentialStoreError::new)?;

            for (i, chunk) in chunks.iter().enumerate().skip(1) {
                let chunk_account = get_chunk_key(account, i);
                let chunk_entry =
                    Entry::new(service, &chunk_account).map_err(CredentialStoreError::new)?;
                chunk_entry
                    .set_password(chunk)
                    .map_err(CredentialStoreError::new)?;
            }
            trace!(
                "keyring.save success (chunked), service={service}, account={account}, chunks={count}"
            );
        } else {
            let entry = Entry::new(service, account).map_err(CredentialStoreError::new)?;
            entry
                .set_password(value)
                .map_err(CredentialStoreError::new)?;
            trace!("keyring.save success, service={service}, account={account}");
        }
        Ok(())
    }

    fn delete(&self, service: &str, account: &str) -> Result<bool, CredentialStoreError> {
        trace!("keyring.delete start, service={service}, account={account}");

        // Check if it's chunked first
        let entry = Entry::new(service, account).map_err(CredentialStoreError::new)?;
        let is_chunked = match entry.get_password() {
            Ok(value) => value.starts_with(CHUNKED_HEADER_PREFIX),
            Err(_) => false,
        };

        if is_chunked {
            let value = entry.get_password().map_err(CredentialStoreError::new)?;
            if let Some(rest) = value.strip_prefix(CHUNKED_HEADER_PREFIX) {
                if let Some((count_str, _)) = rest.split_once(':') {
                    if let Ok(count) = count_str.parse::<usize>() {
                        for i in 1..count {
                            let chunk_account = get_chunk_key(account, i);
                            let chunk_entry = Entry::new(service, &chunk_account)
                                .map_err(CredentialStoreError::new)?;
                            let _ = chunk_entry.delete_credential();
                        }
                    }
                }
            }
        }

        match entry.delete_credential() {
            Ok(()) => {
                trace!("keyring.delete success, service={service}, account={account}");
                Ok(true)
            }
            Err(keyring::Error::NoEntry) => {
                trace!("keyring.delete no entry, service={service}, account={account}");
                Ok(false)
            }
            Err(error) => {
                trace!("keyring.delete error, service={service}, account={account}, error={error}");
                Err(CredentialStoreError::new(error))
            }
        }
    }
}

pub(crate) const MAX_KEYRING_VALUE_LEN: usize = 512;
pub(crate) const CHUNKED_HEADER_PREFIX: &str = "CODEX_CHUNKED:";

pub(crate) fn get_chunk_key(account: &str, index: usize) -> String {
    format!("{account}:{index}")
}

pub mod tests {
    use super::CHUNKED_HEADER_PREFIX;
    use super::CredentialStoreError;
    use super::KeyringStore;
    use super::MAX_KEYRING_VALUE_LEN;
    use super::get_chunk_key;
    use keyring::Error as KeyringError;
    use keyring::credential::CredentialApi as _;
    use keyring::mock::MockCredential;
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::PoisonError;

    #[derive(Default, Clone, Debug)]
    pub struct MockKeyringStore {
        credentials: Arc<Mutex<HashMap<String, Arc<MockCredential>>>>,
    }

    impl MockKeyringStore {
        pub fn credential(&self, account: &str) -> Arc<MockCredential> {
            let mut guard = self
                .credentials
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            guard
                .entry(account.to_string())
                .or_insert_with(|| Arc::new(MockCredential::default()))
                .clone()
        }

        pub fn saved_value(&self, account: &str) -> Option<String> {
            let credential = {
                let guard = self
                    .credentials
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner);
                guard.get(account).cloned()
            }?;
            credential.get_password().ok()
        }

        pub fn set_error(&self, account: &str, error: KeyringError) {
            let credential = self.credential(account);
            credential.set_error(error);
        }

        pub fn contains(&self, account: &str) -> bool {
            let guard = self
                .credentials
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            guard.contains_key(account)
        }
    }

    impl KeyringStore for MockKeyringStore {
        fn load(
            &self,
            _service: &str,
            account: &str,
        ) -> Result<Option<String>, CredentialStoreError> {
            let credential = {
                let guard = self
                    .credentials
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner);
                guard.get(account).cloned()
            };

            let Some(credential) = credential else {
                return Ok(None);
            };

            let main_value = match credential.get_password() {
                Ok(password) => password,
                Err(KeyringError::NoEntry) => return Ok(None),
                Err(error) => return Err(CredentialStoreError::new(error)),
            };

            if let Some(rest) = main_value.strip_prefix(CHUNKED_HEADER_PREFIX) {
                let Some((count_str, first_chunk)) = rest.split_once(':') else {
                    return Ok(Some(main_value));
                };
                let count: usize = count_str.parse().map_err(|_| {
                    CredentialStoreError::Other(KeyringError::Invalid(
                        "failed to parse chunk count".into(),
                        "load".into(),
                    ))
                })?;

                let mut full_value = first_chunk.to_string();
                for i in 1..count {
                    let chunk_account = get_chunk_key(account, i);
                    if let Some(chunk_data) = self.saved_value(&chunk_account) {
                        full_value.push_str(&chunk_data);
                    } else {
                        return Err(CredentialStoreError::Other(KeyringError::NoEntry));
                    }
                }
                Ok(Some(full_value))
            } else {
                Ok(Some(main_value))
            }
        }

        fn save(
            &self,
            _service: &str,
            account: &str,
            value: &str,
        ) -> Result<(), CredentialStoreError> {
            if value.len() > MAX_KEYRING_VALUE_LEN {
                let chunks: Vec<String> = value
                    .chars()
                    .collect::<Vec<char>>()
                    .chunks(MAX_KEYRING_VALUE_LEN)
                    .map(|chunk| chunk.iter().collect::<String>())
                    .collect();

                let count = chunks.len();
                let header_chunk = format!("{CHUNKED_HEADER_PREFIX}{count}:{}", chunks[0]);

                let main_credential = self.credential(account);
                main_credential
                    .set_password(&header_chunk)
                    .map_err(CredentialStoreError::new)?;

                for (i, chunk) in chunks.iter().enumerate().skip(1) {
                    let chunk_account = get_chunk_key(account, i);
                    let chunk_credential = self.credential(&chunk_account);
                    chunk_credential
                        .set_password(chunk)
                        .map_err(CredentialStoreError::new)?;
                }
            } else {
                let credential = self.credential(account);
                credential
                    .set_password(value)
                    .map_err(CredentialStoreError::new)?;
            }
            Ok(())
        }

        fn delete(&self, _service: &str, account: &str) -> Result<bool, CredentialStoreError> {
            let credential = {
                let guard = self
                    .credentials
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner);
                guard.get(account).cloned()
            };

            let Some(credential) = credential else {
                return Ok(false);
            };

            let is_chunked = match credential.get_password() {
                Ok(value) => value.starts_with(CHUNKED_HEADER_PREFIX),
                Err(_) => false,
            };

            if is_chunked {
                let value = credential.get_password().map_err(CredentialStoreError::new)?;
                if let Some(rest) = value.strip_prefix(CHUNKED_HEADER_PREFIX) {
                    if let Some((count_str, _)) = rest.split_once(':') {
                        if let Ok(count) = count_str.parse::<usize>() {
                            for i in 1..count {
                                let chunk_account = get_chunk_key(account, i);
                                let mut guard = self
                                    .credentials
                                    .lock()
                                    .unwrap_or_else(PoisonError::into_inner);
                                guard.remove(&chunk_account);
                            }
                        }
                    }
                }
            }

            let removed = match credential.delete_credential() {
                Ok(()) => Ok(true),
                Err(KeyringError::NoEntry) => Ok(false),
                Err(error) => Err(CredentialStoreError::new(error)),
            }?;

            let mut guard = self
                .credentials
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            guard.remove(account);
            Ok(removed)
        }
    }

    #[test]
    fn test_mock_store_chunking() {
        let store = MockKeyringStore::default();
        let service = "test";
        let account = "account";
        
        // Use a value larger than MAX_KEYRING_VALUE_LEN (1024)
        let large_value = "ABC".repeat(500); // 1500 chars
        
        store.save(service, account, &large_value).unwrap();
        
        // Verify it was chunked by checking if the direct key contains the header
        let raw_value = store.saved_value(account).unwrap();
        assert!(raw_value.starts_with("CODEX_CHUNKED:"));
        
        // Verify we can load it back correctly
        let loaded = store.load(service, account).unwrap().unwrap();
        assert_eq!(loaded, large_value);
        
        // Verify delete cleans up everything
        store.delete(service, account).unwrap();
        assert!(!store.contains(account));
        assert!(!store.contains(&format!("{account}:1")));
    }
}
