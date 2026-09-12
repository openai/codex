//! Dashboard for inspecting and managing the TUI's retained daemon tasks.
//! The shared view state retains the new-task editor across metadata refreshes.

#[path = "agents_overview_grouping.rs"]
mod grouping;
#[path = "agents_overview_input.rs"]
mod input;
#[path = "agents_overview_render.rs"]
mod render;

pub(super) use grouping::AgentsOverviewGrouping;
use grouping::model_name;

use super::agents_overview::AGENTS_OVERVIEW_VIEW_ID;
use super::agents_overview_details::AgentsOverviewDetails;
use crate::app_event::AgentsOverviewAction;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::BottomPaneView;
use crate::bottom_pane::CancellationEvent;
use crate::bottom_pane::ChatComposer;
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
use std::path::PathBuf;
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
    pub(super) details: AgentsOverviewDetails,
    pub(super) thread: Thread,
    pub(super) thread_id: ThreadId,
    pub(super) group: AgentsOverviewGroup,
    pub(super) is_current: bool,
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
    // Search and rename never borrow the new-task draft.
    pub(super) input: String,
    pub(super) composer: Option<ChatComposer>,
    pub(super) key_chord_hint: Option<Vec<(String, String)>>,
    pub(super) focus: AgentsOverviewFocus,
    pub(super) refresh_failed: bool,
    pub(super) connection_notice: Option<&'static str>,
    pub(super) server_version_notice: Option<String>,
    search: String,
    searching: bool,
    pub(super) grouping: AgentsOverviewGrouping,
    pub(super) renaming: bool,
    // The picker can finish this retained view when it selects the already active session.
    pub(super) completion: Option<ViewCompletion>,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum AgentsOverviewFocus {
    #[default]
    Composer,
    List,
}

impl AgentsOverviewViewState {
    pub(super) fn focus_composer(&mut self) {
        self.focus = AgentsOverviewFocus::Composer;
        if let Some(composer) = self.composer.as_mut() {
            composer.resume_text_entry();
        }
    }

    fn editing_metadata(&self) -> bool {
        self.searching || self.renaming
    }

    fn composing(&self) -> bool {
        self.focus == AgentsOverviewFocus::Composer && !self.searching && !self.renaming
    }

    fn composer_owns_escape(&self) -> bool {
        self.composer.as_ref().is_some_and(|composer| {
            composer.popup_active()
                || (composer.is_vim_enabled()
                    && !composer
                        .keymap_contexts()
                        .contains(KeymapContext::VimNormal))
        })
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
    composer_hints: Vec<(String, String)>,
    composer_keymap: crate::keymap::ComposerKeymap,
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
        let selected = selected_thread_id
            .and_then(|thread_id| rows.iter().position(|row| row.thread_id == thread_id))
            .or_else(|| rows.iter().position(|row| row.is_current))
            .unwrap_or(0);
        let composer_hints = [
            (KeymapContext::Composer, "submit", "create task"),
            (KeymapContext::Editor, "insert_newline", "newline"),
        ]
        .into_iter()
        .filter_map(|(context, action, label)| {
            keymap
                .primary_hint(context, action)
                .map(|hint| (hint.display_label().replace(" + ", "+"), label.to_string()))
        })
        .chain([("esc".to_string(), "tasks".to_string())])
        .collect();
        let project_groups = rows
            .iter()
            .map(|row| AgentsOverviewProjectGroup::for_thread(&row.thread, worktrees_enabled))
            .collect();
        let mut view = Self {
            use_theme_colors,
            rows,
            project_groups,
            selected,
            state,
            app_event_tx,
            keymap: keymap.list,
            agents_keymap: keymap.agents,
            composer_hints,
            composer_keymap: keymap.composer,
        };
        view.state().completion = None;
        let visible = view.visible_indices();
        if !visible.contains(&view.selected) {
            view.selected = visible.first().copied().unwrap_or(usize::MAX);
        }
        view
    }

    pub(super) fn thread_ids(&self) -> Vec<ThreadId> {
        self.rows.iter().map(|row| row.thread_id).collect()
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
        let input = self.state().input.clone();
        if self.state().renaming && !input.trim().is_empty() {
            if let Some(row) = self.selected_row() {
                self.app_event_tx
                    .send(AppEvent::RenameAgentsOverviewThread {
                        thread_id: row.thread_id,
                        name: input.trim().to_string(),
                    });
            }
            self.state().renaming = false;
            self.state().input.clear();
        } else if let Some(row) = self.selected_row().filter(|_| !self.state().renaming) {
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
        let mut previous_group_index: Option<usize> = None;
        let grouping = self.state().grouping;
        let visible = self.visible_indices();
        let mut first = visible
            .iter()
            .position(|index| *index == self.selected)
            .unwrap_or_default();
        let mut height = 2;
        while first > 0 {
            let previous_index = visible[first - 1];
            let current_index = visible[first];
            let group_changed = !self.same_group(grouping, previous_index, current_index);
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
            let group = match grouping {
                AgentsOverviewGrouping::Project => {
                    self.project_groups[index].heading.display().to_string()
                }
                AgentsOverviewGrouping::Status => row.group.label().to_string(),
                AgentsOverviewGrouping::Model => model_name(&row.thread).to_string(),
            };
            let group_changed = previous_group_index
                .is_none_or(|previous_index| !self.same_group(grouping, previous_index, index));
            if group_changed {
                offset += u16::from(previous_group_index.is_some());
                if offset >= area.height {
                    break;
                }
                let count = self
                    .rows
                    .iter()
                    .enumerate()
                    .filter(|(candidate_index, _)| {
                        self.same_group(grouping, *candidate_index, index)
                    })
                    .count();
                Line::from(vec![group.clone().bold(), format!("  {count}").dim()])
                    .render(Rect::new(area.x, area.y + offset, area.width, 1), buf);
                offset += 1;
                previous_group_index = Some(index);
            }
            if offset >= area.height {
                break;
            }
            let marker = if self.selected == index {
                "›".cyan().bold()
            } else {
                " ".into()
            };
            let (status, dot) = Self::status(row);
            let current = if row.is_current { "  current" } else { "" };
            let mut spans = vec![
                marker,
                " ".into(),
                dot,
                " ".into(),
                Span::styled(display_title(&row.thread), self.title_style(row.thread_id)),
                current.dim(),
            ];
            if grouping != AgentsOverviewGrouping::Status {
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
        let width = usize::from(area.width);
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
            Line::from("Project".dim()),
            Line::from(row.thread.cwd.display().to_string()),
            Line::from(vec![
                "Model: ".dim(),
                model_name(&row.thread).to_string().into(),
            ]),
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
        let preview = super::agents_overview_details::preview_markdown(&row.thread.preview);
        let prompt_start = crate::wrapping::word_wrap_lines(lines.clone(), width).len();
        lines.extend([Line::default(), Line::from("Prompt".dim())]);
        let prompt = crate::markdown_render::render_markdown_text_with_width_and_cwd(
            match preview.as_str() {
                "" => "No prompt available.",
                preview => preview,
            },
            Some(width),
            Some(row.thread.cwd.as_path()),
        )
        .lines;
        let mut prompt = crate::wrapping::word_wrap_lines(prompt, width);
        if prompt.len() > 2 {
            prompt.truncate(2);
            prompt[1] = "…".dim().into();
        }
        lines.extend(prompt);
        let details_start = crate::wrapping::word_wrap_lines(lines[..4].to_vec(), width).len();
        let mut lines = crate::wrapping::word_wrap_lines(lines, width);
        if self.state().connection_notice.is_none() {
            let mut details = row.details.lines.clone();
            if let Some((message, cwd)) = &row.details.last_message {
                details.extend([Line::default(), "Last message".dim().into()]);
                crate::markdown::append_markdown(
                    &crate::markdown::unwrap_markdown_fences(message),
                    Some(width),
                    Some(cwd.as_path()),
                    &mut details,
                );
            }
            let mut details = crate::wrapping::word_wrap_lines(details, width);
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
                    *last = "…".dim().into();
                }
            }
            lines.splice(details_start..details_start, details);
        }
        Paragraph::new(lines).render(area, buf);
    }
}

impl BottomPaneView for AgentsOverviewView {
    fn next_frame_delay(&self) -> Option<std::time::Duration> {
        self.state().composer.as_ref()?.footer_flash_delay()
    }

    fn view_id(&self) -> Option<&'static str> {
        Some(AGENTS_OVERVIEW_VIEW_ID)
    }

    fn selected_index(&self) -> Option<usize> {
        Some(self.selected)
    }

    fn keymap_contexts(&self) -> KeymapContextSet {
        let state = self.state();
        if state.composing() {
            state
                .composer
                .as_ref()
                .map_or_else(KeymapContextSet::default, ChatComposer::keymap_contexts)
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
            state.renaming = false;
            state.search.clear();
            state.input.clear();
            return CancellationEvent::Handled;
        }
        if let Some(composer) = state.composer.as_mut()
            && (composer.cancel_vim_search()
                || composer.cancel_history_search()
                || composer.clear_for_ctrl_c().is_some())
        {
            return CancellationEvent::Handled;
        }
        CancellationEvent::NotHandled
    }

    fn handle_paste(&mut self, pasted: String) -> bool {
        if self.state().editing_metadata() {
            return self.edit_input(|input| {
                input.push_str(&crate::history_cell::sanitize_user_text(pasted.into()))
            });
        }
        let mut state = self.state();
        if state.focus == AgentsOverviewFocus::List {
            state.focus_composer();
        }
        state
            .composer
            .as_mut()
            .is_some_and(|composer| composer.handle_paste(pasted))
    }

    fn flush_paste_burst_if_due(&mut self) -> bool {
        self.state()
            .composer
            .as_mut()
            .is_some_and(ChatComposer::flush_paste_burst_if_due)
    }

    fn is_in_paste_burst(&self) -> bool {
        self.state()
            .composer
            .as_ref()
            .is_some_and(ChatComposer::is_in_paste_burst)
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        if key.kind == crossterm::event::KeyEventKind::Release {
            return;
        }
        if self.state().composing() {
            self.handle_composer_key(key);
            return;
        }
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
            if self.state().editing_metadata() {
                self.edit_input(|input| input.push(character));
                return;
            }
            if !self
                .state()
                .composer
                .as_ref()
                .is_some_and(ChatComposer::is_vim_enabled)
            {
                self.state().focus_composer();
                self.handle_composer_key(key);
                return;
            }
        }

        if self.agents_keymap.search.is_pressed(key) || {
            let state = self.state();
            state.connection_notice.is_some()
                && state.searching
                && self.keymap.action_for(key) == Some(ListAction::Cancel)
        } {
            let mut state = self.state();
            if !state.renaming {
                state.searching = !state.searching;
                if !state.searching {
                    state.search.clear();
                }
            }
            return;
        }

        if self.state().connection_notice.is_some()
            && !self.agents_keymap.new_task.is_pressed(key)
            && self.keymap.action_for(key) != Some(ListAction::Cancel)
        {
            match self.keymap.action_for(key) {
                Some(ListAction::MoveUp) => self.move_selection(/*forward*/ false),
                Some(ListAction::MoveDown) => self.move_selection(/*forward*/ true),
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
            return;
        }
        if self.agents_keymap.new_task.is_pressed(key) {
            let mut state = self.state();
            state.search.clear();
            state.searching = false;
            state.renaming = false;
            state.input.clear();
            state.focus_composer();
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
                self.app_event_tx.send(AppEvent::HideAgentsOverviewThread {
                    thread_id: row.thread_id,
                });
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
                        state.focus_composer();
                    }
                }
                ListAction::PageUp | ListAction::PageDown => {
                    for _ in 0..5 {
                        self.move_selection(action == ListAction::PageDown);
                    }
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
