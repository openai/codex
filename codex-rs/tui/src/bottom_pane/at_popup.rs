use codex_common::fuzzy_match::fuzzy_match;
use codex_file_search::FileMatch;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::WidgetRef;

use crate::render::Insets;
use crate::render::RectExt;
use crate::text_formatting::truncate_text;

use super::popup_consts::MAX_POPUP_ROWS;
use super::scroll_state::ScrollState;
use super::selection_popup_common::GenericDisplayRow;
use super::selection_popup_common::render_rows_single_line;

use crate::subagent_candidates::SubAgentCandidate;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AtPopupSelectionKind {
    Subagent,
    File,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AtPopupSelection {
    pub(crate) kind: AtPopupSelectionKind,
    pub(crate) value: String,
    pub(crate) disabled: bool,
}

pub(crate) struct AtPopup {
    query: String,
    agents: Vec<SubAgentCandidate>,
    file_matches: Vec<FileMatch>,
    waiting_file: bool,
    waiting_agents: bool,
    state: ScrollState,
}

impl AtPopup {
    pub(crate) fn new() -> Self {
        Self {
            query: String::new(),
            agents: Vec::new(),
            file_matches: Vec::new(),
            waiting_file: false,
            waiting_agents: false,
            state: ScrollState::new(),
        }
    }

    pub(crate) fn set_query(&mut self, query: &str) {
        self.query = query.to_string();
        self.clamp_selection();
    }

    pub(crate) fn set_agents(&mut self, agents: Vec<SubAgentCandidate>) {
        self.agents = agents;
        self.waiting_agents = false;
        self.clamp_selection();
    }

    pub(crate) fn set_waiting_file(&mut self, waiting: bool) {
        self.waiting_file = waiting;
    }

    pub(crate) fn set_waiting_agents(&mut self, waiting: bool) {
        self.waiting_agents = waiting;
    }

    pub(crate) fn set_file_matches(&mut self, query: &str, matches: Vec<FileMatch>) {
        if query != self.query {
            return;
        }
        self.file_matches = matches;
        self.waiting_file = false;
        self.clamp_selection();
    }

    pub(crate) fn calculate_required_height(&self, _width: u16) -> u16 {
        let rows = self.rows();
        let visible = rows.len().clamp(1, MAX_POPUP_ROWS);
        visible as u16
    }

    pub(crate) fn move_up(&mut self) {
        let len = self.rows().len();
        self.state.move_up_wrap(len);
        self.state.ensure_visible(len, MAX_POPUP_ROWS.min(len));
    }

    pub(crate) fn move_down(&mut self) {
        let len = self.rows().len();
        self.state.move_down_wrap(len);
        self.state.ensure_visible(len, MAX_POPUP_ROWS.min(len));
    }

    pub(crate) fn selected(&self) -> Option<AtPopupSelection> {
        let rows = self.rows();
        let idx = self.state.selected_idx?;
        let row = rows.get(idx)?;
        Some(row.selection.clone())
    }

    fn clamp_selection(&mut self) {
        let len = self.rows().len();
        self.state.clamp_selection(len);
        self.state.ensure_visible(len, MAX_POPUP_ROWS.min(len));
    }

    fn should_include_agents(&self) -> bool {
        let q = self.query.trim();
        if q.is_empty() {
            return true;
        }
        q.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    }

    fn agent_matches(&self) -> Vec<(usize, Option<Vec<usize>>, i32)> {
        if !self.should_include_agents() {
            return Vec::new();
        }
        let filter = self.query.trim();
        let mut out: Vec<(usize, Option<Vec<usize>>, i32)> = Vec::new();
        if filter.is_empty() {
            for (idx, _agent) in self.agents.iter().enumerate() {
                out.push((idx, None, 0));
            }
            return out;
        }
        for (idx, agent) in self.agents.iter().enumerate() {
            if let Some((indices, score)) = fuzzy_match(&agent.name, filter) {
                out.push((idx, Some(indices), score));
            }
        }
        out.sort_by(|a, b| {
            a.2.cmp(&b.2).then_with(|| {
                let an = &self.agents[a.0].name;
                let bn = &self.agents[b.0].name;
                an.cmp(bn)
            })
        });
        out
    }

    fn rows(&self) -> Vec<AtRow> {
        let mut out: Vec<AtRow> = Vec::new();

        for (idx, indices, _score) in self.agent_matches() {
            let agent = &self.agents[idx];
            let name_display = truncate_text(&agent.name, 21);
            let desc = agent.description.clone();
            let disabled_reason = agent.disabled_reason.clone();
            out.push(AtRow {
                display: GenericDisplayRow {
                    name: name_display,
                    match_indices: indices,
                    display_shortcut: None,
                    description: desc,
                    disabled_reason: disabled_reason.clone(),
                    wrap_indent: None,
                },
                selection: AtPopupSelection {
                    kind: AtPopupSelectionKind::Subagent,
                    value: agent.name.clone(),
                    disabled: disabled_reason.is_some(),
                },
            });
        }

        for m in &self.file_matches {
            out.push(AtRow {
                display: GenericDisplayRow {
                    name: m.path.clone(),
                    match_indices: m
                        .indices
                        .as_ref()
                        .map(|v| v.iter().map(|&i| i as usize).collect()),
                    display_shortcut: None,
                    description: None,
                    disabled_reason: None,
                    wrap_indent: None,
                },
                selection: AtPopupSelection {
                    kind: AtPopupSelectionKind::File,
                    value: m.path.clone(),
                    disabled: false,
                },
            });
        }

        out
    }
}

struct AtRow {
    display: GenericDisplayRow,
    selection: AtPopupSelection,
}

impl WidgetRef for &AtPopup {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let rows_all: Vec<GenericDisplayRow> = self.rows().into_iter().map(|r| r.display).collect();

        let empty_message = if self.waiting_file || self.waiting_agents {
            "loading..."
        } else {
            "no matches"
        };

        render_rows_single_line(
            area.inset(Insets::tlbr(0, 2, 0, 0)),
            buf,
            &rows_all,
            &self.state,
            MAX_POPUP_ROWS,
            empty_message,
        );
    }
}
