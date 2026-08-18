//! Dashboard for inspecting and managing the daemon's active tasks.

use super::agents_overview::AGENTS_OVERVIEW_VIEW_ID;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::BottomPaneView;
use crate::bottom_pane::ViewCompletion;
use crate::key_hint::KeyBindingListExt;
use crate::key_hint::ShortcutHint;
use crate::key_hint::is_plain_text_key_event;
use crate::keymap::AgentsKeymap;
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
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Margin;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::PoisonError;
use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

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
    pub(super) input: String,
    search: String,
    searching: bool,
    pub(super) status_grouping: bool,
    pub(super) refresh_task: Option<tokio::task::AbortHandle>,
    pub(super) renaming: bool,
}

pub(super) struct AgentsOverviewView {
    pub(super) rows: Vec<AgentsOverviewRow>,
    selected: usize,
    state: Arc<Mutex<AgentsOverviewViewState>>,
    exit_on_cancel: bool,
    completion: Option<ViewCompletion>,
    app_event_tx: AppEventSender,
    keymap: ListKeymap,
    agents_keymap: AgentsKeymap,
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
            agents_keymap: keymap.agents,
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

    fn state(&self) -> MutexGuard<'_, AgentsOverviewViewState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn selected_row(&self) -> Option<&AgentsOverviewRow> {
        self.rows
            .get(self.selected)
            .filter(|_| self.visible_indices().contains(&self.selected))
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
        if self.state().renaming {
            return;
        }
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

    fn activate(&mut self) {
        let state = self.state().clone();
        let input = state.input.clone();
        if !state.searching && !input.is_empty() && input.trim().is_empty() {
            return;
        }
        if !state.searching && !input.trim().is_empty() {
            if state.renaming {
                if let Some(row) = self.selected_row() {
                    self.app_event_tx
                        .send(AppEvent::RenameAgentsOverviewThread {
                            thread_id: row.thread_id,
                            name: input.trim().to_string(),
                        });
                }
                self.state().renaming = false;
            } else {
                self.app_event_tx
                    .send(AppEvent::DispatchAgentsOverviewTask {
                        prompt: input,
                        cwd: (!state.status_grouping)
                            .then(|| self.selected_row().map(|row| row.thread.cwd.clone()))
                            .flatten(),
                    });
            }
            self.state().input.clear();
        } else if let Some(row) = self.selected_row().filter(|_| !state.renaming) {
            self.app_event_tx
                .send(AppEvent::SelectAgentsOverviewThread {
                    thread_id: row.thread_id,
                });
            if state.searching {
                let mut state = self.state();
                state.search.clear();
                state.searching = false;
            }
            self.completion = Some(ViewCompletion::Accepted);
        }
    }

    fn edit_input(&mut self, edit: impl FnOnce(&mut String)) -> bool {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let searching = state.searching;
        edit(if searching {
            &mut state.search
        } else {
            &mut state.input
        });
        drop(state);
        if searching {
            self.selected = self
                .visible_indices()
                .first()
                .copied()
                .unwrap_or(usize::MAX);
        }
        true
    }

    fn status(row: &AgentsOverviewRow) -> (&'static str, Span<'static>) {
        match row.group {
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

    fn render_details(&self, area: Rect, buf: &mut Buffer) {
        let Some(row) = self.selected_row() else {
            return;
        };
        let (status, dot) = Self::status(row);
        let mut lines = vec![
            Line::from("Task details".bold()),
            Line::default(),
            Line::from(row.thread.name.as_deref().unwrap_or("Untitled task").bold()),
            Line::from(vec![dot, " ".into(), status.into()]),
            Line::default(),
            Line::from("Project".dim()),
            Line::from(row.thread.cwd.display().to_string()),
        ];
        if let Some(branch) = row
            .thread
            .git_info
            .as_ref()
            .and_then(|git| git.branch.as_ref())
        {
            lines.push(Line::default());
            lines.push("Branch".dim().into());
            lines.push(branch.clone().into());
        }
        let preview = crate::text_formatting::truncate_text(
            &row.thread.preview,
            usize::from(area.width) * usize::from(area.height),
        );
        lines.extend([
            Line::default(),
            Line::from("Latest activity".dim()),
            Line::from(match preview.as_str() {
                "" => "No activity yet.",
                preview => preview,
            }),
        ]);
        Paragraph::new(crate::wrapping::word_wrap_lines(
            lines,
            usize::from(area.width),
        ))
        .render(area, buf);
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
        KeymapContextSet::new(KeymapContext::List).with(KeymapContext::Agents)
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

    fn handle_paste(&mut self, pasted: String) -> bool {
        self.edit_input(|input| {
            input.push_str(&crate::history_cell::sanitize_user_text(pasted.into()))
        })
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Backspace
            && key.modifiers.is_empty()
            && self.keymap.action_for(key).is_none()
        {
            self.edit_input(|input| {
                input.pop();
            });
            return;
        }
        if is_plain_text_key_event(key)
            && let KeyCode::Char(character) = key.code
        {
            self.edit_input(|input| input.push(character));
            return;
        }

        if self.agents_keymap.toggle_grouping.is_pressed(key) {
            let mut state = self.state();
            state.status_grouping = !state.status_grouping;
            return;
        }
        if self.agents_keymap.search.is_pressed(key) {
            let mut state = self.state();
            if !state.renaming {
                state.searching = !state.searching;
                if !state.searching {
                    state.search.clear();
                }
            }
            return;
        }
        if self.agents_keymap.new_task.is_pressed(key) {
            let mut state = self.state();
            state.search.clear();
            state.searching = false;
            state.renaming = false;
            state.input.clear();
            return;
        }
        if self.agents_keymap.rename.is_pressed(key) {
            if let Some(row) = self.selected_row() {
                let mut state = self.state();
                if state.input.is_empty() {
                    state.input = row.thread.name.clone().unwrap_or_default();
                    state.search.clear();
                    state.searching = false;
                    state.renaming = true;
                }
            }
            return;
        }
        if self.agents_keymap.stop.is_pressed(key) {
            if let Some(row) = self.selected_row()
                && matches!(row.thread.status, ThreadStatus::Active { .. })
            {
                self.app_event_tx.send(AppEvent::StopAgentsOverviewThread {
                    thread_id: row.thread_id,
                });
            }
            return;
        }

        if let Some(action) = self.keymap.action_for(key) {
            if self.state().renaming
                && matches!(action, ListAction::JumpTop | ListAction::JumpBottom)
            {
                return;
            }
            match action {
                ListAction::MoveUp => self.move_selection(/*forward*/ false),
                ListAction::MoveDown => self.move_selection(/*forward*/ true),
                ListAction::JumpTop => {
                    self.selected = self.visible_indices().first().copied().unwrap_or(0);
                }
                ListAction::JumpBottom => {
                    self.selected = self.visible_indices().last().copied().unwrap_or(0);
                }
                ListAction::Accept => self.activate(),
                ListAction::Cancel => {
                    let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
                    if state.searching {
                        state.search.clear();
                        state.searching = false;
                        self.selected = 0;
                    } else if !state.input.is_empty() || state.renaming {
                        state.input.clear();
                        state.renaming = false;
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
                ListAction::MoveLeft | ListAction::MoveRight => {}
            }
        } else if key.code == KeyCode::Backspace {
            self.edit_input(|input| {
                input.pop();
            });
        }
    }
}

impl Renderable for AgentsOverviewView {
    fn desired_height(&self, _width: u16) -> u16 {
        24
    }

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        let state = self.state().clone();
        let (label, input) = if state.searching {
            ("  Search › ", &state.search)
        } else if state.renaming {
            ("  Rename › ", &state.input)
        } else {
            ("  New task › ", &state.input)
        };
        let x = area
            .x
            .saturating_add((label.width() + input.width()) as u16)
            .min(area.right().saturating_sub(3));
        Some((x, area.bottom().saturating_sub(2)))
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.width < 12 || area.height < 8 {
            return;
        }
        Clear.render(area, buf);
        let [header, summary, divider, body, prompt, footer] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
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
        let body = inset(body);
        if body.width >= 90 {
            let [list, gap, details] = Layout::horizontal([
                Constraint::Min(46),
                Constraint::Length(3),
                Constraint::Length(38),
            ])
            .areas(body);
            for y in gap.y..gap.bottom() {
                buf[(gap.x + 1, y)]
                    .set_symbol("│")
                    .set_style(Style::new().dim());
            }
            self.render_rows(list, buf);
            self.render_details(details, buf);
        } else {
            self.render_rows(body, buf);
        }
        let state = self.state().clone();
        let (label, input) = if state.searching {
            ("Search › ", &state.search)
        } else if state.renaming {
            ("Rename › ", &state.input)
        } else {
            ("New task › ", &state.input)
        };
        let placeholder = if input.is_empty() && !state.searching && !state.renaming {
            "Describe a task and press enter to dispatch it"
        } else {
            ""
        };
        let available_width = usize::from(inset(prompt).width)
            .saturating_sub(label.width())
            .saturating_sub(1);
        let mut visible_start = input.len();
        let mut visible_width = 0;
        for (index, character) in input.char_indices().rev() {
            let width = character.width().unwrap_or(0);
            if visible_width + width > available_width {
                break;
            }
            visible_width += width;
            visible_start = index;
        }
        let input = &input[visible_start..];
        Line::from(vec![label.cyan().bold(), input.into(), placeholder.dim()])
            .render(inset(prompt), buf);
        let list_hint = |action| {
            self.keymap.primary_hint(action).filter(|hint| {
                !matches!(hint, ShortcutHint::Single(binding)
                if is_plain_text_key_event(KeyEvent::new(
                    binding.parts().0,
                    binding.parts().1,
                )) || [
                    &self.agents_keymap.search,
                    &self.agents_keymap.new_task,
                    &self.agents_keymap.rename,
                    &self.agents_keymap.stop,
                    &self.agents_keymap.toggle_grouping,
                ]
                .into_iter()
                .any(|bindings| bindings.contains(binding)))
            })
        };
        let navigation_hints = [ListAction::MoveUp, ListAction::MoveDown]
            .into_iter()
            .filter_map(list_hint)
            .map(|hint| hint.display_label().replace(" + ", "+"))
            .collect::<Vec<_>>();
        let navigation_hint = navigation_hints.join(
            if navigation_hints
                .iter()
                .all(|hint| hint.chars().count() == 1)
            {
                ""
            } else {
                " "
            },
        );
        let mut footer_spans = Vec::new();
        if !navigation_hint.is_empty() {
            footer_spans.extend([navigation_hint.bold(), " navigate  ".dim()]);
        }
        let mut add_hint = |hint: Option<ShortcutHint>, label: &'static str, enabled: bool| {
            if let Some(hint) = hint {
                let key = hint.display_label().replace(" + ", "+");
                footer_spans.push(if enabled { key.bold() } else { key.dim() });
                footer_spans.push(format!(" {label}  ").dim());
            }
        };
        add_hint(list_hint(ListAction::Accept), "open", true);
        add_hint(
            self.agents_keymap
                .primary_hint("search", &self.agents_keymap.search),
            "search",
            true,
        );
        add_hint(
            self.agents_keymap
                .primary_hint("toggle_grouping", &self.agents_keymap.toggle_grouping),
            "group",
            true,
        );
        add_hint(
            self.agents_keymap
                .primary_hint("rename", &self.agents_keymap.rename),
            "rename",
            true,
        );
        add_hint(
            self.agents_keymap
                .primary_hint("stop", &self.agents_keymap.stop),
            "stop",
            self.selected_row()
                .is_some_and(|row| matches!(row.thread.status, ThreadStatus::Active { .. })),
        );
        add_hint(list_hint(ListAction::Cancel), "back", true);
        let footer_area = inset(footer);
        let mut footer_line: Line = footer_spans.into();
        if footer_line.width() > usize::from(footer_area.width) {
            for span in &mut footer_line.spans {
                if span.content.ends_with("  ") {
                    span.content.to_mut().pop();
                }
            }
        }
        footer_line.render(footer_area, buf);
    }
}
