use crate::CredentialStoreError;
use crate::KeyringStore;
use serde_json::Value;
use std::fmt;

#[derive(Debug, Clone)]
pub struct FullJsonKeyringError {
    message: String,
}

pub type JsonKeyringError = FullJsonKeyringError;

impl FullJsonKeyringError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for FullJsonKeyringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for FullJsonKeyringError {}

pub fn load_json_from_keyring<K: KeyringStore + ?Sized>(
    keyring_store: &K,
    service: &str,
    base_key: &str,
) -> Result<Option<Value>, JsonKeyringError> {
    if let Some(bytes) = load_secret_from_keyring(keyring_store, service, base_key, "JSON record")?
    {
        let value = serde_json::from_slice(&bytes).map_err(|err| {
            FullJsonKeyringError::new(format!(
                "failed to deserialize JSON record from keyring secret: {err}"
            ))
        })?;
        return Ok(Some(value));
    }

    match keyring_store.load(service, base_key) {
        Ok(Some(serialized)) => serde_json::from_str(&serialized).map(Some).map_err(|err| {
            FullJsonKeyringError::new(format!(
                "failed to deserialize JSON record from keyring password: {err}"
            ))
        }),
        Ok(None) => Ok(None),
        Err(error) => Err(credential_store_error("load", "JSON record", error)),
    }
}

pub fn save_json_to_keyring<K: KeyringStore + ?Sized>(
    keyring_store: &K,
    service: &str,
    base_key: &str,
    value: &Value,
) -> Result<(), JsonKeyringError> {
    let bytes = serde_json::to_vec(value).map_err(|err| {
        FullJsonKeyringError::new(format!("failed to serialize JSON record: {err}"))
    })?;
    save_secret_to_keyring(keyring_store, service, base_key, &bytes, "JSON record")
}

pub fn delete_json_from_keyring<K: KeyringStore + ?Sized>(
    keyring_store: &K,
    service: &str,
    base_key: &str,
) -> Result<bool, JsonKeyringError> {
    delete_keyring_entry(keyring_store, service, base_key, "JSON record")
}

fn load_secret_from_keyring<K: KeyringStore + ?Sized>(
    keyring_store: &K,
    service: &str,
    key: &str,
    field: &str,
) -> Result<Option<Vec<u8>>, FullJsonKeyringError> {
    keyring_store
        .load(service, key)
        .map(|value| value.map(String::into_bytes))
        .map_err(|err| credential_store_error("load", field, err))
}

fn save_secret_to_keyring<K: KeyringStore + ?Sized>(
    keyring_store: &K,
    service: &str,
    key: &str,
    value: &[u8],
    field: &str,
) -> Result<(), FullJsonKeyringError> {
    let value = std::str::from_utf8(value).map_err(|err| {
        FullJsonKeyringError::new(format!("failed to encode {field} as UTF-8: {err}"))
    })?;
    keyring_store
        .save(service, key, value)
        .map_err(|err| credential_store_error("write", field, err))
}

fn delete_keyring_entry<K: KeyringStore + ?Sized>(
    keyring_store: &K,
    service: &str,
    key: &str,
    field: &str,
) -> Result<bool, FullJsonKeyringError> {
    keyring_store
        .delete(service, key)
        .map_err(|err| credential_store_error("delete", field, err))
}

fn credential_store_error(
    action: &str,
    field: &str,
    error: CredentialStoreError,
) -> FullJsonKeyringError {
    FullJsonKeyringError::new(format!(
        "failed to {action} {field} in keyring: {}",
        error.message()
    ))
}

#[cfg(test)]
mod tests {
    use super::delete_json_from_keyring;
    use super::load_json_from_keyring;
    use super::save_json_to_keyring;
    use crate::KeyringStore;
    use crate::tests::MockKeyringStore;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    const SERVICE: &str = "Test Service";
    const BASE_KEY: &str = "base";

    #[test]
    fn json_storage_round_trips_using_full_backend() {
        let store = MockKeyringStore::default();
        let expected = json!({
            "token": "secret",
            "nested": {"id": 7}
        });

        save_json_to_keyring(&store, SERVICE, BASE_KEY, &expected).expect("JSON should save");

        let loaded = load_json_from_keyring(&store, SERVICE, BASE_KEY)
            .expect("JSON should load")
            .expect("JSON should exist");
        assert_eq!(loaded, expected);
        assert_eq!(
            store.saved_value(BASE_KEY),
            Some(serde_json::to_string(&expected).expect("JSON should serialize")),
        );
    }

    #[test]
    fn json_storage_loads_legacy_single_entry() {
        let store = MockKeyringStore::default();
        let expected = json!({
            "token": "secret",
            "nested": {"id": 9}
        });
        store
            .save(
                SERVICE,
                BASE_KEY,
                &serde_json::to_string(&expected).expect("JSON should serialize"),
            )
            .expect("legacy JSON should save");

        let loaded = load_json_from_keyring(&store, SERVICE, BASE_KEY)
            .expect("JSON should load")
            .expect("JSON should exist");
        assert_eq!(loaded, expected);
    }

    #[test]
    fn json_storage_delete_removes_full_entry() {
        let store = MockKeyringStore::default();
        let expected = json!({"current": true});

        save_json_to_keyring(&store, SERVICE, BASE_KEY, &expected).expect("JSON should save");

        let removed = delete_json_from_keyring(&store, SERVICE, BASE_KEY)
            .expect("JSON delete should succeed");

        assert!(removed);
        assert!(
            load_json_from_keyring(&store, SERVICE, BASE_KEY)
                .expect("JSON load should succeed")
                .is_none()
        );
        assert!(!store.contains(BASE_KEY));
    }
}
