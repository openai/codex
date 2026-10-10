//! Entry points for the configuration browser and its existing child editors.

use super::ChatWidget;
use crate::config_panel::ConfigEditor;
use crate::config_panel::ConfigEditorHandoff;
use crate::config_panel::ConfigPanel;
use crate::config_panel::ConfigPanelStatus;
use crate::config_panel::ConfigPanelUpdate;
use crossterm::event::KeyEvent;

impl ChatWidget {
    pub(crate) fn open_config_panel(&mut self) {
        self.bottom_pane.show_view(Box::new(ConfigPanel::new(
            &self.config_panel_tui,
            self.app_event_tx.clone(),
            self.bottom_pane.list_keymap(),
            self.config_panel_status(),
        )));
    }

    pub(crate) fn open_config_editor(
        &mut self,
        handoff: ConfigEditorHandoff,
        resume_current_availability: crate::config_panel::ResumeCurrentAvailability,
    ) {
        match handoff.editor {
            ConfigEditor::Theme => self.open_theme_picker(),
            ConfigEditor::StatusLine => self.open_status_line_setup(),
            ConfigEditor::TerminalTitle => self.open_terminal_title_setup(),
            ConfigEditor::Keyboard => self.open_keymap_picker(),
            ConfigEditor::SessionPickerLayout => {
                let keymap = self.bottom_pane.list_keymap();
                let params =
                    crate::config_panel::session_picker_layout(&self.local_settings.tui, &keymap);
                self.bottom_pane.show_selection_view(params);
            }
            ConfigEditor::ResumeDirectory => {
                let keymap = self.bottom_pane.list_keymap();
                let params = crate::config_panel::resume_directory(
                    &self.local_settings.tui,
                    &keymap,
                    resume_current_availability,
                );
                self.bottom_pane.show_selection_view(params);
            }
            ConfigEditor::CopyOnSelect => {
                let keymap = self.bottom_pane.list_keymap();
                let params = crate::config_panel::copy_on_select(&self.local_settings.tui, &keymap);
                self.bottom_pane.show_selection_view(params);
            }
            ConfigEditor::RightClickPaste => {
                let keymap = self.bottom_pane.list_keymap();
                let params =
                    crate::config_panel::right_click_paste(&self.local_settings.tui, &keymap);
                self.bottom_pane.show_selection_view(params);
            }
            ConfigEditor::NotificationCondition => {
                let keymap = self.bottom_pane.list_keymap();
                let params =
                    crate::config_panel::notification_condition(&self.local_settings.tui, &keymap);
                self.bottom_pane.show_selection_view(params);
            }
        }
        if let Some(activated_at) = handoff.active_at() {
            self.bottom_pane
                .suppress_current_view_key_until_release(handoff.activation_key, activated_at);
        }
        self.bottom_pane.config_editor_opened();
    }

    pub(crate) fn interrupt_config_editor_transition(&mut self) {
        self.bottom_pane.interrupt_config_editor_transition();
    }

    pub(crate) fn observe_config_editor_transition_key_event(&mut self, key: KeyEvent) {
        self.bottom_pane
            .observe_config_editor_transition_key_event(key);
    }

    pub(crate) fn refresh_config_panel(&mut self, notice: Option<String>) -> bool {
        self.bottom_pane.update_config_panel(&ConfigPanelUpdate {
            tui: self.config_panel_tui.clone(),
            keymap: self.bottom_pane.list_keymap(),
            status: self.config_panel_status(),
            notice,
        })
    }

    fn config_panel_status(&self) -> ConfigPanelStatus {
        ConfigPanelStatus {
            copy_on_select: self
                .local_settings
                .copy_on_select(&codex_terminal_detection::terminal_info()),
            right_click_paste: crate::app::PasteEnvironment::detect().status(
                self.local_settings.tui.right_click_paste,
                self.local_settings.transcript_mode,
            ),
            custom_notification_filters: self.local_settings.custom_notification_filters,
        }
    }

    pub(crate) fn report_config_save_error(&mut self, message: String) {
        self.add_error_message(message.clone());
        self.refresh_config_panel(Some(message));
    }

    pub(crate) fn config_editor_open_failed(&mut self, message: String) {
        self.bottom_pane.config_editor_opened();
        self.add_error_message(message.clone());
        self.refresh_config_panel(Some(message));
    }
}
