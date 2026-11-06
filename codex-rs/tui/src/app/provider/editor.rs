//! Single-screen provider form. Validation preserves the draft and each field owns its editing history.

use super::Field;
use crate::app::App;
use crate::bottom_pane::BottomPaneView;
use crate::bottom_pane::CancellationEvent;
use crate::bottom_pane::TextArea;
use crate::bottom_pane::ViewCompletion;
use crate::key_hint;
use crate::keymap::KeymapContextSet;
use crate::line_truncation::truncate_line_to_width;
use crate::onboarding::FieldDisplay;
use crate::onboarding::ProviderInput;
use crate::onboarding::move_provider_field;
use crate::render::renderable::Renderable;
use crate::wrapping::word_wrap_lines;
use codex_app_server_protocol::ModelProviderConfigWriteParams;
use crossterm::cursor::SetCursorStyle;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;
use std::sync::MutexGuard;

mod form;
pub(crate) use form::Draft;

pub(super) const VIEW_ID: &str = "model_provider_editor";

pub(crate) fn view(
    app: &App,
    draft: Draft,
    error: Option<String>,
    on_save: impl Fn(Draft, ModelProviderConfigWriteParams) + Send + 'static,
) -> Box<dyn BottomPaneView> {
    let mut inputs = Vec::new();
    for (field, original) in draft.fields() {
        let input = ProviderInput::from(draft.value(field, original));
        let mut editor = input.editor();
        editor.set_keymap_bindings(&app.keymap);
        if app.chat_widget.composer_is_vim_enabled() {
            editor.set_vim_enabled(/*enabled*/ true);
            editor.enter_vim_insert_mode();
        }
        drop(editor);
        inputs.push((field, input));
    }
    Box::new(Editor {
        inputs,
        draft,
        on_save: Box::new(on_save),
        error,
        completion: None,
    })
}

struct Editor {
    inputs: Vec<(Field, ProviderInput)>,
    draft: Draft,
    on_save: Box<dyn Fn(Draft, ModelProviderConfigWriteParams) + Send>,
    error: Option<String>,
    completion: Option<ViewCompletion>,
}

impl Editor {
    fn active_input(&self) -> Option<MutexGuard<'_, TextArea>> {
        self.inputs
            .get(self.draft.selected)
            .map(|(_, input)| input.editor())
    }

    fn changes(&self) -> impl Iterator<Item = (Field, String)> {
        self.draft
            .fields()
            .into_iter()
            .zip(&self.inputs)
            .filter_map(|((field, original), (_, input))| {
                let value = input.value().unwrap_or_default();
                // A single-line buffer strips line breaks from legacy values on display.
                let displayed = original.unwrap_or_default().replace(['\r', '\n'], "");
                (value != displayed.trim()).then_some((field, value))
            })
    }

    fn save(&mut self) {
        self.draft.changes = self.changes().collect();
        match self.draft.patch() {
            Ok(params) => (self.on_save)(self.draft.clone(), params),
            Err(error) => self.error = Some(error.to_owned()),
        }
    }

    fn suggestion(&self) -> Option<&str> {
        let (Field::Variable, input) = self.inputs.get(self.draft.selected)? else {
            return None;
        };
        let editor = input.editor();
        let prefix = editor.text();
        if prefix.is_empty() || editor.cursor() != prefix.len() {
            return None;
        }
        self.draft
            .variables
            .iter()
            .map(String::as_str)
            .find(|name| *name != prefix && name.starts_with(prefix))
    }

    fn layout(&self, width: u16) -> (Paragraph<'static>, Option<(u16, u16)>) {
        let title = self.draft.provider.as_ref().map_or_else(
            || "Add custom provider".to_owned(),
            |p| format!("Edit {}", p.name),
        );
        let wrap_width = usize::from(width.max(1));
        let mut lines = word_wrap_lines([Line::from(title.bold())], wrap_width);
        lines.push(Line::default());
        let mut cursor = None;
        let key_needs_url = !self.draft.required(Field::Url)
            && self
                .changes()
                .any(|(field, value)| field == Field::Variable && !value.is_empty());
        for (index, (field, input)) in self.inputs.iter().enumerate() {
            let focused = index == self.draft.selected;
            let marker = if focused { "› " } else { "  " };
            let required = if self.draft.required(*field) || (*field == Field::Url && key_needs_url)
            {
                "*"
            } else {
                ""
            };
            let label = format!("{marker}{}{required}  ", field.label()).bold();
            let example = match field {
                Field::Name => "Team gateway",
                Field::Url => "https://gateway.example/v1",
                Field::Variable => "TEAM_API_KEY",
                _ => "",
            };
            let mut line = Line::from(label);
            // Put the value on its own row when the label would crowd the cursor.
            if line.width() + 8 > usize::from(width) {
                lines.extend(word_wrap_lines([line], wrap_width));
                line = Line::default();
            }
            if *field == Field::Variable && width > 1 {
                line.push_span("$".dim());
            }
            let display = if focused {
                FieldDisplay::Focused
            } else {
                FieldDisplay::Unfocused
            };
            let x = line.width() as u16;
            let area = Rect::new(x, lines.len() as u16, width.saturating_sub(x), 1);
            if focused {
                cursor = input.editor().cursor_pos(area);
            }
            let value = input.line("", example, display, area.width);
            line.spans.extend(value.spans);
            lines.push(truncate_line_to_width(line, usize::from(width)));
        }
        lines.push(if self.draft.selected == self.inputs.len() {
            "› Save".bold().into()
        } else {
            "  Save".into()
        });
        lines.push(Line::default());
        let has = |field| self.inputs.iter().any(|(key, _)| *key == field);
        if has(Field::Name) {
            let hint = if has(Field::Variable) {
                "Enter the environment variable name on the app server."
            } else {
                "Authentication is managed on the app server."
            };
            lines.extend(["* Required", hint].map(|text| Line::from(text.dim())));
        }
        if let Some(suggestion) = self.suggestion() {
            lines.push(Line::from(vec![
                key_hint::ctrl(KeyCode::Char(' ')).into(),
                format!(" complete {suggestion}").dim(),
            ]));
        }
        if let Some(error) = &self.error {
            lines.push(error.clone().red().into());
        }
        lines.push(Line::from(vec![
            key_hint::plain(KeyCode::Tab).into(),
            " move · ".dim(),
            key_hint::plain(KeyCode::Enter).into(),
            " next/save · ".dim(),
            key_hint::plain(KeyCode::Esc).into(),
            if self.prefer_esc_to_handle_key_event() {
                " normal mode"
            } else {
                " back"
            }
            .dim(),
        ]));
        (Paragraph::new(lines).wrap(Wrap { trim: false }), cursor)
    }
}

impl Renderable for Editor {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.layout(area.width).0.render(area, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.layout(width).0.desired_height(width)
    }

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        let (x, y) = self.layout(area.width).1?;
        (y < area.height).then(|| (area.x.saturating_add(x), area.y.saturating_add(y)))
    }

    fn cursor_style(&self, _area: Rect) -> SetCursorStyle {
        if self
            .active_input()
            .is_some_and(|editor| editor.uses_vim_insert_cursor())
        {
            SetCursorStyle::SteadyBar
        } else {
            SetCursorStyle::DefaultUserShape
        }
    }
}

impl BottomPaneView for Editor {
    fn view_id(&self) -> Option<&'static str> {
        Some(VIEW_ID)
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        if move_provider_field(key, &mut self.draft.selected, self.inputs.len() + 1) {
            return;
        }
        // Holding Enter on a field must not carry its activation onto Save.
        if key.kind == KeyEventKind::Press && key_hint::plain(KeyCode::Enter).is_press(key) {
            if self.draft.selected == self.inputs.len() {
                self.save();
            } else {
                self.draft.selected += 1;
            }
            return;
        }
        if key_hint::ctrl(KeyCode::Char(' ')).is_press(key)
            && let Some(suggestion) = self.suggestion()
        {
            let mut editor = self.inputs[self.draft.selected].1.editor();
            let suffix = &suggestion[editor.cursor()..];
            editor.insert_str(suffix);
            return;
        }
        if let Some((_, input)) = self.inputs.get_mut(self.draft.selected) {
            input.input(key);
        }
    }

    fn handle_paste(&mut self, pasted: String) -> bool {
        if let Some((_, input)) = self.inputs.get_mut(self.draft.selected) {
            input.paste(&pasted);
            return true;
        }
        false
    }

    fn keymap_contexts(&self) -> KeymapContextSet {
        self.active_input()
            .map(|editor| KeymapContextSet::new(editor.keymap_context()))
            .unwrap_or_default()
    }

    fn prefer_esc_to_handle_key_event(&self) -> bool {
        self.active_input()
            .is_some_and(|editor| editor.should_handle_vim_insert_escape(KeyCode::Esc.into()))
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        self.completion = Some(ViewCompletion::Cancelled);
        CancellationEvent::Handled
    }

    fn is_complete(&self) -> bool {
        self.completion.is_some()
    }

    fn completion(&self) -> Option<ViewCompletion> {
        self.completion
    }
}

#[cfg(test)]
#[path = "editor_tests.rs"]
mod tests;
