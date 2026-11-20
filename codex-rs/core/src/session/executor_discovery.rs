//! Core owns a thread discovery cache shared across turns. Explicit refresh evicts every entry.
//! In-flight readers may keep their inventories; invalidation applies to the next lookup.
//! Prewarming bounds discovery before readiness without publishing MCP state or failing startup.

use super::Session;
use super::TurnContext;
use super::turn_context::TurnEnvironment;
use crate::McpRefreshReason;
use crate::config::Config;
use crate::environment_selection::TurnEnvironmentSnapshot;
use codex_exec_server::DiscoverV2CapabilitiesRequest;
use codex_exec_server::Environment;
use codex_exec_server::ExecutorCapabilityDiscoveryV2Cache;
use codex_exec_server::ExecutorCapabilityDiscoveryV2Snapshot;
use codex_exec_server::supports_capability_discovery_v2;
use codex_features::Feature;
use std::sync::Arc;
use std::time::Duration;

const DISCOVERY_PREWARM_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Default)]
pub(crate) struct ExecutorDiscovery {
    cache: ExecutorCapabilityDiscoveryV2Cache,
}

impl ExecutorDiscovery {
    pub(crate) fn ensure_capacity(&self, minimum: usize) {
        self.cache.ensure_capacity(minimum);
    }

    #[expect(dead_code, reason = "Reserved for future environment-scoped refresh")]
    pub(crate) fn invalidate_environment(&self, environment: &Arc<Environment>) {
        self.cache.invalidate_environment(environment);
    }

    pub(crate) async fn prewarm(&self, selected: &TurnEnvironment) {
        let prewarm = async {
            let Ok(info) = selected.environment().info().await else {
                tracing::warn!(
                    environment_id = selected.selection.environment_id,
                    "could not read executor discovery V2 support during prewarm"
                );
                return;
            };
            if !supports_capability_discovery_v2(&info) {
                return;
            }
            // Discovery logs failures and retains them according to its existing retry policy.
            let _ = self
                .cache
                .discover(
                    &selected.selection.environment_id,
                    selected.environment(),
                    DiscoverV2CapabilitiesRequest {
                        force_reload: false,
                        cwd: selected.cwd().clone(),
                        sandbox: Some(selected.sandbox_context(/*additional_permissions*/ None)),
                    },
                )
                .await;
        };
        if tokio::time::timeout(DISCOVERY_PREWARM_TIMEOUT, prewarm)
            .await
            .is_err()
        {
            // Dropping the load releases the cache's OnceCell for a later step to retry.
            tracing::warn!(
                environment_id = selected.selection.environment_id,
                timeout_ms = DISCOVERY_PREWARM_TIMEOUT.as_millis(),
                "executor discovery V2 prewarm timed out; continuing environment startup"
            );
        }
    }
}

impl Session {
    pub(crate) fn invalidate_executor_discovery(&self) {
        self.services
            .thread_extension_data
            .get_or_init(ExecutorDiscovery::default)
            .cache
            .invalidate();
        self.mark_mcp_runtime_dirty(McpRefreshReason::ExplicitInvalidation);
    }

    /// Acquires V2 inventories and reports changes without scheduling an MCP refresh.
    pub(super) async fn executor_capability_discovery_for_step_v2(
        &self,
        config: &Config,
        environments: &TurnEnvironmentSnapshot,
        skill_turn: Option<&TurnContext>,
    ) -> (Option<Vec<ExecutorCapabilityDiscoveryV2Snapshot>>, bool) {
        if !config
            .features
            .enabled(Feature::ExecutorCapabilityDiscoveryV2)
        {
            return (None, false);
        }
        let discovery = self
            .services
            .thread_extension_data
            .get_or_init(ExecutorDiscovery::default);
        let cache = &discovery.cache;
        cache.ensure_capacity(
            environments
                .turn_environments()
                .filter(|selected| selected.environment().is_remote())
                .count(),
        );
        let mut snapshots = Vec::new();
        let mut skipped_incompatible_executor = false;
        for selected in environments.turn_environments() {
            if !selected.environment().is_remote() {
                continue;
            }
            let environment_id = &selected.selection.environment_id;
            let info = match selected.environment().info().await {
                Ok(info) => info,
                Err(_) => {
                    tracing::warn!(
                        environment_id,
                        "could not read executor discovery V2 support"
                    );
                    continue;
                }
            };
            if !supports_capability_discovery_v2(&info) {
                skipped_incompatible_executor = true;
                continue;
            }
            let request = DiscoverV2CapabilitiesRequest {
                force_reload: false,
                cwd: selected.cwd().clone(),
                sandbox: Some(self.discovery_sandbox(selected, skill_turn)),
            };
            let result = cache
                .discover(environment_id, selected.environment(), request)
                .await;
            snapshots.push(ExecutorCapabilityDiscoveryV2Snapshot {
                environment_id: environment_id.clone(),
                result: result.map_err(|_| "executor discovery V2 failed".to_string()),
            });
        }
        let incompatible = snapshots.is_empty() && skipped_incompatible_executor;
        if skill_turn.is_some() {
            // Only session discovery updates the inventory used to invalidate MCP.
            return ((!incompatible).then_some(snapshots), false);
        }
        if incompatible {
            let changed = self
                .services
                .thread_extension_data
                .remove::<Vec<ExecutorCapabilityDiscoveryV2Snapshot>>()
                .is_some();
            return (None, changed);
        }
        let previous = self
            .services
            .thread_extension_data
            .insert(snapshots.clone());
        let inventory_changed = previous.as_ref().map_or(!snapshots.is_empty(), |previous| {
            previous.len() != snapshots.len()
                || previous
                    .iter()
                    .zip(&snapshots)
                    .any(|(previous, current)| !previous.same_discovery(current))
        });
        (Some(snapshots), inventory_changed)
    }
}
