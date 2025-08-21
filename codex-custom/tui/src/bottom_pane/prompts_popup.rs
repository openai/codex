use std::fs;
use std::path::PathBuf;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Stylize;
use ratatui::text::Line as RLine;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::StatefulWidgetRef;
use ratatui::widgets::Widget;
use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;

use super::BottomPane;
use super::CancellationEvent;
use super::bottom_pane_view::BottomPaneView;
use super::popup_consts::MAX_POPUP_ROWS;
use super::scroll_state::ScrollState;
use super::selection_popup_common::GenericDisplayRow;
use super::selection_popup_common::render_rows;
use super::textarea::TextArea;
use super::textarea::TextAreaState;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SavedPrompt {
    id: Uuid,
    title: String,
    body: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    List,
    AddTitle,
    AddBody,
}

pub(crate) struct PromptsPopup {
    state: ScrollState,
    saved: Vec<SavedPrompt>,
    filter: String,
    mode: Mode,
    new_title: String,
    body_editor: TextArea,
    body_state: std::cell::RefCell<TextAreaState>,
    complete: bool,
    codex_home: PathBuf,
    _sender: crate::app_event_sender::AppEventSender,
}

fn store_path(codex_home: &PathBuf) -> PathBuf {
    codex_home.join("prompts.json")
}

fn load_saved_prompts(codex_home: &PathBuf) -> Vec<SavedPrompt> {
    let path = store_path(codex_home);
    let Ok(data) = fs::read(&path) else {
        return Vec::new();
    };
    serde_json::from_slice::<Vec<SavedPrompt>>(&data).unwrap_or_default()
}

fn save_prompts(codex_home: &PathBuf, prompts: &[SavedPrompt]) -> std::io::Result<()> {
    let path = store_path(codex_home);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_vec_pretty(prompts).unwrap_or_else(|_| b"[]".to_vec());
    fs::write(path, data)
}

impl PromptsPopup {
    pub(crate) fn new(
        sender: crate::app_event_sender::AppEventSender,
        codex_home: PathBuf,
    ) -> Self {
        let mut s = ScrollState::new();
        let saved = load_saved_prompts(&codex_home);
        s.clamp_selection(saved.len());
        s.ensure_visible(saved.len(), MAX_POPUP_ROWS.min(saved.len()));
        Self {
            state: s,
            saved,
            filter: String::new(),
            mode: Mode::List,
            new_title: String::new(),
            body_editor: TextArea::new(),
            body_state: std::cell::RefCell::new(TextAreaState::default()),
            complete: false,
            codex_home,
            _sender: sender,
        }
    }

    fn filtered_indices(&self) -> Vec<usize> {
        if self.filter.is_empty() {
            return (0..self.saved.len()).collect();
        }
        let needle = self.filter.to_lowercase();
        self.saved
            .iter()
            .enumerate()
            .filter_map(|(i, p)| {
                let title = p.title.to_lowercase();
                let body_preview = p.body.to_lowercase();
                if title.contains(&needle) || body_preview.contains(&needle) {
                    Some(i)
                } else {
                    None
                }
            })
            .collect()
    }

    fn to_rows(&self) -> Vec<GenericDisplayRow> {
        let filtered = self.filtered_indices();
        filtered
            .iter()
            .enumerate()
            .map(|(idx, &orig)| {
                let p = &self.saved[orig];
                let name = p.title.clone();
                let desc = first_line_preview(&p.body);
                GenericDisplayRow {
                    name,
                    match_indices: None,
                    is_current: idx == 0,
                    description: Some(desc),
                }
            })
            .collect()
    }

    fn select_current_index(&self) -> Option<usize> {
        let filtered = self.filtered_indices();
        if filtered.is_empty() {
            return None;
        }
        let sel = self
            .state
            .selected_idx
            .unwrap_or(0)
            .min(filtered.len().saturating_sub(1));
        filtered.get(sel).copied()
    }
}

impl<'a> BottomPaneView<'a> for PromptsPopup {
    fn handle_key_event(&mut self, pane: &mut BottomPane<'a>, key_event: KeyEvent) {
        match self.mode {
            Mode::List => match key_event {
                KeyEvent {
                    code: KeyCode::Up, ..
                } => {
                    let len = self.filtered_indices().len();
                    self.state.move_up_wrap(len);
                    self.state.ensure_visible(len, MAX_POPUP_ROWS.min(len));
                }
                KeyEvent {
                    code: KeyCode::Down,
                    ..
                } => {
                    let len = self.filtered_indices().len();
                    self.state.move_down_wrap(len);
                    self.state.ensure_visible(len, MAX_POPUP_ROWS.min(len));
                }
                KeyEvent {
                    code: KeyCode::Char('n'),
                    modifiers: KeyModifiers::NONE,
                    ..
                } => {
                    self.mode = Mode::AddTitle;
                    self.new_title.clear();
                }
                KeyEvent {
                    code: KeyCode::Char('d'),
                    modifiers: KeyModifiers::NONE,
                    ..
                } => {
                    if let Some(idx) = self.select_current_index() {
                        self.saved.remove(idx);
                        let _ = save_prompts(&self.codex_home, &self.saved);
                        let len = self.filtered_indices().len();
                        self.state.clamp_selection(len);
                        self.state.ensure_visible(len, MAX_POPUP_ROWS.min(len));
                    }
                }
                KeyEvent {
                    code: KeyCode::Enter,
                    ..
                } => {
                    if let Some(idx) = self.select_current_index() {
                        let body = self.saved[idx].body.clone();
                        pane.set_composer_text(body);
                    }
                    self.complete = true;
                }
                KeyEvent {
                    code: KeyCode::Esc, ..
                } => {
                    self.complete = true;
                }
                KeyEvent {
                    code: KeyCode::Backspace,
                    ..
                } => {
                    self.filter.pop();
                }
                KeyEvent {
                    code: KeyCode::Char(c),
                    modifiers: KeyModifiers::NONE,
                    ..
                } => {
                    if !c.is_control() {
                        self.filter.push(c);
                    }
                }
                _ => {}
            },
            Mode::AddTitle => match key_event {
                KeyEvent {
                    code: KeyCode::Esc, ..
                } => {
                    self.mode = Mode::List;
                }
                KeyEvent {
                    code: KeyCode::Enter,
                    ..
                } => {
                    if !self.new_title.trim().is_empty() {
                        self.mode = Mode::AddBody;
                        self.body_editor.set_text("");
                        self.body_editor.set_cursor(0);
                    }
                }
                KeyEvent {
                    code: KeyCode::Backspace,
                    ..
                } => {
                    self.new_title.pop();
                }
                KeyEvent {
                    code: KeyCode::Char(c),
                    modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                    ..
                } => {
                    if !c.is_control() {
                        self.new_title.push(c);
                    }
                }
                _ => {}
            },
            Mode::AddBody => match key_event {
                KeyEvent {
                    code: KeyCode::Esc, ..
                } => {
                    self.mode = Mode::List;
                }
                KeyEvent {
                    code: KeyCode::Char('s'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    let title = self.new_title.trim().to_string();
                    let body = self.body_editor.text().to_string();
                    if !title.is_empty() && !body.trim().is_empty() {
                        self.saved.push(SavedPrompt {
                            id: Uuid::new_v4(),
                            title,
                            body,
                        });
                        let _ = save_prompts(&self.codex_home, &self.saved);
                        // Reset and return to list
                        self.mode = Mode::List;
                        self.new_title.clear();
                        self.filter.clear();
                        let len = self.filtered_indices().len();
                        self.state.clamp_selection(len);
                        self.state.ensure_visible(len, MAX_POPUP_ROWS.min(len));
                    }
                }
                other => {
                    // Forward other keys to the embedded textarea editor.
                    let _ = other; // silence unused warning in match arms
                    self.body_editor.input(key_event);
                }
            },
        }
        pane.request_redraw();
    }

    fn is_complete(&self) -> bool {
        self.complete
    }

    fn on_ctrl_c(&mut self, _pane: &mut BottomPane<'a>) -> CancellationEvent {
        self.complete = true;
        CancellationEvent::Handled
    }

    fn desired_height(&self, _width: u16) -> u16 {
        match self.mode {
            Mode::List => (MAX_POPUP_ROWS as u16).saturating_add(2),
            Mode::AddTitle => 3,
            Mode::AddBody => 6,
        }
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        match self.mode {
            Mode::List => {
                // Header + filter
                let header = RLine::from(vec![
                    Span::raw("saved prompts "),
                    Span::raw(format!("({})", self.saved.len())).dim(),
                ]);
                Paragraph::new(header).render(
                    Rect {
                        x: area.x,
                        y: area.y,
                        width: area.width,
                        height: 1,
                    },
                    buf,
                );

                let filter_line = if self.filter.is_empty() {
                    RLine::from(vec![
                        Span::raw("filter: "),
                        Span::raw("(type to search; n=new, d=delete, Enter=insert, Esc=close)")
                            .dim(),
                    ])
                } else {
                    RLine::from(vec![
                        Span::raw("filter: "),
                        Span::raw(self.filter.clone()).bold(),
                    ])
                };
                Paragraph::new(filter_line).render(
                    Rect {
                        x: area.x,
                        y: area.y.saturating_add(1),
                        width: area.width,
                        height: 1,
                    },
                    buf,
                );

                let list_area = Rect {
                    x: area.x,
                    y: area.y.saturating_add(2),
                    width: area.width,
                    height: area.height.saturating_sub(2),
                };
                let rows = self.to_rows();
                render_rows(list_area, buf, &rows, &self.state, MAX_POPUP_ROWS);
            }
            Mode::AddTitle => {
                Paragraph::new(RLine::from(
                    "new prompt title (Enter to continue, Esc to cancel)"
                        .to_string()
                        .magenta(),
                ))
                .render(
                    Rect {
                        x: area.x,
                        y: area.y,
                        width: area.width,
                        height: 1,
                    },
                    buf,
                );
                Paragraph::new(RLine::from(self.new_title.clone())).render(
                    Rect {
                        x: area.x,
                        y: area.y.saturating_add(1),
                        width: area.width,
                        height: 1,
                    },
                    buf,
                );
            }
            Mode::AddBody => {
                Paragraph::new(RLine::from(
                    "prompt body (Ctrl+S to save, Esc to cancel)"
                        .to_string()
                        .magenta(),
                ))
                .render(
                    Rect {
                        x: area.x,
                        y: area.y,
                        width: area.width,
                        height: 1,
                    },
                    buf,
                );
                // Draw an editor box for the body
                let editor_area = Rect {
                    x: area.x,
                    y: area.y.saturating_add(1),
                    width: area.width,
                    height: area.height.saturating_sub(1),
                };
                let mut state = self.body_state.borrow_mut();
                StatefulWidgetRef::render_ref(&(&self.body_editor), editor_area, buf, &mut *state);
            }
        }
    }

    fn should_hide_when_task_is_done(&mut self) -> bool {
        false
    }
}

fn first_line_preview(text: &str) -> String {
    let mut line = text
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim()
        .to_string();
    const MAX_PREVIEW_CHARS: usize = 60;
    if line.chars().count() > MAX_PREVIEW_CHARS {
        let mut s = String::new();
        for ch in line.chars().take(MAX_PREVIEW_CHARS - 1) {
            s.push(ch);
        }
        s.push('…');
        line = s;
    }
    line
}
