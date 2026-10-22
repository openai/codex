//! Persists config-panel preferences, applies live-safe values, and handles notification previews.

use super::*;
use crate::config_panel::ConfigPreference;
use crate::config_panel::ConfigToggle;
use crate::config_update::format_config_error;
use crate::legacy_core::config::load_config_toml_with_layer_stack;
use crate::local_settings::LocalSettings;

impl App {
    pub(super) async fn save_config_preference(
        &mut self,
        tui: &mut tui::Tui,
        preference: ConfigPreference,
    ) {
        let result =
            ConfigEditsBuilder::for_config_path(self.local_settings.user_config_path.as_path())
                .with_edits([preference.edit()])
                .apply()
                .await;
        if let Err(error) = result {
            self.chat_widget
                .refresh_config_panel(/*notice*/ Some(format!(
                    "Could not save: {}",
                    format_config_error(&error)
                )));
            return;
        }

        // TUI preferences are local, so remote sessions reload them from the launch directory.
        let config_cwd = if crate::uses_remote_workspace_or_environment(
            &self.app_server_target,
            &self.environment_manager,
        ) {
            self.launch_cwd.clone()
        } else {
            self.chat_widget.config_ref().cwd.to_path_buf()
        };
        let resolved = match async {
            let cwd = AbsolutePathBuf::relative_to_current_dir(&config_cwd)?;
            let bootstrap = load_config_toml_with_layer_stack(
                self.local_settings.codex_home.as_path(),
                Some(&cwd),
                self.cli_kv_overrides.clone(),
                codex_config::ConfigLoadOptions {
                    loader_overrides: self.loader_overrides.clone(),
                    cloud_config_bundle: self.cloud_config_bundle.clone(),
                    ..Default::default()
                },
            )
            .await?;
            LocalSettings::from_bootstrap(&bootstrap, self.local_settings.codex_home.clone())
                .map_err(std::io::Error::other)
        }
        .await
        .wrap_err_with(|| format!("Failed to rebuild config for cwd {}", config_cwd.display()))
        {
            Ok(settings) => settings,
            Err(error) => {
                self.chat_widget
                    .refresh_config_panel(
                        /*notice*/ Some(format!("Saved, but could not reload: {error}")),
                    );
                return;
            }
        };

        preference.adopt(&mut self.local_settings.tui, &resolved.tui);
        self.chat_widget
            .apply_config_preference(preference, &resolved.tui);
        if matches!(
            preference,
            ConfigPreference::Toggle {
                setting: ConfigToggle::Notifications,
                ..
            }
        ) && !ConfigToggle::Notifications.enabled(&resolved.tui)
        {
            self.chat_widget.pending_notification = None;
            self.config_notification_test_pending = None;
        }
        self.local_settings.custom_notification_filters = resolved.custom_notification_filters;
        self.chat_widget.local_settings.custom_notification_filters =
            resolved.custom_notification_filters;
        preference.adopt(&mut self.chat_widget.config_panel_tui, &resolved.tui);
        if matches!(
            preference,
            ConfigPreference::Toggle {
                setting: ConfigToggle::StatusLineColors,
                ..
            }
        ) {
            self.refresh_status_line();
        }
        if matches!(preference, ConfigPreference::NotificationCondition(_)) {
            tui.set_notification_condition(self.local_settings.tui.notification_settings.condition);
        }
        let effective = match preference {
            ConfigPreference::Toggle { setting, enabled } => {
                setting.enabled(&resolved.tui) == enabled
            }
            ConfigPreference::CopyOnSelect(mode) => resolved.tui.copy_on_select == mode,
            ConfigPreference::NotificationCondition(condition) => {
                resolved.tui.notification_settings.condition == condition
            }
            ConfigPreference::RightClickPaste(mode) => resolved.tui.right_click_paste == mode,
            ConfigPreference::SessionPickerLayout(mode) => {
                resolved.tui.session_picker_view.unwrap_or_default() == mode
            }
            ConfigPreference::ResumeDirectory(mode) => resolved.tui.resume_cwd == mode,
        };
        let notice = if !effective {
            Some("Saved · overridden by config or system setting")
        } else if matches!(
            preference,
            ConfigPreference::Toggle {
                setting: ConfigToggle::StartInVimMode,
                ..
            }
        ) {
            Some("Saved · applies to new composers")
        } else {
            None
        };
        let notice = notice.map(str::to_string);
        if effective {
            self.chat_widget
                .refresh_config_panel_after_effective_save(notice, preference);
        } else {
            self.chat_widget.refresh_config_panel(notice);
        }
        tui.frame_requester().schedule_frame();
    }
}

impl App {
    pub(super) fn schedule_config_notification_test(&mut self) {
        if self.config_notification_test_pending.is_some() {
            self.chat_widget.refresh_config_panel(/*notice*/ Some(
                "Test already scheduled · switch apps".into(),
            ));
            return;
        }
        self.config_notification_test_generation = self
            .config_notification_test_generation
            .wrapping_add(/*rhs*/ 1);
        let generation = self.config_notification_test_generation;
        self.config_notification_test_pending = Some(generation);
        self.chat_widget.refresh_config_panel(
            /*notice*/ Some("Test scheduled in 5s · switch apps".into()),
        );
        let tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(/*secs*/ 5)).await;
            tx.send(AppEvent::SendConfigTestNotification { generation });
        });
    }

    pub(super) fn send_config_notification_test(&mut self, tui: &mut tui::Tui, generation: u64) {
        if self.config_notification_test_pending != Some(generation) {
            return;
        }
        self.config_notification_test_pending = None;
        let notice = if !tui.notify_test("Codex notification test") {
            "Could not send test · check terminal connection"
        } else if codex_terminal_detection::terminal_info().name
            == codex_terminal_detection::TerminalName::Ghostty
            && tui.is_terminal_focused()
        {
            "Test sent · Ghostty hides banners while focused"
        } else {
            "Test sent · check terminal/system settings"
        };
        if !self
            .chat_widget
            .refresh_config_panel(/*notice*/ Some(notice.into()))
        {
            self.chat_widget
                .add_info_message(notice.into(), /*hint*/ None);
        }
        tui.frame_requester().schedule_frame();
    }
}

#[cfg(test)]
#[path = "config_panel_tests.rs"]
mod tests;
