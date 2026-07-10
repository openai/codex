//! Shared runtime snapshot for the host-owned Codex Apps MCP server.
//!
//! Runtime snapshots are process-local live state scoped by the active Codex
//! auth context. Disk is best-effort cold-start persistence; a context reads it
//! once when activated and never rereads it. Full connector metadata is owned by
//! the connector metadata store, not by this module.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;

use arc_swap::ArcSwapOption;
use codex_login::CodexAuth;
use codex_protocol::mcp::McpServerInfo;
use serde::Deserialize;
use serde::Serialize;
use tokio_util::sync::CancellationToken;

use crate::connector_runtime_persistence::load_cached_codex_apps_server_info;
use crate::connector_runtime_persistence::load_cached_connector_runtime_for_identity;
use crate::connector_runtime_persistence::persist_codex_apps_cache;
use crate::connector_runtime_persistence::server_info_cache_path;
use crate::connector_runtime_persistence::tools_cache_path;
use crate::runtime::emit_duration;
use crate::tools::ToolInfo;

const MCP_TOOLS_CACHE_PUBLISH_DURATION_METRIC: &str = "codex.mcp.tools.cache_publish.duration_ms";

/// One atomically published connector runtime state.
///
/// Tools remain raw and in response order. Local and managed configuration is
/// intentionally applied by readers rather than persisted in this snapshot.
#[derive(Debug, Clone)]
pub struct ConnectorRuntimeSnapshot {
    pub(crate) tools: Vec<ToolInfo>,
    pub(crate) refreshed_at: SystemTime,
}

impl ConnectorRuntimeSnapshot {
    pub fn tools(&self) -> &[ToolInfo] {
        &self.tools
    }

    pub fn refreshed_at(&self) -> SystemTime {
        self.refreshed_at
    }

    pub fn age(&self) -> Duration {
        SystemTime::now()
            .duration_since(self.refreshed_at)
            .unwrap_or_default()
    }
}

/// The CodexAuth bits that identify a connector runtime catalog.
///
/// Debug bearer-token overrides bypass the shared runtime manager, so shared
/// snapshots only need the CodexAuth-backed identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConnectorRuntimeContextKey {
    pub(crate) account_id: Option<String>,
    pub(crate) chatgpt_user_id: Option<String>,
    pub(crate) is_workspace_account: bool,
}

/// Builds the CodexAuth-backed connector runtime context key.
pub fn connector_runtime_context_key(auth: Option<&CodexAuth>) -> ConnectorRuntimeContextKey {
    ConnectorRuntimeContextKey {
        account_id: auth.and_then(CodexAuth::get_account_id),
        chatgpt_user_id: auth.and_then(CodexAuth::get_chatgpt_user_id),
        is_workspace_account: auth.is_some_and(CodexAuth::is_workspace_account),
    }
}

/// Compatibility alias for existing cache call sites.
pub type CodexAppsToolsCacheKey = ConnectorRuntimeContextKey;

/// Compatibility helper for existing cache call sites.
pub fn codex_apps_tools_cache_key(auth: Option<&CodexAuth>) -> CodexAppsToolsCacheKey {
    connector_runtime_context_key(auth)
}

/// Process-scoped owner of the active account/workspace connector runtime.
///
/// Activating a different context discards the prior in-memory entry. Handles
/// to a discarded context can no longer read or publish its snapshot, which
/// prevents account A state from bleeding into account B.
#[derive(Clone, Default)]
pub struct ConnectorRuntimeManager {
    active: Arc<Mutex<Option<Arc<ConnectorRuntimeEntry>>>>,
}

/// Compatibility alias for existing cache call sites.
pub type CodexAppsToolsCache = ConnectorRuntimeManager;

impl ConnectorRuntimeManager {
    pub fn current_snapshot(
        &self,
        codex_home: PathBuf,
        key: ConnectorRuntimeContextKey,
    ) -> Option<Arc<ConnectorRuntimeSnapshot>> {
        self.context(codex_home, key).current_snapshot()
    }

    pub(crate) fn context(
        &self,
        codex_home: PathBuf,
        key: ConnectorRuntimeContextKey,
    ) -> ConnectorRuntimeContext {
        let identity = ConnectorRuntimeIdentity { codex_home, key };
        let mut active = lock_unpoisoned(&self.active);
        if let Some(active) = active.as_ref()
            && active.identity == identity
        {
            return ConnectorRuntimeContext {
                active: Arc::clone(&self.active),
                entry: Arc::clone(active),
            };
        }

        let entry = Arc::new(ConnectorRuntimeEntry::new(identity));
        if let Some(previous) = active.as_ref() {
            previous.routing_cancellation_token.cancel();
        }
        *active = Some(Arc::clone(&entry));
        ConnectorRuntimeContext {
            active: Arc::clone(&self.active),
            entry,
        }
    }
}

/// Handle to the active account/workspace connector runtime.
#[derive(Clone)]
pub(crate) struct ConnectorRuntimeContext {
    active: Arc<Mutex<Option<Arc<ConnectorRuntimeEntry>>>>,
    pub(crate) entry: Arc<ConnectorRuntimeEntry>,
}

/// Compatibility alias for existing cache call sites.
pub(crate) type CodexAppsToolsCacheContext = ConnectorRuntimeContext;

impl ConnectorRuntimeContext {
    pub fn current_snapshot(&self) -> Option<Arc<ConnectorRuntimeSnapshot>> {
        let active = lock_unpoisoned(&self.active);
        self.matches_active(active.as_ref())
            .then(|| self.entry.current_snapshot.load_full())
            .flatten()
    }

    pub(crate) fn tools_cache_path(&self) -> PathBuf {
        tools_cache_path(&self.entry.identity)
    }

    pub(crate) fn server_info_cache_path(&self) -> PathBuf {
        server_info_cache_path(&self.entry.identity)
    }

    pub(crate) fn current_tools(&self) -> Option<Vec<ToolInfo>> {
        self.current_snapshot()
            .map(|snapshot| snapshot.tools.clone())
    }

    pub(crate) fn has_current_tools(&self) -> bool {
        self.current_snapshot().is_some()
    }

    /// Shared by MCP clients attached to this context and cancelled before an identity switch.
    pub(crate) fn routing_cancellation_token(&self) -> CancellationToken {
        self.entry.routing_cancellation_token.clone()
    }

    pub(crate) fn begin_fetch(
        &self,
        source: CodexAppsToolsFetchSource,
    ) -> CodexAppsToolsFetchTicket {
        CodexAppsToolsFetchTicket {
            generation: self
                .entry
                .next_fetch_generation
                .fetch_add(1, Ordering::Relaxed)
                + 1,
            source,
        }
    }

    pub(crate) fn publish_runtime_if_newest_accepted(
        &self,
        ticket: CodexAppsToolsFetchTicket,
        server_info: &McpServerInfo,
        tools: Vec<ToolInfo>,
    ) -> Result<Arc<ConnectorRuntimeSnapshot>, ConnectorRuntimeContextDiscarded> {
        self.publish_runtime_if_newest_accepted_with(
            ticket,
            server_info,
            tools,
            persist_codex_apps_cache,
        )
    }

    fn publish_runtime_if_newest_accepted_with(
        &self,
        ticket: CodexAppsToolsFetchTicket,
        server_info: &McpServerInfo,
        tools: Vec<ToolInfo>,
        persist: impl FnOnce(&ConnectorRuntimeContext, &McpServerInfo, &ConnectorRuntimeSnapshot),
    ) -> Result<Arc<ConnectorRuntimeSnapshot>, ConnectorRuntimeContextDiscarded> {
        let publish_start = Instant::now();
        let active = lock_unpoisoned(&self.active);
        if !self.matches_active(active.as_ref()) {
            drop(active);
            emit_duration(
                MCP_TOOLS_CACHE_PUBLISH_DURATION_METRIC,
                publish_start.elapsed(),
                &[("source", ticket.source.as_str()), ("result", "discarded")],
            );
            return Err(ConnectorRuntimeContextDiscarded);
        }

        let mut last_accepted_generation = lock_unpoisoned(&self.entry.last_accepted_generation);
        if ticket.generation <= *last_accepted_generation {
            drop(last_accepted_generation);
            drop(active);
            emit_duration(
                MCP_TOOLS_CACHE_PUBLISH_DURATION_METRIC,
                publish_start.elapsed(),
                &[("source", ticket.source.as_str()), ("result", "stale")],
            );
            return self
                .current_snapshot()
                .ok_or(ConnectorRuntimeContextDiscarded);
        }

        let snapshot = Arc::new(ConnectorRuntimeSnapshot {
            tools,
            refreshed_at: SystemTime::now(),
        });

        *last_accepted_generation = ticket.generation;
        self.entry
            .current_snapshot
            .store(Some(Arc::clone(&snapshot)));
        // Keep both guards through persistence so accepted generations cannot reach disk out of
        // order, and the same account cannot be reactivated with stale disk state mid-commit.
        persist(self, server_info, snapshot.as_ref());
        drop(last_accepted_generation);
        drop(active);
        emit_duration(
            MCP_TOOLS_CACHE_PUBLISH_DURATION_METRIC,
            publish_start.elapsed(),
            &[("source", ticket.source.as_str()), ("result", "published")],
        );
        Ok(snapshot)
    }

    pub(crate) fn publish_if_newest_accepted(
        &self,
        ticket: CodexAppsToolsFetchTicket,
        server_info: &McpServerInfo,
        tools: Vec<ToolInfo>,
    ) -> Result<Vec<ToolInfo>, ConnectorRuntimeContextDiscarded> {
        self.publish_runtime_if_newest_accepted(ticket, server_info, tools)
            .map(|snapshot| snapshot.tools.clone())
    }

    #[cfg(test)]
    pub(crate) fn store_current_tools_for_test(&self, tools: Vec<ToolInfo>) {
        if !self.matches_active(lock_unpoisoned(&self.active).as_ref()) {
            return;
        }
        let snapshot = ConnectorRuntimeSnapshot {
            tools,
            refreshed_at: SystemTime::now(),
        };
        self.entry.current_snapshot.store(Some(Arc::new(snapshot)));
    }

    fn matches_active(&self, active: Option<&Arc<ConnectorRuntimeEntry>>) -> bool {
        active.is_some_and(|active| Arc::ptr_eq(active, &self.entry))
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum CodexAppsToolsFetchSource {
    Startup,
    HardRefresh,
}

impl CodexAppsToolsFetchSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::Startup => "startup",
            Self::HardRefresh => "hard_refresh",
        }
    }
}

pub(crate) struct CodexAppsToolsFetchTicket {
    generation: u64,
    source: CodexAppsToolsFetchSource,
}

#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("connector runtime context was discarded")]
pub(crate) struct ConnectorRuntimeContextDiscarded;

/// All live state owned by one activated connector identity.
pub(crate) struct ConnectorRuntimeEntry {
    pub(crate) identity: ConnectorRuntimeIdentity,
    pub(crate) current_snapshot: ArcSwapOption<ConnectorRuntimeSnapshot>,
    routing_cancellation_token: CancellationToken,
    next_fetch_generation: AtomicU64,
    last_accepted_generation: Mutex<u64>,
}

impl ConnectorRuntimeEntry {
    fn new(identity: ConnectorRuntimeIdentity) -> Self {
        let current_snapshot = load_cached_connector_runtime_for_identity(&identity).map(Arc::new);
        Self {
            identity,
            current_snapshot: ArcSwapOption::from(current_snapshot),
            routing_cancellation_token: CancellationToken::new(),
            next_fetch_generation: AtomicU64::new(0),
            last_accepted_generation: Mutex::new(0),
        }
    }
}

/// Everything that decides whether two connector runtime clients can share a snapshot.
///
/// The auth key says whose runtime catalog we are reading. `codex_home` keeps
/// the persisted cache under the right home directory.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ConnectorRuntimeIdentity {
    pub(crate) codex_home: PathBuf,
    pub(crate) key: ConnectorRuntimeContextKey,
}

pub(crate) fn load_startup_cached_codex_apps_server_info(
    cache_context: &CodexAppsToolsCacheContext,
) -> Option<McpServerInfo> {
    load_cached_codex_apps_server_info(cache_context)
}

fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
#[path = "connector_runtime_tests.rs"]
mod tests;
