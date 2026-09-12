//! Focus and input routing for the agent dashboard. Refreshes retain the shared
//! composer; Right opens the selected task from an empty editor with no pending input.

use super::*;
use crate::bottom_pane::InputResult;
use crate::chatwidget::UserMessage;
use crate::clipboard_paste::paste_image_to_temp_png;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;

impl AgentsOverviewView {
    pub(super) fn handle_composer_key(&mut self, key: KeyEvent) {
        let mut state = self.state();
        let offline = state.connection_notice.is_some();
        let grouping = state.grouping;
        if !offline
            && crate::key_hint::plain(KeyCode::Right).is_press(key)
            && state
                .composer
                .as_ref()
                .is_some_and(ChatComposer::can_leave_empty_prompt_with_right)
        {
            drop(state);
            self.activate();
            return;
        }
        if key.code == KeyCode::Esc && !state.composer_owns_escape() {
            state.focus = AgentsOverviewFocus::List;
            return;
        }
        let Some(composer) = state.composer.as_mut() else {
            return;
        };
        if key.kind == KeyEventKind::Press
            && matches!(key.code, KeyCode::Char('v' | 'V'))
            && key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            match paste_image_to_temp_png() {
                Ok((path, _)) => composer.attach_image(path),
                Err(error) => self
                    .app_event_tx
                    .send(AppEvent::AgentsOverviewError(format!(
                        "Failed to paste image: {error}"
                    ))),
            }
            return;
        }
        if offline
            && !composer.popup_active()
            && (self.composer_keymap.submit.is_pressed(key)
                || self.composer_keymap.queue.is_pressed(key))
        {
            if key.code == KeyCode::Enter {
                composer.handle_paste_enter(std::time::Instant::now());
            }
            return;
        }
        let (result, _) = composer.handle_key_event(key);
        let prompt = if let InputResult::Submitted {
            text,
            text_elements,
        } = result
        {
            Some(UserMessage {
                text,
                text_elements,
                local_images: composer.take_recent_submission_images_with_placeholders(),
                remote_image_urls: Vec::new(),
                mention_bindings: Vec::new(),
            })
        } else {
            None
        };
        drop(state);
        if let Some(prompt) = prompt {
            self.app_event_tx
                .send(AppEvent::DispatchAgentsOverviewTask {
                    prompt,
                    cwd: (grouping == AgentsOverviewGrouping::Project)
                        .then(|| self.selected_row().map(|row| row.thread.cwd.clone()))
                        .flatten(),
                });
        }
    }
    pub(super) fn layout_areas(&self, area: Rect) -> [Rect; 7] {
        let header_height = self
            .state()
            .server_version_notice
            .as_deref()
            .map(|notice| {
                textwrap::wrap(notice, usize::from(area.width.saturating_sub(4).max(1))).len()
                    as u16
            })
            .unwrap_or(1)
            .min(area.height.saturating_sub(7).max(1));
        let footer_height = if self.state().composing() {
            0
        } else {
            (self.footer_lines(area.width.saturating_sub(4)).len() as u16)
                .min(area.height.saturating_sub(7))
        };
        let mut state = self.state();
        let composing = state.composing();
        let mut hints = if state.connection_notice.is_some() && composing {
            vec![("esc".to_string(), "tasks · dispatch paused".to_string())]
        } else if composing {
            self.composer_hints.clone()
        } else {
            Vec::new()
        };
        if let Some((key, _)) = hints.last_mut()
            && state.composer_owns_escape()
        {
            *key = "esc esc".to_string();
        }
        if composing
            && state.connection_notice.is_none()
            && state
                .composer
                .as_ref()
                .is_some_and(ChatComposer::can_leave_empty_prompt_with_right)
            && self.rows.get(self.selected).is_some()
        {
            hints.push(("→".to_string(), "open task".to_string()));
        }
        if area.width < 60 && hints.len() > 2 {
            hints.remove(/*index*/ 1);
        }
        if composing && let Some(override_hints) = &state.key_chord_hint {
            hints = override_hints.clone();
        }
        let metadata = state.editing_metadata();
        let input_height = if metadata {
            1
        } else if let Some(composer) = state.composer.as_mut() {
            composer.set_footer_hint_override(Some(hints));
            composer
                .desired_height(area.width)
                .min((area.height / 3).max(/*other*/ 5))
                .min(area.height.saturating_sub(/*rhs*/ 7))
                .max(/*other*/ 3)
        } else {
            3
        };
        Layout::vertical([
            Constraint::Length(header_height),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(u16::from(!metadata)),
            Constraint::Length(input_height),
            Constraint::Length(footer_height),
        ])
        .areas(area)
    }
}
