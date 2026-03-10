use crate::CredentialStoreError;
use crate::KeyringStore;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Map;
use serde_json::Value;
use std::fmt;
use std::fmt::Write as _;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use tracing::warn;

const LAYOUT_VERSION: &str = "v1";
const ACTIVE_REVISION_ENTRY: &str = "active";
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

#[cfg(windows)]
pub fn load_json_from_keyring<K: KeyringStore + ?Sized>(
    keyring_store: &K,
    service: &str,
    base_key: &str,
) -> Result<Option<Value>, JsonKeyringError> {
    if let Some(value) = load_split_json_from_keyring(keyring_store, service, base_key)? {
        return Ok(Some(value));
    }
    load_full_json_from_keyring(keyring_store, service, base_key)
}

#[cfg(not(windows))]
pub fn load_json_from_keyring<K: KeyringStore + ?Sized>(
    keyring_store: &K,
    service: &str,
    base_key: &str,
) -> Result<Option<Value>, JsonKeyringError> {
    if let Some(value) = load_full_json_from_keyring(keyring_store, service, base_key)? {
        return Ok(Some(value));
    }
    load_split_json_from_keyring(keyring_store, service, base_key)
}

#[cfg(windows)]
pub fn save_json_to_keyring<K: KeyringStore + ?Sized>(
    keyring_store: &K,
    service: &str,
    base_key: &str,
    value: &Value,
) -> Result<(), JsonKeyringError> {
    save_split_json_to_keyring(keyring_store, service, base_key, value)?;
    if let Err(err) = delete_full_json_from_keyring(keyring_store, service, base_key) {
        warn!("failed to remove stale full JSON record from keyring: {err}");
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn save_json_to_keyring<K: KeyringStore + ?Sized>(
    keyring_store: &K,
    service: &str,
    base_key: &str,
    value: &Value,
) -> Result<(), JsonKeyringError> {
    save_full_json_to_keyring(keyring_store, service, base_key, value)?;
    if let Err(err) = delete_split_json_from_keyring(keyring_store, service, base_key) {
        warn!("failed to remove stale split JSON record from keyring: {err}");
    }
    Ok(())
}

#[cfg(windows)]
pub fn delete_json_from_keyring<K: KeyringStore + ?Sized>(
    keyring_store: &K,
    service: &str,
    base_key: &str,
) -> Result<bool, JsonKeyringError> {
    let split_removed = delete_split_json_from_keyring(keyring_store, service, base_key)?;
    let full_removed = delete_full_json_from_keyring(keyring_store, service, base_key)?;
    Ok(split_removed || full_removed)
}

#[cfg(not(windows))]
pub fn delete_json_from_keyring<K: KeyringStore + ?Sized>(
    keyring_store: &K,
    service: &str,
    base_key: &str,
) -> Result<bool, JsonKeyringError> {
    let full_removed = delete_full_json_from_keyring(keyring_store, service, base_key)?;
    let split_removed = delete_split_json_from_keyring(keyring_store, service, base_key)?;
    Ok(full_removed || split_removed)
}

pub fn load_split_json_from_keyring<K: KeyringStore + ?Sized>(
    keyring_store: &K,
    service: &str,
    base_key: &str,
) -> Result<Option<Value>, SplitJsonKeyringError> {
    let Some(revision) = load_active_revision(keyring_store, service, base_key)? else {
        return Ok(None);
    };
    let manifest = load_manifest(keyring_store, service, base_key, &revision)?;
    inflate_split_json(keyring_store, service, base_key, &revision, &manifest).map(Some)
}

pub fn save_split_json_to_keyring<K: KeyringStore + ?Sized>(
    keyring_store: &K,
    service: &str,
    base_key: &str,
    value: &Value,
) -> Result<(), SplitJsonKeyringError> {
    let previous_revision = match load_active_revision(keyring_store, service, base_key) {
        Ok(revision) => revision,
        Err(err) => {
            warn!("failed to read previous split JSON revision from keyring: {err}");
            None
        }
    };
    let revision = next_revision();
    let (manifest, leaf_values) = flatten_split_json(value)?;

    for (path, bytes) in leaf_values {
        let key = value_key(base_key, &revision, &path);
        save_secret_to_keyring(
            keyring_store,
            service,
            &key,
            &bytes,
            &format!("JSON value at {path}"),
        )?;
    }

    let manifest_key = revision_key(base_key, &revision, MANIFEST_ENTRY);
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

    let active_key = layout_key(base_key, ACTIVE_REVISION_ENTRY);
    save_secret_to_keyring(
        keyring_store,
        service,
        &active_key,
        revision.as_bytes(),
        "active split JSON revision",
    )?;

    if let Some(previous_revision) = previous_revision
        && previous_revision != revision
        && let Err(err) =
            delete_revision_entries(keyring_store, service, base_key, &previous_revision)
    {
        warn!("failed to remove stale split JSON revision from keyring: {err}");
    }

    Ok(())
}

pub fn delete_split_json_from_keyring<K: KeyringStore + ?Sized>(
    keyring_store: &K,
    service: &str,
    base_key: &str,
) -> Result<bool, SplitJsonKeyringError> {
    let Some(revision) = load_active_revision(keyring_store, service, base_key)? else {
        return Ok(false);
    };

    let mut removed = delete_revision_entries(keyring_store, service, base_key, &revision)?;
    let active_key = layout_key(base_key, ACTIVE_REVISION_ENTRY);
    removed |= delete_keyring_entry(
        keyring_store,
        service,
        &active_key,
        "active split JSON revision",
    )?;
    Ok(removed)
}

fn load_full_json_from_keyring<K: KeyringStore + ?Sized>(
    keyring_store: &K,
    service: &str,
    base_key: &str,
) -> Result<Option<Value>, SplitJsonKeyringError> {
    if let Some(bytes) = load_secret_from_keyring(keyring_store, service, base_key, "JSON record")?
    {
        let value = serde_json::from_slice(&bytes).map_err(|err| {
            SplitJsonKeyringError::new(format!(
                "failed to deserialize JSON record from keyring secret: {err}"
            ))
        })?;
        return Ok(Some(value));
    }

    match keyring_store.load(service, base_key) {
        Ok(Some(serialized)) => serde_json::from_str(&serialized).map(Some).map_err(|err| {
            SplitJsonKeyringError::new(format!(
                "failed to deserialize JSON record from keyring password: {err}"
            ))
        }),
        Ok(None) => Ok(None),
        Err(error) => Err(credential_store_error("load", "JSON record", error)),
    }
}

#[cfg(not(windows))]
fn save_full_json_to_keyring<K: KeyringStore + ?Sized>(
    keyring_store: &K,
    service: &str,
    base_key: &str,
    value: &Value,
) -> Result<(), SplitJsonKeyringError> {
    let bytes = serde_json::to_vec(value).map_err(|err| {
        SplitJsonKeyringError::new(format!("failed to serialize JSON record: {err}"))
    })?;
    save_secret_to_keyring(keyring_store, service, base_key, &bytes, "JSON record")
}

fn delete_full_json_from_keyring<K: KeyringStore + ?Sized>(
    keyring_store: &K,
    service: &str,
    base_key: &str,
) -> Result<bool, SplitJsonKeyringError> {
    delete_keyring_entry(keyring_store, service, base_key, "JSON record")
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
    revision: &str,
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
        load_value(keyring_store, service, base_key, revision, "")?
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
            load_value(keyring_store, service, base_key, revision, &node.path)?
        };
        insert_value_at_pointer(&mut result, &node.path, value)?;
    }

    Ok(result)
}

fn load_value<K: KeyringStore + ?Sized>(
    keyring_store: &K,
    service: &str,
    base_key: &str,
    revision: &str,
    path: &str,
) -> Result<Value, SplitJsonKeyringError> {
    let key = value_key(base_key, revision, path);
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

fn delete_revision_entries<K: KeyringStore + ?Sized>(
    keyring_store: &K,
    service: &str,
    base_key: &str,
    revision: &str,
) -> Result<bool, SplitJsonKeyringError> {
    let manifest = load_manifest(keyring_store, service, base_key, revision)?;
    let mut removed = false;

    for node in manifest.nodes {
        if node.kind.is_container() {
            continue;
        }
        let key = value_key(base_key, revision, &node.path);
        removed |= delete_keyring_entry(
            keyring_store,
            service,
            &key,
            &format!("JSON value at {}", node.path),
        )?;
    }

    let manifest_key = revision_key(base_key, revision, MANIFEST_ENTRY);
    removed |= delete_keyring_entry(keyring_store, service, &manifest_key, "JSON manifest")?;
    Ok(removed)
}

fn load_manifest<K: KeyringStore + ?Sized>(
    keyring_store: &K,
    service: &str,
    base_key: &str,
    revision: &str,
) -> Result<SplitJsonManifest, SplitJsonKeyringError> {
    let manifest_key = revision_key(base_key, revision, MANIFEST_ENTRY);
    let bytes = load_secret_from_keyring(keyring_store, service, &manifest_key, "JSON manifest")?
        .ok_or_else(|| SplitJsonKeyringError::new("missing JSON manifest in keyring"))?;
    let manifest: SplitJsonManifest = serde_json::from_slice(&bytes).map_err(|err| {
        SplitJsonKeyringError::new(format!("failed to deserialize JSON manifest: {err}"))
    })?;
    if manifest.nodes.is_empty() {
        return Err(SplitJsonKeyringError::new("JSON manifest is empty"));
    }
    Ok(manifest)
}

fn load_active_revision<K: KeyringStore + ?Sized>(
    keyring_store: &K,
    service: &str,
    base_key: &str,
) -> Result<Option<String>, SplitJsonKeyringError> {
    let active_key = layout_key(base_key, ACTIVE_REVISION_ENTRY);
    load_utf8_secret_from_keyring(
        keyring_store,
        service,
        &active_key,
        "active split JSON revision",
    )
}

fn load_utf8_secret_from_keyring<K: KeyringStore + ?Sized>(
    keyring_store: &K,
    service: &str,
    key: &str,
    field: &str,
) -> Result<Option<String>, SplitJsonKeyringError> {
    let Some(secret) = load_secret_from_keyring(keyring_store, service, key, field)? else {
        return Ok(None);
    };
    String::from_utf8(secret).map(Some).map_err(|err| {
        SplitJsonKeyringError::new(format!(
            "failed to decode {field} from keyring as UTF-8: {err}"
        ))
    })
}

fn load_secret_from_keyring<K: KeyringStore + ?Sized>(
    keyring_store: &K,
    service: &str,
    key: &str,
    field: &str,
) -> Result<Option<Vec<u8>>, SplitJsonKeyringError> {
    keyring_store
        .load_secret(service, key)
        .map_err(|err| credential_store_error("load", field, err))
}

fn save_secret_to_keyring<K: KeyringStore + ?Sized>(
    keyring_store: &K,
    service: &str,
    key: &str,
    value: &[u8],
    field: &str,
) -> Result<(), SplitJsonKeyringError> {
    keyring_store
        .save_secret(service, key, value)
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

fn next_revision() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_nanos()
        .to_string()
}

fn layout_key(base_key: &str, suffix: &str) -> String {
    format!("{base_key}|{LAYOUT_VERSION}|{suffix}")
}

fn revision_key(base_key: &str, revision: &str, suffix: &str) -> String {
    format!("{base_key}|{LAYOUT_VERSION}|{revision}|{suffix}")
}

fn value_key(base_key: &str, revision: &str, path: &str) -> String {
    let encoded_path = encode_path(path);
    let suffix = format!("{VALUE_ENTRY_PREFIX}|{encoded_path}");
    revision_key(base_key, revision, &suffix)
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
    use super::ACTIVE_REVISION_ENTRY;
    use super::LAYOUT_VERSION;
    use super::MANIFEST_ENTRY;
    use super::ROOT_PATH_SENTINEL;
    use super::VALUE_ENTRY_PREFIX;
    use super::delete_json_from_keyring;
    use super::delete_split_json_from_keyring;
    use super::layout_key;
    use super::load_json_from_keyring;
    use super::load_split_json_from_keyring;
    use super::revision_key;
    use super::save_json_to_keyring;
    use super::save_split_json_to_keyring;
    use super::value_key;
    use crate::KeyringStore;
    use crate::tests::MockKeyringStore;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    const SERVICE: &str = "Test Service";
    const BASE_KEY: &str = "base";

    #[test]
    fn json_storage_round_trips_using_platform_backend() {
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

        #[cfg(windows)]
        {
            assert!(
                store.saved_secret(BASE_KEY).is_none(),
                "windows should not store the full JSON record under the base key"
            );
            assert!(
                store.contains(&layout_key(BASE_KEY, ACTIVE_REVISION_ENTRY)),
                "windows should track an active split revision"
            );
        }

        #[cfg(not(windows))]
        {
            assert_eq!(
                store.saved_secret(BASE_KEY),
                Some(serde_json::to_vec(&expected).expect("JSON should serialize")),
            );
            assert!(
                !store.contains(&layout_key(BASE_KEY, ACTIVE_REVISION_ENTRY)),
                "non-windows should not create split revision metadata"
            );
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn json_storage_loads_split_json_compatibility_on_non_windows() {
        let store = MockKeyringStore::default();
        let expected = json!({
            "token": "secret",
            "nested": {"id": 9}
        });

        save_split_json_to_keyring(&store, SERVICE, BASE_KEY, &expected)
            .expect("split JSON should save");

        let loaded = load_json_from_keyring(&store, SERVICE, BASE_KEY)
            .expect("JSON should load")
            .expect("JSON should exist");
        assert_eq!(loaded, expected);
    }

    #[test]
    fn json_storage_delete_removes_platform_and_compat_entries() {
        let store = MockKeyringStore::default();
        let current = json!({"current": true});
        let split = json!({"split": true});

        save_json_to_keyring(&store, SERVICE, BASE_KEY, &current).expect("JSON should save");
        save_split_json_to_keyring(&store, SERVICE, BASE_KEY, &split)
            .expect("split JSON should save");
        store
            .save(
                SERVICE,
                BASE_KEY,
                &serde_json::to_string(&current).expect("JSON should serialize"),
            )
            .expect("legacy JSON should save");

        let removed = delete_json_from_keyring(&store, SERVICE, BASE_KEY)
            .expect("JSON delete should succeed");

        assert!(removed);
        assert!(
            load_json_from_keyring(&store, SERVICE, BASE_KEY)
                .expect("JSON load should succeed")
                .is_none()
        );
        assert!(!store.contains(BASE_KEY));
        assert!(!store.contains(&layout_key(BASE_KEY, ACTIVE_REVISION_ENTRY)));
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

        save_split_json_to_keyring(&store, SERVICE, BASE_KEY, &expected)
            .expect("split JSON should save");

        let loaded = load_split_json_from_keyring(&store, SERVICE, BASE_KEY)
            .expect("split JSON should load")
            .expect("split JSON should exist");
        assert_eq!(loaded, expected);
    }

    #[test]
    fn split_json_supports_scalar_root_values() {
        let store = MockKeyringStore::default();
        let expected = json!("value");

        save_split_json_to_keyring(&store, SERVICE, BASE_KEY, &expected)
            .expect("split JSON should save");

        let revision = store
            .saved_secret_utf8(&layout_key(BASE_KEY, ACTIVE_REVISION_ENTRY))
            .expect("active revision should exist");
        let root_value_key = revision_key(
            BASE_KEY,
            &revision,
            &format!("{VALUE_ENTRY_PREFIX}|{ROOT_PATH_SENTINEL}"),
        );
        assert_eq!(
            store.saved_secret_utf8(&root_value_key),
            Some("\"value\"".to_string())
        );

        let loaded = load_split_json_from_keyring(&store, SERVICE, BASE_KEY)
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

        save_split_json_to_keyring(&store, SERVICE, BASE_KEY, &expected)
            .expect("split JSON should save");

        let revision = store
            .saved_secret_utf8(&layout_key(BASE_KEY, ACTIVE_REVISION_ENTRY))
            .expect("active revision should exist");
        let manifest_key = revision_key(BASE_KEY, &revision, MANIFEST_ENTRY);
        let token_key = value_key(BASE_KEY, &revision, "/token");
        let nested_id_key = value_key(BASE_KEY, &revision, "/nested/id");

        let removed = delete_split_json_from_keyring(&store, SERVICE, BASE_KEY)
            .expect("split JSON delete should succeed");

        assert!(removed);
        assert!(!store.contains(&layout_key(BASE_KEY, ACTIVE_REVISION_ENTRY)));
        assert!(!store.contains(&manifest_key));
        assert!(!store.contains(&token_key));
        assert!(!store.contains(&nested_id_key));
    }

    #[test]
    fn split_json_save_replaces_previous_revision() {
        let store = MockKeyringStore::default();
        let first = json!({"value": "first"});
        let second = json!({"value": "second", "extra": 1});

        save_split_json_to_keyring(&store, SERVICE, BASE_KEY, &first)
            .expect("first split JSON save should succeed");
        let first_revision = store
            .saved_secret_utf8(&layout_key(BASE_KEY, ACTIVE_REVISION_ENTRY))
            .expect("first revision should exist");
        let first_manifest_key = revision_key(BASE_KEY, &first_revision, MANIFEST_ENTRY);
        let first_value_key = value_key(BASE_KEY, &first_revision, "/value");

        save_split_json_to_keyring(&store, SERVICE, BASE_KEY, &second)
            .expect("second split JSON save should succeed");
        let second_revision = store
            .saved_secret_utf8(&layout_key(BASE_KEY, ACTIVE_REVISION_ENTRY))
            .expect("second revision should exist");
        let second_manifest_key = revision_key(BASE_KEY, &second_revision, MANIFEST_ENTRY);

        assert_ne!(first_revision, second_revision);
        assert!(!store.contains(&first_manifest_key));
        assert!(!store.contains(&first_value_key));
        assert!(store.contains(&second_manifest_key));

        let loaded = load_split_json_from_keyring(&store, SERVICE, BASE_KEY)
            .expect("split JSON should load")
            .expect("split JSON should exist");
        assert_eq!(loaded, second);
    }

    #[test]
    fn split_json_uses_distinct_layout_version() {
        assert_eq!(LAYOUT_VERSION, "v1");
    }
}
