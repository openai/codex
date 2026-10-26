//! Apply complete session model choices after server acceptance, separately from saved defaults.
//!
//! A successful config write can still be overridden. Report that distinction without
//! replacing the active task's explicit selection with launch-time configuration.

use super::App;
use crate::app_server_session::AppServerSession;
use crate::app_server_session::ThreadSettingsOwner;
use crate::service_tier_resolution;
use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_protocol::ConfigEdit;
use codex_app_server_protocol::WriteStatus;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::SERVICE_TIER_DEFAULT_REQUEST_VALUE;
use codex_protocol::config_types::ServiceTier;
use codex_protocol::openai_models::ReasoningEffort;
use color_eyre::eyre::Result;

impl App {
    pub(super) async fn apply_default_model_selection(
        &mut self,
        app_server: &mut AppServerSession,
        model: String,
        effort: Option<ReasoningEffort>,
    ) {
        if self
            .active_thread_model_setting_update_params(model.clone())
            .is_some_and(|params| params.permissions.is_some())
            && self.reject_pending_permission_change()
        {
            return;
        }
        let model_changed = self.chat_widget.current_model() != model
            || self.chat_widget.current_collaboration_mode().model() != model;
        if model_changed {
            self.chat_widget.set_model(&model);
            self.sync_active_thread_model_setting(app_server, model.clone(), /*effort*/ None)
                .await;
            self.sync_active_thread_service_tier_to_cached_session()
                .await;
        }
        self.on_update_reasoning_effort(effort.clone());
        self.sync_active_thread_reasoning_setting(app_server, effort.clone())
            .await;
        self.persist_default_model_selection(app_server.request_handle(), model, effort)
            .await;
    }

    pub(super) async fn persist_default_model_selection(
        &mut self,
        request_handle: AppServerRequestHandle,
        model: String,
        effort: Option<ReasoningEffort>,
    ) {
        self.app_event_tx
            .send(crate::app_event::AppEvent::FollowTranscript);
        match self
            .persist_model_defaults(
                request_handle,
                crate::config_update::build_model_selection_edits(model.as_str(), effort.as_ref()),
                "default model and reasoning effort",
            )
            .await
        {
            Ok(()) => {
                let effort_label = effort
                    .as_ref()
                    .map(std::string::ToString::to_string)
                    .unwrap_or_else(|| "default".to_string());
                tracing::info!("Selected model: {model}, Selected effort: {effort_label}");
                let mut message = format!("Model changed to {model}");
                if let Some(label) = Self::reasoning_label_for(&model, effort.as_ref()) {
                    message.push(' ');
                    message.push_str(&label);
                }
                self.chat_widget.add_info_message(message, /*hint*/ None);
            }
            Err(err) => {
                let error = crate::config_update::format_config_error(&err);
                tracing::error!(error = %error, "failed to persist model selection");
                self.chat_widget
                    .add_error_message(format!("Failed to save default model: {error}"));
            }
        }
    }

    pub(super) async fn select_session_model(
        &mut self,
        app_server: &mut AppServerSession,
        model: String,
        effort: Option<ReasoningEffort>,
        service_tier: Option<String>,
    ) {
        // Changing effort or speed for the effective model must preserve permissions.
        let model_changed = self.chat_widget.current_model() != model;
        let params = if model_changed {
            self.active_thread_model_setting_update_params(model.clone())
        } else {
            self.active_thread_reasoning_setting_update_params(effort.clone())
        };
        if let Some(mut params) = params {
            if params.permissions.is_some() && self.reject_pending_permission_change() {
                return;
            }
            params.effort = effort.clone();
            params.collaboration_mode = Some(
                self.chat_widget
                    .effective_collaboration_mode()
                    .with_updates(
                        Some(model.clone()),
                        Some(effort.clone()),
                        /*developer_instructions*/ None,
                    ),
            );
            params.service_tier = match service_tier.as_ref() {
                Some(tier) => Some(Some(tier.clone())),
                None => service_tier_resolution::service_tier_update_for_core(
                    self.chat_widget.config_ref(),
                    &self.local_settings.notices,
                    &model,
                    &self.model_catalog.models,
                ),
            };
            if !self.send_model_settings_update(app_server, params).await {
                if app_server.thread_settings_update_supported {
                    return;
                }
                if app_server.thread_params_mode().settings_owner() == ThreadSettingsOwner::Server {
                    self.chat_widget.add_error_message(
                        "This cloud conversation does not support changing its model from the TUI."
                            .into(),
                    );
                    return;
                }
                if service_tier
                    .as_deref()
                    .is_some_and(|tier| tier != SERVICE_TIER_DEFAULT_REQUEST_VALUE)
                {
                    self.chat_widget.add_error_message(
                        "Model selection requires an app server with thread/settings/update support."
                            .into(),
                    );
                    return;
                }
            }
        }
        let in_plan_mode = self.chat_widget.effective_collaboration_mode().mode == ModeKind::Plan;
        let ultra = effort == Some(ReasoningEffort::Ultra);
        let clear_default_ultra = self
            .chat_widget
            .current_collaboration_mode()
            .reasoning_effort()
            == Some(ReasoningEffort::Ultra)
            && self.config.model_reasoning_effort != Some(ReasoningEffort::Ultra);
        let clear_plan_ultra = self.chat_widget.config_ref().plan_mode_reasoning_effort
            == Some(ReasoningEffort::Ultra)
            && self.config.plan_mode_reasoning_effort != Some(ReasoningEffort::Ultra);
        if model_changed {
            self.chat_widget.set_model(&model);
        }
        if !in_plan_mode
            || (self.chat_widget.current_collaboration_mode().model() == model
                && (ultra || clear_default_ultra))
        {
            self.chat_widget.set_reasoning_effort(effort.clone());
        }
        if in_plan_mode || ultra || clear_plan_ultra {
            self.chat_widget
                .set_plan_mode_reasoning_effort(effort.clone());
        }
        if let Some(service_tier) = service_tier.as_ref() {
            self.chat_widget
                .set_service_tier(Some(service_tier.clone()));
        }
        self.sync_active_thread_service_tier_to_cached_session()
            .await;
        let mut message = format!("Model changed to {model}");
        if let Some(label) = Self::reasoning_label_for(&model, effort.as_ref()) {
            message.push(' ');
            message.push_str(&label);
        }
        if let Some(service_tier) = service_tier {
            message.push(' ');
            message.push_str(if service_tier == SERVICE_TIER_DEFAULT_REQUEST_VALUE {
                "standard"
            } else if service_tier == ServiceTier::Fast.request_value() {
                "fast"
            } else {
                &service_tier
            });
        }
        message.push_str(" for this session only");
        self.chat_widget.add_info_message(message, /*hint*/ None);
    }

    pub(super) async fn persist_model_defaults(
        &mut self,
        request_handle: AppServerRequestHandle,
        edits: Vec<ConfigEdit>,
        setting: &str,
    ) -> Result<()> {
        let response = crate::config_update::write_config_batch(request_handle, edits).await?;
        if response.status == WriteStatus::OkOverridden {
            self.chat_widget.add_warning_message(format!(
                "Saved {setting}, but a higher-priority configuration layer overrides the saved value."
            ));
        }
        Ok(())
    }
}
