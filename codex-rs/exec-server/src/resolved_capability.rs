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

impl ResolvedSelectedCapabilityRoot {
    fn new(selected_root: SelectedCapabilityRoot, environment: Arc<Environment>) -> Self {
        Self {
            selected_root,
            environment,
        }
    }

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
    /// Resolves selected roots whose stable environments are ready for the current model step.
    ///
    /// Environment identity comes from the selected root's stable environment ID. Captured
    /// availability keeps this projection consistent with the step's environment World State;
    /// it is not an executor-identity or content cache key. Missing, starting, or failed
    /// environments are omitted. A lazy environment is started for a later step.
    pub async fn resolve_selected_capability_roots(
        &self,
        selected_roots: &[SelectedCapabilityRoot],
        captured_availability: &HashMap<String, bool>,
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
                    let already_ready = match captured_availability.get(environment_id) {
                        Some(true) => true,
                        Some(false) => return None,
                        None => false,
                    };
                    let environment = Arc::clone(environments.get(environment_id)?);
                    Some((
                        ResolvedSelectedCapabilityRoot::new(selected_root.clone(), environment),
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
