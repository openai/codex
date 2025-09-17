use std::path::Path;
use std::path::PathBuf;

use chrono::DateTime;
use chrono::Utc;
use codex_core::ConversationItem;
use codex_core::ConversationsPage;
use codex_core::Cursor;
use codex_core::RolloutRecorder;
use color_eyre::eyre::Result;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Stylize as _;
use ratatui::text::Line;
use tokio_stream::StreamExt;

use crate::text_formatting::truncate_text;
use crate::tui::FrameRequester;
use crate::tui::Tui;
use crate::tui::TuiEvent;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::InputMessageKind;
use codex_protocol::protocol::USER_MESSAGE_BEGIN;

const PAGE_SIZE: usize = 25;

#[derive(Debug, Clone)]
pub enum ResumeSelection {
    StartFresh,
    Resume(PathBuf),
    Exit,
}

/// Interactive session picker that lists recorded rollout files with simple
/// search and pagination. Shows the first user input as the preview, relative
/// time (e.g., "5 seconds ago"), and the absolute path.
pub async fn run_resume_picker(tui: &mut Tui, codex_home: &Path) -> Result<ResumeSelection> {
    let alt = AltScreenGuard::enter(tui);
    let mut state = PickerState::new(codex_home.to_path_buf(), alt.tui.frame_requester());
    state.load_page(None).await?;
    state.request_frame();

    let mut events = alt.tui.event_stream();
    while let Some(ev) = events.next().await {
        match ev {
            TuiEvent::Key(key) => {
                if matches!(key.kind, KeyEventKind::Release) {
                    continue;
                }
                if let Some(sel) = state.handle_key(key).await? {
                    return Ok(sel);
                }
            }
            TuiEvent::Draw => {
                draw_picker(alt.tui, &state)?;
            }
            // Ignore paste and attach-image in picker
            _ => {}
        }
    }

    // Fallback – treat as cancel/new
    Ok(ResumeSelection::StartFresh)
}

/// RAII guard that ensures we leave the alt-screen on scope exit.
struct AltScreenGuard<'a> {
    tui: &'a mut Tui,
}

impl<'a> AltScreenGuard<'a> {
    fn enter(tui: &'a mut Tui) -> Self {
        let _ = tui.enter_alt_screen();
        Self { tui }
    }
}

impl Drop for AltScreenGuard<'_> {
    fn drop(&mut self) {
        let _ = self.tui.leave_alt_screen();
    }
}

struct PickerState {
    codex_home: PathBuf,
    requester: FrameRequester,
    // pagination
    pagination: Pagination,
    // data
    all_rows: Vec<Row>, // unfiltered rows for current page
    filtered_rows: Vec<Row>,
    selected: usize,
    // search
    query: String,
}

#[derive(Debug, Clone)]
struct Pagination {
    current_anchor: Option<Cursor>,
    backstack: Vec<Option<Cursor>>, // track previous anchors for ←/a
    next_cursor: Option<Cursor>,
    page_index: usize,
}

#[derive(Clone)]
struct Row {
    path: PathBuf,
    preview: String,
    ts: Option<DateTime<Utc>>,
}

impl PickerState {
    fn new(codex_home: PathBuf, requester: FrameRequester) -> Self {
        Self {
            codex_home,
            requester,
            pagination: Pagination {
                current_anchor: None,
                backstack: vec![None],
                next_cursor: None,
                page_index: 0,
            },
            all_rows: Vec::new(),
            filtered_rows: Vec::new(),
            selected: 0,
            query: String::new(),
        }
    }

    fn request_frame(&self) {
        self.requester.schedule_frame();
    }

    async fn handle_key(&mut self, key: KeyEvent) -> Result<Option<ResumeSelection>> {
        match key.code {
            KeyCode::Esc => return Ok(Some(ResumeSelection::StartFresh)),
            KeyCode::Char('c')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                return Ok(Some(ResumeSelection::Exit));
            }
            KeyCode::Enter => {
                if let Some(row) = self.filtered_rows.get(self.selected) {
                    return Ok(Some(ResumeSelection::Resume(row.path.clone())));
                }
            }
            KeyCode::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                self.request_frame();
            }
            KeyCode::Down => {
                if self.selected + 1 < self.filtered_rows.len() {
                    self.selected += 1;
                }
                self.request_frame();
            }
            KeyCode::Left | KeyCode::Char('a') => {
                self.prev_page().await?;
            }
            KeyCode::Right | KeyCode::Char('d') => {
                self.next_page().await?;
            }
            KeyCode::Backspace => {
                self.query.pop();
                self.apply_filter();
            }
            KeyCode::Char(c) => {
                // basic text input for search
                if !key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL)
                    && !key.modifiers.contains(crossterm::event::KeyModifiers::ALT)
                {
                    self.query.push(c);
                    self.apply_filter();
                }
            }
            _ => {}
        }
        Ok(None)
    }

    async fn prev_page(&mut self) -> Result<()> {
        if self.pagination.page_index == 0 {
            return Ok(());
        }
        // current_anchor points to the page we just loaded; backstack[page_index-1] is the anchor to reload
        if self.pagination.page_index > 0 {
            self.pagination.page_index -= 1;
            let anchor = self
                .pagination
                .backstack
                .get(self.pagination.page_index)
                .cloned()
                .flatten();
            self.pagination.current_anchor = anchor.clone();
            self.load_page(anchor.as_ref()).await?;
        }
        Ok(())
    }

    async fn next_page(&mut self) -> Result<()> {
        if let Some(next) = self.pagination.next_cursor.clone() {
            // Record the anchor for the page we are moving to at index new_index
            let new_index = self.pagination.page_index + 1;
            if self.pagination.backstack.len() <= new_index {
                self.pagination.backstack.resize(new_index + 1, None);
            }
            self.pagination.backstack[new_index] = Some(next.clone());
            self.pagination.current_anchor = Some(next.clone());
            self.pagination.page_index = new_index;
            let anchor = self.pagination.current_anchor.clone();
            self.load_page(anchor.as_ref()).await?;
        }
        Ok(())
    }

    async fn load_page(&mut self, anchor: Option<&Cursor>) -> Result<()> {
        let page = RolloutRecorder::list_conversations(&self.codex_home, PAGE_SIZE, anchor).await?;
        self.pagination.next_cursor = page.next_cursor.clone();
        self.all_rows = to_rows(page);
        self.apply_filter();
        // reset selection on new page
        self.selected = 0;
        Ok(())
    }

    fn apply_filter(&mut self) {
        if self.query.is_empty() {
            self.filtered_rows = self.all_rows.clone();
        } else {
            let q = self.query.to_lowercase();
            self.filtered_rows = self
                .all_rows
                .iter()
                .filter(|r| r.preview.to_lowercase().contains(&q))
                .cloned()
                .collect();
        }
        if self.selected >= self.filtered_rows.len() {
            self.selected = self.filtered_rows.len().saturating_sub(1);
        }
        self.request_frame();
    }
}

fn to_rows(page: ConversationsPage) -> Vec<Row> {
    page.items.into_iter().map(|it| head_to_row(&it)).collect()
}

fn head_to_row(item: &ConversationItem) -> Row {
    let mut ts: Option<DateTime<Utc>> = None;
    if let Some(first) = item.head.first()
        && let Some(t) = first.get("timestamp").and_then(|v| v.as_str())
        && let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(t)
    {
        ts = Some(parsed.with_timezone(&Utc));
    }

    let preview = preview_from_head(&item.head)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| String::from("(no message yet)"));

    Row {
        path: item.path.clone(),
        preview,
        ts,
    }
}

fn preview_from_head(head: &[serde_json::Value]) -> Option<String> {
    head.iter()
        .filter_map(|value| serde_json::from_value::<ResponseItem>(value.clone()).ok())
        .find_map(|item| match item {
            ResponseItem::Message { content, .. } => {
                // Find the actual user message (as opposed to user instructions or ide context)
                let preview = content
                    .into_iter()
                    .filter_map(|content| match content {
                        ContentItem::InputText { text }
                            if matches!(
                                InputMessageKind::from(("user", text.as_str())),
                                InputMessageKind::Plain
                            ) =>
                        {
                            // Strip ide context.
                            let text = match text.find(USER_MESSAGE_BEGIN) {
                                Some(idx) => {
                                    text[idx + USER_MESSAGE_BEGIN.len()..].trim().to_string()
                                }
                                None => text,
                            };
                            Some(text)
                        }
                        _ => None,
                    })
                    .collect::<String>();

                if preview.is_empty() {
                    None
                } else {
                    Some(preview)
                }
            }
            _ => None,
        })
}

fn draw_picker(tui: &mut Tui, state: &PickerState) -> std::io::Result<()> {
    // Render full-screen overlay
    let height = tui.terminal.size()?.height;
    tui.draw(height, |frame| {
        let area = frame.area();
        let [header, search, list, hint] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(area.height.saturating_sub(3)),
            Constraint::Length(1),
        ])
        .areas(area);

        // Header
        frame.render_widget_ref(
            Line::from(vec!["Resume a previous session".bold().cyan()]),
            header,
        );

        // Search line
        let q = if state.query.is_empty() {
            "Type to search".dim().to_string()
        } else {
            state.query.clone()
        };
        frame.render_widget_ref(Line::from(q), search);

        // List
        render_list(frame, list, state);

        // Hint line
        let hint_line: Line = vec![
            "Enter".bold(),
            " to resume  ".into(),
            "Esc".bold(),
            " to start new  ".into(),
            "Ctrl+C".into(),
            " to quit  ".dim(),
            "←/a".into(),
            " prev  ".dim(),
            "→/d".into(),
            " next".dim(),
        ]
        .into();
        frame.render_widget_ref(hint_line, hint);
    })
}

fn render_list(frame: &mut crate::custom_terminal::Frame, area: Rect, state: &PickerState) {
    let rows = &state.filtered_rows;
    if rows.is_empty() {
        frame.render_widget_ref(Line::from("No sessions found".italic().dim()), area);
        return;
    }

    // Compute how many rows fit (1 line per item)
    let capacity = area.height as usize;
    let start = state.selected.saturating_sub(capacity.saturating_sub(1));
    let visible = &rows[start..rows.len().min(start + capacity)];

    let mut y = area.y;
    for (idx, row) in visible.iter().enumerate() {
        let is_sel = start + idx == state.selected;
        let marker = if is_sel { "> ".bold() } else { "  ".into() };
        let ts = row
            .ts
            .map(human_time_ago)
            .unwrap_or_else(|| "".to_string())
            .dim();
        let max_cols = area.width.saturating_sub(6) as usize;
        let preview = truncate_text(&row.preview, max_cols);

        let line: Line = vec![marker, ts, "  ".into(), preview.into()].into();
        let rect = Rect::new(area.x, y, area.width, 1);
        frame.render_widget_ref(line, rect);
        y = y.saturating_add(1);
    }
}

fn human_time_ago(ts: DateTime<Utc>) -> String {
    let now = Utc::now();
    let delta = now - ts;
    let secs = delta.num_seconds();
    if secs < 60 {
        let n = secs.max(0);
        if n == 1 {
            format!("{n} second ago")
        } else {
            format!("{n} seconds ago")
        }
    } else if secs < 60 * 60 {
        let m = secs / 60;
        if m == 1 {
            format!("{m} minute ago")
        } else {
            format!("{m} minutes ago")
        }
    } else if secs < 60 * 60 * 24 {
        let h = secs / 3600;
        if h == 1 {
            format!("{h} hour ago")
        } else {
            format!("{h} hours ago")
        }
    } else {
        let d = secs / (60 * 60 * 24);
        if d == 1 {
            format!("{d} day ago")
        } else {
            format!("{d} days ago")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn head_with_ts_and_user_text(ts: &str, texts: &[&str]) -> Vec<serde_json::Value> {
        vec![
            json!({ "timestamp": ts }),
            json!({
                "type": "message",
                "role": "user",
                "content": texts
                    .iter()
                    .map(|t| json!({ "type": "input_text", "text": *t }))
                    .collect::<Vec<_>>()
            }),
        ]
    }

    #[test]
    fn preview_uses_first_message_input_text() {
        let head = vec![
            json!({ "timestamp": "2025-01-01T00:00:00Z" }),
            json!({
                "type": "message",
                "role": "user",
                "content": [
                    { "type": "input_text", "text": "<user_instructions>hi</user_instructions>" },
                    { "type": "input_text", "text": "real question" },
                    { "type": "input_image", "image_url": "ignored" }
                ]
            }),
            json!({
                "type": "message",
                "role": "user",
                "content": [ { "type": "input_text", "text": "later text" } ]
            }),
        ];
        let preview = preview_from_head(&head);
        assert_eq!(preview.as_deref(), Some("real question"));
    }

    #[test]
    fn to_rows_preserves_backend_order() {
        // Construct two items with different timestamps and real user text.
        let a = ConversationItem {
            path: PathBuf::from("/tmp/a.jsonl"),
            head: head_with_ts_and_user_text("2025-01-01T00:00:00Z", &["A"]),
        };
        let b = ConversationItem {
            path: PathBuf::from("/tmp/b.jsonl"),
            head: head_with_ts_and_user_text("2025-01-02T00:00:00Z", &["B"]),
        };
        let rows = to_rows(ConversationsPage {
            items: vec![a, b],
            next_cursor: None,
            num_scanned_files: 0,
            reached_scan_cap: false,
        });
        assert_eq!(rows.len(), 2);
        // Preserve the given order; backend already provides newest-first
        assert!(rows[0].preview.contains('A'));
        assert!(rows[1].preview.contains('B'));
    }
}
