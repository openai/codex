use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;

use crate::Environment;
use crate::EnvironmentManager;
use crate::ExecutorFileSystem;

/// A selected capability root paired with its currently ready environment handle.
///
/// Environment IDs have stable identity and contents. This process-local value must not be
/// persisted: it only keeps the current connection handle alive while one model step uses the
/// stable environment.
#[derive(Clone)]
pub struct ResolvedSelectedCapabilityRoot {
    selected_root: SelectedCapabilityRoot,
    environment: Arc<Environment>,
}

/// A passive view of selected capability roots and unavailable environments.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SelectedCapabilityRootsStatus {
    /// Selected roots whose environments are ready.
    pub ready_roots: Vec<SelectedCapabilityRoot>,
    /// Missing environments and terminal connection failures.
    pub warnings: Vec<String>,
}

impl ResolvedSelectedCapabilityRoot {
    pub fn selected_root(&self) -> &SelectedCapabilityRoot {
        &self.selected_root
    }

    pub fn environment(&self) -> &Arc<Environment> {
        &self.environment
    }

    pub fn file_system(&self) -> Arc<dyn ExecutorFileSystem> {
        self.environment.get_filesystem()
    }
}

impl EnvironmentManager {
    /// Inspects selected roots without starting or waiting for an environment.
    ///
    /// Starting or recovering environments are omitted. Missing environments and terminal
    /// connection failures are returned as warnings so read-only catalog clients can distinguish
    /// them from an empty catalog.
    ///
    /// Environment IDs are stable identities, so callers can safely resolve a returned root by
    /// ID when they read it.
    pub fn inspect_selected_capability_roots(
        &self,
        selected_roots: &[SelectedCapabilityRoot],
    ) -> SelectedCapabilityRootsStatus {
        let (candidates, mut warnings) = {
            let environments = self
                .environments
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let mut candidates = Vec::with_capacity(selected_roots.len());
            let mut warnings = Vec::new();
            for selected_root in selected_roots {
                let CapabilityRootLocation::Environment { environment_id, .. } =
                    &selected_root.location;
                let Some(environment) = environments.get(environment_id) else {
                    warnings.push(format!(
                        "selected capability root `{}` references unavailable environment `{environment_id}`",
                        selected_root.id
                    ));
                    continue;
                };
                candidates.push((selected_root.clone(), Arc::clone(environment)));
            }
            (candidates, warnings)
        };
        let mut readiness = HashMap::new();
        for (selected_root, environment) in &candidates {
            let CapabilityRootLocation::Environment { environment_id, .. } =
                &selected_root.location;
            if readiness.contains_key(environment_id) {
                continue;
            }
            let ready = match environment.readiness_result() {
                Some(Ok(())) => true,
                Some(Err(error)) => {
                    warnings.push(format!(
                        "selected capability environment `{environment_id}` is unavailable: {error}"
                    ));
                    false
                }
                None => false,
            };
            readiness.insert(environment_id.clone(), ready);
        }

        let ready_roots = candidates
            .into_iter()
            .filter(|(selected_root, _)| {
                let CapabilityRootLocation::Environment { environment_id, .. } =
                    &selected_root.location;
                readiness.get(environment_id).copied().unwrap_or(false)
            })
            .map(|(selected_root, _)| selected_root)
            .collect();
        SelectedCapabilityRootsStatus {
            ready_roots,
            warnings,
        }
    }

    /// Resolves selected roots whose stable environments are ready for the current model step.
    ///
    /// Environment identity comes from the selected root's stable environment ID. A ready
    /// environment captured for the step carries its exact process-local handle so readiness and
    /// execution cannot come from different registry snapshots. Missing, starting, or failed
    /// environments are omitted. A lazy environment is started for a later step.
    pub async fn resolve_selected_capability_roots(
        &self,
        selected_roots: &[SelectedCapabilityRoot],
        captured_environments: &HashMap<String, Option<Arc<Environment>>>,
    ) -> Vec<ResolvedSelectedCapabilityRoot> {
        let candidates = {
            let environments = self
                .environments
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            selected_roots
                .iter()
                .filter_map(|selected_root| {
                    let CapabilityRootLocation::Environment { environment_id, .. } =
                        &selected_root.location;
                    let (environment, already_ready) =
                        match captured_environments.get(environment_id) {
                            Some(Some(environment)) => (Arc::clone(environment), true),
                            Some(None) => return None,
                            None => (Arc::clone(environments.get(environment_id)?), false),
                        };
                    Some((
                        ResolvedSelectedCapabilityRoot {
                            selected_root: selected_root.clone(),
                            environment,
                        },
                        already_ready,
                    ))
                })
                .collect::<Vec<_>>()
        };

        let mut readiness = HashMap::new();
        for (candidate, already_ready) in &candidates {
            let CapabilityRootLocation::Environment { environment_id, .. } =
                &candidate.selected_root().location;
            if readiness.contains_key(environment_id) {
                continue;
            }
            let environment = candidate.environment();
            let ready = if *already_ready {
                true
            } else if environment.startup_finished() {
                environment.wait_until_ready().await.is_ok()
            } else {
                Environment::start_connecting_for_use(environment);
                false
            };
            readiness.insert(environment_id.clone(), ready);
        }

        candidates
            .into_iter()
            .map(|(candidate, _)| candidate)
            .filter(|candidate| {
                let CapabilityRootLocation::Environment { environment_id, .. } =
                    &candidate.selected_root().location;
                readiness.get(environment_id).copied().unwrap_or(false)
            })
            .collect()
    }
}

impl fmt::Debug for ResolvedSelectedCapabilityRoot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResolvedSelectedCapabilityRoot")
            .field("selected_root", &self.selected_root)
            .finish_non_exhaustive()
    }
}
