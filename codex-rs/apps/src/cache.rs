//! Connection-scoped persistence for raw Codex Apps MCP tool inventories.
//!
//! The cache owns only protocol-level [`Tool`] values. Connector grouping and HTTP-server
//! construction remain derived state so a cache entry cannot preserve stale routing decisions.
//! Volatile private approval context is stripped before persistence and again on read.

use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use codex_utils_path::write_atomically;
use rmcp::model::Tool;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use sha1::Digest;
use sha1::Sha1;

use crate::validate_raw_tool_inventory_size;

const CACHE_DIR: &str = "cache/codex_apps_raw_tools";
const CACHE_SCHEMA_VERSION: u8 = 1;
// A normal Apps inventory is much smaller than this. Keep enough headroom for thousands of tools
// with substantial schemas while bounding allocation and JSON parsing for a corrupted local file.
const MAX_CACHE_BYTES: usize = crate::MAX_CODEX_APPS_TOOL_INVENTORY_BYTES;

const META_CODEX_APPS: &str = "_codex_apps";
const META_CONNECTED_ACCOUNT_EMAIL: &str = "connected_account_email";

/// Authenticated identity used to isolate one Apps tool inventory from another.
///
/// Field names and serialization order remain stable because they contribute to the scoped cache
/// key. Provenance-free legacy paths are intentionally not consulted.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CodexAppsCacheIdentity {
    account_id: Option<String>,
    chatgpt_user_id: Option<String>,
    is_workspace_account: bool,
}

impl CodexAppsCacheIdentity {
    pub fn with_account_id(mut self, account_id: Option<String>) -> Self {
        self.account_id = account_id;
        self
    }

    pub fn with_chatgpt_user_id(mut self, chatgpt_user_id: Option<String>) -> Self {
        self.chatgpt_user_id = chatgpt_user_id;
        self
    }

    pub fn with_workspace_account(mut self, is_workspace_account: bool) -> Self {
        self.is_workspace_account = is_workspace_account;
        self
    }
}

/// Filesystem and identity used to configure an Apps tool cache.
///
/// The upstream URL and product SKU are added by [`CodexAppsConnectConfig`](crate::CodexAppsConnectConfig)
/// when this context is installed, so callers cannot accidentally reuse one inventory across
/// hosted environments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexAppsCacheContext {
    codex_home: PathBuf,
    identity: CodexAppsCacheIdentity,
}

impl CodexAppsCacheContext {
    pub fn new(codex_home: impl Into<PathBuf>, identity: CodexAppsCacheIdentity) -> Self {
        Self {
            codex_home: codex_home.into(),
            identity,
        }
    }

    pub(crate) fn scoped(
        self,
        upstream_url: String,
        product_sku: Option<String>,
    ) -> ScopedCodexAppsCacheContext {
        ScopedCodexAppsCacheContext {
            codex_home: self.codex_home,
            key: CodexAppsCacheKey {
                identity: self.identity,
                upstream_url,
                product_sku,
            },
        }
    }
}

/// Complete cache scope. This type is intentionally crate-private: it can only be constructed
/// from a connection config that supplies every input which changes the hosted inventory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ScopedCodexAppsCacheContext {
    codex_home: PathBuf,
    key: CodexAppsCacheKey,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct CodexAppsCacheKey {
    identity: CodexAppsCacheIdentity,
    upstream_url: String,
    product_sku: Option<String>,
}

impl ScopedCodexAppsCacheContext {
    /// Loads the current scoped raw-tool cache.
    ///
    /// Missing caches return `Ok(None)`. Provenance-free caches from older versions are never
    /// consulted, because their upstream and product SKU cannot be established safely.
    pub fn load_tools(&self) -> Result<Option<Vec<Tool>>> {
        match read_cache::<ToolsDiskCache>(&self.cache_path()) {
            CacheRead::Hit(cache) if cache.schema_version == CACHE_SCHEMA_VERSION => {
                let tool_count = cache
                    .tools
                    .as_array()
                    .context("Codex Apps raw tool cache `tools` must be an array")?
                    .len();
                validate_raw_tool_inventory_size(tool_count)?;
                let tools = serde_json::from_value(cache.tools)
                    .context("failed to deserialize Codex Apps raw tool cache")?;
                Ok(Some(without_private_approval_context(tools)))
            }
            CacheRead::Hit(cache) => Err(anyhow!(
                "unsupported Codex Apps cache schema {}; expected {}",
                cache.schema_version,
                CACHE_SCHEMA_VERSION
            )),
            CacheRead::Missing => Ok(None),
            CacheRead::Invalid(error) => Err(error),
        }
    }

    /// Atomically replaces the cache with a raw MCP tool inventory.
    pub fn write_tools(&self, tools: &[Tool]) -> Result<()> {
        validate_raw_tool_inventory_size(tools.len())?;
        let cache = RawToolsDiskCache {
            schema_version: CACHE_SCHEMA_VERSION,
            tools: without_private_approval_context(tools.to_vec()),
        };
        let contents = serde_json::to_string_pretty(&cache)
            .context("failed to serialize Codex Apps raw tool cache")?;
        if contents.len() > MAX_CACHE_BYTES {
            return Err(anyhow!(
                "Codex Apps tool cache exceeds the {MAX_CACHE_BYTES}-byte limit"
            ));
        }
        let path = self.cache_path();
        write_atomically(&path, &contents).with_context(|| {
            format!(
                "failed to atomically write Codex Apps tool cache `{}`",
                path.display()
            )
        })
    }

    fn cache_path(&self) -> PathBuf {
        self.codex_home
            .join(CACHE_DIR)
            .join(format!("{}.json", cache_key_hash(&self.key)))
    }
}

fn without_private_approval_context(mut tools: Vec<Tool>) -> Vec<Tool> {
    for tool in &mut tools {
        let Some(meta) = tool.meta.as_mut() else {
            continue;
        };
        meta.remove(codex_protocol::mcp::MCP_APPROVAL_CONTEXT_META_KEY);
        if let Some(JsonValue::Object(source)) = meta.get_mut(META_CODEX_APPS) {
            source.remove(META_CONNECTED_ACCOUNT_EMAIL);
        }
    }
    tools
}

#[derive(Clone, Debug, Serialize)]
struct RawToolsDiskCache {
    schema_version: u8,
    tools: Vec<Tool>,
}

#[derive(Debug, Deserialize)]
struct ToolsDiskCache {
    schema_version: u8,
    tools: JsonValue,
}

enum CacheRead<T> {
    Missing,
    Hit(T),
    Invalid(anyhow::Error),
}

fn read_cache<T>(path: &Path) -> CacheRead<T>
where
    T: for<'de> Deserialize<'de>,
{
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return CacheRead::Missing;
        }
        Err(error) => {
            return CacheRead::Invalid(anyhow::Error::from(error).context(format!(
                "failed to read Codex Apps cache `{}`",
                path.display()
            )));
        }
    };
    let mut bytes = Vec::new();
    if let Err(error) = file
        .take(MAX_CACHE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
    {
        return CacheRead::Invalid(anyhow::Error::from(error).context(format!(
            "failed to read Codex Apps cache `{}`",
            path.display()
        )));
    }
    if bytes.len() > MAX_CACHE_BYTES {
        return CacheRead::Invalid(anyhow!(
            "Codex Apps cache `{}` exceeds the {MAX_CACHE_BYTES}-byte limit",
            path.display()
        ));
    }
    match serde_json::from_slice(&bytes) {
        Ok(cache) => CacheRead::Hit(cache),
        Err(error) => CacheRead::Invalid(anyhow::Error::from(error).context(format!(
            "failed to read Codex Apps cache `{}`",
            path.display()
        ))),
    }
}

fn cache_key_hash(key: &CodexAppsCacheKey) -> String {
    stable_json_hash(key)
}

fn stable_json_hash(value: &impl Serialize) -> String {
    let identity_json = match serde_json::to_string(value) {
        Ok(identity_json) => identity_json,
        Err(error) => {
            unreachable!("Codex Apps cache keys contain only JSON-serializable fields: {error}")
        }
    };
    let mut hasher = Sha1::new();
    hasher.update(identity_json.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;
