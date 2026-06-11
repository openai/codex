//! Runtime configuration persistence helpers for the TUI app.
//!
//! This module owns the app-level glue between config.toml edits, in-memory `Config` refreshes,
//! and the ChatWidget copy of session settings, keeping persistence-heavy code out of the main app
//! loop.

use super::*;
#[cfg(target_os = "windows")]
use codex_utils_approval_presets::ApprovalPreset;

#[cfg(target_os = "windows")]
pub(super) struct WindowsSetupPermissions {
    pub(super) permission_profile: PermissionProfile,
    pub(super) workspace_roots: Vec<AbsolutePathBuf>,
}

async fn build_config_on_runtime_worker(
    builder: ConfigBuilder,
    error_context: String,
) -> Result<Config> {
    match tokio::spawn(async move { builder.build().await }).await {
        Ok(build_result) => build_result.wrap_err(error_context),
        Err(err) if err.is_panic() => std::panic::resume_unwind(err.into_panic()),
        Err(err) => Err(err).wrap_err_with(|| format!("{error_context} task failed")),
    }
}

impl App {
    pub(super) async fn rebuild_config_for_cwd(&self, cwd: PathBuf) -> Result<Config> {
        let mut overrides = self.harness_overrides.clone();
        overrides.cwd = Some(cwd.clone());
        let cwd_display = cwd.display().to_string();
        let builder = ConfigBuilder::default()
            .codex_home(self.config.codex_home.to_path_buf())
            .cli_overrides(self.cli_kv_overrides.clone())
            .harness_overrides(overrides)
            .loader_overrides(self.loader_overrides.clone())
            .cloud_config_bundle(self.cloud_config_bundle.clone());
        build_config_on_runtime_worker(
            builder,
            format!("Failed to rebuild config for cwd {cwd_display}"),
        )
        .await
    }

    pub(super) async fn rebuild_config_for_permission_profile(
        &self,
        profile_id: &str,
    ) -> Result<Config> {
        let mut overrides = self.harness_overrides.clone();
        overrides.cwd = Some(self.chat_widget.config_ref().cwd.to_path_buf());
        overrides.sandbox_mode = None;
        overrides.permission_profile = None;
        overrides.default_permissions = Some(profile_id.to_string());
        let builder = ConfigBuilder::default()
            .codex_home(self.config.codex_home.to_path_buf())
            .cli_overrides(self.cli_kv_overrides.clone())
            .harness_overrides(overrides)
            .loader_overrides(self.loader_overrides.clone())
            .cloud_config_bundle(self.cloud_config_bundle.clone());
        build_config_on_runtime_worker(
            builder,
            format!("Failed to rebuild config for permission profile {profile_id}"),
        )
        .await
    }

    #[cfg(target_os = "windows")]
    pub(super) async fn windows_setup_permissions(
        &self,
        preset: &ApprovalPreset,
        profile_selection: Option<&PermissionProfileSelection>,
    ) -> Result<WindowsSetupPermissions> {
        match profile_selection {
            Some(selection) => {
                let selected_config = self
                    .rebuild_config_for_permission_profile(selection.profile_id.as_str())
                    .await?;
                Ok(WindowsSetupPermissions {
                    permission_profile: selected_config.permissions.permission_profile().clone(),
                    workspace_roots: selected_config.effective_workspace_roots(),
                })
            }
            None => Ok(WindowsSetupPermissions {
                permission_profile: preset.permission_profile.clone(),
                workspace_roots: self.config.effective_workspace_roots(),
            }),
        }
    }

    pub(super) async fn apply_permission_profile_selection(
        &mut self,
        selection: PermissionProfileSelection,
    ) -> bool {
        let PermissionProfileSelection {
            profile_id,
            approval_policy,
            approvals_reviewer,
            display_label,
        } = selection;
        let selected_config = match self
            .rebuild_config_for_permission_profile(profile_id.as_str())
            .await
        {
            Ok(config) => config,
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    profile_id,
                    "failed to resolve selected permission profile"
                );
                self.chat_widget.add_error_message(format!(
                    "Failed to set permission profile `{profile_id}`: {err}"
                ));
                return false;
            }
        };
        let permission_profile = selected_config.permissions.permission_profile();
        let active_permission_profile = selected_config.permissions.active_permission_profile();
        let network = selected_config.permissions.network.clone();

        let mut config = self.config.clone();
        if let Some(policy) = approval_policy
            && !self.try_set_approval_policy_on_config(
                &mut config,
                policy,
                "Failed to set approval policy",
                "failed to set selected permission profile approval policy on app config",
            )
        {
            return false;
        }
        if let Err(err) = config
            .permissions
            .set_permission_profile_from_session_snapshot(
                PermissionProfileSnapshot::from_session_snapshot(
                    permission_profile.clone(),
                    active_permission_profile.clone(),
                ),
            )
        {
            tracing::warn!(
                error = %err,
                profile_id,
                "failed to set selected permission profile on app config"
            );
            self.chat_widget.add_error_message(format!(
                "Failed to set permission profile `{profile_id}`: {err}"
            ));
            return false;
        }
        if let Some(reviewer) = approvals_reviewer {
            config.approvals_reviewer = reviewer;
        }
        config.permissions.network = network.clone();
        self.config = config;

        if let Some(policy) = approval_policy {
            self.runtime_approval_policy_override = Some(policy);
            self.chat_widget.set_approval_policy(policy);
        }
        if let Err(err) = self.chat_widget.set_permission_profile_with_active_profile(
            permission_profile.clone(),
            active_permission_profile.clone(),
        ) {
            tracing::warn!(
                error = %err,
                profile_id,
                "failed to set selected permission profile on chat config"
            );
            self.chat_widget.add_error_message(format!(
                "Failed to set permission profile `{profile_id}`: {err}"
            ));
            return false;
        }
        if let Some(reviewer) = approvals_reviewer {
            self.chat_widget.set_approvals_reviewer(reviewer);
        }
        self.chat_widget.set_permission_network(network);
        self.runtime_permission_profile_override =
            Some(RuntimePermissionProfileOverride::from_config(&self.config));
        self.sync_active_thread_permission_settings_to_cached_session()
            .await;
        self.app_event_tx
            .send(AppEvent::CodexOp(AppCommand::override_turn_context(
                /*cwd*/ None,
                approval_policy,
                approvals_reviewer,
                Some(permission_profile.clone()),
                active_permission_profile,
                /*windows_sandbox_level*/ None,
                /*model*/ None,
                /*effort*/ None,
                /*summary*/ None,
                /*service_tier*/ None,
                /*collaboration_mode*/ None,
                /*personality*/ None,
            )));
        self.app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
            history_cell::new_info_event(
                format!("Permissions updated to {display_label}"),
                /*hint*/ None,
            ),
        )));
        true
    }

    pub(super) async fn refresh_in_memory_config_from_disk(&mut self) -> Result<()> {
        let mut config = self
            .rebuild_config_for_cwd(self.chat_widget.config_ref().cwd.to_path_buf())
            .await?;
        self.apply_runtime_policy_overrides(&mut config);
        self.config = config;
        self.chat_widget.sync_plugin_mentions_config(&self.config);
        Ok(())
    }

    pub(super) async fn refresh_in_memory_config_from_disk_best_effort(&mut self, action: &str) {
        if let Err(err) = self.refresh_in_memory_config_from_disk().await {
            tracing::warn!(
                error = %err,
                action,
                "failed to refresh config before thread transition; continuing with current in-memory config"
            );
        }
    }

    pub(super) async fn read_effective_config_after_overridden_write(
        &mut self,
        app_server: &mut AppServerSession,
        setting: &str,
    ) -> Option<ConfigReadResponse> {
        let cwd = self.chat_widget.config_ref().cwd.display().to_string();
        match crate::config_update::read_effective_config(app_server.request_handle(), cwd).await {
            Ok(response) => Some(response),
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    setting,
                    "failed to refresh effective config after an overridden write"
                );
                self.chat_widget.add_error_message(format!(
                    "{setting} were saved, but Codex could not refresh the effective config: {err}"
                ));
                None
            }
        }
    }

    pub(super) async fn rebuild_config_for_resume_or_fallback(
        &mut self,
        current_cwd: &Path,
        resume_cwd: PathBuf,
    ) -> Result<Config> {
        match self.rebuild_config_for_cwd(resume_cwd.clone()).await {
            Ok(config) => Ok(config),
            Err(err) => {
                if crate::session_resume::cwds_differ(current_cwd, &resume_cwd) {
                    Err(err)
                } else {
                    let resume_cwd_display = resume_cwd.display().to_string();
                    tracing::warn!(
                        error = %err,
                        cwd = %resume_cwd_display,
                        "failed to rebuild config for same-cwd resume; using current in-memory config"
                    );
                    Ok(self.config.clone())
                }
            }
        }
    }

    pub(super) fn apply_runtime_policy_overrides(&mut self, config: &mut Config) {
        if let Some(policy) = self.runtime_approval_policy_override.as_ref()
            && let Err(err) = config.permissions.approval_policy.set(policy.to_core())
        {
            tracing::warn!(%err, "failed to carry forward approval policy override");
            self.chat_widget.add_error_message(format!(
                "Failed to carry forward approval policy override: {err}"
            ));
        }
        if let Some(profile_override) = self.runtime_permission_profile_override.as_ref() {
            match config
                .permissions
                .set_permission_profile_from_session_snapshot(
                    PermissionProfileSnapshot::from_session_snapshot(
                        profile_override.permission_profile.clone(),
                        profile_override.active_permission_profile.clone(),
                    ),
                ) {
                Ok(()) => {
                    config.permissions.network = profile_override.network.clone();
                }
                Err(err) => {
                    tracing::warn!(%err, "failed to carry forward permission profile override");
                    self.chat_widget.add_error_message(format!(
                        "Failed to carry forward permission profile override: {err}"
                    ));
                }
            }
        }
    }

    pub(super) fn set_approvals_reviewer_in_app_and_widget(&mut self, reviewer: ApprovalsReviewer) {
        self.config.approvals_reviewer = reviewer;
        self.chat_widget.set_approvals_reviewer(reviewer);
    }

    pub(super) fn try_set_approval_policy_on_config(
        &mut self,
        config: &mut Config,
        policy: AskForApproval,
        user_message_prefix: &str,
        log_message: &str,
    ) -> bool {
        if let Err(err) = config.permissions.approval_policy.set(policy.to_core()) {
            tracing::warn!(error = %err, "{log_message}");
            self.chat_widget
                .add_error_message(format!("{user_message_prefix}: {err}"));
            return false;
        }

        true
    }

    pub(super) fn try_set_builtin_active_permission_profile_on_config(
        &mut self,
        config: &mut Config,
        active_permission_profile: ActivePermissionProfile,
        user_message_prefix: &str,
        log_message: &str,
    ) -> Option<PermissionProfile> {
        let Some(permission_profile) =
            builtin_permission_profile_for_active_permission_profile(&active_permission_profile)
        else {
            tracing::warn!(
                id = %active_permission_profile.id,
                "{log_message}: unsupported active permission profile"
            );
            self.chat_widget.add_error_message(format!(
                "{user_message_prefix}: unsupported active permission profile `{}`",
                active_permission_profile.id
            ));
            return None;
        };

        if let Err(err) = config
            .permissions
            .set_permission_profile_from_session_snapshot(PermissionProfileSnapshot::active(
                permission_profile.clone(),
                active_permission_profile,
            ))
        {
            tracing::warn!(error = %err, "{log_message}");
            self.chat_widget
                .add_error_message(format!("{user_message_prefix}: {err}"));
            return None;
        }

        Some(permission_profile)
    }

    pub(super) async fn update_feature_flags(
        &mut self,
        app_server: &mut AppServerSession,
        updates: Vec<(Feature, bool)>,
    ) {
        if updates.is_empty() {
            return;
        }

        let auto_review_preset = auto_review_mode();
        let mut next_config = self.config.clone();
        let windows_sandbox_changed = updates.iter().any(|(feature, _)| {
            matches!(
                feature,
                Feature::WindowsSandbox | Feature::WindowsSandboxElevated
            )
        });
        let mut approval_policy_override = None;
        let mut approvals_reviewer_override = None;
        let mut permission_profile_override = None;
        let mut active_permission_profile_override = None;
        let mut feature_updates_to_apply = Vec::with_capacity(updates.len());
        let mut permissions_history_label: Option<&'static str> = None;
        let mut config_edits = Vec::new();

        for (feature, enabled) in updates {
            let feature_key = feature.key();
            let mut feature_edits = Vec::new();
            let mut feature_config = next_config.clone();
            if let Err(err) = feature_config.features.set_enabled(feature, enabled) {
                tracing::error!(
                    error = %err,
                    feature = feature_key,
                    "failed to update constrained feature flags"
                );
                self.chat_widget.add_error_message(format!(
                    "Failed to update experimental feature `{feature_key}`: {err}"
                ));
                continue;
            }
            let effective_enabled = feature_config.features.enabled(feature);
            if feature == Feature::GuardianApproval {
                let previous_approvals_reviewer = feature_config.approvals_reviewer;
                if effective_enabled {
                    // Persist the reviewer setting so future sessions keep the
                    // experiment's matching `/permissions` mode until the user
                    // changes it explicitly.
                    feature_config.approvals_reviewer = auto_review_preset.approvals_reviewer;
                    feature_edits.push(crate::config_update::replace_config_value(
                        "approvals_reviewer",
                        serde_json::json!(auto_review_preset.approvals_reviewer.to_string()),
                    ));
                    if previous_approvals_reviewer != auto_review_preset.approvals_reviewer {
                        permissions_history_label = Some("Approve for me");
                    }
                } else if !effective_enabled {
                    feature_edits.push(crate::config_update::clear_config_value(
                        "approvals_reviewer",
                    ));
                    feature_config.approvals_reviewer = ApprovalsReviewer::User;
                    if previous_approvals_reviewer != ApprovalsReviewer::User {
                        permissions_history_label = Some("Ask for approval");
                    }
                }
                approvals_reviewer_override = Some(feature_config.approvals_reviewer);
            }
            if feature == Feature::GuardianApproval && effective_enabled {
                // The feature flag alone is not enough for the live session.
                // We also align approval policy + sandbox to the Auto-review
                // preset so enabling the experiment immediately
                // makes guardian review observable in the current thread.
                if !self.try_set_approval_policy_on_config(
                    &mut feature_config,
                    auto_review_preset.approval_policy,
                    "Failed to enable Approve for me",
                    "failed to set auto-review approval policy on staged config",
                ) {
                    continue;
                }
                let Some(permission_profile) = self
                    .try_set_builtin_active_permission_profile_on_config(
                        &mut feature_config,
                        auto_review_preset.active_permission_profile.clone(),
                        "Failed to enable Approve for me",
                        "failed to set auto-review permission profile on staged config",
                    )
                else {
                    continue;
                };
                feature_edits.extend([
                    crate::config_update::replace_config_value(
                        "approval_policy",
                        serde_json::json!("on-request"),
                    ),
                    crate::config_update::replace_config_value(
                        "sandbox_mode",
                        serde_json::json!("workspace-write"),
                    ),
                ]);
                approval_policy_override = Some(auto_review_preset.approval_policy);
                permission_profile_override = Some(permission_profile);
                active_permission_profile_override =
                    Some(auto_review_preset.active_permission_profile.clone());
            }
            next_config = feature_config;
            feature_updates_to_apply.push((feature, effective_enabled));
            config_edits.extend(feature_edits);
            config_edits.push(crate::config_update::build_feature_enabled_edit(
                feature_key,
                effective_enabled,
            ));
        }

        // Persist first so the live session does not diverge from disk if the
        // config edit fails. Runtime/UI state is patched below only after the
        // durable config update succeeds.
        let write_response = match crate::config_update::write_config_batch(
            app_server.request_handle(),
            config_edits,
        )
        .await
        {
            Ok(response) => response,
            Err(err) => {
                let error = crate::config_update::format_config_error(&err);
                tracing::error!(error = %error, "failed to persist feature flags");
                self.chat_widget
                    .add_error_message(format!("Failed to update experimental features: {error}"));
                return;
            }
        };
        if write_response.status == WriteStatus::OkOverridden {
            let message = overridden_write_message(&write_response);
            tracing::warn!(
                message,
                "feature flag config write was overridden by effective config"
            );
            self.chat_widget.add_error_message(format!(
                "Experimental feature changes were saved but not applied: {message}"
            ));
            if let Some(effective_config) = self
                .read_effective_config_after_overridden_write(
                    app_server,
                    "Experimental feature changes",
                )
                .await
            {
                self.sync_feature_state_from_effective_config(
                    &effective_config,
                    &feature_updates_to_apply,
                );
                self.sync_auto_review_runtime_state_from_effective_config(
                    &effective_config,
                    &feature_updates_to_apply,
                )
                .await;
                if windows_sandbox_changed {
                    self.propagate_windows_sandbox_turn_context();
                }
            }
            return;
        }

        let memory_tool_was_enabled = self.config.features.enabled(Feature::MemoryTool);
        self.config = next_config;
        let show_memory_enable_notice =
            feature_updates_to_apply.iter().any(|(feature, enabled)| {
                *feature == Feature::MemoryTool && *enabled && !memory_tool_was_enabled
            });
        for (feature, effective_enabled) in feature_updates_to_apply {
            self.chat_widget
                .set_feature_enabled(feature, effective_enabled);
        }
        if show_memory_enable_notice {
            self.chat_widget.add_memories_enable_notice();
        }
        if approvals_reviewer_override.is_some() {
            self.set_approvals_reviewer_in_app_and_widget(self.config.approvals_reviewer);
        }
        if approval_policy_override.is_some() {
            self.chat_widget.set_approval_policy(AskForApproval::from(
                self.config.permissions.approval_policy.value(),
            ));
        }
        let permission_profile_override_value = permission_profile_override
            .is_some()
            .then(|| self.config.permissions.permission_profile().clone());
        if let Some(permission_profile) = permission_profile_override_value.as_ref()
            && let Err(err) = self
                .chat_widget
                .set_permission_profile_from_session_snapshot(
                    PermissionProfileSnapshot::from_session_snapshot(
                        permission_profile.clone(),
                        active_permission_profile_override.clone(),
                    ),
                )
        {
            tracing::error!(
                error = %err,
                "failed to set auto-review permission profile on chat config"
            );
            self.chat_widget
                .add_error_message(format!("Failed to enable Approve for me: {err}"));
        }
        if permission_profile_override.is_some() {
            self.runtime_permission_profile_override =
                Some(RuntimePermissionProfileOverride::from_config(&self.config));
        }

        if approval_policy_override.is_some()
            || approvals_reviewer_override.is_some()
            || permission_profile_override.is_some()
        {
            self.sync_active_thread_permission_settings_to_cached_session()
                .await;
            // This uses `OverrideTurnContext` intentionally: toggling the
            // experiment should update the active thread's effective approval
            // settings immediately, just like a `/permissions` selection. Without
            // this runtime patch, the config edit would only affect future
            // sessions or turns recreated from disk.
            let op = AppCommand::override_turn_context(
                /*cwd*/ None,
                approval_policy_override,
                approvals_reviewer_override,
                permission_profile_override,
                active_permission_profile_override,
                /*windows_sandbox_level*/ None,
                /*model*/ None,
                /*effort*/ None,
                /*summary*/ None,
                /*service_tier*/ None,
                /*collaboration_mode*/ None,
                /*personality*/ None,
            );
            let replay_state_op =
                ThreadEventStore::op_can_change_pending_replay_state(&op).then(|| op.clone());
            let submitted = self.chat_widget.submit_op(op);
            if submitted && let Some(op) = replay_state_op.as_ref() {
                self.note_active_thread_outbound_op(op).await;
                self.refresh_pending_thread_approvals().await;
            }
        }

        if windows_sandbox_changed {
            self.propagate_windows_sandbox_turn_context();
        }

        if let Some(label) = permissions_history_label {
            self.chat_widget.add_info_message(
                format!("Permissions updated to {label}"),
                /*hint*/ None,
            );
        }
    }

    pub(super) async fn update_memory_settings(
        &mut self,
        app_server: &mut AppServerSession,
        use_memories: bool,
        generate_memories: bool,
    ) -> bool {
        let edits =
            crate::config_update::build_memory_settings_edits(use_memories, generate_memories);

        let write_response = match crate::config_update::write_config_batch(
            app_server.request_handle(),
            edits,
        )
        .await
        {
            Ok(response) => response,
            Err(err) => {
                tracing::error!(error = %err, "failed to persist memory settings");
                self.chat_widget
                    .add_error_message(format!("Failed to save memory settings: {err}"));
                return false;
            }
        };
        if write_response.status == WriteStatus::OkOverridden {
            let message = overridden_write_message(&write_response);
            tracing::warn!(
                message,
                "memory settings config write was overridden by effective config"
            );
            self.chat_widget.add_error_message(format!(
                "Memory setting changes were saved but not applied: {message}"
            ));
            let Some(effective_config) = self
                .read_effective_config_after_overridden_write(app_server, "Memory setting changes")
                .await
            else {
                return false;
            };
            return self.sync_memory_state_from_effective_config(&effective_config);
        }

        self.config.memories.use_memories = use_memories;
        self.config.memories.generate_memories = generate_memories;
        self.chat_widget
            .set_memory_settings(use_memories, generate_memories);
        true
    }

    pub(super) async fn update_memory_settings_with_app_server(
        &mut self,
        app_server: &mut AppServerSession,
        use_memories: bool,
        generate_memories: bool,
    ) {
        let previous_generate_memories = self.config.memories.generate_memories;
        if !self
            .update_memory_settings(app_server, use_memories, generate_memories)
            .await
        {
            return;
        }

        let generate_memories = self.config.memories.generate_memories;
        if previous_generate_memories == generate_memories {
            return;
        }

        let Some(thread_id) = self.current_displayed_thread_id() else {
            return;
        };

        let mode = if generate_memories {
            ThreadMemoryMode::Enabled
        } else {
            ThreadMemoryMode::Disabled
        };

        if let Err(err) = app_server.thread_memory_mode_set(thread_id, mode).await {
            tracing::error!(error = %err, %thread_id, "failed to update thread memory mode");
            self.chat_widget.add_error_message(format!(
                "Saved memory settings, but failed to update the current thread: {err}"
            ));
        }
    }

    pub(super) async fn reset_memories_with_app_server(
        &mut self,
        app_server: &mut AppServerSession,
    ) {
        if let Err(err) = app_server.memory_reset().await {
            tracing::error!(error = %err, "failed to reset memories");
            self.chat_widget
                .add_error_message(format!("Failed to reset memories: {err}"));
            return;
        }

        self.chat_widget
            .add_info_message("Reset local memories.".to_string(), /*hint*/ None);
    }

    pub(super) fn reasoning_label(reasoning_effort: Option<&ReasoningEffortConfig>) -> String {
        match reasoning_effort {
            None | Some(ReasoningEffortConfig::None) => "default".to_string(),
            Some(reasoning_effort) => reasoning_effort.as_str().to_string(),
        }
    }

    pub(super) fn reasoning_label_for(
        model: &str,
        reasoning_effort: Option<&ReasoningEffortConfig>,
    ) -> Option<String> {
        (!model.starts_with("codex-auto-")).then(|| Self::reasoning_label(reasoning_effort))
    }

    pub(crate) fn token_usage(&self) -> crate::token_usage::TokenUsage {
        self.chat_widget.token_usage()
    }

    pub(super) fn on_update_reasoning_effort(&mut self, effort: Option<ReasoningEffortConfig>) {
        // TODO(aibrahim): Remove this and don't use config as a state object.
        // Instead, explicitly pass the stored collaboration mode's effort into new sessions.
        self.config.model_reasoning_effort = effort.clone();
        self.chat_widget.set_reasoning_effort(effort);
    }

    pub(super) fn on_update_personality(&mut self, personality: Personality) {
        self.config.personality = Some(personality);
        self.chat_widget.set_personality(personality);
    }

    pub(super) fn sync_tui_theme_selection(&mut self, name: String) {
        self.config.tui_theme = Some(name.clone());
        self.chat_widget.set_tui_theme(Some(name));
    }

    #[cfg(test)]
    pub(super) fn sync_tui_pet_selection(&mut self, pet: String) {
        self.config.tui_pet = Some(pet.clone());
        self.chat_widget.set_tui_pet(Some(pet));
    }

    pub(super) fn sync_tui_pet_disabled(&mut self) {
        let pet = crate::pets::DISABLED_PET_ID.to_string();
        self.config.tui_pet = Some(pet.clone());
        self.chat_widget.set_tui_pet(Some(pet));
    }

    pub(super) fn restore_runtime_theme_from_config(&self) {
        if let Some(name) = self.config.tui_theme.as_deref()
            && let Some(theme) =
                crate::render::highlight::resolve_theme_by_name(name, Some(&self.config.codex_home))
        {
            crate::render::highlight::set_syntax_theme(theme);
            return;
        }

        let auto_theme_name = crate::render::highlight::adaptive_default_theme_name();
        if let Some(theme) = crate::render::highlight::resolve_theme_by_name(
            auto_theme_name,
            Some(&self.config.codex_home),
        ) {
            crate::render::highlight::set_syntax_theme(theme);
        }
    }

    pub(super) fn personality_label(personality: Personality) -> &'static str {
        match personality {
            Personality::None => "None",
            Personality::Friendly => "Friendly",
            Personality::Pragmatic => "Pragmatic",
        }
    }

    fn sync_feature_state_from_effective_config(
        &mut self,
        effective_config: &ConfigReadResponse,
        feature_updates: &[(Feature, bool)],
    ) {
        for (feature, _) in feature_updates {
            let enabled = feature_enabled_from_effective_config(effective_config, *feature);
            if let Err(err) = self.config.features.set_enabled(*feature, enabled) {
                tracing::warn!(
                    error = %err,
                    feature = feature.key(),
                    "failed to sync effective feature state after an overridden write"
                );
                continue;
            }
            self.chat_widget.set_feature_enabled(*feature, enabled);
        }

        if feature_updates
            .iter()
            .any(|(feature, _)| *feature == Feature::GuardianApproval)
            && !self.config.features.enabled(Feature::GuardianApproval)
        {
            self.set_approvals_reviewer_in_app_and_widget(ApprovalsReviewer::User);
            return;
        }

        if let Some(reviewer) = approvals_reviewer_from_effective_config(effective_config) {
            self.set_approvals_reviewer_in_app_and_widget(reviewer);
        }
        if let Some(policy) = approval_policy_from_effective_config(effective_config) {
            if let Err(err) = self
                .config
                .permissions
                .approval_policy
                .set(policy.to_core())
            {
                tracing::warn!(
                    error = %err,
                    "failed to sync effective approval policy after an overridden write"
                );
                self.chat_widget.add_error_message(format!(
                    "Failed to refresh overridden Approve for me settings: {err}"
                ));
            } else {
                self.chat_widget.set_approval_policy(policy);
            }
        }
    }

    async fn sync_auto_review_runtime_state_from_effective_config(
        &mut self,
        effective_config: &ConfigReadResponse,
        feature_updates: &[(Feature, bool)],
    ) {
        if !feature_updates
            .iter()
            .any(|(feature, _)| *feature == Feature::GuardianApproval)
            || !self.config.features.enabled(Feature::GuardianApproval)
            || sandbox_mode_from_effective_config(effective_config)
                != Some(AppServerSandboxMode::WorkspaceWrite)
        {
            return;
        }

        let auto_review_preset = auto_review_mode();
        let mut config = self.config.clone();
        let Some(permission_profile) = self.try_set_builtin_active_permission_profile_on_config(
            &mut config,
            auto_review_preset.active_permission_profile.clone(),
            "Failed to refresh overridden Approve for me settings",
            "failed to sync overridden Auto-review permission profile",
        ) else {
            return;
        };
        self.config = config;
        if let Err(err) = self
            .chat_widget
            .set_permission_profile_from_session_snapshot(PermissionProfileSnapshot::active(
                permission_profile.clone(),
                auto_review_preset.active_permission_profile.clone(),
            ))
        {
            tracing::warn!(
                error = %err,
                "failed to sync overridden Auto-review permission profile on chat config"
            );
            self.chat_widget.add_error_message(format!(
                "Failed to refresh overridden Approve for me settings: {err}"
            ));
            return;
        }

        self.runtime_permission_profile_override =
            Some(RuntimePermissionProfileOverride::from_config(&self.config));
        self.sync_active_thread_permission_settings_to_cached_session()
            .await;

        let approval_policy = AskForApproval::from(self.config.permissions.approval_policy.value());
        let op = AppCommand::override_turn_context(
            /*cwd*/ None,
            Some(approval_policy),
            Some(self.config.approvals_reviewer),
            /*permission_profile*/ None,
            Some(auto_review_preset.active_permission_profile),
            /*windows_sandbox_level*/ None,
            /*model*/ None,
            /*effort*/ None,
            /*summary*/ None,
            /*service_tier*/ None,
            /*collaboration_mode*/ None,
            /*personality*/ None,
        );
        let replay_state_op =
            ThreadEventStore::op_can_change_pending_replay_state(&op).then(|| op.clone());
        let submitted = self.chat_widget.submit_op(op);
        if submitted && let Some(op) = replay_state_op.as_ref() {
            self.note_active_thread_outbound_op(op).await;
            self.refresh_pending_thread_approvals().await;
        }
    }

    fn sync_memory_state_from_effective_config(
        &mut self,
        effective_config: &ConfigReadResponse,
    ) -> bool {
        let Some(memories) = memories_from_effective_config(effective_config) else {
            tracing::warn!(
                "config/read omitted memories after an overridden memory settings write"
            );
            return false;
        };
        let use_memories = memories
            .use_memories
            .unwrap_or(self.config.memories.use_memories);
        let generate_memories = memories
            .generate_memories
            .unwrap_or(self.config.memories.generate_memories);
        self.config.memories.use_memories = use_memories;
        self.config.memories.generate_memories = generate_memories;
        self.chat_widget
            .set_memory_settings(use_memories, generate_memories);
        true
    }

    #[cfg(target_os = "windows")]
    pub(super) async fn sync_windows_sandbox_after_overridden_write(
        &mut self,
        app_server: &mut AppServerSession,
        write_response: &ConfigWriteResponse,
    ) {
        let message = overridden_write_message(write_response);
        tracing::warn!(
            message,
            "Windows sandbox config write was overridden by effective config"
        );
        self.chat_widget.add_error_message(format!(
            "Windows sandbox changes were saved but not applied: {message}"
        ));
        let Some(effective_config) = self
            .read_effective_config_after_overridden_write(app_server, "Windows sandbox changes")
            .await
        else {
            return;
        };
        let Some(mode) = windows_sandbox_mode_from_effective_config(&effective_config) else {
            return;
        };
        self.config.permissions.windows_sandbox_mode = Some(mode);
        self.chat_widget.set_windows_sandbox_mode(Some(mode));
        self.propagate_windows_sandbox_turn_context();
    }

    fn propagate_windows_sandbox_turn_context(&self) {
        #[cfg(target_os = "windows")]
        {
            let windows_sandbox_level = crate::windows_sandbox::level_from_config(&self.config);
            self.app_event_tx
                .send(AppEvent::CodexOp(AppCommand::override_turn_context(
                    /*cwd*/ None,
                    /*approval_policy*/ None,
                    /*approvals_reviewer*/ None,
                    /*permission_profile*/ None,
                    /*active_permission_profile*/ None,
                    Some(windows_sandbox_level),
                    /*model*/ None,
                    /*effort*/ None,
                    /*summary*/ None,
                    /*service_tier*/ None,
                    /*collaboration_mode*/ None,
                    /*personality*/ None,
                )));
        }
    }
}

fn overridden_write_message(write_response: &ConfigWriteResponse) -> &str {
    write_response
        .overridden_metadata
        .as_ref()
        .map(|metadata| metadata.message.as_str())
        .unwrap_or("the effective config is overridden by a higher-priority layer")
}

fn feature_enabled_from_effective_config(
    effective_config: &ConfigReadResponse,
    feature: Feature,
) -> bool {
    let root_features = effective_config
        .config
        .additional
        .get("features")
        .and_then(features_toml_from_json);
    root_features
        .as_ref()
        .and_then(|features| features.entries().get(feature.key()).copied())
        .unwrap_or_else(|| feature.default_enabled())
}

fn approvals_reviewer_from_effective_config(
    effective_config: &ConfigReadResponse,
) -> Option<ApprovalsReviewer> {
    effective_config
        .config
        .approvals_reviewer
        .map(codex_app_server_protocol::ApprovalsReviewer::to_core)
}

fn approval_policy_from_effective_config(
    effective_config: &ConfigReadResponse,
) -> Option<AskForApproval> {
    effective_config.config.approval_policy
}

fn sandbox_mode_from_effective_config(
    effective_config: &ConfigReadResponse,
) -> Option<AppServerSandboxMode> {
    effective_config.config.sandbox_mode
}

fn memories_from_effective_config(effective_config: &ConfigReadResponse) -> Option<MemoriesToml> {
    effective_config
        .config
        .additional
        .get("memories")
        .and_then(|memories| serde_json::from_value(memories.clone()).ok())
}

fn features_toml_from_json(value: &serde_json::Value) -> Option<FeaturesToml> {
    serde_json::from_value(value.clone()).ok()
}

#[cfg(target_os = "windows")]
fn windows_sandbox_mode_from_effective_config(
    effective_config: &ConfigReadResponse,
) -> Option<codex_config::types::WindowsSandboxModeToml> {
    let root_windows = effective_config
        .config
        .additional
        .get("windows")
        .and_then(windows_toml_from_json);
    root_windows.and_then(|windows| windows.sandbox)
}

#[cfg(target_os = "windows")]
fn windows_toml_from_json(value: &serde_json::Value) -> Option<WindowsToml> {
    serde_json::from_value(value.clone()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::test_support::app_enabled_in_effective_config;
    use crate::app::test_support::make_test_app;
    use crate::legacy_core::config::edit::ConfigEdit;
    use crate::test_support::PathBufExt;
    use codex_protocol::models::PermissionProfile;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    #[tokio::test]
    async fn update_reasoning_effort_updates_collaboration_mode() {
        let mut app = make_test_app().await;
        app.chat_widget
            .set_reasoning_effort(Some(ReasoningEffortConfig::Medium));

        app.on_update_reasoning_effort(Some(ReasoningEffortConfig::High));

        assert_eq!(
            app.chat_widget.current_reasoning_effort(),
            Some(ReasoningEffortConfig::High)
        );
        assert_eq!(
            app.config.model_reasoning_effort,
            Some(ReasoningEffortConfig::High)
        );
    }

    #[tokio::test]
    async fn refresh_in_memory_config_from_disk_loads_latest_apps_state() -> Result<()> {
        let mut app = make_test_app().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        let app_id = "unit_test_refresh_in_memory_config_connector".to_string();

        assert_eq!(app_enabled_in_effective_config(&app.config, &app_id), None);

        ConfigEditsBuilder::for_config(&app.config)
            .with_edits([
                ConfigEdit::SetPath {
                    segments: vec!["apps".to_string(), app_id.clone(), "enabled".to_string()],
                    value: false.into(),
                },
                ConfigEdit::SetPath {
                    segments: vec![
                        "apps".to_string(),
                        app_id.clone(),
                        "disabled_reason".to_string(),
                    ],
                    value: "user".into(),
                },
            ])
            .apply()
            .await
            .expect("persist app toggle");

        assert_eq!(app_enabled_in_effective_config(&app.config, &app_id), None);

        app.refresh_in_memory_config_from_disk().await?;

        assert_eq!(
            app_enabled_in_effective_config(&app.config, &app_id),
            Some(false)
        );
        Ok(())
    }

    // Regression coverage for `/new` and `/clear`: cloud requirements
    // must survive the config refresh that runs before thread transitions.
    #[tokio::test]
    async fn refresh_in_memory_config_from_disk_keeps_cloud_requirements_for_thread_transitions()
    -> Result<()> {
        let mut app = make_test_app().await;
        let codex_home = tempdir()?;
        let required_policy = codex_protocol::protocol::AskForApproval::Never;
        let cloud_config_bundle =
            codex_config::test_support::CloudConfigBundleFixture::loader_with_enterprise_requirement(
                r#"allowed_approval_policies = ["never"]"#,
            );

        let config = ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
            .cloud_config_bundle(cloud_config_bundle.clone())
            .build()
            .await?;
        app.config = config;
        app.cloud_config_bundle = cloud_config_bundle;
        let app_id = "unit_test_cloud_requirements_reload_marker";
        std::fs::write(
            codex_home.path().join("config.toml"),
            format!(
                r#"
[apps.{app_id}]
enabled = false
"#
            ),
        )?;

        let assert_cloud_requirements = |app: &App| {
            let config = app.fresh_session_config();
            assert_eq!(
                config
                    .config_layer_stack
                    .requirements_toml()
                    .allowed_approval_policies
                    .clone(),
                Some(vec![required_policy])
            );
            assert_eq!(config.permissions.approval_policy.value(), required_policy);
        };

        assert_cloud_requirements(&app);
        assert_eq!(app_enabled_in_effective_config(&app.config, app_id), None);

        // This is the fallible reload that the best-effort `/new`, `/clear`,
        // `/fork`, side-conversation, and session-picker paths wrap.
        app.refresh_in_memory_config_from_disk().await?;

        assert_eq!(
            app_enabled_in_effective_config(&app.config, app_id),
            Some(false)
        );
        assert_cloud_requirements(&app);
        Ok(())
    }

    #[tokio::test]
    async fn refresh_in_memory_config_from_disk_best_effort_keeps_current_config_on_error()
    -> Result<()> {
        let mut app = make_test_app().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        std::fs::write(codex_home.path().join("config.toml"), "[broken")?;
        let original_config = app.config.clone();

        app.refresh_in_memory_config_from_disk_best_effort("starting a new thread")
            .await;

        assert_eq!(app.config, original_config);
        Ok(())
    }

    #[tokio::test]
    async fn refresh_in_memory_config_from_disk_uses_active_chat_widget_cwd() -> Result<()> {
        let mut app = make_test_app().await;
        let original_cwd = app.config.cwd.clone();
        let next_cwd_tmp = tempdir()?;
        let next_cwd = next_cwd_tmp.path().to_path_buf();

        app.chat_widget
            .handle_thread_session(crate::session_state::ThreadSessionState {
                thread_id: ThreadId::new(),
                forked_from_id: None,
                fork_parent_title: None,
                thread_name: None,
                model: "gpt-test".to_string(),
                model_provider_id: "test-provider".to_string(),
                service_tier: None,
                approval_policy: AskForApproval::Never,
                approvals_reviewer: ApprovalsReviewer::User,
                permission_profile: PermissionProfile::read_only(),
                active_permission_profile: None,
                cwd: next_cwd.clone().abs(),
                runtime_workspace_roots: Vec::new(),
                instruction_source_paths: Vec::new(),
                reasoning_effort: None,
                collaboration_mode: None,
                personality: None,
                message_history: None,
                network_proxy: None,
                rollout_path: Some(PathBuf::new()),
            });

        assert_eq!(app.chat_widget.config_ref().cwd.to_path_buf(), next_cwd);
        assert_eq!(app.config.cwd, original_cwd);

        app.refresh_in_memory_config_from_disk().await?;

        assert_eq!(app.config.cwd, app.chat_widget.config_ref().cwd);
        Ok(())
    }

    #[tokio::test]
    async fn refresh_in_memory_config_from_disk_updates_resize_reflow_config() -> Result<()> {
        let mut app = make_test_app().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        std::fs::write(
            codex_home.path().join("config.toml"),
            r#"
[tui]
terminal_resize_reflow_max_rows = 9000
"#,
        )?;

        app.refresh_in_memory_config_from_disk().await?;

        assert_eq!(
            app.config.terminal_resize_reflow.max_rows,
            crate::legacy_core::config::TerminalResizeReflowMaxRows::Limit(9000)
        );
        Ok(())
    }

    #[tokio::test]
    async fn overridden_disabled_guardian_does_not_apply_auto_review_companions() -> Result<()> {
        let mut app = make_test_app().await;
        let original_policy = app.config.permissions.approval_policy.value();
        let effective_config: ConfigReadResponse = serde_json::from_value(serde_json::json!({
            "config": {
                "approval_policy": AskForApproval::OnRequest,
                "approvals_reviewer": codex_app_server_protocol::ApprovalsReviewer::AutoReview,
                "sandbox_mode": AppServerSandboxMode::WorkspaceWrite,
                "features": {
                    "guardian_approval": false,
                },
            },
            "origins": {},
        }))?;

        app.sync_feature_state_from_effective_config(
            &effective_config,
            &[(Feature::GuardianApproval, /*enabled*/ true)],
        );

        assert!(!app.config.features.enabled(Feature::GuardianApproval));
        assert!(
            !app.chat_widget
                .config_ref()
                .features
                .enabled(Feature::GuardianApproval)
        );
        assert_eq!(app.config.approvals_reviewer, ApprovalsReviewer::User);
        assert_eq!(
            app.chat_widget.config_ref().approvals_reviewer,
            ApprovalsReviewer::User
        );
        assert_eq!(
            app.config.permissions.approval_policy.value(),
            original_policy
        );
        Ok(())
    }

    #[tokio::test]
    async fn rebuild_config_for_resume_or_fallback_uses_current_config_on_same_cwd_error()
    -> Result<()> {
        let mut app = make_test_app().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        std::fs::write(codex_home.path().join("config.toml"), "[broken")?;
        let current_config = app.config.clone();
        let current_cwd = current_config.cwd.clone();

        let resume_config = app
            .rebuild_config_for_resume_or_fallback(&current_cwd, current_cwd.to_path_buf())
            .await?;

        assert_eq!(resume_config, current_config);
        Ok(())
    }

    #[tokio::test]
    async fn rebuild_config_for_resume_or_fallback_errors_when_cwd_changes() -> Result<()> {
        let mut app = make_test_app().await;
        let codex_home = tempdir()?;
        app.config.codex_home = codex_home.path().to_path_buf().abs();
        std::fs::write(codex_home.path().join("config.toml"), "[broken")?;
        let current_cwd = app.config.cwd.clone();
        let next_cwd_tmp = tempdir()?;
        let next_cwd = next_cwd_tmp.path().to_path_buf();

        let result = app
            .rebuild_config_for_resume_or_fallback(&current_cwd, next_cwd)
            .await;

        assert!(result.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn sync_tui_theme_selection_updates_chat_widget_config_copy() {
        let mut app = make_test_app().await;

        app.sync_tui_theme_selection("dracula".to_string());

        assert_eq!(app.config.tui_theme.as_deref(), Some("dracula"));
        assert_eq!(
            app.chat_widget.config_ref().tui_theme.as_deref(),
            Some("dracula")
        );
    }

    #[tokio::test]
    async fn sync_tui_pet_selection_updates_chat_widget_config_copy() {
        let mut app = make_test_app().await;

        app.sync_tui_pet_selection("chefito".to_string());

        assert_eq!(app.config.tui_pet.as_deref(), Some("chefito"));
        assert_eq!(
            app.chat_widget.config_ref().tui_pet.as_deref(),
            Some("chefito")
        );
    }

    #[tokio::test]
    async fn sync_tui_pet_disabled_updates_chat_widget_config_copy() {
        let mut app = make_test_app().await;

        app.sync_tui_pet_disabled();

        assert_eq!(
            app.config.tui_pet.as_deref(),
            Some(crate::pets::DISABLED_PET_ID)
        );
        assert_eq!(
            app.chat_widget.config_ref().tui_pet.as_deref(),
            Some(crate::pets::DISABLED_PET_ID)
        );
    }
}
