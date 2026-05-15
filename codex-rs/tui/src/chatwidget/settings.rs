//! Runtime settings state and model/collaboration coordination for `ChatWidget`.

use super::*;

impl ChatWidget {
    /// Set the approval policy in the widget's config copy.
    pub(crate) fn set_approval_policy(&mut self, policy: AskForApproval) {
        if let Err(err) = self
            .config
            .permissions
            .approval_policy
            .set(policy.to_core())
        {
            tracing::warn!(%err, "failed to set approval_policy on chat config");
        } else {
            self.refresh_status_surfaces();
        }
    }

    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub(crate) fn set_permission_profile_from_session_snapshot(
        &mut self,
        snapshot: PermissionProfileSnapshot,
    ) -> ConstraintResult<()> {
        self.config
            .permissions
            .set_permission_profile_from_session_snapshot(snapshot)?;
        self.refresh_status_surfaces();
        Ok(())
    }

    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub(crate) fn set_windows_sandbox_mode(&mut self, mode: Option<WindowsSandboxModeToml>) {
        self.config.permissions.windows_sandbox_mode = mode;
        #[cfg(target_os = "windows")]
        self.bottom_pane.set_windows_degraded_sandbox_active(
            crate::legacy_core::windows_sandbox::ELEVATED_SANDBOX_NUX_ENABLED
                && matches!(
                    WindowsSandboxLevel::from_config(&self.config),
                    WindowsSandboxLevel::RestrictedToken
                ),
        );
    }

    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub(crate) fn set_feature_enabled(&mut self, feature: Feature, enabled: bool) -> bool {
        if let Err(err) = self.config.features.set_enabled(feature, enabled) {
            tracing::warn!(
                error = %err,
                feature = feature.key(),
                "failed to update constrained chat widget feature state"
            );
        }
        let enabled = self.config.features.enabled(feature);
        if feature == Feature::RealtimeConversation {
            let realtime_conversation_enabled = self.realtime_conversation_enabled();
            self.bottom_pane
                .set_realtime_conversation_enabled(realtime_conversation_enabled);
            self.bottom_pane
                .set_audio_device_selection_enabled(self.realtime_audio_device_selection_enabled());
            if !realtime_conversation_enabled && self.realtime_conversation.is_live() {
                self.request_realtime_conversation_close(Some(
                    "Realtime voice mode was closed because the feature was disabled.".to_string(),
                ));
            }
        }
        if feature == Feature::FastMode {
            self.sync_service_tier_commands();
        }
        if feature == Feature::Personality {
            self.sync_personality_command_enabled();
        }
        if feature == Feature::Plugins {
            self.sync_plugins_command_enabled();
            self.refresh_plugin_mentions();
        }
        if feature == Feature::Goals {
            self.sync_goal_command_enabled();
            if !enabled {
                self.current_goal_status_indicator = None;
                self.current_goal_status = None;
                self.turn_lifecycle.goal_status_active_turn_started_at = None;
                self.turn_lifecycle.budget_limited_turn_ids.clear();
                self.update_collaboration_mode_indicator();
            }
        }
        if feature == Feature::MentionsV2 {
            self.sync_mentions_v2_enabled();
        }
        if feature == Feature::PreventIdleSleep {
            self.turn_lifecycle.set_prevent_idle_sleep(enabled);
        }
        #[cfg(target_os = "windows")]
        if matches!(
            feature,
            Feature::WindowsSandbox | Feature::WindowsSandboxElevated
        ) {
            self.bottom_pane.set_windows_degraded_sandbox_active(
                crate::legacy_core::windows_sandbox::ELEVATED_SANDBOX_NUX_ENABLED
                    && matches!(
                        WindowsSandboxLevel::from_config(&self.config),
                        WindowsSandboxLevel::RestrictedToken
                    ),
            );
        }
        enabled
    }

    pub(crate) fn set_approvals_reviewer(&mut self, policy: ApprovalsReviewer) {
        self.config.approvals_reviewer = policy;
        self.refresh_status_surfaces();
    }

    pub(crate) fn set_full_access_warning_acknowledged(&mut self, acknowledged: bool) {
        self.config.notices.hide_full_access_warning = Some(acknowledged);
    }

    pub(crate) fn set_world_writable_warning_acknowledged(&mut self, acknowledged: bool) {
        self.config.notices.hide_world_writable_warning = Some(acknowledged);
    }

    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub(crate) fn world_writable_warning_hidden(&self) -> bool {
        self.config
            .notices
            .hide_world_writable_warning
            .unwrap_or(false)
    }

    /// Override the reasoning effort used when Plan mode is active.
    ///
    /// When the active mask is already Plan, the override is applied immediately
    /// so the footer reflects it without waiting for the next mode switch.
    /// Passing `None` resets to the Plan-mode preset default.
    pub(crate) fn set_plan_mode_reasoning_effort(&mut self, effort: Option<ReasoningEffortConfig>) {
        self.config.plan_mode_reasoning_effort = effort;
        if self.collaboration_modes_enabled()
            && let Some(mask) = self.active_collaboration_mask.as_mut()
            && mask.mode == Some(ModeKind::Plan)
        {
            if let Some(effort) = effort {
                mask.reasoning_effort = Some(Some(effort));
            } else if let Some(plan_mask) =
                collaboration_modes::plan_mask(self.model_catalog.as_ref())
            {
                mask.reasoning_effort = plan_mask.reasoning_effort;
            }
        }
        self.refresh_model_dependent_surfaces();
    }

    /// Set the reasoning effort for the non-Plan collaboration mode.
    ///
    /// Does not touch the active Plan mask — Plan reasoning is controlled
    /// exclusively by the Plan preset and `set_plan_mode_reasoning_effort`.
    pub(crate) fn set_reasoning_effort(&mut self, effort: Option<ReasoningEffortConfig>) {
        self.current_collaboration_mode = self.current_collaboration_mode.with_updates(
            /*model*/ None,
            Some(effort),
            /*developer_instructions*/ None,
        );
        if self.collaboration_modes_enabled()
            && let Some(mask) = self.active_collaboration_mask.as_mut()
            && mask.mode != Some(ModeKind::Plan)
        {
            // Generic "global default" updates should not mutate the active Plan mask.
            // Plan reasoning is controlled by the Plan preset and Plan-only override updates.
            mask.reasoning_effort = Some(effort);
        }
        self.refresh_model_dependent_surfaces();
    }

    /// Set the personality in the widget's config copy.
    pub(crate) fn set_personality(&mut self, personality: Personality) {
        self.config.personality = Some(personality);
    }

    pub(crate) fn status_account_display(&self) -> Option<&StatusAccountDisplay> {
        self.status_account_display.as_ref()
    }

    pub(crate) fn runtime_model_provider_base_url(&self) -> Option<&str> {
        self.runtime_model_provider_base_url.as_deref()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn model_catalog(&self) -> Arc<ModelCatalog> {
        self.model_catalog.clone()
    }

    pub(crate) fn current_plan_type(&self) -> Option<PlanType> {
        self.plan_type
    }

    pub(crate) fn has_chatgpt_account(&self) -> bool {
        self.has_chatgpt_account
    }

    pub(crate) fn update_account_state(
        &mut self,
        status_account_display: Option<StatusAccountDisplay>,
        plan_type: Option<PlanType>,
        has_chatgpt_account: bool,
    ) {
        self.status_account_display = status_account_display;
        self.plan_type = plan_type;
        self.has_chatgpt_account = has_chatgpt_account;
        self.bottom_pane
            .set_connectors_enabled(self.connectors_enabled());
    }

    pub(crate) fn set_realtime_audio_device(
        &mut self,
        kind: RealtimeAudioDeviceKind,
        name: Option<String>,
    ) {
        match kind {
            RealtimeAudioDeviceKind::Microphone => self.config.realtime_audio.microphone = name,
            RealtimeAudioDeviceKind::Speaker => self.config.realtime_audio.speaker = name,
        }
    }

    /// Set the syntax theme override in the widget's config copy.
    pub(crate) fn set_tui_theme(&mut self, theme: Option<String>) {
        self.config.tui_theme = theme;
    }

    /// Set the model in the widget's config copy and stored collaboration mode.
    pub(crate) fn set_model(&mut self, model: &str) {
        self.current_collaboration_mode = self.current_collaboration_mode.with_updates(
            Some(model.to_string()),
            /*effort*/ None,
            /*developer_instructions*/ None,
        );
        if self.collaboration_modes_enabled()
            && let Some(mask) = self.active_collaboration_mask.as_mut()
        {
            mask.model = Some(model.to_string());
        }
        self.refresh_model_dependent_surfaces();
    }

    pub(crate) fn current_model(&self) -> &str {
        if !self.collaboration_modes_enabled() {
            return self.current_collaboration_mode.model();
        }
        self.active_collaboration_mask
            .as_ref()
            .and_then(|mask| mask.model.as_deref())
            .unwrap_or_else(|| self.current_collaboration_mode.model())
    }

    pub(crate) fn realtime_conversation_is_live(&self) -> bool {
        self.realtime_conversation.is_live()
    }

    pub(super) fn current_realtime_audio_device_name(
        &self,
        kind: RealtimeAudioDeviceKind,
    ) -> Option<String> {
        match kind {
            RealtimeAudioDeviceKind::Microphone => self.config.realtime_audio.microphone.clone(),
            RealtimeAudioDeviceKind::Speaker => self.config.realtime_audio.speaker.clone(),
        }
    }

    pub(super) fn current_realtime_audio_selection_label(
        &self,
        kind: RealtimeAudioDeviceKind,
    ) -> String {
        self.current_realtime_audio_device_name(kind)
            .unwrap_or_else(|| "System default".to_string())
    }

    pub(super) fn sync_personality_command_enabled(&mut self) {
        self.bottom_pane
            .set_personality_command_enabled(self.config.features.enabled(Feature::Personality));
    }

    pub(super) fn sync_plugins_command_enabled(&mut self) {
        self.bottom_pane
            .set_plugins_command_enabled(self.config.features.enabled(Feature::Plugins));
    }

    pub(super) fn sync_goal_command_enabled(&mut self) {
        self.bottom_pane
            .set_goal_command_enabled(self.config.features.enabled(Feature::Goals));
    }

    pub(super) fn sync_mentions_v2_enabled(&mut self) {
        self.bottom_pane
            .set_mentions_v2_enabled(self.config.features.enabled(Feature::MentionsV2));
    }

    pub(super) fn current_model_supports_personality(&self) -> bool {
        let model = self.current_model();
        self.model_catalog
            .try_list_models()
            .ok()
            .and_then(|models| {
                models
                    .into_iter()
                    .find(|preset| preset.model == model)
                    .map(|preset| preset.supports_personality)
            })
            .unwrap_or(false)
    }

    /// Return whether the effective model currently advertises image-input support.
    ///
    /// We intentionally default to `true` when model metadata cannot be read so transient catalog
    /// failures do not hard-block user input in the UI.
    pub(super) fn current_model_supports_images(&self) -> bool {
        let model = self.current_model();
        self.model_catalog
            .try_list_models()
            .ok()
            .and_then(|models| {
                models
                    .into_iter()
                    .find(|preset| preset.model == model)
                    .map(|preset| preset.input_modalities.contains(&InputModality::Image))
            })
            .unwrap_or(true)
    }

    pub(super) fn sync_image_paste_enabled(&mut self) {
        let enabled = self.current_model_supports_images();
        self.bottom_pane.set_image_paste_enabled(enabled);
    }

    pub(super) fn image_inputs_not_supported_message(&self) -> String {
        format!(
            "Model {} does not support image inputs. Remove images or switch models.",
            self.current_model()
        )
    }

    #[allow(dead_code)] // Used in tests
    pub(crate) fn current_collaboration_mode(&self) -> &CollaborationMode {
        &self.current_collaboration_mode
    }

    pub(crate) fn current_reasoning_effort(&self) -> Option<ReasoningEffortConfig> {
        self.effective_reasoning_effort()
    }

    #[cfg(test)]
    pub(crate) fn active_collaboration_mode_kind(&self) -> ModeKind {
        self.active_mode_kind()
    }

    pub(super) fn is_session_configured(&self) -> bool {
        self.thread_id.is_some()
    }

    pub(super) fn collaboration_modes_enabled(&self) -> bool {
        true
    }

    /// Returns the dismissal scope that applies to the currently visible draft.
    fn plan_mode_nudge_scope(&self) -> PlanModeNudgeScope {
        self.thread_id
            .map_or(PlanModeNudgeScope::NewThread, PlanModeNudgeScope::Thread)
    }

    /// Returns whether the current draft should replace the normal footer with the Plan-mode nudge.
    ///
    /// `ChatWidget` owns this policy because it can combine lexical draft matching with mode
    /// availability, interaction state, and thread-scoped dismissal. `ChatComposer` only renders
    /// the resulting visibility bit. Keeping slash and shell drafts out here avoids advertising a
    /// mode switch while the user is intentionally composing another local command.
    pub(super) fn should_show_plan_mode_nudge(&self) -> bool {
        let text = self.bottom_pane.composer_text();
        let trimmed = text.trim_start();
        self.collaboration_modes_enabled()
            && collaboration_modes::plan_mask(self.model_catalog.as_ref()).is_some()
            && self.active_mode_kind() != ModeKind::Plan
            && self.bottom_pane.composer_input_enabled()
            && !self.bottom_pane.is_task_running()
            && self.bottom_pane.no_modal_or_popup_active()
            && !trimmed.starts_with('/')
            && !trimmed.starts_with('!')
            && contains_plan_keyword(&text)
            && !self
                .dismissed_plan_mode_nudge_scopes
                .contains(&self.plan_mode_nudge_scope())
    }

    /// Synchronizes the footer presentation with the current Plan-mode nudge policy.
    pub(super) fn refresh_plan_mode_nudge(&mut self) {
        self.bottom_pane
            .set_plan_mode_nudge_visible(self.should_show_plan_mode_nudge());
    }

    /// Hides the nudge for the current thread scope until the user changes conversation context.
    pub(super) fn dismiss_plan_mode_nudge(&mut self) {
        self.dismissed_plan_mode_nudge_scopes
            .insert(self.plan_mode_nudge_scope());
        self.refresh_plan_mode_nudge();
    }

    pub(super) fn initial_collaboration_mask(
        _config: &Config,
        model_catalog: &ModelCatalog,
        model_override: Option<&str>,
    ) -> Option<CollaborationModeMask> {
        let mut mask = collaboration_modes::default_mask(model_catalog)?;
        if let Some(model_override) = model_override {
            mask.model = Some(model_override.to_string());
        }
        Some(mask)
    }

    pub(super) fn active_mode_kind(&self) -> ModeKind {
        self.active_collaboration_mask
            .as_ref()
            .and_then(|mask| mask.mode)
            .unwrap_or(ModeKind::Default)
    }

    pub(super) fn effective_reasoning_effort(&self) -> Option<ReasoningEffortConfig> {
        if !self.collaboration_modes_enabled() {
            return self.current_collaboration_mode.reasoning_effort();
        }
        let current_effort = self.current_collaboration_mode.reasoning_effort();
        self.active_collaboration_mask
            .as_ref()
            .and_then(|mask| mask.reasoning_effort)
            .unwrap_or(current_effort)
    }

    pub(super) fn effective_collaboration_mode(&self) -> CollaborationMode {
        if !self.collaboration_modes_enabled() {
            return self.current_collaboration_mode.clone();
        }
        self.active_collaboration_mask.as_ref().map_or_else(
            || self.current_collaboration_mode.clone(),
            |mask| self.current_collaboration_mode.apply_mask(mask),
        )
    }

    pub(super) fn refresh_model_display(&mut self) {
        let effective = self.effective_collaboration_mode();
        self.session_header.set_model(effective.model());
        // Keep composer paste affordances aligned with the currently effective model.
        self.sync_image_paste_enabled();
        self.sync_service_tier_commands();
        self.refresh_terminal_title();
    }

    /// Refresh every UI surface that depends on the effective model, reasoning
    /// effort, or collaboration mode.
    ///
    /// Call this at the end of any setter that mutates `current_collaboration_mode`,
    /// `active_collaboration_mask`, or per-mode reasoning-effort overrides.
    /// Consolidating both refreshes here prevents the bug where callers update the
    /// header/title (`refresh_model_display`) but forget the footer status line
    /// (`refresh_status_line`).
    pub(super) fn refresh_model_dependent_surfaces(&mut self) {
        self.refresh_model_display();
        self.refresh_status_line();
    }

    pub(super) fn model_display_name(&self) -> &str {
        let model = self.current_model();
        if model.is_empty() {
            DEFAULT_MODEL_DISPLAY_NAME
        } else {
            model
        }
    }

    /// Get the label for the current collaboration mode.
    pub(super) fn collaboration_mode_label(&self) -> Option<&'static str> {
        if !self.collaboration_modes_enabled() {
            return None;
        }
        let active_mode = self.active_mode_kind();
        active_mode
            .is_tui_visible()
            .then_some(active_mode.display_name())
    }

    fn collaboration_mode_indicator(&self) -> Option<CollaborationModeIndicator> {
        if !self.collaboration_modes_enabled() {
            return None;
        }
        match self.active_mode_kind() {
            ModeKind::Plan => Some(CollaborationModeIndicator::Plan),
            ModeKind::Default | ModeKind::PairProgramming | ModeKind::Execute => None,
        }
    }

    pub(super) fn update_collaboration_mode_indicator(&mut self) {
        let indicator = self.collaboration_mode_indicator();
        let goal_indicator = if indicator.is_none() {
            self.goal_status_indicator(Instant::now())
        } else {
            None
        };
        self.current_goal_status_indicator = goal_indicator.clone();
        self.bottom_pane.set_collaboration_mode_indicator(indicator);
        self.bottom_pane.set_goal_status_indicator(goal_indicator);
    }

    pub(super) fn refresh_goal_status_indicator_for_time_tick(&mut self) {
        if self.collaboration_mode_indicator().is_some() {
            return;
        }
        let goal_indicator = self.goal_status_indicator(Instant::now());
        if goal_indicator != self.current_goal_status_indicator {
            self.current_goal_status_indicator = goal_indicator.clone();
            self.bottom_pane.set_goal_status_indicator(goal_indicator);
        }
    }

    fn goal_status_indicator(&self, now: Instant) -> Option<GoalStatusIndicator> {
        if !self.config.features.enabled(Feature::Goals) {
            return None;
        }
        self.current_goal_status.as_ref().and_then(|state| {
            state.indicator(now, self.turn_lifecycle.goal_status_active_turn_started_at)
        })
    }

    pub(super) fn on_thread_goal_updated(&mut self, goal: AppThreadGoal, turn_id: Option<String>) {
        if let Some(active_thread_id) = self.thread_id
            && active_thread_id.to_string() != goal.thread_id
        {
            return;
        }
        if !self.config.features.enabled(Feature::Goals) {
            self.current_goal_status_indicator = None;
            self.current_goal_status = None;
            self.update_collaboration_mode_indicator();
            return;
        }
        if goal.status == AppThreadGoalStatus::BudgetLimited
            && let Some(turn_id) = turn_id
        {
            self.turn_lifecycle.mark_budget_limited(turn_id);
        }
        self.current_goal_status = Some(GoalStatusState::new(goal, Instant::now()));
        self.update_collaboration_mode_indicator();
    }

    /// Cycle to the next collaboration mode variant (Plan -> Default -> Plan).
    pub(super) fn cycle_collaboration_mode(&mut self) {
        if !self.collaboration_modes_enabled() {
            return;
        }

        if let Some(next_mask) = collaboration_modes::next_mask(
            self.model_catalog.as_ref(),
            self.active_collaboration_mask.as_ref(),
        ) {
            self.set_collaboration_mask(next_mask);
        }
    }

    /// Update the active collaboration mask.
    ///
    /// When collaboration modes are enabled and a preset is selected,
    /// the current mode is attached to submissions as `Op::UserTurn { collaboration_mode: Some(...) }`.
    pub(crate) fn set_collaboration_mask(&mut self, mut mask: CollaborationModeMask) {
        if !self.collaboration_modes_enabled() {
            return;
        }
        let previous_mode = self.active_mode_kind();
        let previous_model = self.current_model().to_string();
        let previous_effort = self.effective_reasoning_effort();
        if mask.mode == Some(ModeKind::Plan)
            && let Some(effort) = self.config.plan_mode_reasoning_effort
        {
            mask.reasoning_effort = Some(Some(effort));
        }
        if mask.mode == Some(ModeKind::Plan) {
            self.dismissed_plan_mode_nudge_scopes
                .insert(self.plan_mode_nudge_scope());
        }
        self.active_collaboration_mask = Some(mask);
        self.update_collaboration_mode_indicator();
        self.refresh_plan_mode_nudge();
        self.refresh_model_dependent_surfaces();
        let next_mode = self.active_mode_kind();
        let next_model = self.current_model();
        let next_effort = self.effective_reasoning_effort();
        if previous_mode != next_mode
            && (previous_model != next_model || previous_effort != next_effort)
        {
            let mut message = format!("Model changed to {next_model}");
            if !next_model.starts_with("codex-auto-") {
                let reasoning_label = match next_effort {
                    Some(ReasoningEffortConfig::Minimal) => "minimal",
                    Some(ReasoningEffortConfig::Low) => "low",
                    Some(ReasoningEffortConfig::Medium) => "medium",
                    Some(ReasoningEffortConfig::High) => "high",
                    Some(ReasoningEffortConfig::XHigh) => "xhigh",
                    None | Some(ReasoningEffortConfig::None) => "default",
                };
                message.push(' ');
                message.push_str(reasoning_label);
            }
            message.push_str(" for ");
            message.push_str(next_mode.display_name());
            message.push_str(" mode.");
            self.add_info_message(message, /*hint*/ None);
        }
        self.request_redraw();
    }
}
