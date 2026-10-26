use super::RemotePluginCatalogCacheMode;
use super::RemotePluginDirectoryItem;
use super::RemotePluginScope;
use super::RemotePluginServiceConfig;
use chrono::DateTime;
use chrono::Utc;
use codex_login::CodexAuth;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde::de::DeserializeSeed;
use serde::de::IgnoredAny;
use serde::de::MapAccess;
use serde::de::SeqAccess;
use serde::de::Visitor;
use std::fmt;
use std::fs::File;
use std::io;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use tracing::warn;

// `plugin/list` and other callers that assume all plugins are available locally will be
// deprecated soon. Remove this catalog cache when those callers migrate to on-demand fetching.
const REMOTE_PLUGIN_CATALOG_DISK_CACHE_SCHEMA_VERSION: u8 = 1;
const REMOTE_PLUGIN_CATALOG_DISK_CACHE_DIR: &str = "cache/remote_plugin_catalog";
const REMOTE_PLUGIN_CATALOG_DISK_CACHE_TTL: Duration = Duration::from_secs(60 * 60 * 3);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct RemotePluginCatalogCacheKey {
    chatgpt_base_url: String,
    account_id: Option<String>,
    chatgpt_user_id: Option<String>,
    is_workspace_account: bool,
    // Global catalogs predate scoped cache keys and must keep their existing filenames.
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<RemotePluginScope>,
    #[serde(skip_serializing_if = "Option::is_none")]
    collection: Option<String>,
}

impl RemotePluginCatalogCacheKey {
    fn new(
        config: &RemotePluginServiceConfig,
        auth: &CodexAuth,
        scope: RemotePluginScope,
        collection: Option<&str>,
    ) -> Option<Self> {
        let cache_key = Self {
            chatgpt_base_url: config.chatgpt_base_url.clone(),
            account_id: auth.get_account_id(),
            chatgpt_user_id: auth.get_chatgpt_user_id(),
            is_workspace_account: auth.is_workspace_account(),
            scope: (scope != RemotePluginScope::Global).then_some(scope),
            collection: collection.map(str::to_owned),
        };
        // Preserve global catalog caching for existing header-auth clients, but never share
        // user or workspace catalogs when the auth mode cannot identify their owner.
        if !matches!(scope, RemotePluginScope::Global)
            && cache_key.account_id.is_none()
            && cache_key.chatgpt_user_id.is_none()
        {
            return None;
        }

        Some(cache_key)
    }
}

#[derive(Serialize, Deserialize)]
struct RemotePluginCatalogDiskCache<Plugins> {
    schema_version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fetched_at: Option<DateTime<Utc>>,
    plugins: Plugins,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RemotePluginCatalogCacheFreshness {
    Fresh,
    Stale,
}

pub(super) struct CachedDirectoryPlugins<T> {
    pub plugins: Vec<T>,
    pub freshness: RemotePluginCatalogCacheFreshness,
}

pub(super) fn has_cached_directory_plugins(
    codex_home: &Path,
    config: &RemotePluginServiceConfig,
    auth: &CodexAuth,
    scope: RemotePluginScope,
    cache_mode: RemotePluginCatalogCacheMode,
) -> bool {
    // Ignore each entry so Serde still requires `plugins` to be an array.
    load_cached_directory_plugins::<IgnoredAny>(
        codex_home, config, auth, scope, /*collection*/ None, cache_mode,
    )
    .is_some()
}

pub(super) fn load_cached_directory_plugins<T: DeserializeOwned>(
    codex_home: &Path,
    config: &RemotePluginServiceConfig,
    auth: &CodexAuth,
    scope: RemotePluginScope,
    collection: Option<&str>,
    cache_mode: RemotePluginCatalogCacheMode,
) -> Option<CachedDirectoryPlugins<T>> {
    if cache_mode == RemotePluginCatalogCacheMode::ForceRefetch {
        return None;
    }
    let cache_key = RemotePluginCatalogCacheKey::new(config, auth, scope, collection)?;
    let cache_path = cache_path(codex_home, &cache_key);
    let bytes = match read_catalog_bytes(&cache_path, cache_mode) {
        Ok(bytes) => bytes?,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return None,
        Err(err) => {
            warn!(
                cache_path = %cache_path.display(),
                "failed to read remote plugin catalog disk cache: {err}"
            );
            return None;
        }
    };
    let cache: RemotePluginCatalogDiskCache<Vec<T>> = match serde_json::from_slice(&bytes) {
        Ok(cache) => cache,
        Err(err) => {
            warn!(
                cache_path = %cache_path.display(),
                "failed to parse remote plugin catalog disk cache: {err}"
            );
            let _ = std::fs::remove_file(cache_path);
            return None;
        }
    };
    if cache.schema_version != REMOTE_PLUGIN_CATALOG_DISK_CACHE_SCHEMA_VERSION {
        let _ = std::fs::remove_file(cache_path);
        return None;
    }

    let freshness = if is_fresh(cache.fetched_at, Utc::now()) {
        RemotePluginCatalogCacheFreshness::Fresh
    } else if cache_mode == RemotePluginCatalogCacheMode::PreferFreshCache {
        return None;
    } else {
        RemotePluginCatalogCacheFreshness::Stale
    };
    Some(CachedDirectoryPlugins {
        plugins: cache.plugins,
        freshness,
    })
}

fn read_catalog_bytes(
    path: &Path,
    cache_mode: RemotePluginCatalogCacheMode,
) -> io::Result<Option<Vec<u8>>> {
    let mut file = File::open(path)?;
    let mut bytes = Vec::new();
    if cache_mode == RemotePluginCatalogCacheMode::PreferFreshCache {
        const PREFIX_LEN: usize = 256;
        bytes.reserve_exact(PREFIX_LEN);
        file.by_ref()
            .take(PREFIX_LEN as u64)
            .read_to_end(&mut bytes)?;
        if bytes.len() < PREFIX_LEN {
            return Ok(Some(bytes));
        }
        if CatalogHeader::is_stale_prefix(&bytes, Utc::now()) {
            return Ok(None);
        }
    }
    file.read_to_end(&mut bytes)?;
    Ok(Some(bytes))
}

#[derive(Default)]
struct CatalogHeader {
    schema_version: Option<u8>,
    fetched_at: Option<DateTime<Utc>>,
    metadata_before_plugins: bool,
}

impl CatalogHeader {
    fn is_stale_prefix(bytes: &[u8], now: DateTime<Utc>) -> bool {
        let mut header = Self::default();
        let mut json = serde_json::Deserializer::from_slice(bytes);
        // A bounded read can end inside the payload. Only fully decoded fields count.
        let parsed = json.deserialize_map(&mut header).and_then(|()| json.end());
        if parsed.is_err_and(|err| !err.is_eof()) {
            return false;
        }
        header.metadata_before_plugins && !is_fresh(header.fetched_at, now)
    }
}

// Only the `plugins` field uses the seed. Serde calls `visit_seq` when it enters
// that array, so a missing or wrongly typed payload still goes through the full reader.
impl<'de> DeserializeSeed<'de> for &mut CatalogHeader {
    type Value = ();

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for &mut CatalogHeader {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a plugin catalog object or its plugins array")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            SchemaVersion,
            FetchedAt,
            Plugins,
            #[serde(other)]
            Other,
        }

        let mut saw_plugins = false;
        while let Some(field) = map.next_key()? {
            match field {
                Field::SchemaVersion if self.schema_version.is_none() => {
                    self.schema_version = Some(map.next_value()?);
                }
                Field::FetchedAt if self.fetched_at.is_none() => {
                    self.fetched_at = Some(map.next_value()?);
                }
                Field::Plugins if !saw_plugins => {
                    saw_plugins = true;
                    map.next_value_seed(&mut *self)?;
                }
                Field::SchemaVersion => {
                    return Err(serde::de::Error::duplicate_field("schema_version"));
                }
                Field::FetchedAt => return Err(serde::de::Error::duplicate_field("fetched_at")),
                Field::Plugins => return Err(serde::de::Error::duplicate_field("plugins")),
                Field::Other => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        Ok(())
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut plugins: A) -> Result<(), A::Error> {
        self.metadata_before_plugins = self.schema_version
            == Some(REMOTE_PLUGIN_CATALOG_DISK_CACHE_SCHEMA_VERSION)
            && self.fetched_at.is_some();
        while plugins.next_element::<IgnoredAny>()?.is_some() {}
        Ok(())
    }
}

pub(crate) fn write_cached_directory_plugins(
    codex_home: &Path,
    config: &RemotePluginServiceConfig,
    auth: &CodexAuth,
    scope: RemotePluginScope,
    collection: Option<&str>,
    plugins: &[RemotePluginDirectoryItem],
) {
    let Some(cache_key) = RemotePluginCatalogCacheKey::new(config, auth, scope, collection) else {
        return;
    };
    let cache_path = cache_path(codex_home, &cache_key);
    let Ok(contents) = serde_json::to_string_pretty(&RemotePluginCatalogDiskCache {
        schema_version: REMOTE_PLUGIN_CATALOG_DISK_CACHE_SCHEMA_VERSION,
        fetched_at: Some(Utc::now()),
        plugins,
    }) else {
        return;
    };
    let _ = codex_utils_path::write_atomically(&cache_path, &contents);
}

pub(crate) fn remove_cached_directory_plugins(
    codex_home: &Path,
    config: &RemotePluginServiceConfig,
    auth: &CodexAuth,
    scope: RemotePluginScope,
    collection: Option<&str>,
) {
    let Some(cache_key) = RemotePluginCatalogCacheKey::new(config, auth, scope, collection) else {
        return;
    };
    let cache_path = cache_path(codex_home, &cache_key);
    match std::fs::remove_file(&cache_path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => {
            warn!(
                cache_path = %cache_path.display(),
                "failed to remove remote plugin catalog disk cache: {err}"
            );
        }
    }
}

fn cache_path(codex_home: &Path, cache_key: &RemotePluginCatalogCacheKey) -> PathBuf {
    let cache_key_json = serde_json::to_vec(cache_key).unwrap_or_default();
    let mut cache_key_hash = 0xcbf29ce484222325_u64;
    for byte in cache_key_json {
        cache_key_hash ^= u64::from(byte);
        cache_key_hash = cache_key_hash.wrapping_mul(0x100000001b3);
    }
    codex_home
        .join(REMOTE_PLUGIN_CATALOG_DISK_CACHE_DIR)
        .join(format!("{cache_key_hash:016x}.json"))
}

fn is_fresh(fetched_at: Option<DateTime<Utc>>, now: DateTime<Utc>) -> bool {
    let Some(fetched_at) = fetched_at else {
        return false;
    };
    let Ok(ttl) = chrono::Duration::from_std(REMOTE_PLUGIN_CATALOG_DISK_CACHE_TTL) else {
        return false;
    };
    let age = now.signed_duration_since(fetched_at);
    age >= chrono::Duration::zero() && age <= ttl
}

#[cfg(test)]
#[path = "catalog_cache_tests.rs"]
mod tests;
