//! Client-local catalog revisions and synchronization for MCP calls.
//!
//! Apps clients read the provider's current catalog without retaining old arrays.
//! Only active readers, calls, and frozen model bindings pin earlier snapshots.
//! Explicit refreshes publish after calls using this client's current revision finish.
//! Equivalent shared publications preserve calls, but explicit refreshes invalidate them.

use std::collections::HashSet;
use std::future::Future;
use std::sync::Arc;

use anyhow::Result;
use codex_connectors::ConnectorRuntimeSnapshot;
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use tokio::sync::RwLockReadGuard;
use tokio::sync::watch;

use crate::tools::ToolInfo;

type ToolCatalogUpdates = watch::Receiver<Option<Arc<ConnectorRuntimeSnapshot<ToolInfo>>>>;

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
    /// Serialize fetches without blocking calls against the current catalog.
    refresh_lock: Mutex<()>,
}

struct CatalogState {
    revision: u64,
    /// The catalog revision installed by this client's last explicit refresh.
    last_refresh_revision: u64,
    source: CatalogSource,
}

/// Ordinary MCP clients own their tools; Apps clients observe the shared provider.
enum CatalogSource {
    Local(Arc<[ToolInfo]>),
    Live {
        updates: ToolCatalogUpdates,
        tools_version: u64,
    },
}

pub(crate) struct ToolCatalogSnapshot {
    pub(crate) revision: u64,
    pub(crate) tools: Arc<[ToolInfo]>,
}

impl ClientToolCatalog {
    /// Live clients read the provider; callers publish startup tools before subscribing.
    pub(crate) fn new(
        tools: impl Into<Arc<[ToolInfo]>>,
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
            None => CatalogSource::Local(tools.into()),
        };
        Self {
            current: RwLock::new(CatalogState {
                revision: 0,
                last_refresh_revision: 0,
                source,
            }),
            refresh_lock: Mutex::new(()),
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
                let tools = match &current.source {
                    CatalogSource::Local(tools) => Some(Arc::clone(tools)),
                    CatalogSource::Live { updates, .. } => {
                        // Hold the watch borrow while checking its version so the tools and
                        // revision come from the same publication.
                        let snapshot = updates.borrow();
                        (!updates.has_changed().unwrap_or(false)).then(|| {
                            snapshot
                                .as_ref()
                                .map(|snapshot| snapshot.shared_tools())
                                .unwrap_or_default()
                        })
                    }
                };
                if let Some(tools) = tools {
                    let snapshot = ToolCatalogSnapshot {
                        revision: current.revision,
                        tools,
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
                CatalogSource::Local(_) => false,
            };
            if changed {
                current.revision += 1;
            }
        }
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
        Fut: Future<Output = Result<(Vec<ToolInfo>, C)>>,
        P: FnOnce(&[ToolInfo], C) -> R,
    {
        let _refresh = self.refresh_lock.lock().await;
        let (tools, context) = fetch().await?;
        let mut current = self.current.write().await;
        let result = publish(&tools, context);
        match &mut current.source {
            CatalogSource::Local(current_tools) => *current_tools = tools.into(),
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

    /// Reject stale calls before preparation and hold catalog authority until execution finishes.
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
        let (current, snapshot) = self.read_current().await;
        if current.last_refresh_revision > expected.revision
            || (snapshot.revision != expected.revision
                && !catalogs_match(&snapshot.tools, &expected.tools))
        {
            return None;
        }
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
