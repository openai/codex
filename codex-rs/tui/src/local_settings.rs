//! Client-owned preferences and persistence paths, independent of active server settings.
//!
//! The resolved core config is a temporary input at local load/reload boundaries. Server thread
//! responses must never refresh these values; live preference changes belong here. The remaining
//! Config-based lifecycle adapters also use this conversion until their interfaces are migrated.

use crate::legacy_core::config::Config;
use crate::legacy_core::config::TerminalResizeReflowConfig;
use crate::legacy_core::config::TerminalResizeReflowMaxRows;
use codex_config::types::History;
use codex_config::types::Notice;
use codex_config::types::Tui;
use codex_utils_absolute_path::AbsolutePathBuf;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LocalSettings {
    pub(crate) tui: Tui,
    pub(crate) history: History,
    pub(crate) notices: Notice,
    pub(crate) codex_home: AbsolutePathBuf,
    pub(crate) user_config_path: AbsolutePathBuf,
}

impl From<&Config> for LocalSettings {
    fn from(config: &Config) -> Self {
        Self {
            tui: Tui {
                notification_settings: config.tui_notifications.clone(),
                animations: config.animations,
                whimsy: config.tui_whimsy,
                show_tooltips: config.show_tooltips,
                auto_recap: config.tui_auto_recap,
                disable_paste_burst: Some(config.disable_paste_burst),
                vim_mode_default: config.tui_vim_mode_default,
                question_esc_back: config.tui_question_esc_back,
                raw_output_mode: config.tui_raw_output_mode,
                alternate_screen: config.tui_alternate_screen,
                status_line: config.tui_status_line.clone(),
                status_line_use_colors: config.tui_status_line_use_colors,
                terminal_title: config.tui_terminal_title.clone(),
                theme: config.tui_theme.clone(),
                pet: config.tui_pet.clone(),
                pet_anchor: config.tui_pet_anchor,
                session_picker_view: Some(config.tui_session_picker_view),
                resume_cwd: config.tui_resume_cwd,
                keymap: config.tui_keymap.clone(),
                model_availability_nux: config.model_availability_nux.clone(),
                terminal_resize_reflow_max_rows: match config.terminal_resize_reflow.max_rows {
                    TerminalResizeReflowMaxRows::Auto => None,
                    TerminalResizeReflowMaxRows::Disabled => Some(0),
                    TerminalResizeReflowMaxRows::Limit(rows) => Some(rows),
                },
            },
            history: config.history.clone(),
            notices: config.notices.clone(),
            codex_home: config.codex_home.clone(),
            user_config_path: config
                .config_layer_stack
                .get_user_config_file()
                .cloned()
                .unwrap_or_else(|| config.codex_home.join("config.toml")),
        }
    }
}

impl LocalSettings {
    pub(crate) fn terminal_resize_reflow(&self) -> TerminalResizeReflowConfig {
        TerminalResizeReflowConfig {
            max_rows: match self.tui.terminal_resize_reflow_max_rows {
                None => TerminalResizeReflowMaxRows::Auto,
                Some(0) => TerminalResizeReflowMaxRows::Disabled,
                Some(rows) => TerminalResizeReflowMaxRows::Limit(rows),
            },
        }
    }
}

#[cfg(test)]
#[path = "local_settings_tests.rs"]
mod tests;
