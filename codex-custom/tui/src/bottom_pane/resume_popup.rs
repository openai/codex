use std::fs;
use std::io::BufRead;
use std::io::BufReader;
use std::path::Path;
use std::path::PathBuf;

use chrono::DateTime;
use chrono::Utc;
use codex_core::git_info::GitInfo;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Stylize;
use ratatui::text::Line as RLine;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

use super::BottomPane;
use super::CancellationEvent;
use super::bottom_pane_view::BottomPaneView;
use super::popup_consts::MAX_POPUP_ROWS;
use super::scroll_state::ScrollState;
use crate::app_event::AppEvent;
use crate::bottom_pane::selection_popup_common::GenericDisplayRow;
use crate::bottom_pane::selection_popup_common::render_rows;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
struct ResumeMetaLine {
    pub id: String,
    pub timestamp: String,
    #[serde(default)]
    pub git: Option<GitInfo>,
    #[serde(default)]
    pub cwd: Option<String>,
}

#[derive(Debug, Clone)]
struct ResumeEntry {
    path: PathBuf,
    timestamp: String,
    mtime: Option<std::time::SystemTime>,
    branch: Option<String>,
    commit_short: Option<String>,
    repo_url: Option<String>,
}

fn scan_rollouts(codex_home: &Path, filter_cwd: Option<&Path>) -> Vec<ResumeEntry> {
    let mut entries: Vec<ResumeEntry> = Vec::new();
    let sessions_dir = codex_home.join("sessions");
    let walker = walk_flatten(&sessions_dir);
    for path in walker {
        if !is_rollout_file(&path) {
            continue;
        }
        // Read first line for metadata
        let meta = read_first_line(&path)
            .and_then(|line| serde_json::from_str::<ResumeMetaLine>(&line).ok());

        // Optionally enforce directory scoping: only include rollouts whose recorded cwd
        // matches the user's current working directory when a filter is provided.
        if let Some(current_cwd) = filter_cwd {
            let cwd_matches = meta
                .as_ref()
                .and_then(|m| m.cwd.as_ref())
                .map(|m_cwd| Path::new(m_cwd) == current_cwd)
                .unwrap_or(false);
            if !cwd_matches {
                continue;
            }
        }

        let (timestamp, branch, commit_short, repo_url) = match meta {
            Some(m) => {
                let commit_short = m
                    .git
                    .as_ref()
                    .and_then(|g| g.commit_hash.clone())
                    .map(|s| s.chars().take(7).collect());
                (
                    m.timestamp,
                    m.git.as_ref().and_then(|g| g.branch.clone()),
                    commit_short,
                    m.git.as_ref().and_then(|g| g.repository_url.clone()),
                )
            }
            None => (String::from("unknown"), None, None, None),
        };

        let mtime = fs::metadata(&path).ok().and_then(|m| m.modified().ok());
        entries.push(ResumeEntry {
            path,
            timestamp,
            mtime,
            branch,
            commit_short,
            repo_url,
        });
    }

    // Sort by mtime desc, fallback to timestamp desc
    entries.sort_by(|a, b| match (a.mtime, b.mtime) {
        (Some(ta), Some(tb)) => tb.cmp(&ta),
        _ => parse_ts(&b.timestamp).cmp(&parse_ts(&a.timestamp)),
    });
    entries
}

fn parse_ts(ts: &str) -> Option<DateTime<Utc>> {
    // Try RFC3339 first, else None
    DateTime::parse_from_rfc3339(ts)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

fn is_rollout_file(p: &Path) -> bool {
    match p.file_name().and_then(|n| n.to_str()) {
        Some(name) => name.starts_with("rollout-") && name.ends_with(".jsonl"),
        None => false,
    }
}

fn walk_flatten(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let _ = visit_dirs(root, &mut out);
    out
}

fn visit_dirs(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let _ = visit_dirs(&path, out);
        } else {
            out.push(path);
        }
    }
    Ok(())
}

fn read_first_line(path: &Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    if line.trim().is_empty() {
        None
    } else {
        Some(line)
    }
}

pub(crate) struct ResumePopup {
    state: ScrollState,
    entries: Vec<ResumeEntry>,
    filtered_indices: Vec<usize>,
    filter: String,
    complete: bool,
    sender: crate::app_event_sender::AppEventSender,
    show_all: bool,
    codex_home: PathBuf,
    cwd: PathBuf,
}

impl ResumePopup {
    fn match_indices_in_ts(&self, ts: &str) -> Option<Vec<usize>> {
        if self.filter.is_empty() {
            return None;
        }
        let _hay: Vec<char> = ts.chars().collect();
        let hay_lc: Vec<char> = ts.to_lowercase().chars().collect();
        let needle_lc: Vec<char> = self.filter.to_lowercase().chars().collect();
        if needle_lc.is_empty() || needle_lc.len() > hay_lc.len() {
            return None;
        }
        for start in 0..=hay_lc.len() - needle_lc.len() {
            if hay_lc[start..start + needle_lc.len()] == needle_lc[..] {
                return Some((start..start + needle_lc.len()).collect());
            }
        }
        None
    }
    pub(crate) fn new(
        sender: crate::app_event_sender::AppEventSender,
        codex_home: PathBuf,
        cwd: PathBuf,
    ) -> Self {
        let mut s = ScrollState::new();
        let entries = scan_rollouts(&codex_home, Some(&cwd));
        let filtered_indices = (0..entries.len()).collect::<Vec<_>>();
        s.clamp_selection(filtered_indices.len());
        s.ensure_visible(
            filtered_indices.len(),
            MAX_POPUP_ROWS.min(filtered_indices.len()),
        );
        Self {
            state: s,
            entries,
            filtered_indices,
            filter: String::new(),
            complete: false,
            sender,
            show_all: false,
            codex_home,
            cwd,
        }
    }

    fn refill_entries(&mut self) {
        self.entries = if self.show_all {
            scan_rollouts(&self.codex_home, None)
        } else {
            scan_rollouts(&self.codex_home, Some(&self.cwd))
        };
        // Reset indices and apply the current query filter
        self.filtered_indices = (0..self.entries.len()).collect();
        self.apply_filter();
    }

    fn to_rows(&self) -> Vec<GenericDisplayRow> {
        self.filtered_indices
            .iter()
            .enumerate()
            .map(|(idx, &orig_idx)| {
                let e = &self.entries[orig_idx];
                let ts = e
                    .timestamp
                    .replace('T', " ")
                    .trim_end_matches('Z')
                    .to_string();
                let name = format!("{}", ts);
                let mut desc_parts: Vec<String> = Vec::new();
                if let Some(b) = &e.branch {
                    desc_parts.push(b.clone());
                }
                if let Some(c) = &e.commit_short {
                    desc_parts.push(c.clone());
                }
                if let Some(url) = &e.repo_url {
                    desc_parts.push(url.clone());
                }
                let description = if desc_parts.is_empty() {
                    Some(e.path.display().to_string())
                } else {
                    Some(desc_parts.join(" · "))
                };
                GenericDisplayRow {
                    name,
                    match_indices: self.match_indices_in_ts(&ts),
                    is_current: idx == 0,
                    description,
                }
            })
            .collect::<Vec<_>>()
    }

    fn select_current(&self) -> Option<PathBuf> {
        self.state
            .selected_idx
            .and_then(|i| self.filtered_indices.get(i).cloned())
            .and_then(|orig| self.entries.get(orig))
            .map(|e| e.path.clone())
    }

    fn apply_filter(&mut self) {
        let q = self.filter.trim();
        if q.is_empty() {
            self.filtered_indices = (0..self.entries.len()).collect();
        } else {
            let q_lc = q.to_lowercase();
            let mut out = Vec::new();
            for (i, e) in self.entries.iter().enumerate() {
                let ts = &e.timestamp;
                let branch = e.branch.as_deref().unwrap_or("");
                let commit = e.commit_short.as_deref().unwrap_or("");
                let url = e.repo_url.as_deref().unwrap_or("");
                let path = e.path.display().to_string();
                let haystacks = [ts, branch, commit, url, &path];
                if haystacks.iter().any(|h| h.to_lowercase().contains(&q_lc)) {
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
}

impl<'a> BottomPaneView<'a> for ResumePopup {
    fn handle_key_event(&mut self, pane: &mut BottomPane<'a>, key_event: KeyEvent) {
        match key_event {
            KeyEvent {
                code: KeyCode::Tab, ..
            } => {
                self.show_all = !self.show_all;
                self.refill_entries();
            }
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
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if let Some(path) = self.select_current() {
                    self.sender.send(AppEvent::ResumeSelected(path));
                    self.complete = true;
                } else {
                    self.complete = true; // nothing to select -> close
                }
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
                    // Append printable ASCII to search filter
                    self.filter.push(c);
                    self.apply_filter();
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
        // Reserve two lines: filter banner + search input.
        rows.saturating_add(2)
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        // Render filter banner + search header line
        let scope_text = if self.show_all {
            RLine::from(vec![
                Span::raw("showing all sessions "),
                Span::raw("(press Tab to filter to current directory)").dim(),
            ])
        } else {
            let cwd_str = self.cwd.display().to_string();
            RLine::from(vec![
                Span::raw(format!("showing sessions for: {} ", cwd_str)).into(),
                Span::raw("(press Tab to show all)").dim(),
            ])
        };
        Paragraph::new(scope_text).render(
            Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: 1,
            },
            buf,
        );

        let header_text = if self.filter.is_empty() {
            RLine::from(vec![
                Span::raw("search"),
                Span::raw(": ").into(),
                Span::raw("(type to filter; Esc clears)").into(),
            ])
            .dim()
        } else {
            RLine::from(vec![
                Span::raw("search: "),
                Span::raw(self.filter.clone()).bold(),
            ])
        };
        Paragraph::new(header_text).render(
            Rect {
                x: area.x,
                y: area.y.saturating_add(1),
                width: area.width,
                height: 1,
            },
            buf,
        );

        // Render filtered rows below the headers
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
