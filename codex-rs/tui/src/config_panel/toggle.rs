//! Maps config-panel preference changes to persisted and effective TUI values.

use crate::legacy_core::config::edit::ConfigEdit;
use crate::legacy_core::config::edit::session_picker_view_edit;
use codex_config::types::CopyOnSelect;
use codex_config::types::NotificationCondition;
use codex_config::types::Notifications;
use codex_config::types::ResumeCwdMode;
use codex_config::types::RightClickPaste;
use codex_config::types::SessionPickerViewMode;
use codex_config::types::Tui;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ConfigToggle {
    Animations,
    AutomaticRecaps,
    ComposerStars,
    CopyOnSelectClearSelection,
    Notifications,
    PasteDetection,
    StartInVimMode,
    StatusLineColors,
    Tips,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ConfigPreference {
    Toggle {
        setting: ConfigToggle,
        enabled: bool,
    },
    CopyOnSelect(CopyOnSelect),
    NotificationCondition(NotificationCondition),
    RightClickPaste(RightClickPaste),
    SessionPickerLayout(SessionPickerViewMode),
    ResumeDirectory(Option<ResumeCwdMode>),
}

impl ConfigPreference {
    pub(crate) fn edit(self) -> ConfigEdit {
        match self {
            Self::Toggle { setting, enabled } => setting.edit(enabled),
            Self::CopyOnSelect(mode) => ConfigEdit::SetPath {
                segments: vec!["tui".to_string(), "copy_on_select".to_string()],
                value: toml_edit::value(match mode {
                    CopyOnSelect::Auto => "auto",
                    CopyOnSelect::Always => "always",
                    CopyOnSelect::Never => "never",
                }),
            },
            Self::NotificationCondition(condition) => ConfigEdit::SetPath {
                segments: vec!["tui".to_string(), "notification_condition".to_string()],
                value: toml_edit::value(condition.to_string()),
            },
            Self::RightClickPaste(mode) => ConfigEdit::SetPath {
                segments: vec!["tui".to_string(), "right_click_paste".to_string()],
                value: toml_edit::value(match mode {
                    RightClickPaste::Auto => "auto",
                    RightClickPaste::On => "on",
                    RightClickPaste::Off => "off",
                }),
            },
            Self::SessionPickerLayout(mode) => session_picker_view_edit(mode),
            Self::ResumeDirectory(None) => ConfigEdit::ClearPath {
                segments: vec!["tui".to_string(), "resume_cwd".to_string()],
            },
            Self::ResumeDirectory(Some(mode)) => ConfigEdit::SetPath {
                segments: vec!["tui".to_string(), "resume_cwd".to_string()],
                value: toml_edit::value(mode.as_str()),
            },
        }
    }

    /// Copies only this preference, leaving unrelated settings unchanged.
    pub(crate) fn adopt(self, target: &mut Tui, source: &Tui) {
        match self {
            Self::Toggle { setting, .. } => setting.adopt(target, source),
            Self::CopyOnSelect(_) => target.copy_on_select = source.copy_on_select,
            Self::NotificationCondition(_) => {
                target.notification_settings.condition = source.notification_settings.condition;
            }
            Self::RightClickPaste(_) => target.right_click_paste = source.right_click_paste,
            Self::SessionPickerLayout(_) => {
                target.session_picker_view = source.session_picker_view;
            }
            Self::ResumeDirectory(_) => target.resume_cwd = source.resume_cwd,
        }
    }
}

impl ConfigToggle {
    const RESTART_REQUIRED: [Self; 5] = [
        Self::Animations,
        Self::AutomaticRecaps,
        Self::ComposerStars,
        Self::PasteDetection,
        Self::Tips,
    ];

    pub(crate) fn requires_restart(self) -> bool {
        Self::RESTART_REQUIRED.contains(&self)
    }

    pub(crate) fn preserve_restart_required(target: &mut Tui, source: &Tui) {
        for setting in Self::RESTART_REQUIRED {
            setting.adopt(target, source);
        }
    }

    pub(crate) fn edit(self, enabled: bool) -> ConfigEdit {
        let (key, value) = match self {
            Self::ComposerStars => {
                return ConfigEdit::SetPath {
                    segments: vec!["tui".into(), "effects".into(), "starfield".into()],
                    value: toml_edit::value(enabled),
                };
            }
            Self::Notifications => {
                return ConfigEdit::SetTuiNotificationsEnabled { enabled };
            }
            Self::Animations => ("animations", enabled),
            Self::AutomaticRecaps => ("auto_recap", enabled),
            Self::CopyOnSelectClearSelection => ("copy_on_select_clear_selection", enabled),
            Self::PasteDetection => ("disable_paste_burst", !enabled),
            Self::StartInVimMode => ("vim_mode_default", enabled),
            Self::StatusLineColors => ("status_line_use_colors", enabled),
            Self::Tips => ("show_tooltips", enabled),
        };
        ConfigEdit::SetPath {
            segments: vec!["tui".into(), key.into()],
            value: toml_edit::value(value),
        }
    }

    pub(crate) fn enabled(self, tui: &Tui) -> bool {
        match self {
            Self::Animations => tui.animations,
            Self::AutomaticRecaps => tui.auto_recap,
            Self::ComposerStars => tui.effects.starfield,
            Self::CopyOnSelectClearSelection => tui.copy_on_select_clear_selection,
            Self::Notifications => !matches!(
                tui.notification_settings.notifications,
                Notifications::Enabled(false)
            ),
            Self::PasteDetection => !tui.disable_paste_burst.unwrap_or(/*default*/ false),
            Self::StartInVimMode => tui.vim_mode_default,
            Self::StatusLineColors => tui.status_line_use_colors,
            Self::Tips => tui.show_tooltips,
        }
    }

    /// Copies only this preference, leaving unrelated settings unchanged.
    pub(crate) fn adopt(self, target: &mut Tui, source: &Tui) {
        match self {
            Self::Animations => target.animations = source.animations,
            Self::AutomaticRecaps => target.auto_recap = source.auto_recap,
            Self::ComposerStars => target.effects.starfield = source.effects.starfield,
            Self::CopyOnSelectClearSelection => {
                target.copy_on_select_clear_selection = source.copy_on_select_clear_selection;
            }
            Self::Notifications => {
                target.notification_settings.notifications =
                    source.notification_settings.notifications.clone();
            }
            Self::PasteDetection => target.disable_paste_burst = source.disable_paste_burst,
            Self::StartInVimMode => target.vim_mode_default = source.vim_mode_default,
            Self::StatusLineColors => {
                target.status_line_use_colors = source.status_line_use_colors;
            }
            Self::Tips => target.show_tooltips = source.show_tooltips,
        }
    }
}
