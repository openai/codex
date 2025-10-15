use std::fmt::Write;

use chrono::DateTime;
use chrono::Local;
use chrono::Utc;
use codex_multi_agent::DelegateSessionSummary;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::WidgetRef;

use crate::render::Insets;
use crate::render::RectExt;

use super::popup_consts::MAX_POPUP_ROWS;
use super::scroll_state::ScrollState;
use super::selection_popup_common::GenericDisplayRow;
use super::selection_popup_common::render_rows;

/// Visual state for the delegate selection popup (triggered via `#` tokens).
pub(crate) struct DelegatePopup {
    display_query: String,
    pending_query: String,
    waiting: bool,
    matches: Vec<DelegateSessionSummary>,
    state: ScrollState,
}

impl DelegatePopup {
    pub(crate) fn new() -> Self {
        Self {
            display_query: String::new(),
            pending_query: String::new(),
            waiting: true,
            matches: Vec::new(),
            state: ScrollState::new(),
        }
    }

    pub(crate) fn set_query(&mut self, query: &str) {
        if query == self.pending_query {
            return;
        }

        let keep_existing = query.starts_with(&self.display_query);

        self.pending_query.clear();
        self.pending_query.push_str(query);
        self.waiting = true;

        if !keep_existing {
            self.matches.clear();
            self.state.reset();
        }
    }

    pub(crate) fn set_empty_prompt(&mut self) {
        self.display_query.clear();
        self.pending_query.clear();
        self.waiting = false;
        self.matches.clear();
        self.state.reset();
    }

    pub(crate) fn set_matches(&mut self, query: &str, matches: Vec<DelegateSessionSummary>) {
        if query != self.pending_query {
            return;
        }

        self.display_query = query.to_string();
        self.matches = matches;
        self.waiting = false;
        let len = self.matches.len();
        self.state.clamp_selection(len);
        self.state.ensure_visible(len, len.min(MAX_POPUP_ROWS));
    }

    pub(crate) fn move_up(&mut self) {
        let len = self.matches.len();
        self.state.move_up_wrap(len);
        self.state.ensure_visible(len, len.min(MAX_POPUP_ROWS));
    }

    pub(crate) fn move_down(&mut self) {
        let len = self.matches.len();
        self.state.move_down_wrap(len);
        self.state.ensure_visible(len, len.min(MAX_POPUP_ROWS));
    }

    pub(crate) fn selected_session(&self) -> Option<&DelegateSessionSummary> {
        self.state
            .selected_idx
            .and_then(|idx| self.matches.get(idx))
    }

    pub(crate) fn calculate_required_height(&self) -> u16 {
        self.matches.len().clamp(1, MAX_POPUP_ROWS) as u16
    }

    fn rows(&self) -> Vec<GenericDisplayRow> {
        if self.matches.is_empty() {
            return Vec::new();
        }

        self.matches
            .iter()
            .map(|summary| {
                let mut description = String::new();
                let _ = write!(
                    description,
                    "{} · {}",
                    format_timestamp(summary.last_interacted_at),
                    summary.cwd.display()
                );

                GenericDisplayRow {
                    name: format!("#{}", summary.agent_id.as_str()),
                    match_indices: None,
                    is_current: false,
                    display_shortcut: None,
                    description: Some(description),
                }
            })
            .collect()
    }
}

impl WidgetRef for &DelegatePopup {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let rows = self.rows();
        let empty_message = if self.waiting {
            "loading..."
        } else {
            "no delegates"
        };

        render_rows(
            area.inset(Insets::tlbr(0, 2, 0, 0)),
            buf,
            &rows,
            &self.state,
            MAX_POPUP_ROWS,
            empty_message,
        );
    }
}

fn format_timestamp(time: std::time::SystemTime) -> String {
    let datetime: DateTime<Utc> = time.into();
    datetime
        .with_timezone(&Local)
        .format("%Y-%m-%d %H:%M")
        .to_string()
}
