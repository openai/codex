use std::collections::BTreeMap;
use std::sync::Arc;

use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use tokio::sync::Mutex;

use crate::CapabilityRootDiscoverRequest;
use crate::CapabilityRootDiscovery;
use crate::CapabilityRootsDiscoverParams;
use crate::EnvironmentManager;
use crate::ExecutorCapabilityDiscoverySnapshot;

/// Thread-scoped cache shared by capability consumers using the high-level executor API.
///
/// A single miss batches every requested root by environment. The cache deliberately has no
/// invalidation: selected roots are already treated as stable for the lifetime of a thread by the
/// existing plugin and skill providers.
pub struct ExecutorCapabilityDiscoveryCache {
    environment_manager: Arc<EnvironmentManager>,
    entries: Mutex<Vec<CachedRoot>>,
}

impl std::fmt::Debug for ExecutorCapabilityDiscoveryCache {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ExecutorCapabilityDiscoveryCache")
            .finish_non_exhaustive()
    }
}

struct CachedRoot {
    selected_root: SelectedCapabilityRoot,
    // Both successes and failures are memoized for the thread. Retrying a transient failure for
    // the same stable selected root requires explicit invalidation or a new thread.
    result: Result<Arc<CapabilityRootDiscovery>, String>,
}

impl ExecutorCapabilityDiscoveryCache {
    pub fn new(environment_manager: Arc<EnvironmentManager>) -> Self {
        Self {
            environment_manager,
            entries: Mutex::new(Vec::new()),
        }
    }

    /// Returns discoveries in the same order as `selected_roots`.
    #[tracing::instrument(
        name = "capability_roots.discovery_cache.resolve",
        skip_all,
        fields(root_count = selected_roots.len())
    )]
    pub async fn discover(
        &self,
        selected_roots: &[SelectedCapabilityRoot],
    ) -> Vec<Result<Arc<CapabilityRootDiscovery>, String>> {
        let missing = {
            let entries = self.entries.lock().await;
            selected_roots
                .iter()
                .filter(|selected_root| {
                    !entries
                        .iter()
                        .any(|cached| cached.selected_root == **selected_root)
                })
                .cloned()
                .collect::<Vec<_>>()
        };
        let discovered = self.discover_missing(missing).await;
        let mut entries = self.entries.lock().await;
        for discovered_root in discovered {
            if !entries
                .iter()
                .any(|cached| cached.selected_root == discovered_root.selected_root)
            {
                entries.push(discovered_root);
            }
        }
        selected_roots
            .iter()
            .map(|selected_root| {
                match entries
                    .iter()
                    .find(|cached| cached.selected_root == *selected_root)
                {
                    Some(cached) => cached.result.clone(),
                    None => Err(format!(
                        "selected capability root `{}` was not discovered",
                        selected_root.id
                    )),
                }
            })
            .collect()
    }

    /// Resolves the selected roots once and freezes their results for one model step.
    pub async fn snapshot(
        &self,
        selected_roots: &[SelectedCapabilityRoot],
    ) -> ExecutorCapabilityDiscoverySnapshot {
        ExecutorCapabilityDiscoverySnapshot::new(
            selected_roots,
            self.discover(selected_roots).await,
        )
    }

    async fn discover_missing(&self, missing: Vec<SelectedCapabilityRoot>) -> Vec<CachedRoot> {
        let mut grouped = BTreeMap::<String, Vec<SelectedCapabilityRoot>>::new();
        for selected_root in missing {
            let CapabilityRootLocation::Environment { environment_id, .. } =
                &selected_root.location;
            grouped
                .entry(environment_id.clone())
                .or_default()
                .push(selected_root);
        }

        let discoveries = futures::future::join_all(grouped.into_iter().map(
            |(environment_id, selected_roots)| async move {
                let Some(environment) = self.environment_manager.get_environment(&environment_id)
                else {
                    let error = format!("environment `{environment_id}` is unavailable");
                    return selected_roots
                        .into_iter()
                        .map(|selected_root| CachedRoot {
                            selected_root,
                            result: Err(error.clone()),
                        })
                        .collect::<Vec<_>>();
                };
                let params = CapabilityRootsDiscoverParams {
                    roots: selected_roots
                        .iter()
                        .map(|selected_root| {
                            let CapabilityRootLocation::Environment { path, .. } =
                                &selected_root.location;
                            CapabilityRootDiscoverRequest {
                                id: selected_root.id.clone(),
                                path: path.clone(),
                            }
                        })
                        .collect(),
                };
                let response = match environment.discover_capability_roots(params).await {
                    Ok(response) => response,
                    Err(error) => {
                        let error = error.to_string();
                        return selected_roots
                            .into_iter()
                            .map(|selected_root| CachedRoot {
                                selected_root,
                                result: Err(error.clone()),
                            })
                            .collect();
                    }
                };
                if response.roots.len() != selected_roots.len() {
                    let error = format!(
                        "exec-server returned {} capability roots for {} requests",
                        response.roots.len(),
                        selected_roots.len()
                    );
                    return selected_roots
                        .into_iter()
                        .map(|selected_root| CachedRoot {
                            selected_root,
                            result: Err(error.clone()),
                        })
                        .collect();
                }
                selected_roots
                    .into_iter()
                    .zip(response.roots)
                    .map(|(selected_root, discovery)| {
                        let CapabilityRootLocation::Environment { path, .. } =
                            &selected_root.location;
                        let result = if discovery.id == selected_root.id && discovery.path == *path
                        {
                            Ok(Arc::new(discovery))
                        } else {
                            Err(format!(
                                "exec-server returned mismatched capability root `{}` at {}",
                                discovery.id, discovery.path
                            ))
                        };
                        CachedRoot {
                            selected_root,
                            result,
                        }
                    })
                    .collect()
            },
        ))
        .await;
        discoveries.into_iter().flatten().collect()
    }
}
