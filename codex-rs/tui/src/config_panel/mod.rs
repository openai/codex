//! Catalog of TUI configuration editors and directly editable preferences shown by `/config`.
//!
//! This module defines the stable grouping, copy, search terms, and displayed values. Rendering
//! and interaction live in later modules so they can consume one ordered catalog.

mod input;
mod render;
mod toggle;
pub(crate) use render::copy_on_select;
pub(crate) use render::notification_condition;
pub(crate) use render::resume_directory;
pub(crate) use render::right_click_paste;
pub(crate) use render::session_picker_layout;
pub(crate) use toggle::ConfigPreference;
pub(crate) use toggle::ConfigToggle;

#[cfg(test)]
#[path = "config_panel_tests.rs"]
mod tests;

use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::ListSelectionView;
use crate::key_hint::KeyBinding;
use crate::keymap::ListKeymap;
use codex_config::types::CopyOnSelect;
use codex_config::types::NotificationCondition;
use codex_config::types::Notifications;
use codex_config::types::RightClickPaste;
use codex_config::types::Tui;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;

const RIGHT_CLICK_PASTE_DESCRIPTION: &str =
    "Paste when nothing is selected. Auto follows the platform default; On also enables macOS.";

pub(crate) struct ConfigPanelUpdate {
    pub(crate) tui: Tui,
    pub(crate) keymap: ListKeymap,
    pub(crate) status: ConfigPanelStatus,
    pub(crate) notice: Option<String>,
}

#[derive(Default)]
struct PickerState {
    tab: Option<String>,
    selected: Option<usize>,
    query: Option<String>,
    notice: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ConfigEditor {
    Theme,
    StatusLine,
    TerminalTitle,
    Keyboard,
    SessionPickerLayout,
    ResumeDirectory,
    CopyOnSelect,
    RightClickPaste,
    NotificationCondition,
}

#[derive(Clone, Copy)]
pub(crate) enum ResumeCurrentAvailability {
    Available,
    RequiresCwdOverride,
}

/// Effective state the panel cannot derive from the `Tui` preferences alone.
#[derive(Clone, Copy)]
pub(crate) struct ConfigPanelStatus {
    pub(crate) copy_on_select: bool,
    pub(crate) right_click_paste: Result<bool, &'static str>,
    pub(crate) custom_notification_filters: bool,
}

/// State shared by the panel that queued an editor and the event that will open it.
#[derive(Clone, Debug)]
pub(crate) struct ConfigEditorHandoff {
    pub(crate) editor: ConfigEditor,
    pub(crate) activation_key: KeyBinding,
    activation: Arc<Mutex<ConfigEditorActivation>>,
}

#[derive(Debug)]
struct ConfigEditorActivation {
    active: bool,
    last_seen_at: Instant,
}

impl ConfigEditorHandoff {
    pub(crate) fn new(editor: ConfigEditor, activation_key: KeyBinding) -> Self {
        Self {
            editor,
            activation_key,
            activation: Arc::new(Mutex::new(ConfigEditorActivation {
                active: true,
                last_seen_at: Instant::now(),
            })),
        }
    }

    pub(crate) fn mark_activation_seen(&self) {
        self.mark_activation_seen_at(Instant::now());
    }

    fn mark_activation_seen_at(&self, seen_at: Instant) {
        let mut activation = self
            .activation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if activation.active {
            activation.last_seen_at = activation.last_seen_at.max(seen_at);
        }
    }

    pub(crate) fn mark_activation_inactive(&self) {
        self.activation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .active = false;
    }

    pub(crate) fn active_at(&self) -> Option<Instant> {
        let activation = self
            .activation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        activation.active.then_some(activation.last_seen_at)
    }
}

impl ConfigEditor {
    fn value(self, tui: &Tui, status: ConfigPanelStatus) -> ConfigSettingValue {
        let label = match self {
            Self::Theme => tui.theme.clone().unwrap_or_else(|| "Automatic".to_string()),
            Self::SessionPickerLayout => match tui.session_picker_view.unwrap_or_default() {
                codex_config::types::SessionPickerViewMode::Comfortable => "Comfortable",
                codex_config::types::SessionPickerViewMode::Dense => "Dense",
            }
            .to_string(),
            Self::ResumeDirectory => match tui.resume_cwd {
                None => "Ask",
                Some(codex_config::types::ResumeCwdMode::Current) => "Current directory",
                Some(codex_config::types::ResumeCwdMode::Session) => "Session directory",
            }
            .to_string(),
            Self::CopyOnSelect => match tui.copy_on_select {
                CopyOnSelect::Auto => format!(
                    "Auto · {}",
                    if status.copy_on_select { "On" } else { "Off" }
                ),
                CopyOnSelect::Always => "On".to_string(),
                CopyOnSelect::Never => "Off".to_string(),
            },
            Self::RightClickPaste => {
                let preference = match tui.right_click_paste {
                    RightClickPaste::Auto => "Auto",
                    RightClickPaste::On => "On",
                    RightClickPaste::Off => "Off",
                };
                match status.right_click_paste {
                    Ok(enabled) if tui.right_click_paste == RightClickPaste::Auto => {
                        format!("Auto · {}", if enabled { "On" } else { "Off" })
                    }
                    Ok(_) => preference.to_string(),
                    Err(_) => format!("{preference} · Unavailable"),
                }
            }
            Self::NotificationCondition => match tui.notification_settings.condition {
                NotificationCondition::Unfocused => "Unfocused",
                NotificationCondition::Always => "Always",
            }
            .to_string(),
            Self::StatusLine | Self::TerminalTitle | Self::Keyboard => "Configure".to_string(),
        };
        ConfigSettingValue::Editor {
            label,
            editor: self,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConfigSection {
    Appearance,
    Notifications,
    Input,
    Sessions,
    TextSelection,
}

impl ConfigSection {
    const ALL: [Self; 5] = [
        Self::Appearance,
        Self::Notifications,
        Self::Input,
        Self::Sessions,
        Self::TextSelection,
    ];

    fn title(self) -> &'static str {
        match self {
            Self::Appearance => "Appearance",
            Self::Notifications => "Notifications",
            Self::Input => "Input",
            Self::Sessions => "Sessions",
            Self::TextSelection => "Text selection",
        }
    }
}

struct ConfigSetting {
    section: ConfigSection,
    label: &'static str,
    description: &'static str,
    keywords: &'static str,
    value: ConfigSettingValue,
}

impl ConfigSetting {
    fn display_label(&self) -> String {
        if matches!(self.value, ConfigSettingValue::DependentToggle { .. }) {
            format!("    {}", self.label)
        } else {
            self.label.to_string()
        }
    }
}

enum ConfigSettingValue {
    Toggle {
        setting: ConfigToggle,
        enabled: bool,
    },
    Editor {
        label: String,
        editor: ConfigEditor,
    },
    DependentToggle {
        setting: ConfigToggle,
        enabled: bool,
        dependency_met: bool,
        unavailable_reason: &'static str,
    },
    NotificationTest,
    Unavailable {
        reason: &'static str,
    },
}

impl ConfigSettingValue {
    fn label(&self) -> String {
        match self {
            Self::Toggle { enabled, .. }
            | Self::DependentToggle {
                enabled,
                dependency_met: true,
                ..
            } => if *enabled { "[x] On" } else { "[ ] Off" }.to_string(),
            Self::Editor { label, .. } => format!("{label} ›"),
            Self::NotificationTest => "Send in 5s ›".to_string(),
            Self::DependentToggle {
                dependency_met: false,
                ..
            }
            | Self::Unavailable { .. } => "Unavailable".to_string(),
        }
    }
}

fn settings(tui: &Tui, status: ConfigPanelStatus) -> Vec<ConfigSetting> {
    let notifications_enabled = !matches!(
        tui.notification_settings.notifications,
        Notifications::Enabled(false)
    );
    vec![
        ConfigSetting {
            section: ConfigSection::Appearance,
            label: "Theme",
            description: "Choose a syntax color palette with a live preview.",
            keywords: "colors dark light palette",
            value: ConfigEditor::Theme.value(tui, status),
        },
        ConfigSetting {
            section: ConfigSection::Appearance,
            label: "Animations",
            description: "Animate the welcome screen, working indicator, and transitions. Restart Codex to apply.",
            keywords: "motion accessibility spinner",
            value: ConfigSettingValue::Toggle {
                setting: ConfigToggle::Animations,
                enabled: ConfigToggle::Animations.enabled(tui),
            },
        },
        ConfigSetting {
            section: ConfigSection::Appearance,
            label: "Composer stars",
            description: "Animate stars in the composer. Requires animations and a restart.",
            keywords: "effects starfield stars motion",
            value: ConfigSettingValue::DependentToggle {
                setting: ConfigToggle::ComposerStars,
                enabled: ConfigToggle::ComposerStars.enabled(tui),
                dependency_met: tui.animations,
                unavailable_reason: "Turn on animations, then restart Codex to enable composer stars.",
            },
        },
        ConfigSetting {
            section: ConfigSection::Appearance,
            label: "Tips",
            description: "Show helpful shortcuts and suggestions. Restart Codex to apply.",
            keywords: "welcome hints tooltips",
            value: ConfigSettingValue::Toggle {
                setting: ConfigToggle::Tips,
                enabled: ConfigToggle::Tips.enabled(tui),
            },
        },
        ConfigSetting {
            section: ConfigSection::Appearance,
            label: "Status line colors",
            description: "Use theme colors for the model, branch, and other status line fields.",
            keywords: "statusline footer colour",
            value: ConfigSettingValue::Toggle {
                setting: ConfigToggle::StatusLineColors,
                enabled: ConfigToggle::StatusLineColors.enabled(tui),
            },
        },
        ConfigSetting {
            section: ConfigSection::Appearance,
            label: "Status line fields",
            description: "Choose and reorder the information below the composer.",
            keywords: "statusline footer model branch context",
            value: ConfigEditor::StatusLine.value(tui, status),
        },
        ConfigSetting {
            section: ConfigSection::Appearance,
            label: "Terminal title fields",
            description: "Choose and reorder the information in your terminal tab or window title.",
            keywords: "title tab window name",
            value: ConfigEditor::TerminalTitle.value(tui, status),
        },
        ConfigSetting {
            section: ConfigSection::Notifications,
            label: "Notifications",
            description: "Let your terminal notify you when a turn finishes or approval is needed.",
            keywords: "alerts bell completion approval",
            value: if status.custom_notification_filters {
                ConfigSettingValue::Unavailable {
                    reason: "Event-specific notifications are configured. Edit them in config.toml.",
                }
            } else {
                ConfigSettingValue::Toggle {
                    setting: ConfigToggle::Notifications,
                    enabled: notifications_enabled,
                }
            },
        },
        ConfigSetting {
            section: ConfigSection::Notifications,
            label: "Notify when",
            description: "Only notify while you are away, or allow notifications while this terminal is focused too.",
            keywords: "focus alerts background always unfocused",
            value: if notifications_enabled {
                ConfigEditor::NotificationCondition.value(tui, status)
            } else {
                ConfigSettingValue::Unavailable {
                    reason: "Turn on notifications to choose when they appear.",
                }
            },
        },
        ConfigSetting {
            section: ConfigSection::Notifications,
            label: "Test notification",
            description: "Sends in 5 seconds. Switch apps: your terminal may hide notifications while focused.",
            keywords: "alerts bell sound test",
            value: ConfigSettingValue::NotificationTest,
        },
        ConfigSetting {
            section: ConfigSection::Input,
            label: "Keyboard shortcuts",
            description: "Browse actions and remap their keys.",
            keywords: "keymap bindings hotkeys",
            value: ConfigEditor::Keyboard.value(tui, status),
        },
        ConfigSetting {
            section: ConfigSection::Input,
            label: "Start in Vim mode",
            description: "Applies to new composers. Use /vim to switch the current composer.",
            keywords: "editor modal normal",
            value: ConfigSettingValue::Toggle {
                setting: ConfigToggle::StartInVimMode,
                enabled: ConfigToggle::StartInVimMode.enabled(tui),
            },
        },
        ConfigSetting {
            section: ConfigSection::Input,
            label: "Paste detection",
            description: "Recognize pasted-text bursts and keep multiline input together. Restart Codex to apply.",
            keywords: "burst clipboard multiline",
            value: ConfigSettingValue::Toggle {
                setting: ConfigToggle::PasteDetection,
                enabled: ConfigToggle::PasteDetection.enabled(tui),
            },
        },
        ConfigSetting {
            section: ConfigSection::Sessions,
            label: "Automatic recaps",
            description: "Prepare recaps while the terminal is unfocused. Restart Codex to apply.",
            keywords: "summary background",
            value: ConfigSettingValue::Toggle {
                setting: ConfigToggle::AutomaticRecaps,
                enabled: ConfigToggle::AutomaticRecaps.enabled(tui),
            },
        },
        ConfigSetting {
            section: ConfigSection::Sessions,
            label: "Session picker layout",
            description: "Choose tighter rows or a roomier list when resuming and forking conversations.",
            keywords: "dense comfortable spacing resume fork",
            value: ConfigEditor::SessionPickerLayout.value(tui, status),
        },
        ConfigSetting {
            section: ConfigSection::Sessions,
            label: "Resume directory",
            description: "Choose which working directory to use when the current directory and saved session differ.",
            keywords: "cwd folder path fork ask",
            value: ConfigEditor::ResumeDirectory.value(tui, status),
        },
        ConfigSetting {
            section: ConfigSection::TextSelection,
            label: "Copy on select",
            description: "Copy transcript selections on release. Auto uses terminal defaults.",
            keywords: "clipboard mouse selection automatic drag",
            value: ConfigEditor::CopyOnSelect.value(tui, status),
        },
        ConfigSetting {
            section: ConfigSection::TextSelection,
            label: "Deselect after copy",
            description: "Remove the selection highlight after Copy on select succeeds.",
            keywords: "clipboard deselect clear highlight automatic mouse selection",
            value: ConfigSettingValue::DependentToggle {
                setting: ConfigToggle::CopyOnSelectClearSelection,
                enabled: ConfigToggle::CopyOnSelectClearSelection.enabled(tui),
                dependency_met: status.copy_on_select,
                unavailable_reason: "Turn on Copy on select to deselect after copying.",
            },
        },
        ConfigSetting {
            section: ConfigSection::TextSelection,
            label: "Right-click paste",
            description: status
                .right_click_paste
                .err()
                .unwrap_or(RIGHT_CLICK_PASTE_DESCRIPTION),
            keywords: "clipboard mouse paste automatic",
            value: ConfigEditor::RightClickPaste.value(tui, status),
        },
    ]
}

pub(crate) struct ConfigPanel {
    settings: Vec<ConfigSetting>,
    picker: ListSelectionView,
    saving: bool,
    editor_opening: Option<ConfigEditorHandoff>,
    app_event_tx: AppEventSender,
}

impl ConfigPanel {
    pub(crate) fn new(
        tui: &Tui,
        app_event_tx: AppEventSender,
        keymap: ListKeymap,
        status: ConfigPanelStatus,
    ) -> Self {
        let settings = settings(tui, status);
        let picker = render::picker(
            &settings,
            app_event_tx.clone(),
            keymap,
            PickerState::default(),
        );
        Self {
            settings,
            picker,
            saving: false,
            editor_opening: None,
            app_event_tx,
        }
    }
}
