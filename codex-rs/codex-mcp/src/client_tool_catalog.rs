//! Client-local catalog revisions and synchronization for MCP calls.
//!
//! Apps clients read the provider's current catalog without retaining old arrays.
//! Only active readers, calls, and frozen model bindings pin earlier snapshots.
//! Explicit refreshes publish after calls using this client's current revision finish.
//! Equivalent shared publications preserve calls, but explicit refreshes invalidate them.
//! Ordinary servers coalesce list-change notifications and publish only a stable listing.
//! Notifications update discovery without waiting for admitted calls to finish.
//! Failed refreshes retain the last catalog and share a short retry cooldown.

use std::collections::HashSet;
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_connectors::ConnectorRuntimeSnapshot;
use codex_rmcp_client::ToolListRevision;
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use tokio::sync::RwLockReadGuard;
use tokio::sync::watch;
use tokio::time::Instant;

use crate::tools::ToolInfo;

type ToolCatalogUpdates = watch::Receiver<Option<Arc<ConnectorRuntimeSnapshot<ToolInfo>>>>;
const NOTIFICATION_REFRESH_COOLDOWN: Duration = Duration::from_secs(5);

/// The exact Apps catalog returned by an awaited refresh of one published runtime.
pub struct CodexAppsToolSnapshot {
    /// Raw installed tools, including tools hidden or disabled for the model.
    pub tools: Vec<ToolInfo>,
    /// Raw MCP tool names allowed by the same runtime's generic MCP policy.
    /// App-specific policy is applied by the caller.
    pub model_visible_tool_names: HashSet<String>,
}

pub(crate) struct ClientToolCatalog {
    current: RwLock<CatalogState>,
    /// Explicit refreshes still wait for call preparation and execution.
    calls: RwLock<()>,
    /// Serialize fetches and retain the next retry time after failure or cancellation.
    refresh_lock: Mutex<Option<Instant>>,
    /// Starts at zero so notifications during the initial listing stay pending.
    listed_notification_revision: AtomicU64,
}

struct CatalogState {
    revision: u64,
    /// The catalog revision installed by this client's last explicit refresh.
    last_refresh_revision: u64,
    source: CatalogSource,
}

/// Ordinary MCP clients own their tools; Apps clients observe the shared provider.
enum CatalogSource {
    Local {
        tools: Arc<[ToolInfo]>,
        meta: Option<Arc<serde_json::Value>>,
    },
    Live {
        updates: ToolCatalogUpdates,
        tools_version: u64,
    },
}

#[derive(Clone)]
pub(crate) struct ToolCatalogSnapshot {
    pub(crate) revision: u64,
    pub(crate) tools: Arc<[ToolInfo]>,
    pub(crate) meta: Option<Arc<serde_json::Value>>,
}

impl ClientToolCatalog {
    /// Live clients read the provider; callers publish startup tools before subscribing.
    pub(crate) fn new(
        tools: impl Into<Arc<[ToolInfo]>>,
        meta: Option<Arc<serde_json::Value>>,
        updates: Option<ToolCatalogUpdates>,
    ) -> Self {
        let source = match updates {
            Some(mut updates) => {
                let tools_version = updates
                    .borrow_and_update()
                    .as_ref()
                    .map_or(0, |snapshot| snapshot.tools_version());
                CatalogSource::Live {
                    updates,
                    tools_version,
                }
            }
            None => CatalogSource::Local {
                tools: tools.into(),
                meta,
            },
        };
        Self {
            current: RwLock::new(CatalogState {
                revision: 0,
                last_refresh_revision: 0,
                source,
            }),
            refresh_lock: Mutex::new(None),
            calls: RwLock::new(()),
            listed_notification_revision: AtomicU64::new(0),
        }
    }

    pub(crate) async fn read<R>(&self, read: impl FnOnce(ToolCatalogSnapshot) -> R) -> R {
        let (_current, snapshot) = self.read_current().await;
        read(snapshot)
    }

    /// Captures tools and their client-local revision together, without retaining them in the client.
    async fn read_current(&self) -> (RwLockReadGuard<'_, CatalogState>, ToolCatalogSnapshot) {
        loop {
            {
                let current = self.current.read().await;
                let catalog = match &current.source {
                    CatalogSource::Local { tools, meta } => Some((Arc::clone(tools), meta.clone())),
                    CatalogSource::Live { updates, .. } => {
                        // Hold the watch borrow while checking its version so the tools and
                        // revision come from the same publication.
                        let snapshot = updates.borrow();
                        (!updates.has_changed().unwrap_or(false)).then(|| {
                            snapshot
                                .as_ref()
                                .map(|snapshot| {
                                    (snapshot.shared_tools(), snapshot.shared_tool_list_meta())
                                })
                                .unwrap_or_default()
                        })
                    }
                };
                if let Some((tools, meta)) = catalog {
                    let snapshot = ToolCatalogSnapshot {
                        revision: current.revision,
                        tools,
                        meta,
                    };
                    return (current, snapshot);
                }
            }
            let mut current = self.current.write().await;
            let changed = match &mut current.source {
                CatalogSource::Live {
                    updates,
                    tools_version,
                } => {
                    let version = updates
                        .borrow_and_update()
                        .as_ref()
                        .map_or(0, |snapshot| snapshot.tools_version());
                    let changed = *tools_version != version;
                    *tools_version = version;
                    changed
                }
                CatalogSource::Local { .. } => false,
            };
            if changed {
                current.revision += 1;
            }
        }
    }

    pub(crate) fn notifications_current(&self, notifications: &ToolListRevision) -> bool {
        self.listed_notification_revision.load(Ordering::Acquire) == notifications.current()
    }

    /// Refresh ordinary server catalogs, coalescing concurrent readers. Never mark a
    /// listing current if another notification arrived while fetching its pages.
    /// Retry once; persistent churn stays pending until the retry cooldown expires.
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "refreshes must remain serialized through fetching and catalog publication"
    )]
    pub(crate) async fn refresh_after_notifications<F, Fut>(
        &self,
        notifications: &ToolListRevision,
        timeout: Duration,
        mut fetch: F,
    ) -> Result<()>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<(Vec<ToolInfo>, Option<Arc<serde_json::Value>>)>>,
    {
        if self.notifications_current(notifications) {
            return Ok(());
        }
        let deadline = Instant::now() + timeout;
        let mut retry_after = tokio::time::timeout_at(deadline, self.refresh_lock.lock())
            .await
            .context("MCP tool catalog refresh timed out")?;
        if self.notifications_current(notifications)
            || retry_after.is_some_and(|deadline| Instant::now() < deadline)
        {
            return Ok(());
        }
        // Also bound retries if the caller cancels this future during the fetch.
        *retry_after = Some(deadline + NOTIFICATION_REFRESH_COOLDOWN);
        let result = tokio::time::timeout_at(deadline, async {
            for _ in 0..2 {
                let revision = notifications.current();
                let tools = fetch().await;
                let mut current = self.current.write().await;
                if notifications.current() != revision {
                    continue;
                }
                let (tools, meta) = tools?;
                current.source = CatalogSource::Local {
                    tools: tools.into(),
                    meta,
                };
                current.revision += 1;
                self.listed_notification_revision
                    .store(revision, Ordering::Release);
                return Ok(());
            }
            bail!("MCP tool list changed during catalog refresh")
        })
        .await
        .context("MCP tool catalog refresh timed out")
        .and_then(std::convert::identity);
        *retry_after = result
            .is_err()
            .then(|| Instant::now() + NOTIFICATION_REFRESH_COOLDOWN);
        result
    }

    /// Serialize fetching and publication, leaving the current catalog usable during the fetch.
    /// The publication callback runs alongside the exact-client update under the write lock.
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "refreshes must remain serialized through fetching and catalog publication"
    )]
    pub(crate) async fn refresh<C, R, F, Fut, P>(&self, fetch: F, publish: P) -> Result<R>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<(Vec<ToolInfo>, Option<Arc<serde_json::Value>>, C)>>,
        P: FnOnce(&[ToolInfo], Option<Arc<serde_json::Value>>, C) -> R,
    {
        let _refresh = self.refresh_lock.lock().await;
        let (tools, meta, context) = fetch().await?;
        let _calls = self.calls.write().await;
        let mut current = self.current.write().await;
        let result = publish(&tools, meta.clone(), context);
        match &mut current.source {
            CatalogSource::Local {
                tools: current_tools,
                meta: current_meta,
            } => {
                *current_tools = tools.into();
                *current_meta = meta;
            }
            CatalogSource::Live {
                updates,
                tools_version,
            } => {
                *tools_version = updates
                    .borrow_and_update()
                    .as_ref()
                    .map_or(0, |snapshot| snapshot.tools_version());
            }
        }
        current.revision += 1;
        current.last_refresh_revision = current.revision;
        Ok(result)
    }

    /// Reject stale calls before preparation. Explicit refreshes wait for execution;
    /// ordinary list-change notifications may publish while an admitted call runs.
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "catalog publication must wait for call preparation and execution"
    )]
    pub(crate) async fn run_with_snapshot<R, F, Fut>(
        &self,
        expected: &ToolCatalogSnapshot,
        run: F,
    ) -> Option<R>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = R>,
    {
        let _calls = self.calls.read().await;
        let (current, snapshot) = self.read_current().await;
        if current.last_refresh_revision > expected.revision
            || (snapshot.revision != expected.revision
                && !catalogs_match(&snapshot.tools, &expected.tools))
        {
            return None;
        }
        let current = match current.source {
            CatalogSource::Local { .. } => {
                drop(current);
                None
            }
            CatalogSource::Live { .. } => Some(current),
        };
        let result = run().await;
        drop(current);
        Some(result)
    }
}

/// Compares complete definitions independently of tool-list order. Stable sorting keeps
/// conflicting duplicate identities in their original order, since deduplication can pick the
/// first definition. This slow path runs only after the client's revision has changed.
fn catalogs_match(left: &[ToolInfo], right: &[ToolInfo]) -> bool {
    if left == right {
        return true;
    }
    if left.len() != right.len() {
        return false;
    }
    let mut left = left.iter().collect::<Vec<_>>();
    let mut right = right.iter().collect::<Vec<_>>();
    for tools in [&mut left, &mut right] {
        tools.sort_by_key(|tool| {
            (
                tool.server_name.as_str(),
                tool.tool.name.as_ref(),
                tool.connector_id.as_deref(),
                tool.callable_namespace.as_str(),
                tool.callable_name.as_str(),
            )
        });
    }
    left == right
}

/// A binding cache key includes client identity, since new clients start at zero.
#[derive(Clone)]
pub(crate) struct ClientToolCatalogRevision {
    pub(crate) catalog: Arc<ClientToolCatalog>,
    pub(crate) revision: u64,
}

impl PartialEq for ClientToolCatalogRevision {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.catalog, &other.catalog) && self.revision == other.revision
    }
}

impl Eq for ClientToolCatalogRevision {}
