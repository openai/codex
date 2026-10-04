//! Dashboard for inspecting and managing the TUI's retained daemon tasks.
//! Search, rename, status filters and selection survive metadata refreshes.

#[path = "agent_center/mod.rs"]
pub(super) mod command_center;

#[path = "agents_overview_grouping.rs"]
mod grouping;

pub(super) use codex_config::types::AgentsOverviewGrouping;
use grouping::model_name;

use super::agents_overview::AGENTS_OVERVIEW_VIEW_ID;
use super::agents_overview_details::AgentsOverviewDetails;
use crate::app_event::AgentsOverviewAction;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::BottomPaneView;
use crate::bottom_pane::CancellationEvent;
use crate::bottom_pane::TextArea;
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
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::mark_buffer_hyperlinks;
use crate::terminal_hyperlinks::plain_hyperlink_lines;
use crate::terminal_hyperlinks::remap_wrapped_line;
use crate::terminal_hyperlinks::visible_lines;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadActiveFlag;
use codex_app_server_protocol::ThreadStatus;
use codex_protocol::ThreadId;
use codex_protocol::openai_models::ReasoningEffort;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
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
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::PoisonError;
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
            Self::Finished => "Inactive",
        }
    }
}

#[derive(Clone)]
pub(super) struct AgentsOverviewRow {
    pub(super) details: AgentsOverviewDetails,
    pub(super) thread: Thread,
    pub(super) thread_id: ThreadId,
    pub(super) group: AgentsOverviewGroup,
    pub(super) is_current: bool,
    pub(super) has_voice: bool,
}

fn display_title(thread: &Thread) -> &str {
    let title = thread.name.as_deref().unwrap_or(&thread.preview);
    title.trim().lines().next().unwrap_or("Untitled task")
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AgentsOverviewProjectGroup {
    key: (PathBuf, PathBuf),
    heading: PathBuf,
}

impl AgentsOverviewProjectGroup {
    fn for_thread(thread: &Thread, worktrees_enabled: bool) -> Self {
        if worktrees_enabled
            && let Some(identity) = codex_git_utils::repository_identity(thread.cwd.as_path())
        {
            Self {
                key: (
                    identity.common_dir.into_path_buf(),
                    identity.relative_cwd.clone(),
                ),
                heading: identity.primary_root.as_path().join(identity.relative_cwd),
            }
        } else {
            Self {
                key: (thread.cwd.to_path_buf(), PathBuf::new()),
                heading: thread.cwd.to_path_buf(),
            }
        }
    }
}

#[derive(Default)]
pub(super) struct AgentsOverviewViewState {
    scroll: usize,
    page_height: usize,
    status_filter: usize,
    help: bool,
    pub(super) input: TextArea,
    pub(super) vim_enabled: bool,
    pub(super) key_chord_hint: Option<Vec<(String, String)>>,
    pub(super) creating_worktree: bool,
    pub(super) refresh_failed: bool,
    pub(super) loading: bool,
    pub(super) has_more: bool,
    show_more_selected: bool,
    pub(super) connection_notice: Option<&'static str>,
    pub(super) server_version_notice: Option<String>,
    search: String,
    searching: bool,
    pub(super) grouping: AgentsOverviewGrouping,
    pub(super) rename_target: Option<ThreadId>,
    // The picker can finish this retained view when it selects the already active session.
    pub(super) completion: Option<ViewCompletion>,
}

impl AgentsOverviewViewState {
    pub(super) fn set_rename_input(&mut self, text: &str, keymap: &RuntimeKeymap) {
        let mut input = TextArea::new_single_line();
        input.set_keymap_bindings(keymap);
        input.set_vim_enabled(self.vim_enabled);
        input.set_text_clearing_elements(text);
        input.set_cursor(input.text().len());
        input.enter_vim_insert_mode();
        self.input = input;
    }

    pub(super) fn editing_metadata(&self) -> bool {
        self.searching || self.rename_target.is_some()
    }
}

pub(super) struct AgentsOverviewView {
    use_theme_colors: bool,
    pub(super) rows: Vec<AgentsOverviewRow>,
    project_groups: Vec<AgentsOverviewProjectGroup>,
    selected: usize,
    state: Arc<Mutex<AgentsOverviewViewState>>,
    app_event_tx: AppEventSender,
    keymap: ListKeymap,
    agents_keymap: AgentsKeymap,
    center_shortcut_keys: Vec<crate::key_hint::KeyBinding>,
    worktrees_enabled: bool,
    editor_keymap: RuntimeKeymap,
}

impl AgentsOverviewView {
    pub(super) fn new(
        rows: Vec<AgentsOverviewRow>,
        selected_thread_id: Option<ThreadId>,
        worktrees_enabled: bool,
        use_theme_colors: bool,
        app_event_tx: AppEventSender,
        keymap: RuntimeKeymap,
        state: Arc<Mutex<AgentsOverviewViewState>>,
    ) -> Self {
        let selected = state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .rename_target
            .and_then(|thread_id| rows.iter().position(|row| row.thread_id == thread_id))
            .or_else(|| {
                selected_thread_id.and_then(|id| rows.iter().position(|row| row.thread_id == id))
            })
            .or_else(|| rows.iter().position(|row| row.is_current))
            .unwrap_or(0);
        let project_groups = rows
            .iter()
            .map(|row| AgentsOverviewProjectGroup::for_thread(&row.thread, worktrees_enabled))
            .collect();
        let center_shortcut_keys = crate::keymap::keymap_action_ids()
            .filter(|action| matches!(action.context, KeymapContext::List | KeymapContext::Agents))
            .flat_map(|action| {
                crate::keymap::bindings_for_action(
                    &keymap,
                    action.context.config_name(),
                    action.action,
                )
                .unwrap_or_default()
                .iter()
                .copied()
            })
            .chain(keymap.chords.bindings.iter().filter_map(|binding| {
                matches!(
                    binding.action.context,
                    KeymapContext::List | KeymapContext::Agents
                )
                .then_some(binding.chord.prefix)
            }))
            .collect();
        {
            let mut state = state.lock().unwrap_or_else(PoisonError::into_inner);
            let vim_enabled = state.vim_enabled;
            if state.rename_target.is_some() {
                state.input.set_keymap_bindings(&keymap);
                if state.input.is_vim_enabled() != vim_enabled {
                    state.input.set_vim_enabled(vim_enabled);
                    state.input.enter_vim_insert_mode();
                }
            }
        }
        let mut view = Self {
            editor_keymap: keymap.clone(),
            use_theme_colors,
            rows,
            project_groups,
            selected,
            state,
            app_event_tx,
            keymap: keymap.list,
            agents_keymap: keymap.agents,
            center_shortcut_keys,
            worktrees_enabled,
        };
        if selected_thread_id.is_none() && view.state().show_more_selected {
            view.selected = usize::MAX;
        }
        view.state().completion = None;
        view.reconcile_command_center_selection();
        view
    }

    pub(super) fn thread_ids(&self) -> Vec<ThreadId> {
        self.rows.iter().map(|row| row.thread_id).collect()
    }

    pub(super) fn selection_after_removal(
        &self,
        removed: &std::collections::HashSet<ThreadId>,
    ) -> Option<ThreadId> {
        let visible = self.visible_indices();
        let position = visible.iter().position(|index| *index == self.selected)?;
        visible[position..]
            .iter()
            .chain(visible[..position].iter().rev())
            .map(|index| self.rows[*index].thread_id)
            .find(|id| !removed.contains(id))
    }

    fn title_style(&self, thread_id: ThreadId) -> Style {
        if self.use_theme_colors {
            Style::default().fg(crate::thread_color::thread_color(thread_id))
        } else {
            Style::default()
        }
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
        let (_, status_group) = command_center::TASK_FILTERS[state.status_filter];
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
                ((search.is_empty() || searchable.contains(&search))
                    && (state.rename_target == Some(row.thread_id)
                        || status_group.is_none_or(|group| group == row.group)))
                .then_some(index)
            })
            .collect::<Vec<_>>();
        match state.grouping {
            AgentsOverviewGrouping::Project => visible.sort_by_key(|index| {
                (
                    &self.project_groups[*index].key,
                    std::cmp::Reverse(self.rows[*index].thread.updated_at),
                )
            }),
            AgentsOverviewGrouping::Status => {}
            AgentsOverviewGrouping::Model => visible.sort_by_key(|index| {
                (
                    model_name(&self.rows[*index].thread),
                    std::cmp::Reverse(self.rows[*index].thread.updated_at),
                )
            }),
        }
        visible
    }

    fn move_selection(&mut self, forward: bool) {
        if self.state().rename_target.is_some() {
            return;
        }
        let visible = self.selectable_indices();
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
        self.state().show_more_selected = self.selected == usize::MAX;
    }

    fn activate(&mut self) {
        if self.selected == usize::MAX && self.state().has_more {
            if !self.state().loading {
                self.state().loading = true;
                self.app_event_tx.send(AppEvent::ShowMoreAgentsOverview);
            }
            return;
        }
        let input = self.state().input.text().to_owned();
        if self.state().rename_target.is_some() && !input.trim().is_empty() {
            if let Some(row) = self.selected_row() {
                self.app_event_tx
                    .send(AppEvent::RenameAgentsOverviewThread {
                        thread_id: row.thread_id,
                        name: input.trim().to_string(),
                    });
            }
            self.state().rename_target = None;
            self.state().input.set_text_clearing_elements("");
            self.reconcile_command_center_selection();
        } else if let Some(row) = self
            .selected_row()
            .filter(|_| self.state().rename_target.is_none())
        {
            self.app_event_tx
                .send(AppEvent::SelectAgentsOverviewThread {
                    thread_id: row.thread_id,
                });
            if self.state().searching {
                let mut state = self.state();
                state.search.clear();
                state.searching = false;
            }
        }
    }

    fn edit_input(&mut self, edit: impl FnOnce(&mut String)) -> bool {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let searching = state.searching;
        if searching {
            edit(&mut state.search);
        }
        drop(state);
        if searching {
            self.selected = self
                .selectable_indices()
                .first()
                .copied()
                .unwrap_or(usize::MAX);
        }
        true
    }

    fn status(row: &AgentsOverviewRow) -> (&'static str, Span<'static>) {
        match row.group {
            AgentsOverviewGroup::NeedsYou if row.thread.status == ThreadStatus::SystemError => {
                ("Error", "!".red())
            }
            AgentsOverviewGroup::NeedsYou => ("Needs input", "●".red()),
            AgentsOverviewGroup::Working => ("Working", "●".green()),
            AgentsOverviewGroup::Ready => ("Ready", "○".cyan()),
            AgentsOverviewGroup::Finished => ("Inactive", "○".dim()),
        }
    }

    fn render_details(&self, area: Rect, buf: &mut Buffer) {
        let Some(row) = self.selected_row() else {
            return;
        };
        let (status, dot) = Self::status(row);
        let width = usize::from(area.width);
        let model = crate::line_truncation::truncate_line_with_ellipsis_if_overflow(
            Line::from(vec![
                "Model: ".dim(),
                model_name(&row.thread).to_string().into(),
            ]),
            width,
        );
        let reasoning = crate::line_truncation::truncate_line_with_ellipsis_if_overflow(
            Line::from(vec![
                "Reasoning: ".dim(),
                row.thread
                    .reasoning_effort
                    .as_ref()
                    .map_or("Unknown", ReasoningEffort::as_str)
                    .to_string()
                    .into(),
            ]),
            width,
        );
        let mut lines = vec![
            Line::from("Task details".bold()),
            Line::default(),
            crate::line_truncation::truncate_line_with_ellipsis_if_overflow(
                Line::from(Span::styled(
                    display_title(&row.thread).to_owned(),
                    self.title_style(row.thread_id).bold(),
                )),
                width,
            ),
            Line::from(vec![dot, " ".into(), status.into()]),
            Line::default(),
            model,
            reasoning,
            Line::default(),
            Line::from("Project".dim()),
            Line::from(row.thread.cwd.display().to_string()),
        ];
        lines.extend(row.details.usage_lines.clone());
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
        // Keep destinations attached to their text through wrapping and clipping.
        let wrap = |lines: Vec<HyperlinkLine>| {
            lines
                .into_iter()
                .flat_map(|line| {
                    let wrapped = crate::wrapping::word_wrap_lines([line.line.clone()], width);
                    remap_wrapped_line(&line, wrapped)
                })
                .collect::<Vec<_>>()
        };
        let preview = super::agents_overview_details::preview_markdown(&row.thread.preview);
        let prompt_start = crate::wrapping::word_wrap_lines(lines.clone(), width).len();
        lines.extend([Line::default(), Line::from("Prompt".dim())]);
        let prompt = crate::markdown_render::render_markdown_lines_with_width_and_cwd(
            match preview.as_str() {
                "" => "No prompt available.",
                preview => preview,
            },
            Some(width),
            Some(row.thread.cwd.as_path()),
        );
        let mut prompt = wrap(prompt);
        if prompt.len() > 2 {
            prompt.truncate(2);
            prompt[1] = HyperlinkLine::new("…".dim().into());
        }
        let details_start = prompt_start;
        let mut lines = wrap(plain_hyperlink_lines(lines));
        lines.extend(prompt);
        if self.state().connection_notice.is_none() {
            let mut details = plain_hyperlink_lines(row.details.lines.clone());
            if let Some((message, cwd)) = &row.details.last_message {
                details.extend(plain_hyperlink_lines(vec![
                    Line::default(),
                    "Last message".dim().into(),
                ]));
                details.extend(
                    crate::markdown_render::render_markdown_lines_with_width_and_cwd(
                        &crate::markdown::normalize_markdown_for_rendering(message),
                        Some(width),
                        Some(cwd.as_path()),
                    ),
                );
            }
            let mut details = wrap(details);
            if !row.details.usage_lines.is_empty()
                && details.len() > usize::from(area.height).saturating_sub(lines.len())
            {
                // Activity and usage take precedence over repeating the original prompt.
                lines.truncate(prompt_start);
            }
            let available = usize::from(area.height).saturating_sub(lines.len());
            if details.len() > available {
                details.truncate(available);
                if let Some(last) = details.last_mut() {
                    *last = HyperlinkLine::new("…".dim().into());
                }
            }
            lines.splice(details_start..details_start, details);
        }
        Paragraph::new(visible_lines(lines.clone())).render(area, buf);
        mark_buffer_hyperlinks(buf, area, &lines, /*scroll_rows*/ 0);
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
        let state = self.state();
        if state.rename_target.is_some() {
            let contexts = KeymapContextSet::new(state.input.keymap_context());
            if state.input.is_vim_operator_pending() {
                contexts
            } else {
                contexts
                    .with(KeymapContext::List)
                    .with(KeymapContext::Agents)
            }
        } else {
            KeymapContextSet::new(KeymapContext::List).with(KeymapContext::Agents)
        }
    }

    fn completion(&self) -> Option<ViewCompletion> {
        self.state().completion
    }

    fn is_complete(&self) -> bool {
        self.completion().is_some()
    }

    fn prefer_esc_to_handle_key_event(&self) -> bool {
        true
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        let mut state = self.state();
        if state.editing_metadata() {
            state.searching = false;
            state.rename_target = None;
            state.search.clear();
            state.input.set_text_clearing_elements("");
            drop(state);
            self.reconcile_command_center_selection();
            return CancellationEvent::Handled;
        }
        CancellationEvent::NotHandled
    }

    fn handle_paste(&mut self, pasted: String) -> bool {
        if self.state().rename_target.is_some() {
            self.state()
                .input
                .insert_str(&crate::history_cell::sanitize_user_text(pasted.into()));
            return true;
        }
        if self.state().editing_metadata() {
            return self.edit_input(|input| {
                input.push_str(&crate::history_cell::sanitize_user_text(pasted.into()))
            });
        }
        false
    }

    fn handle_key_event(&mut self, mut key: KeyEvent) {
        // Terminals encode Shift-Tab as either BackTab or Tab with the shift modifier.
        if key.code == KeyCode::BackTab {
            key.code = KeyCode::Tab;
            key.modifiers.insert(KeyModifiers::SHIFT);
        }
        if key.kind == crossterm::event::KeyEventKind::Release {
            return;
        }
        if self.rename_key(key) || self.command_center_key(key) {
            return;
        }
        if key.code == KeyCode::Backspace
            && self.state().editing_metadata()
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
            && self.state().editing_metadata()
        {
            self.edit_input(|input| input.push(character));
            return;
        }

        if self.agents_keymap.search.is_pressed(key) {
            let mut state = self.state();
            if state.rename_target.is_none() {
                state.searching = !state.searching;
                if !state.searching {
                    state.search.clear();
                }
            }
            return;
        }

        if (self.state().connection_notice.is_some() || self.state().creating_worktree)
            && self.keymap.action_for(key) != Some(ListAction::Cancel)
        {
            match self.keymap.action_for(key) {
                Some(ListAction::MoveUp) => self.move_selection(/*forward*/ false),
                Some(ListAction::MoveDown) => self.move_selection(/*forward*/ true),
                Some(action @ (ListAction::PageUp | ListAction::PageDown)) => {
                    self.page_selection(action)
                }
                _ => {}
            }
            return;
        }

        if self.agents_keymap.resume.is_pressed(key) {
            self.app_event_tx.send(AppEvent::OpenResumePicker);
            return;
        }
        if self.agents_keymap.toggle_grouping.is_pressed(key) {
            let mut state = self.state();
            state.grouping = match state.grouping {
                AgentsOverviewGrouping::Project => AgentsOverviewGrouping::Status,
                AgentsOverviewGrouping::Status => AgentsOverviewGrouping::Model,
                AgentsOverviewGrouping::Model => AgentsOverviewGrouping::Project,
            };
            self.app_event_tx
                .send(AppEvent::PersistAgentsOverviewGrouping(state.grouping));
            return;
        }
        if self.agents_keymap.new_task.is_pressed(key) {
            self.app_event_tx.send(AppEvent::NewAgentsOverviewSession {
                cwd: self.selected_row().map(|row| row.thread.cwd.clone()),
            });
            return;
        }
        if self.agents_keymap.new_worktree.is_pressed(key) {
            if self.worktrees_enabled {
                self.app_event_tx.send(AppEvent::NewAgentsOverviewWorktree {
                    cwd: self.selected_row().map(|row| row.thread.cwd.clone()),
                });
            }
            return;
        }
        if self.agents_keymap.fork.is_pressed(key) {
            if let Some(row) = self.selected_row() {
                self.app_event_tx.send(AppEvent::ForkAgentsOverviewThread {
                    thread_id: row.thread_id,
                });
            }
            return;
        }
        if self.agents_keymap.rename.is_pressed(key) {
            if let Some(row) = self.selected_row() {
                let mut state = self.state();
                if state.rename_target.is_none() {
                    state.set_rename_input(
                        row.thread.name.as_deref().unwrap_or_default(),
                        &self.editor_keymap,
                    );
                    state.search.clear();
                    state.searching = false;
                    state.rename_target = Some(row.thread_id);
                }
            }
            return;
        }
        for (bindings, action) in [
            (&self.agents_keymap.archive, AgentsOverviewAction::Archive),
            (&self.agents_keymap.delete, AgentsOverviewAction::Delete),
        ] {
            if bindings.is_pressed(key) {
                if let Some(row) = self.selected_row() {
                    self.app_event_tx
                        .send(AppEvent::ConfirmAgentsOverviewAction {
                            thread_id: row.thread_id,
                            action,
                        });
                }
                return;
            }
        }
        if self.agents_keymap.hide.is_pressed(key) {
            if let Some(row) = self.selected_row() {
                let thread_id = row.thread_id;
                self.app_event_tx
                    .send(AppEvent::HideAgentsOverviewThread { thread_id });
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
            if self.state().rename_target.is_some()
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
                    if matches!(self.on_ctrl_c(), CancellationEvent::NotHandled) {
                        self.state().completion = Some(ViewCompletion::Cancelled);
                    }
                }
                ListAction::PageUp | ListAction::PageDown => {
                    self.page_selection(action);
                }
                ListAction::MoveRight if !self.state().editing_metadata() => self.activate(),
                ListAction::MoveLeft | ListAction::MoveRight => {}
            }
        } else if key.code == KeyCode::Backspace {
            self.edit_input(|input| {
                input.pop();
            });
        }
    }
}
