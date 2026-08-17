//! Read-only dashboard of the daemon's active tasks.

use super::agents_overview::AGENTS_OVERVIEW_VIEW_ID;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::BottomPaneView;
use crate::bottom_pane::ViewCompletion;
use crate::keymap::KeymapContext;
use crate::keymap::KeymapContextSet;
use crate::keymap::ListAction;
use crate::keymap::ListKeymap;
use crate::keymap::RuntimeKeymap;
use crate::render::renderable::Renderable;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadActiveFlag;
use codex_app_server_protocol::ThreadStatus;
use codex_protocol::ThreadId;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Margin;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Clear;
use ratatui::widgets::Widget;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum AgentsOverviewGroup {
    NeedsYou,
    Working,
    Ready,
    Finished,
}

impl AgentsOverviewGroup {
    pub(super) fn for_status(status: &ThreadStatus) -> Self {
        match status {
            ThreadStatus::Active { active_flags }
                if active_flags.contains(&ThreadActiveFlag::WaitingOnApproval)
                    || active_flags.contains(&ThreadActiveFlag::WaitingOnUserInput) =>
            {
                Self::NeedsYou
            }
            ThreadStatus::Active { .. } => Self::Working,
            ThreadStatus::Idle => Self::Ready,
            ThreadStatus::SystemError => Self::NeedsYou,
            ThreadStatus::NotLoaded => Self::Finished,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::NeedsYou => "Needs input",
            Self::Working => "Working",
            Self::Ready => "Ready",
            Self::Finished => "Finished",
        }
    }
}

#[derive(Clone)]
pub(super) struct AgentsOverviewRow {
    pub(super) thread: Thread,
    pub(super) thread_id: ThreadId,
    pub(super) group: AgentsOverviewGroup,
    pub(super) is_current: bool,
}

#[derive(Clone, Default)]
pub(super) struct AgentsOverviewViewState {
    search: String,
    searching: bool,
    pub(super) status_grouping: bool,
    pub(super) refresh_task: Option<tokio::task::AbortHandle>,
}

pub(super) struct AgentsOverviewView {
    pub(super) rows: Vec<AgentsOverviewRow>,
    selected: usize,
    state: Arc<Mutex<AgentsOverviewViewState>>,
    exit_on_cancel: bool,
    completion: Option<ViewCompletion>,
    app_event_tx: AppEventSender,
    keymap: ListKeymap,
}

impl Drop for AgentsOverviewView {
    fn drop(&mut self) {
        if let Some(task) = self
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .refresh_task
            .take()
        {
            task.abort();
        }
    }
}

impl AgentsOverviewView {
    pub(super) fn new(
        rows: Vec<AgentsOverviewRow>,
        selected_thread_id: Option<ThreadId>,
        exit_on_cancel: bool,
        app_event_tx: AppEventSender,
        keymap: RuntimeKeymap,
        state: Arc<Mutex<AgentsOverviewViewState>>,
    ) -> Self {
        let selected = selected_thread_id
            .and_then(|thread_id| rows.iter().position(|row| row.thread_id == thread_id))
            .or_else(|| rows.iter().position(|row| row.is_current))
            .unwrap_or(0);
        let mut view = Self {
            rows,
            selected,
            state,
            exit_on_cancel,
            completion: None,
            app_event_tx,
            keymap: keymap.list,
        };
        let visible = view.visible_indices();
        if !visible.contains(&view.selected) {
            view.selected = visible.first().copied().unwrap_or(usize::MAX);
        }
        view
    }

    pub(super) fn thread_ids(&self) -> Vec<ThreadId> {
        self.rows.iter().map(|row| row.thread_id).collect()
    }

    fn visible_indices(&self) -> Vec<usize> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let search = state.search.to_lowercase();
        let mut visible = self
            .rows
            .iter()
            .enumerate()
            .filter_map(|(index, row)| {
                let searchable = format!(
                    "{} {} {}",
                    row.thread.name.as_deref().unwrap_or_default(),
                    row.thread.preview,
                    row.thread.cwd.display(),
                )
                .to_lowercase();
                (search.is_empty() || searchable.contains(&search)).then_some(index)
            })
            .collect::<Vec<_>>();
        if !state.status_grouping {
            visible.sort_by_key(|index| {
                (
                    &self.rows[*index].thread.cwd,
                    std::cmp::Reverse(self.rows[*index].thread.updated_at),
                )
            });
        }
        visible
    }

    fn move_selection(&mut self, forward: bool) {
        let visible = self.visible_indices();
        if visible.is_empty() {
            return;
        }
        let current = visible
            .iter()
            .position(|index| *index == self.selected)
            .unwrap_or(0);
        self.selected = if forward {
            visible[(current + 1) % visible.len()]
        } else {
            visible[current.checked_sub(1).unwrap_or(visible.len() - 1)]
        };
    }

    fn status(row: &AgentsOverviewRow) -> (&'static str, Span<'static>) {
        match AgentsOverviewGroup::for_status(&row.thread.status) {
            AgentsOverviewGroup::NeedsYou => ("Needs input", "●".red()),
            AgentsOverviewGroup::Working => ("Working", "●".green()),
            AgentsOverviewGroup::Ready => ("Ready", "○".cyan()),
            AgentsOverviewGroup::Finished => ("Finished", "✓".dim()),
        }
    }

    fn render_rows(&self, area: Rect, buf: &mut Buffer) {
        let mut offset = 0;
        let mut previous_group: Option<String> = None;
        let project_grouping = !self
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .status_grouping;
        let visible = self.visible_indices();
        let mut first = visible
            .iter()
            .position(|index| *index == self.selected)
            .unwrap_or_default();
        let mut height = 2;
        while first > 0 {
            let previous = &self.rows[visible[first - 1]];
            let current = &self.rows[visible[first]];
            let group_changed = if project_grouping {
                previous.thread.cwd != current.thread.cwd
            } else {
                previous.group != current.group
            };
            let added_height = 1 + 2 * u16::from(group_changed);
            if height + added_height > area.height {
                break;
            }
            height += added_height;
            first -= 1;
        }
        for index in visible.into_iter().skip(first) {
            if offset >= area.height {
                break;
            }
            let row = &self.rows[index];
            let group = if project_grouping {
                row.thread.cwd.display().to_string()
            } else {
                row.group.label().to_string()
            };
            if previous_group.as_deref() != Some(group.as_str()) {
                offset += u16::from(previous_group.is_some());
                if offset >= area.height {
                    break;
                }
                let count = self
                    .rows
                    .iter()
                    .filter(|candidate| {
                        if project_grouping {
                            candidate.thread.cwd == row.thread.cwd
                        } else {
                            candidate.group == row.group
                        }
                    })
                    .count();
                Line::from(vec![group.clone().bold(), format!("  {count}").dim()])
                    .render(Rect::new(area.x, area.y + offset, area.width, 1), buf);
                offset += 1;
                previous_group = Some(group);
            }
            if offset >= area.height {
                break;
            }
            let marker = if self.selected == index {
                "›".cyan().bold()
            } else {
                " ".into()
            };
            let title = row
                .thread
                .name
                .as_deref()
                .or_else(|| (!row.thread.preview.is_empty()).then_some(row.thread.preview.as_str()))
                .unwrap_or("Untitled task");
            let (status, dot) = Self::status(row);
            let current = if row.is_current { "  current" } else { "" };
            let mut spans = vec![
                marker,
                " ".into(),
                dot,
                " ".into(),
                title.into(),
                current.dim(),
            ];
            if project_grouping {
                spans.extend(["  ".into(), status.dim()]);
            }
            Line::from(spans).render(Rect::new(area.x, area.y + offset, area.width, 1), buf);
            offset += 1;
        }
    }
}

impl BottomPaneView for AgentsOverviewView {
    fn view_id(&self) -> Option<&'static str> {
        Some(AGENTS_OVERVIEW_VIEW_ID)
    }

    fn selected_index(&self) -> Option<usize> {
        Some(self.selected)
    }

    fn keymap_contexts(&self) -> KeymapContextSet {
        KeymapContextSet::new(KeymapContext::List)
    }

    fn completion(&self) -> Option<ViewCompletion> {
        self.completion
    }

    fn is_complete(&self) -> bool {
        self.completion.is_some()
    }

    fn prefer_esc_to_handle_key_event(&self) -> bool {
        true
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Char('s') && key.modifiers == KeyModifiers::CONTROL {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            state.status_grouping = !state.status_grouping;
            return;
        }
        if key.code == KeyCode::Char('f') && key.modifiers == KeyModifiers::CONTROL {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            state.searching = !state.searching;
            return;
        }

        if let Some(action) = self.keymap.action_for(key)
            && (!matches!(key.code, KeyCode::Char(_))
                || self.state.lock().is_ok_and(|state| !state.searching))
        {
            match action {
                ListAction::MoveUp => self.move_selection(/*forward*/ false),
                ListAction::MoveDown => self.move_selection(/*forward*/ true),
                ListAction::JumpTop => {
                    self.selected = self.visible_indices().first().copied().unwrap_or(0);
                }
                ListAction::JumpBottom => {
                    self.selected = self.visible_indices().last().copied().unwrap_or(0);
                }
                ListAction::Cancel => {
                    let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
                    if state.searching {
                        state.search.clear();
                        state.searching = false;
                        self.selected = 0;
                    } else {
                        if self.exit_on_cancel {
                            self.app_event_tx
                                .send(AppEvent::Exit(crate::app::ExitMode::Immediate));
                        }
                        self.completion = Some(ViewCompletion::Cancelled);
                    }
                }
                ListAction::PageUp | ListAction::PageDown => {
                    for _ in 0..5 {
                        self.move_selection(action == ListAction::PageDown);
                    }
                }
                ListAction::Accept | ListAction::MoveLeft | ListAction::MoveRight => {}
            }
            return;
        }

        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if !state.searching {
            return;
        }
        match key.code {
            KeyCode::Backspace => {
                state.search.pop();
            }
            KeyCode::Char(character)
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                state.search.push(character);
            }
            _ => return,
        }
        drop(state);
        self.selected = self
            .visible_indices()
            .first()
            .copied()
            .unwrap_or(usize::MAX);
    }
}

impl Renderable for AgentsOverviewView {
    fn desired_height(&self, _width: u16) -> u16 {
        24
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.width < 12 || area.height < 8 {
            return;
        }
        Clear.render(area, buf);
        let [header, summary, divider, body, footer] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .areas(area);
        let inset =
            |rect: Rect| rect.inner(Margin::new(/*horizontal*/ 2, /*vertical*/ 0));
        Line::from("Agent command center".bold()).render(inset(header), buf);
        let (needs_you, working, ready) = self.rows.iter().fold((0, 0, 0), |counts, row| {
            let (needs_you, working, ready) = counts;
            match row.group {
                AgentsOverviewGroup::NeedsYou => (needs_you + 1, working, ready),
                AgentsOverviewGroup::Working => (needs_you, working + 1, ready),
                AgentsOverviewGroup::Ready => (needs_you, working, ready + 1),
                AgentsOverviewGroup::Finished => counts,
            }
        });
        let attention = format!("{needs_you} need input");
        Line::from(format!("{attention}   {working} working   {ready} ready").dim())
            .render(inset(summary), buf);
        Line::from("─".repeat(usize::from(area.width.saturating_sub(4))).dim())
            .render(inset(divider), buf);
        self.render_rows(inset(body), buf);
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let footer_line = if state.searching {
            Line::from(vec!["Search › ".cyan().bold(), state.search.clone().into()])
        } else {
            Line::from(vec![
                "↑↓".bold(),
                " navigate  ".dim(),
                "ctrl+f".bold(),
                " search  ".dim(),
                "ctrl+s".bold(),
                " group  ".dim(),
                "esc".bold(),
                " back".dim(),
            ])
        };
        footer_line.render(inset(footer), buf);
    }
}
