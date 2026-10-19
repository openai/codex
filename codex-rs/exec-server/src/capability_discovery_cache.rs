//! Caches root discovery by captured executor/session, root, and sandbox.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Weak;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use tokio::sync::Mutex;

use crate::CapabilityRootDiscoverRequest;
use crate::CapabilityRootDiscovery;
use crate::CapabilityRootsDiscoverParams;
use crate::Environment;
use crate::ExecutorCapabilityDiscoverySnapshot;
use crate::FileSystemSandboxContext;
use crate::ResolvedSelectedCapabilityRoot;

/// Thread-scoped cache shared by capability consumers using the high-level executor API.
///
/// A single miss batches every requested root by environment. Successful discoveries and
/// permanent failures remain cached by executor/session, root, and sandbox; transient failures are retried on the
/// next request. Recovery is reported so dependent MCP projections can be invalidated.
#[derive(Default)]
pub struct ExecutorCapabilityDiscoveryCache {
    entries: Mutex<Vec<CachedRoot>>,
    recovered_discovery: AtomicBool,
}

impl std::fmt::Debug for ExecutorCapabilityDiscoveryCache {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ExecutorCapabilityDiscoveryCache")
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug)]
struct RootDiscoveryKey {
    selected_root: SelectedCapabilityRoot,
    sandbox: Option<FileSystemSandboxContext>,
    environment: Weak<Environment>,
    session_id: Option<String>,
}

impl PartialEq for RootDiscoveryKey {
    fn eq(&self, other: &Self) -> bool {
        self.selected_root == other.selected_root
            && self.sandbox == other.sandbox
            && self.environment.ptr_eq(&other.environment)
            && self.session_id == other.session_id
    }
}

#[derive(Clone)]
struct RootDiscoveryRequest {
    key: RootDiscoveryKey,
    environment: Result<Arc<Environment>, String>,
}

struct CachedRoot {
    key: RootDiscoveryKey,
    result: Result<Arc<CapabilityRootDiscovery>, String>,
    // Preserve transport classification after the public snapshot reduces errors to strings.
    retryable: bool,
}

impl ExecutorCapabilityDiscoveryCache {
    /// Reports whether a previously failed root has recovered since the last observation.
    pub fn take_recovered_discovery(&self) -> bool {
        self.recovered_discovery.swap(false, Ordering::AcqRel)
    }

    /// Returns discoveries in root order using only the captured environment handles.
    #[tracing::instrument(
        name = "capability_roots.discovery_cache.resolve",
        skip_all,
        fields(root_count = roots.len())
    )]
    pub async fn discover(
        &self,
        roots: &[ResolvedSelectedCapabilityRoot],
        sandbox_contexts: &HashMap<String, FileSystemSandboxContext>,
    ) -> Vec<Result<Arc<CapabilityRootDiscovery>, String>> {
        let environments = roots
            .iter()
            .map(|root| {
                let CapabilityRootLocation::Environment { environment_id, .. } =
                    &root.selected_root().location;
                (environment_id.clone(), Arc::clone(root.environment()))
            })
            .collect::<HashMap<_, _>>();
        // Resolve a session once per captured environment, before matching cached results.
        let bindings = futures::future::join_all(environments.into_iter().map(
            |(id, environment)| async move {
                let session = environment
                    .executor_session_id()
                    .await
                    .map_err(|error| error.to_string());
                (id, (environment, session))
            },
        ))
        .await
        .into_iter()
        .collect::<HashMap<_, _>>();
        let requests = roots
            .iter()
            .map(|root| {
                let selected_root = root.selected_root();
                let CapabilityRootLocation::Environment { environment_id, .. } =
                    &selected_root.location;
                let (environment, session) = &bindings[environment_id];
                RootDiscoveryRequest {
                    key: RootDiscoveryKey {
                        selected_root: selected_root.clone(),
                        sandbox: sandbox_contexts.get(environment_id).cloned(),
                        environment: Arc::downgrade(environment),
                        session_id: session.as_ref().ok().cloned().flatten(),
                    },
                    environment: session
                        .as_ref()
                        .map(|_| Arc::clone(environment))
                        .map_err(Clone::clone),
                }
            })
            .collect::<Vec<_>>();
        let keys = requests
            .iter()
            .map(|request| request.key.clone())
            .collect::<Vec<_>>();
        let mut results = {
            let entries = self.entries.lock().await;
            requests
                .iter()
                .map(|request| {
                    request.environment.as_ref().ok().and_then(|_| {
                        entries
                            .iter()
                            .find(|cached| cached.key == request.key && !cached.retryable)
                            .map(|cached| cached.result.clone())
                    })
                })
                .collect::<Vec<_>>()
        };
        let missing = requests
            .into_iter()
            .zip(&results)
            .filter_map(|(request, result)| result.is_none().then_some(request))
            .collect();
        let discovered = self.discover_missing(missing).await;
        let mut entries = self.entries.lock().await;
        for discovered_root in discovered {
            // Reuse a concurrent success only for the same captured binding.
            let resolved = entries
                .iter()
                .find(|cached| cached.key == discovered_root.key && cached.result.is_ok())
                .map_or(&discovered_root.result, |cached| &cached.result);
            for (key, result) in keys.iter().zip(&mut results) {
                if *key == discovered_root.key && result.is_none() {
                    *result = Some(resolved.clone());
                }
            }
            if let Some(cached) = entries
                .iter_mut()
                .find(|cached| cached.key.selected_root == discovered_root.key.selected_root)
            {
                if cached.key != discovered_root.key || cached.result.is_err() {
                    if cached.result.is_err() && discovered_root.result.is_ok() {
                        self.recovered_discovery.store(true, Ordering::Release);
                    }
                    *cached = discovered_root;
                }
            } else {
                entries.push(discovered_root);
            }
        }
        keys.into_iter()
            .zip(results)
            .map(|(key, result)| {
                result.unwrap_or_else(|| {
                    Err(format!(
                        "selected capability root `{}` was not discovered",
                        key.selected_root.id
                    ))
                })
            })
            .collect()
    }

    /// Freezes discoveries and sandbox contexts for the captured roots.
    pub async fn snapshot(
        &self,
        roots: &[ResolvedSelectedCapabilityRoot],
        sandbox_contexts: &HashMap<String, FileSystemSandboxContext>,
    ) -> ExecutorCapabilityDiscoverySnapshot {
        let selected_roots = roots
            .iter()
            .map(|root| root.selected_root().clone())
            .collect::<Vec<_>>();
        let results = self.discover(roots, sandbox_contexts).await;
        ExecutorCapabilityDiscoverySnapshot::new(&selected_roots, results, sandbox_contexts.clone())
    }

    async fn discover_missing(&self, missing: Vec<RootDiscoveryRequest>) -> Vec<CachedRoot> {
        let mut grouped = BTreeMap::<String, Vec<RootDiscoveryRequest>>::new();
        for selected_root in missing {
            let CapabilityRootLocation::Environment { environment_id, .. } =
                &selected_root.key.selected_root.location;
            grouped
                .entry(environment_id.clone())
                .or_default()
                .push(selected_root);
        }

        let batches = grouped.into_values().flat_map(|roots| {
            roots
                .chunks(crate::capability_discovery::MAX_ROOTS_PER_REQUEST)
                .map(<[RootDiscoveryRequest]>::to_vec)
                .collect::<Vec<_>>()
        });
        let discoveries = futures::future::join_all(batches.map(|selected_roots| async move {
            let environment = match &selected_roots[0].environment {
                Ok(environment) => Arc::clone(environment),
                Err(error) => {
                    let error = error.clone();
                    return selected_roots
                        .into_iter()
                        .map(|request| CachedRoot {
                            key: request.key,
                            result: Err(error.clone()),
                            retryable: true,
                        })
                        .collect::<Vec<_>>();
                }
            };
            let params = CapabilityRootsDiscoverParams {
                roots: selected_roots
                    .iter()
                    .map(|request| {
                        let selected_root = &request.key.selected_root;
                        let CapabilityRootLocation::Environment { path, .. } =
                            &selected_root.location;
                        CapabilityRootDiscoverRequest {
                            id: selected_root.id.clone(),
                            path: path.clone(),
                            sandbox: request.key.sandbox.clone(),
                        }
                    })
                    .collect(),
            };
            let response = match environment.discover_capability_roots(params).await {
                Ok(response) => response,
                Err(error) => {
                    let retryable = crate::client::can_retry_connection_attempt(&error);
                    let error = error.to_string();
                    return selected_roots
                        .into_iter()
                        .map(|request| CachedRoot {
                            key: request.key,
                            result: Err(error.clone()),
                            retryable,
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
                    .map(|request| CachedRoot {
                        key: request.key,
                        result: Err(error.clone()),
                        retryable: false,
                    })
                    .collect();
            }
            selected_roots
                .into_iter()
                .zip(response.roots)
                .map(|(request, discovery)| {
                    let selected_root = &request.key.selected_root;
                    let CapabilityRootLocation::Environment { path, .. } = &selected_root.location;
                    let result = if discovery.id == selected_root.id && discovery.path == *path {
                        Ok(Arc::new(discovery))
                    } else {
                        Err(format!(
                            "exec-server returned mismatched capability root `{}` at {}",
                            discovery.id, discovery.path
                        ))
                    };
                    CachedRoot {
                        key: request.key,
                        result,
                        retryable: false,
                    }
                })
                .collect()
        }))
        .await;
        discoveries.into_iter().flatten().collect()
    }
}
