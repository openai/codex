//! Delegate navigation and selection to the shared list picker.

use super::ConfigEditorHandoff;
use super::ConfigPanel;
use super::ConfigPanelUpdate;
use super::ConfigPreference;
use super::ConfigSection;
use super::ConfigSettingValue;
use super::PickerState;
use super::settings;
use crate::app_event::AppEvent;
use crate::bottom_pane::BottomPaneView;
use crate::bottom_pane::CancellationEvent;
use crate::keymap::KeymapContext;
use crate::keymap::KeymapContextSet;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;

impl ConfigPanel {
    fn setting_index(&self, selected: usize) -> Option<usize> {
        let tab = self
            .picker
            .active_tab_id()
            .unwrap_or(ConfigSection::Appearance.title());
        self.settings
            .iter()
            .enumerate()
            .filter(|(_, setting)| setting.section.title() == tab)
            .nth(selected)
            .map(|(index, _)| index)
    }

    fn activate(&mut self, selected: usize, activation_key: crate::key_hint::KeyBinding) {
        if self.saving || self.editor_opening.is_some() {
            return;
        }
        let Some(index) = self.setting_index(selected) else {
            return;
        };
        match &self.settings[index].value {
            ConfigSettingValue::Toggle { setting, enabled }
            | ConfigSettingValue::DependentToggle {
                setting,
                enabled,
                dependency_met: true,
                ..
            } => {
                self.saving = true;
                self.app_event_tx
                    .send(AppEvent::SaveConfigPreference(ConfigPreference::Toggle {
                        setting: *setting,
                        enabled: !*enabled,
                    }));
            }
            ConfigSettingValue::Editor { editor, .. } => {
                let handoff = ConfigEditorHandoff::new(*editor, activation_key);
                self.editor_opening = Some(handoff.clone());
                self.app_event_tx.send(AppEvent::OpenConfigEditor(handoff));
            }
            ConfigSettingValue::NotificationTest => {
                self.app_event_tx.send(AppEvent::TestConfigNotification);
            }
            ConfigSettingValue::DependentToggle {
                dependency_met: false,
                ..
            }
            | ConfigSettingValue::Unavailable { .. } => {}
        }
    }

    fn observe_pending_handoff_key(&self, activation_key: crate::key_hint::KeyBinding) -> bool {
        let Some(handoff) = &self.editor_opening else {
            return false;
        };
        if handoff.activation_key.is_same_physical_key(activation_key) {
            handoff.mark_activation_seen();
        } else {
            handoff.mark_activation_inactive();
        }
        true
    }
}

impl BottomPaneView for ConfigPanel {
    fn update_config_panel(&mut self, update: &ConfigPanelUpdate) -> bool {
        let state = PickerState {
            tab: self.picker.active_tab_id().map(str::to_string),
            selected: self.picker.selected_index(),
            query: self.picker.search_query().map(str::to_string),
            notice: update.notice.clone(),
        };
        self.settings = settings(&update.tui, update.status);
        self.saving = false;
        self.picker = super::render::picker(
            &self.settings,
            self.app_event_tx.clone(),
            update.keymap.clone(),
            state,
        );
        true
    }

    fn keymap_contexts(&self) -> KeymapContextSet {
        KeymapContextSet::new(KeymapContext::List)
    }

    fn accepts_input_when_disconnected(&self) -> bool {
        true
    }

    fn config_editor_opened(&mut self) {
        self.editor_opening = None;
    }

    fn interrupt_config_editor_transition(&mut self) {
        if let Some(handoff) = &self.editor_opening {
            handoff.mark_activation_inactive();
        }
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        self.handle_key_event_with_activation_key(key, /*activation_key*/ None);
    }

    fn handle_key_event_with_activation_key(
        &mut self,
        key: KeyEvent,
        activation_key: Option<crate::key_hint::KeyBinding>,
    ) {
        let activation_key =
            activation_key.unwrap_or_else(|| crate::key_hint::KeyBinding::from_event(key));
        if key.kind == KeyEventKind::Release {
            return;
        }
        if self.observe_pending_handoff_key(activation_key) {
            return;
        }
        self.picker.handle_key_event(key);
        if let Some(selected) = self.picker.take_last_selected_index() {
            self.activate(selected, activation_key);
        }
    }

    fn handle_key_release(&mut self, key: KeyEvent) {
        let activation_key = crate::key_hint::KeyBinding::from_event(key);
        if let Some(handoff) = &self.editor_opening
            && handoff.activation_key.is_same_physical_key(activation_key)
        {
            handoff.mark_activation_inactive();
        }
    }

    fn observe_key_event_while_covered(
        &mut self,
        _key: KeyEvent,
        activation_key: crate::key_hint::KeyBinding,
    ) {
        self.observe_pending_handoff_key(activation_key);
    }

    fn handle_paste(&mut self, text: String) -> bool {
        if let Some(handoff) = &self.editor_opening {
            handoff.mark_activation_inactive();
            false
        } else {
            self.picker.handle_paste(text)
        }
    }

    fn prefer_esc_to_handle_key_event(&self) -> bool {
        true
    }

    fn is_complete(&self) -> bool {
        self.picker.is_complete()
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        if let Some(handoff) = &self.editor_opening {
            handoff.mark_activation_inactive();
            CancellationEvent::Handled
        } else {
            self.picker.on_ctrl_c()
        }
    }
}
