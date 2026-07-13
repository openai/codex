//! Bounded, atomic persistence for connector runtime snapshots.

use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
#[cfg(test)]
use std::sync::Arc;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Context;
use anyhow::anyhow;
use codex_protocol::mcp::McpServerInfo;
use serde::Deserialize;
use serde::Serialize;
use sha1::Digest;
use sha1::Sha1;
use tempfile::NamedTempFile;
use tracing::instrument;

use super::ConnectorRuntimeContext;
use super::ConnectorRuntimeIdentity;
use super::ConnectorRuntimePayload;
use super::ConnectorRuntimeSnapshot;
use super::emit_duration;

const MCP_TOOLS_CACHE_WRITE_DURATION_METRIC: &str = "codex.mcp.tools.cache_write.duration_ms";
const CODEX_APPS_TOOLS_CACHE_DIR: &str = "cache/codex_apps_tools";
pub(crate) const CODEX_APPS_TOOLS_CACHE_SCHEMA_VERSION: u8 = 4;
const CODEX_APPS_SERVER_INFO_CACHE_DIR: &str = "cache/codex_apps_server_info";
const CODEX_APPS_SERVER_INFO_CACHE_SCHEMA_VERSION: u8 = 1;
pub(crate) const CODEX_APPS_TOOLS_CACHE_MAX_BYTES: u64 = 32 * 1024 * 1024;

pub(crate) fn tools_cache_path(identity: &ConnectorRuntimeIdentity) -> PathBuf {
    cache_path_in(identity, CODEX_APPS_TOOLS_CACHE_DIR)
}

pub(crate) fn server_info_cache_path(identity: &ConnectorRuntimeIdentity) -> PathBuf {
    cache_path_in(identity, CODEX_APPS_SERVER_INFO_CACHE_DIR)
}

fn cache_path_in(identity: &ConnectorRuntimeIdentity, cache_dir: &str) -> PathBuf {
    // `codex_home` is already the parent directory. Keep it out of the
    // filename hash so non-UTF-8 Unix paths cannot collapse distinct auth keys.
    let identity_json = serde_json::to_string(&identity.key).unwrap_or_default();
    let identity_hash = sha1_hex(&identity_json);
    identity
        .codex_home
        .join(cache_dir)
        .join(format!("{identity_hash}.json"))
}

#[instrument(level = "trace", skip_all)]
pub(crate) fn load_cached_connector_runtime_for_identity<T: ConnectorRuntimePayload>(
    identity: &ConnectorRuntimeIdentity,
) -> Option<ConnectorRuntimeSnapshot<T>> {
    let cache_path = tools_cache_path(identity);
    let (bytes, modified_at) = read_bounded_cache_file(&cache_path).ok()?;
    let cache: CodexAppsToolsDiskCache<T> = serde_json::from_slice(&bytes).ok()?;
    (cache.schema_version == CODEX_APPS_TOOLS_CACHE_SCHEMA_VERSION).then_some(
        ConnectorRuntimeSnapshot {
            tools: cache.tools,
            refreshed_at: modified_at,
        },
    )
}

pub(crate) fn write_cached_connector_runtime<T>(
    cache_context: &ConnectorRuntimeContext<T>,
    snapshot: &ConnectorRuntimeSnapshot<T>,
) -> anyhow::Result<()>
where
    T: ConnectorRuntimePayload,
{
    let cache_path = cache_context.tools_cache_path();
    let bytes = serde_json::to_vec_pretty(&CodexAppsToolsDiskCache {
        schema_version: CODEX_APPS_TOOLS_CACHE_SCHEMA_VERSION,
        tools: snapshot.tools.clone(),
    })
    .context("failed to serialize connector runtime cache")?;
    write_codex_apps_cache_file(&cache_path, "runtime", bytes)
}

#[instrument(level = "trace", skip_all)]
pub(crate) fn load_cached_codex_apps_server_info<T: ConnectorRuntimePayload>(
    cache_context: &ConnectorRuntimeContext<T>,
) -> Option<McpServerInfo> {
    let (bytes, _) = read_bounded_cache_file(&cache_context.server_info_cache_path()).ok()?;
    let cache: CodexAppsServerInfoDiskCache = serde_json::from_slice(&bytes).ok()?;
    (cache.schema_version == CODEX_APPS_SERVER_INFO_CACHE_SCHEMA_VERSION)
        .then_some(cache.server_info)
}

fn write_cached_codex_apps_server_info<T: ConnectorRuntimePayload>(
    cache_context: &ConnectorRuntimeContext<T>,
    server_info: &McpServerInfo,
) -> anyhow::Result<()> {
    let cache_path = cache_context.server_info_cache_path();
    let bytes = serde_json::to_vec_pretty(&CodexAppsServerInfoDiskCache {
        schema_version: CODEX_APPS_SERVER_INFO_CACHE_SCHEMA_VERSION,
        server_info: server_info.clone(),
    })
    .context("failed to serialize Codex Apps server info cache")?;
    write_codex_apps_cache_file(&cache_path, "server info", bytes)
}

pub(crate) fn persist_codex_apps_cache<T>(
    cache_context: &ConnectorRuntimeContext<T>,
    server_info: &McpServerInfo,
    snapshot: &ConnectorRuntimeSnapshot<T>,
) where
    T: ConnectorRuntimePayload,
{
    let cache_write_start = Instant::now();
    let tools_result = write_cached_connector_runtime(cache_context, snapshot);
    if let Err(err) = &tools_result {
        tracing::warn!("failed to write connector runtime cache: {err:#}");
    }
    let server_info_result = write_cached_codex_apps_server_info(cache_context, server_info);
    if let Err(err) = &server_info_result {
        tracing::warn!("failed to write Codex Apps server info cache: {err:#}");
    }
    let status = if tools_result.is_ok() && server_info_result.is_ok() {
        "success"
    } else {
        "failure"
    };
    emit_duration(
        MCP_TOOLS_CACHE_WRITE_DURATION_METRIC,
        cache_write_start.elapsed(),
        &[("status", status)],
    );
}

fn read_bounded_cache_file(cache_path: &Path) -> anyhow::Result<(Vec<u8>, SystemTime)> {
    let mut file = File::open(cache_path)
        .with_context(|| format!("failed to open cache `{}`", cache_path.display()))?;
    let metadata = file
        .metadata()
        .with_context(|| format!("failed to stat cache `{}`", cache_path.display()))?;
    if metadata.len() > CODEX_APPS_TOOLS_CACHE_MAX_BYTES {
        return Err(anyhow!(
            "cache `{}` is {} bytes, exceeding the {} byte limit",
            cache_path.display(),
            metadata.len(),
            CODEX_APPS_TOOLS_CACHE_MAX_BYTES
        ));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    std::io::Read::by_ref(&mut file)
        .take(CODEX_APPS_TOOLS_CACHE_MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .with_context(|| format!("failed to read cache `{}`", cache_path.display()))?;
    if bytes.len() as u64 > CODEX_APPS_TOOLS_CACHE_MAX_BYTES {
        return Err(anyhow!(
            "cache `{}` grew beyond the {} byte limit while reading",
            cache_path.display(),
            CODEX_APPS_TOOLS_CACHE_MAX_BYTES
        ));
    }
    Ok((bytes, metadata.modified().unwrap_or(UNIX_EPOCH)))
}

fn write_codex_apps_cache_file(
    cache_path: &Path,
    cache_name: &str,
    bytes: Vec<u8>,
) -> anyhow::Result<()> {
    let parent = cache_path.parent().ok_or_else(|| {
        anyhow!(
            "Codex Apps {cache_name} cache path `{}` has no parent",
            cache_path.display()
        )
    })?;
    std::fs::create_dir_all(parent).with_context(|| {
        format!(
            "failed to create Codex Apps {cache_name} cache directory `{}`",
            parent.display()
        )
    })?;
    let mut temporary = NamedTempFile::new_in(parent).with_context(|| {
        format!(
            "failed to create temporary Codex Apps {cache_name} cache in `{}`",
            parent.display()
        )
    })?;
    temporary.write_all(&bytes).with_context(|| {
        format!(
            "failed to write temporary Codex Apps {cache_name} cache for `{}`",
            cache_path.display()
        )
    })?;
    temporary.persist(cache_path).map_err(|error| {
        anyhow!(
            "failed to atomically replace Codex Apps {cache_name} cache `{}`: {}",
            cache_path.display(),
            error.error
        )
    })?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CodexAppsToolsDiskCache<T> {
    schema_version: u8,
    tools: Vec<T>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CodexAppsServerInfoDiskCache {
    schema_version: u8,
    server_info: McpServerInfo,
}

fn sha1_hex(s: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(s.as_bytes());
    let sha1 = hasher.finalize();
    format!("{sha1:x}")
}

#[cfg(test)]
pub(crate) fn write_cached_codex_apps_tools_for_test<T>(
    cache_context: &ConnectorRuntimeContext<T>,
    server_info: &McpServerInfo,
    tools: &[T],
) where
    T: ConnectorRuntimePayload,
{
    let snapshot = ConnectorRuntimeSnapshot {
        tools: tools.to_vec(),
        refreshed_at: SystemTime::now(),
    };
    cache_context
        .entry
        .current_snapshot
        .store(Some(Arc::new(snapshot.clone())));
    persist_codex_apps_cache(cache_context, server_info, &snapshot);
}

#[cfg(test)]
pub(crate) fn read_cached_codex_apps_tools<T>(
    cache_context: &ConnectorRuntimeContext<T>,
) -> Option<Vec<T>>
where
    T: ConnectorRuntimePayload,
{
    load_cached_connector_runtime_for_identity(&cache_context.entry.identity)
        .map(|snapshot| snapshot.tools)
}

#[cfg(test)]
pub(crate) fn write_cached_codex_apps_tools<T>(
    cache_context: &ConnectorRuntimeContext<T>,
    tools: &[T],
) -> anyhow::Result<()>
where
    T: ConnectorRuntimePayload,
{
    let snapshot = ConnectorRuntimeSnapshot {
        tools: tools.to_vec(),
        refreshed_at: SystemTime::now(),
    };
    write_cached_connector_runtime(cache_context, &snapshot)
}
