//! Shared runtime snapshot for connector-backed MCP tools.
//!
//! Runtime snapshots are process-local live state scoped by the active account
//! and workspace. Disk is best-effort cold-start persistence; a context reads
//! it once when activated and never rereads it. Full connector metadata is
//! owned by the connector metadata store, not by this module.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;

use arc_swap::ArcSwapOption;
use codex_protocol::mcp::McpServerInfo;
use serde::Deserialize;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio_util::sync::CancellationToken;

use self::persistence::load_cached_codex_apps_server_info;
use self::persistence::load_cached_connector_runtime_for_identity;
use self::persistence::persist_codex_apps_cache;
use self::persistence::server_info_cache_path;
use self::persistence::tools_cache_path;

const MCP_TOOLS_CACHE_PUBLISH_DURATION_METRIC: &str = "codex.mcp.tools.cache_publish.duration_ms";

/// Defines the stable on-disk contract for one connector runtime payload.
///
/// Implementations must use cache directories that are unique to their payload
/// shape and bump the corresponding schema version whenever that shape changes.
pub trait ConnectorRuntimePayload: Clone + Serialize + DeserializeOwned {
    const TOOLS_CACHE_DIR: &'static str;
    const TOOLS_CACHE_SCHEMA_VERSION: u8;
    const SERVER_INFO_CACHE_DIR: &'static str;
    const SERVER_INFO_CACHE_SCHEMA_VERSION: u8;
}

/// The account and workspace identity of a connector runtime catalog.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConnectorRuntimeContextKey {
    account_id: Option<String>,
    chatgpt_user_id: Option<String>,
    is_workspace_account: bool,
}

impl ConnectorRuntimeContextKey {
    pub fn personal(account_id: Option<String>, chatgpt_user_id: Option<String>) -> Self {
        Self {
            account_id,
            chatgpt_user_id,
            is_workspace_account: false,
        }
    }

    pub fn workspace(account_id: Option<String>, chatgpt_user_id: Option<String>) -> Self {
        Self {
            account_id,
            chatgpt_user_id,
            is_workspace_account: true,
        }
    }
}

/// One atomically published connector runtime state.
///
/// Tools remain raw and in response order. Local and managed configuration is
/// intentionally applied by readers rather than persisted in this snapshot.
#[derive(Debug, Clone)]
pub struct ConnectorRuntimeSnapshot<T> {
    tools: Vec<T>,
    refreshed_at: SystemTime,
}

impl<T> ConnectorRuntimeSnapshot<T> {
    pub fn tools(&self) -> &[T] {
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

/// Process-scoped owner of the active account/workspace connector runtime.
///
/// Activating a different context discards the prior in-memory entry. Handles
/// to a discarded context can no longer read or publish its snapshot, which
/// prevents account A state from bleeding into account B.
pub struct ConnectorRuntimeManager<T: ConnectorRuntimePayload> {
    active: Arc<Mutex<Option<Arc<ConnectorRuntimeEntry<T>>>>>,
}

impl<T: ConnectorRuntimePayload> Clone for ConnectorRuntimeManager<T> {
    fn clone(&self) -> Self {
        Self {
            active: Arc::clone(&self.active),
        }
    }
}

impl<T: ConnectorRuntimePayload> Default for ConnectorRuntimeManager<T> {
    fn default() -> Self {
        Self {
            active: Arc::new(Mutex::new(None)),
        }
    }
}

impl<T: ConnectorRuntimePayload> ConnectorRuntimeManager<T> {
    pub fn current_snapshot(
        &self,
        codex_home: PathBuf,
        key: ConnectorRuntimeContextKey,
    ) -> Option<Arc<ConnectorRuntimeSnapshot<T>>> {
        self.context(codex_home, key).current_snapshot()
    }

    pub fn context(
        &self,
        codex_home: PathBuf,
        key: ConnectorRuntimeContextKey,
    ) -> ConnectorRuntimeContext<T> {
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
pub struct ConnectorRuntimeContext<T: ConnectorRuntimePayload> {
    active: Arc<Mutex<Option<Arc<ConnectorRuntimeEntry<T>>>>>,
    entry: Arc<ConnectorRuntimeEntry<T>>,
}

impl<T: ConnectorRuntimePayload> Clone for ConnectorRuntimeContext<T> {
    fn clone(&self) -> Self {
        Self {
            active: Arc::clone(&self.active),
            entry: Arc::clone(&self.entry),
        }
    }
}

impl<T: ConnectorRuntimePayload> ConnectorRuntimeContext<T> {
    pub fn current_snapshot(&self) -> Option<Arc<ConnectorRuntimeSnapshot<T>>> {
        let active = lock_unpoisoned(&self.active);
        self.matches_active(active.as_ref())
            .then(|| self.entry.current_snapshot.load_full())
            .flatten()
    }

    pub fn has_current_tools(&self) -> bool {
        self.current_snapshot().is_some()
    }

    /// Shared by clients attached to this context and cancelled before an identity switch.
    pub fn routing_cancellation_token(&self) -> CancellationToken {
        self.entry.routing_cancellation_token.clone()
    }

    pub fn begin_fetch(&self, source: ConnectorRuntimeFetchSource) -> ConnectorRuntimeFetchTicket {
        ConnectorRuntimeFetchTicket {
            generation: self
                .entry
                .next_fetch_generation
                .fetch_add(1, Ordering::Relaxed)
                + 1,
            source,
        }
    }

    pub fn cached_server_info(&self) -> Option<McpServerInfo> {
        load_cached_codex_apps_server_info(self)
    }

    fn tools_cache_path(&self) -> PathBuf {
        tools_cache_path::<T>(&self.entry.identity)
    }

    fn server_info_cache_path(&self) -> PathBuf {
        server_info_cache_path::<T>(&self.entry.identity)
    }

    fn matches_active(&self, active: Option<&Arc<ConnectorRuntimeEntry<T>>>) -> bool {
        active.is_some_and(|active| Arc::ptr_eq(active, &self.entry))
    }

    pub fn current_tools(&self) -> Option<Vec<T>> {
        self.current_snapshot()
            .map(|snapshot| snapshot.tools.clone())
    }

    pub fn publish_runtime_if_newest_accepted(
        &self,
        ticket: ConnectorRuntimeFetchTicket,
        server_info: &McpServerInfo,
        tools: Vec<T>,
    ) -> Result<Arc<ConnectorRuntimeSnapshot<T>>, ConnectorRuntimeDiscarded> {
        self.publish_runtime_if_newest_accepted_with(
            ticket,
            server_info,
            tools,
            persist_codex_apps_cache,
        )
    }

    fn publish_runtime_if_newest_accepted_with(
        &self,
        ticket: ConnectorRuntimeFetchTicket,
        server_info: &McpServerInfo,
        tools: Vec<T>,
        persist: impl FnOnce(&ConnectorRuntimeContext<T>, &McpServerInfo, &ConnectorRuntimeSnapshot<T>),
    ) -> Result<Arc<ConnectorRuntimeSnapshot<T>>, ConnectorRuntimeDiscarded> {
        let publish_start = Instant::now();
        let active = lock_unpoisoned(&self.active);
        if !self.matches_active(active.as_ref()) {
            drop(active);
            emit_duration(
                MCP_TOOLS_CACHE_PUBLISH_DURATION_METRIC,
                publish_start.elapsed(),
                &[("source", ticket.source.as_str()), ("result", "discarded")],
            );
            return Err(ConnectorRuntimeDiscarded);
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
            return self.current_snapshot().ok_or(ConnectorRuntimeDiscarded);
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

    pub fn publish_if_newest_accepted(
        &self,
        ticket: ConnectorRuntimeFetchTicket,
        server_info: &McpServerInfo,
        tools: Vec<T>,
    ) -> Result<Vec<T>, ConnectorRuntimeDiscarded> {
        self.publish_runtime_if_newest_accepted(ticket, server_info, tools)
            .map(|snapshot| snapshot.tools.clone())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ConnectorRuntimeFetchSource {
    Startup,
    HardRefresh,
}

impl ConnectorRuntimeFetchSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::Startup => "startup",
            Self::HardRefresh => "hard_refresh",
        }
    }
}

pub struct ConnectorRuntimeFetchTicket {
    generation: u64,
    source: ConnectorRuntimeFetchSource,
}

#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("connector runtime context was discarded")]
pub struct ConnectorRuntimeDiscarded;

/// All live state owned by one activated connector identity.
struct ConnectorRuntimeEntry<T: ConnectorRuntimePayload> {
    identity: ConnectorRuntimeIdentity,
    current_snapshot: ArcSwapOption<ConnectorRuntimeSnapshot<T>>,
    routing_cancellation_token: CancellationToken,
    next_fetch_generation: AtomicU64,
    last_accepted_generation: Mutex<u64>,
}

impl<T: ConnectorRuntimePayload> ConnectorRuntimeEntry<T> {
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
struct ConnectorRuntimeIdentity {
    codex_home: PathBuf,
    key: ConnectorRuntimeContextKey,
}

fn emit_duration(metric: &str, duration: Duration, tags: &[(&str, &str)]) {
    if let Some(metrics) = codex_otel::global() {
        let _ = metrics.record_duration(metric, duration, tags);
    }
}

fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

mod persistence;

#[cfg(test)]
mod tests;
