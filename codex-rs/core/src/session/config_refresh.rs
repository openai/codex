//! Publishes independent Guardian policy, MCP configuration, and host rollouts.
//! Preserves unrelated session settings.

use crate::McpRefreshReason;
use std::sync::Arc;

use crate::ConfigRefreshOutcome;
use crate::config::Config;
use crate::config::Constrained;
use crate::config::RuntimeConfigRefresh;
use crate::session::Session;
use codex_config::CloudConfigBundleBindingStatus;
use codex_config::McpServerAuth;
use codex_features::Feature;
use tracing::warn;

impl Session {
    pub(crate) async fn disable_mcp_enterprise_auth(&self) {
        {
            let mut state = self.state.lock().await;
            let mut config = state
                .session_configuration
                .original_config_do_not_use
                .as_ref()
                .clone();
            config.config_layer_stack.mcp_ema_cancellation().cancel();
            config.disable_mcp_enterprise_auth();
            state.session_configuration.original_config_do_not_use = Arc::new(config);
            self.services.mcp_runtime.invalidate_resource_caches();
            self.mark_mcp_runtime_dirty(McpRefreshReason::EnterpriseAuthDisabled);
        }
        self.services.mcp_runtime.quiesce_ema().await;
        self.schedule_mcp_prewarm();
    }

    /// Publishes managed Guardian requirements without validating or restarting MCP.
    /// User and session fallbacks remain pinned until their own reload.
    pub(crate) async fn refresh_guardian_policy(
        &self,
        snapshot: &codex_config::GuardianPolicySnapshot,
    ) -> ConfigRefreshOutcome {
        let mut state = self.state.lock().await;
        let current = &state.session_configuration.original_config_do_not_use;
        if snapshot.revision <= current.guardian_policy_revision {
            return ConfigRefreshOutcome::Stale;
        }
        let binding = snapshot
            .binding
            .as_ref()
            .map(codex_config::CloudConfigBundleBinding::read);
        if binding
            .as_ref()
            .is_some_and(|guard| guard.status != CloudConfigBundleBindingStatus::Current)
        {
            return ConfigRefreshOutcome::Rejected;
        }
        let mut config = current.as_ref().clone();
        if config.apply_guardian_policy(snapshot).is_err() {
            return ConfigRefreshOutcome::Rejected;
        }
        let configuration = &state.session_configuration;
        let was_blocked = configuration
            .step_settings
            .validate(&configuration.step_settings_constraints(&configuration.environments))
            .is_err();
        let model = (!configuration.trusted_guardian_reviewer)
            .then_some(configuration.step_settings.collaboration_mode.model());
        let requested = config
            .requested_approvals_reviewer
            .unwrap_or(configuration.step_settings.approvals_reviewer);
        let requirements = config.config_layer_stack.requirements();
        let resolved = crate::config::resolve_guardian_reviewer(requested, model, requirements);
        // Publish both restrictions even when they rule out the current model.
        let reviewer = resolved
            .as_ref()
            .copied()
            .unwrap_or_else(|_| requirements.approvals_reviewer.value());
        config.requested_approvals_reviewer = Some(requested);
        config.approvals_reviewer = reviewer;
        let notify_contributors = configuration.step_settings.approvals_reviewer != reviewer
            && !self.services.extensions.config_contributors().is_empty();
        let previous_config =
            notify_contributors.then(|| Self::build_effective_session_config(configuration));
        let previous = Arc::downgrade(current);
        let config = Arc::new(config);
        let snapshots = &mut state.guardian_config_snapshots;
        if snapshots
            .last()
            .is_none_or(|latest| !latest.ptr_eq(&previous))
        {
            snapshots.clear();
            snapshots.push(previous);
        }
        snapshots.retain(|owner| owner.strong_count() != 0);
        snapshots.push(Arc::downgrade(&config));
        let settings = &mut state.session_configuration.step_settings;
        if settings.approvals_reviewer != reviewer {
            Arc::make_mut(settings).approvals_reviewer = reviewer;
        }
        state.session_configuration.original_config_do_not_use = config;
        let configuration = &state.session_configuration;
        let validation = resolved.and_then(|_| {
            configuration
                .step_settings
                .validate(&configuration.step_settings_constraints(&configuration.environments))
        });
        // The reviewer change is published even if the new policy blocks the current model.
        let new_config =
            notify_contributors.then(|| Self::build_effective_session_config(configuration));
        let outcome = match validation {
            Ok(()) => {
                if was_blocked && !state.shutting_down {
                    self.input_queue.pending_work_wake.notify_one();
                }
                ConfigRefreshOutcome::Published
            }
            Err(error) => {
                warn!(%error, "Guardian policy blocks the current model");
                ConfigRefreshOutcome::Rejected
            }
        };
        drop(binding);
        drop(state);
        self.emit_config_changed_contributors(previous_config.as_ref(), new_config.as_ref());
        outcome
    }

    pub(crate) async fn refresh_runtime_config(
        &self,
        expected_config: Arc<Config>,
        next_config: Config,
    ) -> ConfigRefreshOutcome {
        self.refresh_config(expected_config, next_config, RuntimeConfigRefresh::User)
            .await
    }

    pub(crate) async fn refresh_mcp_config(
        &self,
        expected_config: Arc<Config>,
        next_config: Config,
    ) -> ConfigRefreshOutcome {
        self.refresh_config(expected_config, next_config, RuntimeConfigRefresh::Mcp)
            .await
    }

    pub(super) async fn refresh_config(
        &self,
        expected_config: Arc<Config>,
        next_config: Config,
        scope: RuntimeConfigRefresh,
    ) -> ConfigRefreshOutcome {
        let (mut config, mut outcome, rejection_kind) =
            match expected_config.resolve_runtime_refresh(&next_config, scope) {
                Ok(config) => (config, ConfigRefreshOutcome::Published, None),
                Err(error) => {
                    let mut config = expected_config.as_ref().clone();
                    if matches!(scope, RuntimeConfigRefresh::Mcp) {
                        // The host-resolved map already carries fresh requirements and
                        // preserved session overrides. Keep ordinary-server revocations even
                        // when retained layers make the enterprise merge invalid.
                        let mut refreshed_servers = next_config.mcp_servers.clone();
                        let mut servers = refreshed_servers.get().clone();
                        // Retain enterprise identities so a later corrected refresh cannot
                        // admit an ordinary-auth replacement under a retired server's name.
                        for (name, server) in expected_config.mcp_servers.get() {
                            if matches!(server.auth, McpServerAuth::EmaAuth)
                                || server.ema_registration().is_some()
                            {
                                let mut retired = server.clone();
                                retired.enabled = false;
                                servers.insert(name.clone(), retired);
                            }
                        }
                        config.mcp_servers = if refreshed_servers.set(servers).is_ok() {
                            refreshed_servers
                        } else {
                            Constrained::allow_any(Default::default())
                        };
                    }
                    config
                        .config_layer_stack
                        .refresh_mcp_requirements(&next_config.config_layer_stack);
                    if let Err(err) = config.features.refresh_mcp_features(&next_config.features) {
                        warn!(%err, "failed to apply managed MCP feature restrictions");
                        config.mcp_servers = Constrained::allow_any(Default::default());
                    }
                    config.disable_mcp_enterprise_auth();
                    (config, ConfigRefreshOutcome::Rejected, Some(error.kind()))
                }
            };
        if !matches!(scope, RuntimeConfigRefresh::UserFiles) {
            for (feature, description) in [
                (Feature::Mcp20260728, "MCP protocol"),
                (
                    Feature::DaybreakProgramPropagation,
                    "Daybreak helper propagation",
                ),
                (Feature::CodexAppsMcp20260728, "Codex Apps MCP protocol"),
            ] {
                if let Err(err) = config
                    .features
                    .set_enabled(feature, next_config.features.enabled(feature))
                {
                    warn!("failed to refresh {description} config: {err}");
                }
            }
        }
        // Publish host rollout changes only to future turn snapshots. Active turns
        // must retain one capture policy across tool dispatch and Guardian review.
        if matches!(scope, RuntimeConfigRefresh::User)
            && let Err(err) = config.features.set_enabled(
                Feature::CriticalPathTiming,
                next_config.features.enabled(Feature::CriticalPathTiming),
            )
        {
            warn!("failed to refresh critical path timing config: {err}");
        }
        let mut config = Arc::new(config);
        let notify_contributors = !matches!(scope, RuntimeConfigRefresh::Mcp)
            && !self.services.extensions.config_contributors().is_empty();
        let (previous_config, new_config) = {
            let mut state = self.state.lock().await;
            let current = &state.session_configuration.original_config_do_not_use;
            if !Arc::ptr_eq(current, &expected_config) {
                let snapshots = &state.guardian_config_snapshots;
                if !matches!(scope, RuntimeConfigRefresh::Mcp)
                    || snapshots
                        .last()
                        .is_none_or(|latest| !std::ptr::eq(latest.as_ptr(), Arc::as_ptr(current)))
                    || !snapshots
                        .iter()
                        .any(|owner| std::ptr::eq(owner.as_ptr(), Arc::as_ptr(&expected_config)))
                {
                    return ConfigRefreshOutcome::Stale;
                }
                // All MCP inputs still belong to the captured owner. Carry forward
                // the Guardian policy that was published while we loaded them.
                Arc::make_mut(&mut config).retain_guardian_policy_from(current);
            }
            // User-only refreshes retain the current managed inputs and their
            // admission binding. Guardian policy has a separate publication path.
            let policy_guard = matches!(scope, RuntimeConfigRefresh::Mcp)
                .then(|| next_config.config_layer_stack.cloud_config_binding())
                .flatten()
                .map(codex_config::CloudConfigBundleBinding::read);
            let policy_status = policy_guard
                .as_ref()
                .map(|guard| guard.status)
                .unwrap_or(CloudConfigBundleBindingStatus::Current);
            match policy_status {
                CloudConfigBundleBindingStatus::Current => {}
                CloudConfigBundleBindingStatus::Stale => return ConfigRefreshOutcome::Stale,
                CloudConfigBundleBindingStatus::Suspended => {
                    // Publish ordinary changes under the last valid requirements,
                    // but revoke enterprise authority until a valid policy is loaded.
                    let config = Arc::make_mut(&mut config);
                    config.disable_mcp_enterprise_auth();
                    config.config_layer_stack = config
                        .config_layer_stack
                        .clone()
                        .with_cloud_config_binding(/*binding*/ None);
                    outcome = ConfigRefreshOutcome::Rejected;
                }
            }
            if let Some(rejection_kind) = rejection_kind {
                warn!(
                    ?rejection_kind,
                    "disabling enterprise MCP after rejected configuration refresh"
                );
            }
            if matches!(scope, RuntimeConfigRefresh::UserFiles) {
                self.services.skills_service.clear_cache();
                self.services.plugins_manager.clear_cache();
            }
            // A host refresh carries current rollout settings for recording. A
            // legacy file reload uses its original snapshot and bypasses this path.
            if matches!(scope, RuntimeConfigRefresh::User) {
                self.services
                    .executed_tool_calls
                    .refresh(&next_config.features);
            }
            let previous_config = notify_contributors
                .then(|| Self::build_effective_session_config(&state.session_configuration));
            if config.mcp_enterprise_managed_auth.is_none() {
                config.config_layer_stack.mcp_ema_cancellation().cancel();
            }
            if !matches!(scope, RuntimeConfigRefresh::UserFiles) {
                let cancellation = config
                    .config_layer_stack
                    .mcp_ema_cancellation()
                    .refreshed(next_config.config_layer_stack.mcp_ema_cancellation());
                let config = Arc::make_mut(&mut config);
                config.config_layer_stack = config
                    .config_layer_stack
                    .clone()
                    .with_mcp_ema_cancellation(cancellation);
            }
            state.session_configuration.original_config_do_not_use = Arc::clone(&config);
            state.guardian_config_snapshots.clear();
            drop(policy_guard);
            self.mark_mcp_runtime_dirty(match scope {
                RuntimeConfigRefresh::User => McpRefreshReason::UserConfigReloaded,
                RuntimeConfigRefresh::Mcp => McpRefreshReason::McpConfigReloaded,
                RuntimeConfigRefresh::UserFiles => McpRefreshReason::UserConfigFilesReloaded,
            });
            let new_config = notify_contributors
                .then(|| Self::build_effective_session_config(&state.session_configuration));
            (previous_config, new_config)
        };
        if config
            .config_layer_stack
            .mcp_ema_cancellation()
            .is_cancelled()
        {
            self.services.mcp_runtime.quiesce_ema().await;
        }
        self.emit_config_changed_contributors(previous_config.as_ref(), new_config.as_ref());
        self.schedule_mcp_prewarm();
        self.refresh_hooks(config).await;
        outcome
    }
}
