//! New-session configuration from server defaults and explicit launch settings.
//!
//! Build the replacement configuration without replacing the active task's settings.

use super::resume_preparation::BackendSettings;
use super::*;
use codex_config::ConfigLayerSource;
use codex_protocol::config_types::ServiceTier;

pub(crate) fn has_launch_setting(
    config: &Config,
    cli_kv_overrides: &[(String, TomlValue)],
    key: &str,
) -> bool {
    // A remote server cannot resolve this invocation's explicitly selected local profile.
    // Only count the profile when it supplies the effective value; project settings can shadow it.
    cli_kv_overrides.iter().any(|(path, _)| path == key)
        || config
            .config_layer_stack
            .layers_high_to_low()
            .find(|layer| layer.config.get(key).is_some())
            .is_some_and(|layer| {
                matches!(
                    layer.name,
                    ConfigLayerSource::User {
                        profile: Some(_),
                        ..
                    }
                )
            })
}

pub(super) fn overlay_new_session_defaults(
    config: &mut Config,
    defaults: &codex_app_server_protocol::Config,
    cli_kv_overrides: &[(String, TomlValue)],
    harness_overrides: &ConfigOverrides,
) {
    if harness_overrides.model.is_none() && !has_launch_setting(config, cli_kv_overrides, "model") {
        config.model = defaults.model.clone();
    }
    if !has_launch_setting(config, cli_kv_overrides, "model_reasoning_effort") {
        config.model_reasoning_effort = defaults.model_reasoning_effort.clone();
    }
    if harness_overrides.service_tier.is_none()
        && !has_launch_setting(config, cli_kv_overrides, "service_tier")
    {
        config.service_tier = defaults.service_tier.as_deref().map(|service_tier| {
            ServiceTier::from_request_value(service_tier)
                .map(|tier| tier.request_value().to_string())
                .unwrap_or_else(|| service_tier.to_string())
        });
    }
    if !has_launch_setting(config, cli_kv_overrides, "daybreak") {
        config.daybreak_enabled = defaults
            .additional
            .get("daybreak")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
    }
}

impl App {
    pub(super) async fn load_new_session_config(
        &mut self,
        app_server: &AppServerSession,
    ) -> Result<(Config, crate::local_settings::LocalSettings)> {
        let settings = self.backend_settings();
        let cwd = self.chat_widget.config_ref().cwd.to_path_buf();
        self.load_new_session_config_from(app_server, &settings, cwd, /*cloud_switch*/ None)
            .await
    }

    pub(super) async fn load_new_session_config_from(
        &mut self,
        app_server: &AppServerSession,
        settings: &BackendSettings,
        cwd: PathBuf,
        mut cloud_switch: Option<&mut tui::Tui>,
    ) -> Result<(Config, crate::local_settings::LocalSettings)> {
        let defaults_cwd = match app_server.thread_params_mode() {
            crate::app_server_session::ThreadParamsMode::Embedded => cwd.as_path(),
            crate::app_server_session::ThreadParamsMode::Remote
            | crate::app_server_session::ThreadParamsMode::Cloud => {
                app_server.remote_cwd_override().unwrap_or(Path::new("."))
            }
        };
        let refresh = self.rebuild_config_for_cwd_from(&settings.config, cwd.clone());
        let load = async {
            let defaults = crate::config_update::read_effective_config_if_supported(
                app_server.request_handle(),
                defaults_cwd,
            )
            .await?;
            Ok::<_, color_eyre::eyre::Report>((defaults, refresh.await))
        };
        let (defaults, refreshed_config) = if let Some(tui) = cloud_switch.as_deref_mut() {
            let events = tui.event_stream();
            super::cloud_switch_wait::wait_for_cloud_switch(
                self,
                tui,
                load,
                events,
                super::cloud_switch_wait::ClipboardHandoff::ContinueEditing,
            )
            .await
        } else {
            load.await
        }?;
        // Stage local preferences and permission carryover without changing the active task.
        let (mut config, local_settings) = match refreshed_config {
            Ok(config) => {
                let local_settings = settings.local_settings.reloaded(&config);
                (config, local_settings)
            }
            Err(err) => {
                tracing::warn!(%err, "failed to refresh local settings before a new thread");
                (settings.config.clone(), settings.local_settings.clone())
            }
        };
        if let Some(defaults) = defaults.as_ref() {
            crate::projectless::apply_defaults(
                &mut config,
                &self.harness_overrides,
                app_server,
                &self.environment_manager,
                defaults,
            );
        }
        if let Err(error) = self.apply_runtime_policy_overrides_with_reviewer(
            &mut config,
            RuntimePolicyOverrideScope::ExplicitOnly,
            settings.config.approvals_reviewer,
        ) {
            if let Some(tui) = cloud_switch {
                let events = tui.event_stream();
                super::cloud_switch_wait::finish_failed_cloud_switch(self, tui, events).await;
            }
            return Err(error);
        }
        if let Some(defaults) = defaults.as_ref() {
            overlay_new_session_defaults(
                &mut config,
                &defaults.config,
                &self.cli_kv_overrides,
                &self.harness_overrides,
            );
        }
        apply_managed_new_thread_defaults(
            &mut config,
            app_server.managed_new_thread_defaults(),
            &self.cli_kv_overrides,
            &self.harness_overrides,
        );
        Ok((config, local_settings))
    }
}
