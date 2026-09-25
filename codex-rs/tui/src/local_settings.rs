//! Client-owned preferences and persistence paths, independent of active server settings.
//!
//! The resolved core config is a temporary input at local load/reload boundaries. Server thread
//! responses must never refresh these values; live preference changes belong here. The remaining
//! Config-based lifecycle adapters also use this conversion until their interfaces are migrated.
//! Effective animations also respect the TUI host's launch-time accessibility preference.
//! The selected transcript ownership and alternate-screen restrictions survive local reloads.

use crate::legacy_core::config::Config;
use crate::legacy_core::config::TerminalResizeReflowConfig;
use crate::legacy_core::config::TerminalResizeReflowMaxRows;
use crate::transcript_mode::TranscriptMode;
use codex_config::types::CopyOnSelect;
use codex_config::types::History;
use codex_config::types::Notice;
use codex_config::types::Tui;
use codex_terminal_detection::Multiplexer;
use codex_terminal_detection::TerminalInfo;
use codex_terminal_detection::TerminalName;
use codex_utils_absolute_path::AbsolutePathBuf;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LocalSettings {
    pub(crate) tui: Tui,
    pub(crate) transcript_mode: TranscriptMode,
    pub(crate) history: History,
    pub(crate) notices: Notice,
    pub(crate) codex_home: AbsolutePathBuf,
    pub(crate) user_config_path: AbsolutePathBuf,
}

impl From<&Config> for LocalSettings {
    fn from(config: &Config) -> Self {
        Self::with_accessibility_preferences(
            config,
            crate::system_motion::mode(),
            crate::screen_reader::animation_default(),
        )
    }
}

impl LocalSettings {
    fn with_accessibility_preferences(
        config: &Config,
        system_motion: crate::motion::MotionMode,
        screen_reader_default: crate::motion::MotionMode,
    ) -> Self {
        let animations = if screen_reader_default == crate::motion::MotionMode::Reduced {
            // Consult the current layers so preferences edited after startup still win.
            config
                .config_layer_stack
                .effective_config()
                .get("tui")
                .and_then(|tui| tui.get("animations"))
                .is_some()
                && config.animations
        } else {
            config.animations
        };
        Self {
            transcript_mode: TranscriptMode::resolve(
                config.tui_fullscreen_transcript,
                config.tui_alternate_screen != codex_config::types::AltScreenMode::Never,
            ),
            tui: Tui {
                notification_settings: config.tui_notifications.clone(),
                animations: animations && system_motion == crate::motion::MotionMode::Animated,
                screen_reader_detection_done: None,
                effects: config.tui_effects,
                rendering: config.tui_rendering,
                show_tooltips: config.show_tooltips,
                show_server_version_notice: config.tui_show_server_version_notice,
                auto_recap: config.tui_auto_recap,
                prompt_suggestions: config.tui_prompt_suggestions,
                disable_paste_burst: Some(config.disable_paste_burst),
                vim_mode_default: config.tui_vim_mode_default,
                question_esc_back: config.tui_question_esc_back,
                raw_output_mode: config.tui_raw_output_mode,
                fullscreen_transcript: config.tui_fullscreen_transcript,
                copy_on_select: config.tui_copy_on_select,
                right_click_paste: config.tui_right_click_paste,
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
    /// Adopt the screen selected before first paint, including command-line restrictions.
    pub(crate) fn for_tui(config: &Config, tui: &crate::tui::Tui) -> Self {
        let mut settings = Self::from(config);
        settings.transcript_mode = tui.transcript_mode();
        if !tui.is_alt_screen_enabled() {
            settings.tui.alternate_screen = codex_config::types::AltScreenMode::Never;
        } else if settings.tui.alternate_screen == codex_config::types::AltScreenMode::Never {
            settings.tui.alternate_screen = codex_config::types::AltScreenMode::Auto;
        }
        settings
    }

    /// Refresh editable preferences without changing this launch's terminal ownership.
    pub(crate) fn reloaded(&self, config: &Config) -> Self {
        let mut settings = Self::from(config);
        settings.transcript_mode = self.transcript_mode;
        settings.tui.alternate_screen = self.tui.alternate_screen;
        settings
    }

    /// Prefer explicit overrides; tmux/Zellij always default to copying on release.
    /// Direct macOS terminals default on except Ghostty/Kitty, which forward Cmd-C.
    pub(crate) fn copy_on_select(&self, terminal: &TerminalInfo) -> bool {
        match self.tui.copy_on_select {
            CopyOnSelect::Always => true,
            CopyOnSelect::Never => false,
            CopyOnSelect::Auto => match terminal.multiplexer {
                Some(Multiplexer::Tmux { .. } | Multiplexer::Zellij { .. }) => true,
                None if cfg!(target_os = "macos") => {
                    !matches!(terminal.name, TerminalName::Ghostty | TerminalName::Kitty)
                }
                None => matches!(
                    terminal.name,
                    TerminalName::Iterm2 | TerminalName::AppleTerminal
                ),
            },
        }
    }

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
