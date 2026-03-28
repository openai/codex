use crate::CredentialStoreError;
use crate::KeyringStore;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Map;
use serde_json::Value;
use std::fmt;
use std::fmt::Write as _;
use tracing::warn;

const LAYOUT_VERSION: &str = "v1";
const MANIFEST_ENTRY: &str = "manifest";
const VALUE_ENTRY_PREFIX: &str = "value";
const ROOT_PATH_SENTINEL: &str = "root";

#[derive(Debug, Clone)]
pub struct SplitJsonKeyringError {
    message: String,
}

pub type JsonKeyringError = SplitJsonKeyringError;

impl SplitJsonKeyringError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for SplitJsonKeyringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for SplitJsonKeyringError {}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum JsonNodeKind {
    Null,
    Bool,
    Number,
    String,
    Object,
    Array,
}

impl JsonNodeKind {
    fn from_value(value: &Value) -> Self {
        match value {
            Value::Null => Self::Null,
            Value::Bool(_) => Self::Bool,
            Value::Number(_) => Self::Number,
            Value::String(_) => Self::String,
            Value::Object(_) => Self::Object,
            Value::Array(_) => Self::Array,
        }
    }

    fn is_container(self) -> bool {
        matches!(self, Self::Object | Self::Array)
    }

    fn empty_value(self) -> Option<Value> {
        match self {
            Self::Object => Some(Value::Object(Map::new())),
            Self::Array => Some(Value::Array(Vec::new())),
            Self::Null | Self::Bool | Self::Number | Self::String => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct SplitJsonNode {
    path: String,
    kind: JsonNodeKind,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct SplitJsonManifest {
    nodes: Vec<SplitJsonNode>,
}

type SplitJsonLeafValues = Vec<(String, Vec<u8>)>;

pub fn load_json_from_keyring<K: KeyringStore + ?Sized>(
    keyring_store: &K,
    service: &str,
    base_key: &str,
) -> Result<Option<Value>, JsonKeyringError> {
    let Some(manifest) = load_manifest(keyring_store, service, base_key)? else {
        return Ok(None);
    };
    inflate_split_json(keyring_store, service, base_key, &manifest).map(Some)
}

pub fn save_json_to_keyring<K: KeyringStore + ?Sized>(
    keyring_store: &K,
    service: &str,
    base_key: &str,
    value: &Value,
) -> Result<(), JsonKeyringError> {
    let previous_manifest = match load_manifest(keyring_store, service, base_key) {
        Ok(manifest) => manifest,
        Err(err) => {
            warn!("failed to read previous split JSON manifest from keyring: {err}");
            None
        }
    };
    let (manifest, leaf_values) = flatten_split_json(value)?;
    let current_scalar_paths = manifest
        .nodes
        .iter()
        .filter(|node| !node.kind.is_container())
        .map(|node| node.path.as_str())
        .collect::<std::collections::HashSet<_>>();

    for (path, bytes) in leaf_values {
        let key = value_key(base_key, &path);
        save_secret_to_keyring(
            keyring_store,
            service,
            &key,
            &bytes,
            &format!("JSON value at {path}"),
        )?;
    }

    let manifest_key = layout_key(base_key, MANIFEST_ENTRY);
    let manifest_bytes = serde_json::to_vec(&manifest).map_err(|err| {
        SplitJsonKeyringError::new(format!("failed to serialize JSON manifest: {err}"))
    })?;
    save_secret_to_keyring(
        keyring_store,
        service,
        &manifest_key,
        &manifest_bytes,
        "JSON manifest",
    )?;

    if let Some(previous_manifest) = previous_manifest {
        for node in previous_manifest.nodes {
            if node.kind.is_container() || current_scalar_paths.contains(node.path.as_str()) {
                continue;
            }
            let key = value_key(base_key, &node.path);
            if let Err(err) = delete_keyring_entry(
                keyring_store,
                service,
                &key,
                &format!("stale JSON value at {}", node.path),
            ) {
                warn!("failed to remove stale split JSON value from keyring: {err}");
            }
        }
    }

    Ok(())
}

pub fn delete_json_from_keyring<K: KeyringStore + ?Sized>(
    keyring_store: &K,
    service: &str,
    base_key: &str,
) -> Result<bool, JsonKeyringError> {
    let Some(manifest) = load_manifest(keyring_store, service, base_key)? else {
        return Ok(false);
    };

    let mut removed = false;
    for node in manifest.nodes {
        if node.kind.is_container() {
            continue;
        }
        let key = value_key(base_key, &node.path);
        removed |= delete_keyring_entry(
            keyring_store,
            service,
            &key,
            &format!("JSON value at {}", node.path),
        )?;
    }

    let manifest_key = layout_key(base_key, MANIFEST_ENTRY);
    removed |= delete_keyring_entry(keyring_store, service, &manifest_key, "JSON manifest")?;
    Ok(removed)
}

fn flatten_split_json(
    value: &Value,
) -> Result<(SplitJsonManifest, SplitJsonLeafValues), SplitJsonKeyringError> {
    let mut nodes = Vec::new();
    let mut leaf_values = Vec::new();
    collect_nodes("", value, &mut nodes, &mut leaf_values)?;
    nodes.sort_by(|left, right| {
        path_depth(&left.path)
            .cmp(&path_depth(&right.path))
            .then_with(|| left.path.cmp(&right.path))
    });
    leaf_values.sort_by(|left, right| left.0.cmp(&right.0));
    Ok((SplitJsonManifest { nodes }, leaf_values))
}

fn collect_nodes(
    path: &str,
    value: &Value,
    nodes: &mut Vec<SplitJsonNode>,
    leaf_values: &mut SplitJsonLeafValues,
) -> Result<(), SplitJsonKeyringError> {
    let kind = JsonNodeKind::from_value(value);
    nodes.push(SplitJsonNode {
        path: path.to_string(),
        kind,
    });

    match value {
        Value::Object(map) => {
            let mut keys = map.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            for key in keys {
                let child_path = append_json_pointer_token(path, &key);
                let child_value = map.get(&key).ok_or_else(|| {
                    SplitJsonKeyringError::new(format!(
                        "missing object value for path {child_path}"
                    ))
                })?;
                collect_nodes(&child_path, child_value, nodes, leaf_values)?;
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                let child_path = append_json_pointer_token(path, &index.to_string());
                collect_nodes(&child_path, item, nodes, leaf_values)?;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {
            let bytes = serde_json::to_vec(value).map_err(|err| {
                SplitJsonKeyringError::new(format!(
                    "failed to serialize JSON value at {path}: {err}"
                ))
            })?;
            leaf_values.push((path.to_string(), bytes));
        }
    }

    Ok(())
}

fn inflate_split_json<K: KeyringStore + ?Sized>(
    keyring_store: &K,
    service: &str,
    base_key: &str,
    manifest: &SplitJsonManifest,
) -> Result<Value, SplitJsonKeyringError> {
    let root_node = manifest
        .nodes
        .iter()
        .find(|node| node.path.is_empty())
        .ok_or_else(|| SplitJsonKeyringError::new("missing root JSON node in keyring manifest"))?;

    let mut result = if let Some(value) = root_node.kind.empty_value() {
        value
    } else {
        load_value(keyring_store, service, base_key, "")?
    };

    let mut nodes = manifest.nodes.clone();
    nodes.sort_by(|left, right| {
        path_depth(&left.path)
            .cmp(&path_depth(&right.path))
            .then_with(|| left.path.cmp(&right.path))
    });

    for node in nodes.into_iter().filter(|node| !node.path.is_empty()) {
        let value = if let Some(value) = node.kind.empty_value() {
            value
        } else {
            load_value(keyring_store, service, base_key, &node.path)?
        };
        insert_value_at_pointer(&mut result, &node.path, value)?;
    }

    Ok(result)
}

fn load_value<K: KeyringStore + ?Sized>(
    keyring_store: &K,
    service: &str,
    base_key: &str,
    path: &str,
) -> Result<Value, SplitJsonKeyringError> {
    let key = value_key(base_key, path);
    let bytes = load_secret_from_keyring(
        keyring_store,
        service,
        &key,
        &format!("JSON value at {path}"),
    )?
    .ok_or_else(|| {
        SplitJsonKeyringError::new(format!("missing JSON value at {path} in keyring"))
    })?;
    serde_json::from_slice(&bytes).map_err(|err| {
        SplitJsonKeyringError::new(format!("failed to deserialize JSON value at {path}: {err}"))
    })
}

fn insert_value_at_pointer(
    root: &mut Value,
    pointer: &str,
    value: Value,
) -> Result<(), SplitJsonKeyringError> {
    if pointer.is_empty() {
        *root = value;
        return Ok(());
    }

    let tokens = decode_json_pointer(pointer)?;
    let Some((last, parents)) = tokens.split_last() else {
        return Err(SplitJsonKeyringError::new(
            "missing JSON pointer path tokens",
        ));
    };

    let mut current = root;
    for token in parents {
        current = match current {
            Value::Object(map) => map.get_mut(token).ok_or_else(|| {
                SplitJsonKeyringError::new(format!(
                    "missing parent object entry for JSON pointer {pointer}"
                ))
            })?,
            Value::Array(items) => {
                let index = parse_array_index(token, pointer)?;
                items.get_mut(index).ok_or_else(|| {
                    SplitJsonKeyringError::new(format!(
                        "missing parent array entry for JSON pointer {pointer}"
                    ))
                })?
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {
                return Err(SplitJsonKeyringError::new(format!(
                    "encountered scalar while walking JSON pointer {pointer}"
                )));
            }
        };
    }

    match current {
        Value::Object(map) => {
            map.insert(last.to_string(), value);
            Ok(())
        }
        Value::Array(items) => {
            let index = parse_array_index(last, pointer)?;
            if index >= items.len() {
                items.resize(index + 1, Value::Null);
            }
            items[index] = value;
            Ok(())
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {
            Err(SplitJsonKeyringError::new(format!(
                "encountered scalar while assigning JSON pointer {pointer}"
            )))
        }
    }
}

fn load_manifest<K: KeyringStore + ?Sized>(
    keyring_store: &K,
    service: &str,
    base_key: &str,
) -> Result<Option<SplitJsonManifest>, SplitJsonKeyringError> {
    let manifest_key = layout_key(base_key, MANIFEST_ENTRY);
    let Some(bytes) =
        load_secret_from_keyring(keyring_store, service, &manifest_key, "JSON manifest")?
    else {
        return Ok(None);
    };
    let manifest: SplitJsonManifest = serde_json::from_slice(&bytes).map_err(|err| {
        SplitJsonKeyringError::new(format!("failed to deserialize JSON manifest: {err}"))
    })?;
    if manifest.nodes.is_empty() {
        return Err(SplitJsonKeyringError::new("JSON manifest is empty"));
    }
    Ok(Some(manifest))
}

fn load_secret_from_keyring<K: KeyringStore + ?Sized>(
    keyring_store: &K,
    service: &str,
    key: &str,
    field: &str,
) -> Result<Option<Vec<u8>>, SplitJsonKeyringError> {
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
) -> Result<(), SplitJsonKeyringError> {
    let value = std::str::from_utf8(value).map_err(|err| {
        SplitJsonKeyringError::new(format!("failed to encode {field} as UTF-8: {err}"))
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
) -> Result<bool, SplitJsonKeyringError> {
    keyring_store
        .delete(service, key)
        .map_err(|err| credential_store_error("delete", field, err))
}

fn credential_store_error(
    action: &str,
    field: &str,
    error: CredentialStoreError,
) -> SplitJsonKeyringError {
    SplitJsonKeyringError::new(format!(
        "failed to {action} {field} in keyring: {}",
        error.message()
    ))
}

fn layout_key(base_key: &str, suffix: &str) -> String {
    format!("{base_key}|{LAYOUT_VERSION}|{suffix}")
}

fn value_key(base_key: &str, path: &str) -> String {
    let encoded_path = encode_path(path);
    layout_key(base_key, &format!("{VALUE_ENTRY_PREFIX}|{encoded_path}"))
}

fn encode_path(path: &str) -> String {
    if path.is_empty() {
        return ROOT_PATH_SENTINEL.to_string();
    }

    let mut encoded = String::with_capacity(path.len() * 2);
    for byte in path.as_bytes() {
        let _ = write!(&mut encoded, "{byte:02x}");
    }
    encoded
}

fn append_json_pointer_token(path: &str, token: &str) -> String {
    let escaped = token.replace('~', "~0").replace('/', "~1");
    if path.is_empty() {
        format!("/{escaped}")
    } else {
        format!("{path}/{escaped}")
    }
}

fn decode_json_pointer(pointer: &str) -> Result<Vec<String>, SplitJsonKeyringError> {
    if pointer.is_empty() {
        return Ok(Vec::new());
    }
    if !pointer.starts_with('/') {
        return Err(SplitJsonKeyringError::new(format!(
            "invalid JSON pointer {pointer}: expected leading slash"
        )));
    }

    pointer[1..]
        .split('/')
        .map(unescape_json_pointer_token)
        .collect()
}

fn unescape_json_pointer_token(token: &str) -> Result<String, SplitJsonKeyringError> {
    let mut result = String::with_capacity(token.len());
    let mut chars = token.chars();

    while let Some(ch) = chars.next() {
        if ch != '~' {
            result.push(ch);
            continue;
        }

        match chars.next() {
            Some('0') => result.push('~'),
            Some('1') => result.push('/'),
            Some(other) => {
                return Err(SplitJsonKeyringError::new(format!(
                    "invalid JSON pointer escape sequence ~{other}"
                )));
            }
            None => {
                return Err(SplitJsonKeyringError::new(
                    "invalid JSON pointer escape sequence at end of token",
                ));
            }
        }
    }

    Ok(result)
}

fn parse_array_index(token: &str, pointer: &str) -> Result<usize, SplitJsonKeyringError> {
    token.parse::<usize>().map_err(|err| {
        SplitJsonKeyringError::new(format!(
            "invalid array index '{token}' in JSON pointer {pointer}: {err}"
        ))
    })
}

fn path_depth(path: &str) -> usize {
    path.chars().filter(|ch| *ch == '/').count()
}

#[cfg(test)]
mod tests {
    use super::LAYOUT_VERSION;
    use super::MANIFEST_ENTRY;
    use super::delete_json_from_keyring;
    use super::layout_key;
    use super::load_json_from_keyring;
    use super::save_json_to_keyring;
    use super::value_key;
    use crate::KeyringStore;
    use crate::tests::MockKeyringStore;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    const SERVICE: &str = "Test Service";
    const BASE_KEY: &str = "base";

    #[test]
    fn json_storage_round_trips_using_split_backend() {
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
        assert!(
            store.saved_secret(BASE_KEY).is_none(),
            "split storage should not write the full record under the base key"
        );
        assert!(
            store.contains(&layout_key(BASE_KEY, MANIFEST_ENTRY)),
            "split storage should write manifest metadata"
        );
    }

    #[test]
    fn json_storage_does_not_load_legacy_single_entry() {
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

        let loaded = load_json_from_keyring(&store, SERVICE, BASE_KEY).expect("JSON should load");
        assert_eq!(loaded, None);
    }

    #[test]
    fn json_storage_save_preserves_legacy_single_entry() {
        let store = MockKeyringStore::default();
        let current = json!({"current": true});
        let legacy = json!({"legacy": true});
        store
            .save(
                SERVICE,
                BASE_KEY,
                &serde_json::to_string(&legacy).expect("JSON should serialize"),
            )
            .expect("legacy JSON should save");

        save_json_to_keyring(&store, SERVICE, BASE_KEY, &current).expect("JSON should save");

        let loaded = load_json_from_keyring(&store, SERVICE, BASE_KEY)
            .expect("JSON should load")
            .expect("JSON should exist");
        assert_eq!(loaded, current);
        assert_eq!(
            store.saved_value(BASE_KEY),
            Some(serde_json::to_string(&legacy).expect("JSON should serialize"))
        );
    }

    #[test]
    fn json_storage_delete_removes_only_split_entries() {
        let store = MockKeyringStore::default();
        let current = json!({"current": true});
        let legacy = json!({"legacy": true});
        store
            .save(
                SERVICE,
                BASE_KEY,
                &serde_json::to_string(&legacy).expect("JSON should serialize"),
            )
            .expect("legacy JSON should save");
        save_json_to_keyring(&store, SERVICE, BASE_KEY, &current).expect("JSON should save");

        let removed = delete_json_from_keyring(&store, SERVICE, BASE_KEY)
            .expect("JSON delete should succeed");

        assert!(removed);
        assert!(
            load_json_from_keyring(&store, SERVICE, BASE_KEY)
                .expect("JSON load should succeed")
                .is_none()
        );
        assert!(store.contains(BASE_KEY));
        assert!(!store.contains(&layout_key(BASE_KEY, MANIFEST_ENTRY)));
    }

    #[test]
    fn split_json_round_trips_nested_values() {
        let store = MockKeyringStore::default();
        let expected = json!({
            "name": "codex",
            "enabled": true,
            "count": 3,
            "nested": {
                "items": [null, {"hello": "world"}],
                "slash/key": "~value~",
            },
        });

        save_json_to_keyring(&store, SERVICE, BASE_KEY, &expected).expect("split JSON should save");

        let loaded = load_json_from_keyring(&store, SERVICE, BASE_KEY)
            .expect("split JSON should load")
            .expect("split JSON should exist");
        assert_eq!(loaded, expected);
    }

    #[test]
    fn split_json_supports_scalar_root_values() {
        let store = MockKeyringStore::default();
        let expected = json!("value");

        save_json_to_keyring(&store, SERVICE, BASE_KEY, &expected).expect("split JSON should save");

        let root_value_key = value_key(BASE_KEY, "");
        assert_eq!(
            store.saved_value(&root_value_key),
            Some("\"value\"".to_string())
        );

        let loaded = load_json_from_keyring(&store, SERVICE, BASE_KEY)
            .expect("split JSON should load")
            .expect("split JSON should exist");
        assert_eq!(loaded, expected);
    }

    #[test]
    fn split_json_delete_removes_saved_entries() {
        let store = MockKeyringStore::default();
        let expected = json!({
            "token": "secret",
            "nested": {
                "id": 123,
            },
        });

        save_json_to_keyring(&store, SERVICE, BASE_KEY, &expected).expect("split JSON should save");

        let manifest_key = layout_key(BASE_KEY, MANIFEST_ENTRY);
        let token_key = value_key(BASE_KEY, "/token");
        let nested_id_key = value_key(BASE_KEY, "/nested/id");

        let removed = delete_json_from_keyring(&store, SERVICE, BASE_KEY)
            .expect("split JSON delete should succeed");

        assert!(removed);
        assert!(!store.contains(&manifest_key));
        assert!(!store.contains(&token_key));
        assert!(!store.contains(&nested_id_key));
    }

    #[test]
    fn split_json_save_replaces_previous_values() {
        let store = MockKeyringStore::default();
        let first = json!({"value": "first", "stale": true});
        let second = json!({"value": "second", "extra": 1});

        save_json_to_keyring(&store, SERVICE, BASE_KEY, &first)
            .expect("first split JSON save should succeed");
        let manifest_key = layout_key(BASE_KEY, MANIFEST_ENTRY);
        let stale_value_key = value_key(BASE_KEY, "/stale");
        assert!(store.contains(&manifest_key));
        assert!(store.contains(&stale_value_key));

        save_json_to_keyring(&store, SERVICE, BASE_KEY, &second)
            .expect("second split JSON save should succeed");
        assert!(!store.contains(&stale_value_key));
        assert!(store.contains(&manifest_key));
        assert_eq!(
            store.saved_value(&value_key(BASE_KEY, "/value")),
            Some("\"second\"".to_string())
        );
        assert_eq!(
            store.saved_value(&value_key(BASE_KEY, "/extra")),
            Some("1".to_string())
        );

        let loaded = load_json_from_keyring(&store, SERVICE, BASE_KEY)
            .expect("split JSON should load")
            .expect("split JSON should exist");
        assert_eq!(loaded, second);
    }

    #[test]
    fn split_json_uses_distinct_layout_version() {
        assert_eq!(LAYOUT_VERSION, "v1");
    }
}
