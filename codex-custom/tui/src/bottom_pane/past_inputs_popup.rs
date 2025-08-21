use std::fs;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::path::Path;
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
use ratatui::widgets::Widget;
use serde::Deserialize;
use uuid::Uuid;

use super::BottomPane;
use super::CancellationEvent;
use super::bottom_pane_view::BottomPaneView;
use super::popup_consts::MAX_POPUP_ROWS;
use super::scroll_state::ScrollState;
use super::selection_popup_common::GenericDisplayRow;
use super::selection_popup_common::render_rows;
use crate::app_event::AppEvent;

#[derive(Debug, Clone, Deserialize)]
struct MetaLine {
    pub id: Uuid,
    // The rollout meta uses the JSON key "timestamp"; we don't need it here,
    // but include it to make deserialization succeed when scanning files.
    #[serde(default, rename = "timestamp")]
    pub _timestamp: Option<String>,
}

#[derive(Debug, Clone)]
struct PastInputEntry {
    line_index: usize,
    text: String,
}

// no-op timestamp parsing in this module; retained for symmetry with resume popup

fn find_rollout_for_session(codex_home: &Path, session_id: Uuid) -> Option<PathBuf> {
    let sessions_dir = codex_home.join("sessions");
    let mut matches: Vec<(PathBuf, Option<std::time::SystemTime>)> = Vec::new();
    let mut stack = vec![sessions_dir];
    while let Some(dir) = stack.pop() {
        if !dir.exists() {
            continue;
        }
        let Ok(read) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in read.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if !file_name.starts_with("rollout-") || !file_name.ends_with(".jsonl") {
                continue;
            }
            // read first line
            if let Ok(file) = fs::File::open(&path) {
                let mut reader = BufReader::new(file);
                let mut line = String::new();
                if reader.read_line(&mut line).is_ok() {
                    if let Ok(meta) = serde_json::from_str::<MetaLine>(&line) {
                        if meta.id == session_id {
                            let mtime = fs::metadata(&path).ok().and_then(|m| m.modified().ok());
                            matches.push((path.clone(), mtime));
                        }
                    }
                }
            }
        }
    }
    // Pick most recently modified file
    matches
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1))
        .map(|(p, _)| p)
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentItemLocal {
    InputText {
        text: String,
    },
    OutputText {
        text: String,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum RolloutItemLocal {
    Message {
        role: String,
        content: Vec<ContentItemLocal>,
    },
    #[serde(other)]
    Other,
}

fn load_user_inputs_from_rollout(path: &Path) -> Vec<PastInputEntry> {
    let mut out = Vec::new();
    let Ok(f) = fs::File::open(path) else {
        return out;
    };
    let reader = BufReader::new(f);
    for (idx, line_res) in reader.lines().enumerate() {
        let Ok(line) = line_res else { continue };
        if idx == 0 {
            continue;
        } // meta line
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
            if v.get("record_type").and_then(|rt| rt.as_str()) == Some("state") {
                continue;
            }
        }
        let Ok(item) = serde_json::from_str::<RolloutItemLocal>(&line) else {
            continue;
        };
        match item {
            RolloutItemLocal::Message { role, content } if role == "user" => {
                let mut text = String::new();
                for c in content {
                    match c {
                        ContentItemLocal::InputText { text: t }
                        | ContentItemLocal::OutputText { text: t } => {
                            if !text.is_empty() {
                                text.push('\n');
                            }
                            text.push_str(&t);
                        }
                        _ => {}
                    }
                }
                if !text.trim().is_empty() {
                    out.push(PastInputEntry {
                        line_index: idx,
                        text,
                    });
                }
            }
            _ => {}
        }
    }
    out
}

fn ensure_parent_dir(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn create_branch_rollout(
    codex_home: &Path,
    src: &Path,
    truncate_before: usize,
) -> std::io::Result<PathBuf> {
    // Place branch alongside sessions directory with a distinct prefix.
    let ts = chrono::Utc::now().format("%Y-%m-%dT%H-%M-%S");
    let dest_name = format!("rollout-branch-{}.jsonl", ts);
    let dest_path = codex_home.join("sessions").join(dest_name);
    ensure_parent_dir(&dest_path)?;

    let mut in_file = BufReader::new(fs::File::open(src)?);
    let mut out_file = fs::File::create(&dest_path)?;
    let mut line = String::new();
    let mut idx = 0usize;
    loop {
        line.clear();
        let n = in_file.read_line(&mut line)?;
        if n == 0 {
            break;
        }
        if idx >= truncate_before {
            break;
        }
        out_file.write_all(line.as_bytes())?;
        idx += 1;
    }
    out_file.flush()?;
    Ok(dest_path)
}

pub(crate) struct PastInputsPopup {
    state: ScrollState,
    filtered_indices: Vec<usize>,
    entries: Vec<PastInputEntry>,
    filter: String,
    complete: bool,
    sender: crate::app_event_sender::AppEventSender,
    session_rollout_path: Option<PathBuf>,
    codex_home: PathBuf,
}

impl PastInputsPopup {
    pub(crate) fn new(
        sender: crate::app_event_sender::AppEventSender,
        codex_home: PathBuf,
        session_id: Uuid,
    ) -> Self {
        let mut s = ScrollState::new();
        let rollout_path = find_rollout_for_session(&codex_home, session_id);
        let entries = rollout_path
            .as_ref()
            .map(|p| load_user_inputs_from_rollout(p))
            .unwrap_or_default();

        let filtered_indices = (0..entries.len()).collect::<Vec<_>>();
        s.clamp_selection(filtered_indices.len());
        s.ensure_visible(
            filtered_indices.len(),
            MAX_POPUP_ROWS.min(filtered_indices.len()),
        );
        Self {
            state: s,
            filtered_indices,
            entries,
            filter: String::new(),
            complete: false,
            sender,
            session_rollout_path: rollout_path,
            codex_home,
        }
    }

    fn to_rows(&self) -> Vec<GenericDisplayRow> {
        self.filtered_indices
            .iter()
            .enumerate()
            .map(|(idx, &orig)| {
                let e = &self.entries[orig];
                let name = first_line_preview(&e.text);
                let match_indices_vec = match_indices(&name, &self.filter);
                GenericDisplayRow {
                    name,
                    match_indices: match_indices_vec,
                    is_current: idx == 0,
                    description: Some(format!("line {}", e.line_index)),
                }
            })
            .collect()
    }

    fn apply_filter(&mut self) {
        if self.filter.is_empty() {
            self.filtered_indices = (0..self.entries.len()).collect();
        } else {
            let mut out = Vec::new();
            for (i, e) in self.entries.iter().enumerate() {
                if e.text.to_lowercase().contains(&self.filter.to_lowercase()) {
                    out.push(i);
                }
            }
            self.filtered_indices = out;
        }
        self.state.clamp_selection(self.filtered_indices.len());
        self.state.ensure_visible(
            self.filtered_indices.len(),
            MAX_POPUP_ROWS.min(self.filtered_indices.len()),
        );
    }

    fn select_current(&self) -> Option<&PastInputEntry> {
        if self.filtered_indices.is_empty() {
            return None;
        }
        let sel = self
            .state
            .selected_idx
            .unwrap_or(0)
            .min(self.filtered_indices.len().saturating_sub(1));
        self.filtered_indices
            .get(sel)
            .and_then(|&orig| self.entries.get(orig))
    }
}

impl<'a> BottomPaneView<'a> for PastInputsPopup {
    fn handle_key_event(&mut self, pane: &mut BottomPane<'a>, key_event: KeyEvent) {
        match key_event {
            KeyEvent {
                code: KeyCode::Up, ..
            } => {
                let len = self.filtered_indices.len();
                self.state.move_up_wrap(len);
                self.state.ensure_visible(len, MAX_POPUP_ROWS.min(len));
            }
            KeyEvent {
                code: KeyCode::Down,
                ..
            } => {
                let len = self.filtered_indices.len();
                self.state.move_down_wrap(len);
                self.state.ensure_visible(len, MAX_POPUP_ROWS.min(len));
            }
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                if self.filter.is_empty() {
                    self.complete = true;
                } else {
                    self.filter.clear();
                    self.apply_filter();
                }
            }
            KeyEvent {
                code: KeyCode::Backspace,
                ..
            } => {
                self.filter.pop();
                self.apply_filter();
            }
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if !c.is_control() {
                    self.filter.push(c);
                    self.apply_filter();
                }
            }
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if let (Some(sel), Some(src)) =
                    (self.select_current(), self.session_rollout_path.as_ref())
                {
                    match create_branch_rollout(&self.codex_home, src, sel.line_index) {
                        Ok(dest) => {
                            self.sender.send(AppEvent::BranchToPastInput {
                                resume_path: dest,
                                prefill_text: sel.text.clone(),
                            });
                        }
                        Err(err) => {
                            let mut lines = Vec::new();
                            lines.push(
                                RLine::from(format!("failed to create branch: {}", err)).red(),
                            );
                            lines.push(RLine::from(""));
                            pane.send_app_event(AppEvent::InsertHistory(lines));
                        }
                    }
                    self.complete = true;
                } else {
                    self.complete = true;
                }
            }
            _ => {}
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
        let rows = self.filtered_indices.len().clamp(1, MAX_POPUP_ROWS) as u16;
        rows.saturating_add(2)
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        // Header with scope
        let header = if let Some(path) = &self.session_rollout_path {
            RLine::from(vec![
                Span::raw("past inputs for session "),
                Span::raw(path.file_name().and_then(|s| s.to_str()).unwrap_or("")).dim(),
            ])
        } else {
            RLine::from("no session transcript found").red()
        };
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
                Span::raw("search: "),
                Span::raw("(type to filter; Esc clears)").dim(),
            ])
        } else {
            RLine::from(vec![
                Span::raw("search: "),
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
    const MAX_PREVIEW_CHARS: usize = 80;
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

fn match_indices(haystack: &str, needle: &str) -> Option<Vec<usize>> {
    if needle.is_empty() {
        return None;
    }
    let hay: Vec<char> = haystack.to_lowercase().chars().collect();
    let needle: Vec<char> = needle.to_lowercase().chars().collect();
    if needle.is_empty() || needle.len() > hay.len() {
        return None;
    }
    for start in 0..=hay.len() - needle.len() {
        if hay[start..start + needle.len()] == needle[..] {
            return Some((start..start + needle.len()).collect());
        }
    }
    None
}
