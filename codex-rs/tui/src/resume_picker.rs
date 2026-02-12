use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use crate::diff_render::display_path_for;
use crate::key_hint;
use crate::text_formatting::truncate_text;
use crate::tui::FrameRequester;
use crate::tui::Tui;
use crate::tui::TuiEvent;
use chrono::DateTime;
use chrono::Utc;
use codex_core::Cursor;
use codex_core::INTERACTIVE_SESSION_SOURCES;
use codex_core::RolloutRecorder;
use codex_core::ThreadItem;
use codex_core::ThreadSortKey;
use codex_core::ThreadsPage;
use codex_core::config::Config;
use codex_core::find_thread_names_by_ids;
use codex_core::path_utils;
use codex_protocol::ThreadId;
use color_eyre::eyre::Result;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Stylize as _;
use ratatui::text::Line;
use ratatui::text::Span;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::UnboundedReceiverStream;
use unicode_width::UnicodeWidthStr;

const PAGE_SIZE: usize = 25;
const LOAD_NEAR_THRESHOLD: usize = 5;
#[derive(Debug, Clone)]
pub enum SessionSelection {
    StartFresh,
    Resume(PathBuf),
    Fork(PathBuf),
    Exit,
}

#[derive(Clone, Copy, Debug)]
pub enum SessionPickerAction {
    Resume,
    Fork,
}

impl SessionPickerAction {
    fn title(self) -> &'static str {
        match self {
            SessionPickerAction::Resume => "Resume a previous session",
            SessionPickerAction::Fork => "Fork a previous session",
        }
    }

    fn action_label(self) -> &'static str {
        match self {
            SessionPickerAction::Resume => "resume",
            SessionPickerAction::Fork => "fork",
        }
    }

    fn selection(self, path: PathBuf) -> SessionSelection {
        match self {
            SessionPickerAction::Resume => SessionSelection::Resume(path),
            SessionPickerAction::Fork => SessionSelection::Fork(path),
        }
    }
}

#[derive(Clone)]
struct PageLoadRequest {
    cursor: Option<Cursor>,
    request_token: usize,
    search_token: Option<usize>,
    default_provider: String,
    sort_key: ThreadSortKey,
}

type PageLoader = Arc<dyn Fn(PageLoadRequest) + Send + Sync>;

enum BackgroundEvent {
    PageLoaded {
        request_token: usize,
        search_token: Option<usize>,
        page: std::io::Result<ThreadsPage>,
    },
}

/// Interactive session picker that lists recorded rollout files with simple
/// search and pagination.
///
/// The picker displays sessions in a table with timestamp columns (created/updated),
/// git branch, working directory, and conversation preview. Users can toggle
/// between sorting by creation time and last-updated time using the Tab key.
///
/// Sessions are loaded on-demand via cursor-based pagination. The backend
/// `RolloutRecorder::list_threads` returns pages ordered by the selected sort key,
/// and the picker deduplicates across pages to handle overlapping windows when
/// new sessions appear during pagination.
///
/// Filtering happens in two layers:
/// 1. Provider and source filtering at the backend (only interactive CLI sessions
///    for the current model provider).
/// 2. Working-directory filtering at the picker (unless `--all` is passed).
pub async fn run_resume_picker(
    tui: &mut Tui,
    config: &Config,
    show_all: bool,
) -> Result<SessionSelection> {
    run_session_picker(tui, config, show_all, SessionPickerAction::Resume).await
}

pub async fn run_fork_picker(
    tui: &mut Tui,
    config: &Config,
    show_all: bool,
) -> Result<SessionSelection> {
    run_session_picker(tui, config, show_all, SessionPickerAction::Fork).await
}

async fn run_session_picker(
    tui: &mut Tui,
    config: &Config,
    show_all: bool,
    action: SessionPickerAction,
) -> Result<SessionSelection> {
    let alt = AltScreenGuard::enter(tui);
    let (bg_tx, bg_rx) = mpsc::unbounded_channel();

    let default_provider = config.model_provider_id.to_string();
    let codex_home = config.codex_home.as_path();
    let filter_cwd = if show_all {
        None
    } else {
        std::env::current_dir().ok()
    };

    let config = config.clone();
    let loader_tx = bg_tx.clone();
    let page_loader: PageLoader = Arc::new(move |request: PageLoadRequest| {
        let tx = loader_tx.clone();
        let config = config.clone();
        tokio::spawn(async move {
            let provider_filter = vec![request.default_provider.clone()];
            let page = RolloutRecorder::list_threads(
                &config,
                PAGE_SIZE,
                request.cursor.as_ref(),
                request.sort_key,
                INTERACTIVE_SESSION_SOURCES,
                Some(provider_filter.as_slice()),
                request.default_provider.as_str(),
            )
            .await;
            let _ = tx.send(BackgroundEvent::PageLoaded {
                request_token: request.request_token,
                search_token: request.search_token,
                page,
            });
        });
    });

    let mut state = PickerState::new(
        codex_home.to_path_buf(),
        alt.tui.frame_requester(),
        page_loader,
        default_provider.clone(),
        show_all,
        filter_cwd,
        action,
    );
    state.start_initial_load();
    state.request_frame();

    let mut tui_events = alt.tui.event_stream().fuse();
    let mut background_events = UnboundedReceiverStream::new(bg_rx).fuse();

    loop {
        tokio::select! {
            Some(ev) = tui_events.next() => {
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
                        if let Ok(size) = alt.tui.terminal.size() {
                            let list_height = size.height.saturating_sub(4) as usize;
                            state.update_view_rows(list_height);
                            state.ensure_minimum_rows_for_view(list_height);
                        }
                        draw_picker(alt.tui, &state)?;
                    }
                    _ => {}
                }
            }
            Some(event) = background_events.next() => {
                state.handle_background_event(event).await?;
            }
            else => break,
        }
    }

    // Fallback – treat as cancel/new
    Ok(SessionSelection::StartFresh)
}

/// Returns the human-readable column header for the given sort key.
fn sort_key_label(sort_key: ThreadSortKey) -> &'static str {
    match sort_key {
        ThreadSortKey::CreatedAt => "Created at",
        ThreadSortKey::UpdatedAt => "Updated at",
    }
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
    pagination: PaginationState,
    all_rows: Vec<Row>,
    filtered_rows: Vec<Row>,
    seen_paths: HashSet<PathBuf>,
    selected: usize,
    scroll_top: usize,
    query: String,
    search_state: SearchState,
    next_request_token: usize,
    next_search_token: usize,
    page_loader: PageLoader,
    view_rows: Option<usize>,
    default_provider: String,
    show_all: bool,
    filter_cwd: Option<PathBuf>,
    action: SessionPickerAction,
    sort_key: ThreadSortKey,
    thread_name_cache: HashMap<ThreadId, Option<String>>,
}

struct PaginationState {
    next_cursor: Option<Cursor>,
    num_scanned_files: usize,
    reached_scan_cap: bool,
    loading: LoadingState,
}

#[derive(Clone, Copy, Debug)]
enum LoadingState {
    Idle,
    Pending(PendingLoad),
}

#[derive(Clone, Copy, Debug)]
struct PendingLoad {
    request_token: usize,
    search_token: Option<usize>,
}

#[derive(Clone, Copy, Debug)]
enum SearchState {
    Idle,
    Active { token: usize },
}

enum LoadTrigger {
    Scroll,
    Search { token: usize },
}

impl LoadingState {
    fn is_pending(&self) -> bool {
        matches!(self, LoadingState::Pending(_))
    }
}

impl SearchState {
    fn active_token(&self) -> Option<usize> {
        match self {
            SearchState::Idle => None,
            SearchState::Active { token } => Some(*token),
        }
    }

    fn is_active(&self) -> bool {
        self.active_token().is_some()
    }
}

#[derive(Clone)]
struct Row {
    path: PathBuf,
    preview: String,
    thread_id: Option<ThreadId>,
    thread_name: Option<String>,
    created_at: Option<DateTime<Utc>>,
    updated_at: Option<DateTime<Utc>>,
    cwd: Option<PathBuf>,
    git_branch: Option<String>,
}

impl Row {
    fn display_preview(&self) -> &str {
        self.thread_name.as_deref().unwrap_or(&self.preview)
    }

    fn matches_query(&self, query: &str) -> bool {
        if self.preview.to_lowercase().contains(query) {
            return true;
        }
        if let Some(thread_name) = self.thread_name.as_ref()
            && thread_name.to_lowercase().contains(query)
        {
            return true;
        }
        false
    }
}

impl PickerState {
    fn new(
        codex_home: PathBuf,
        requester: FrameRequester,
        page_loader: PageLoader,
        default_provider: String,
        show_all: bool,
        filter_cwd: Option<PathBuf>,
        action: SessionPickerAction,
    ) -> Self {
        Self {
            codex_home,
            requester,
            pagination: PaginationState {
                next_cursor: None,
                num_scanned_files: 0,
                reached_scan_cap: false,
                loading: LoadingState::Idle,
            },
            all_rows: Vec::new(),
            filtered_rows: Vec::new(),
            seen_paths: HashSet::new(),
            selected: 0,
            scroll_top: 0,
            query: String::new(),
            search_state: SearchState::Idle,
            next_request_token: 0,
            next_search_token: 0,
            page_loader,
            view_rows: None,
            default_provider,
            show_all,
            filter_cwd,
            action,
            sort_key: ThreadSortKey::CreatedAt,
            thread_name_cache: HashMap::new(),
        }
    }

    fn request_frame(&self) {
        self.requester.schedule_frame();
    }

    async fn handle_key(&mut self, key: KeyEvent) -> Result<Option<SessionSelection>> {
        match key.code {
            KeyCode::Esc => return Ok(Some(SessionSelection::StartFresh)),
            KeyCode::Char('c')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                return Ok(Some(SessionSelection::Exit));
            }
            KeyCode::Enter => {
                if let Some(row) = self.filtered_rows.get(self.selected) {
                    return Ok(Some(self.action.selection(row.path.clone())));
                }
            }
            KeyCode::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                    self.ensure_selected_visible();
                }
                self.request_frame();
            }
            KeyCode::Down => {
                if self.selected + 1 < self.filtered_rows.len() {
                    self.selected += 1;
                    self.ensure_selected_visible();
                }
                self.maybe_load_more_for_scroll();
                self.request_frame();
            }
            KeyCode::PageUp => {
                let step = self.view_rows.unwrap_or(10).max(1);
                if self.selected > 0 {
                    self.selected = self.selected.saturating_sub(step);
                    self.ensure_selected_visible();
                    self.request_frame();
                }
            }
            KeyCode::PageDown => {
                if !self.filtered_rows.is_empty() {
                    let step = self.view_rows.unwrap_or(10).max(1);
                    let max_index = self.filtered_rows.len().saturating_sub(1);
                    self.selected = (self.selected + step).min(max_index);
                    self.ensure_selected_visible();
                    self.maybe_load_more_for_scroll();
                    self.request_frame();
                }
            }
            KeyCode::Tab => {
                self.toggle_sort_key();
                self.request_frame();
            }
            KeyCode::Backspace => {
                let mut new_query = self.query.clone();
                new_query.pop();
                self.set_query(new_query);
            }
            KeyCode::Char(c) => {
                // basic text input for search
                if !key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL)
                    && !key.modifiers.contains(crossterm::event::KeyModifiers::ALT)
                {
                    let mut new_query = self.query.clone();
                    new_query.push(c);
                    self.set_query(new_query);
                }
            }
            _ => {}
        }
        Ok(None)
    }

    fn start_initial_load(&mut self) {
        self.reset_pagination();
        self.all_rows.clear();
        self.filtered_rows.clear();
        self.seen_paths.clear();
        self.selected = 0;

        let search_token = if self.query.is_empty() {
            self.search_state = SearchState::Idle;
            None
        } else {
            let token = self.allocate_search_token();
            self.search_state = SearchState::Active { token };
            Some(token)
        };

        let request_token = self.allocate_request_token();
        self.pagination.loading = LoadingState::Pending(PendingLoad {
            request_token,
            search_token,
        });
        self.request_frame();

        (self.page_loader)(PageLoadRequest {
            cursor: None,
            request_token,
            search_token,
            default_provider: self.default_provider.clone(),
            sort_key: self.sort_key,
        });
    }

    async fn handle_background_event(&mut self, event: BackgroundEvent) -> Result<()> {
        match event {
            BackgroundEvent::PageLoaded {
                request_token,
                search_token,
                page,
            } => {
                let pending = match self.pagination.loading {
                    LoadingState::Pending(pending) => pending,
                    LoadingState::Idle => return Ok(()),
                };
                if pending.request_token != request_token {
                    return Ok(());
                }
                self.pagination.loading = LoadingState::Idle;
                let page = page.map_err(color_eyre::Report::from)?;
                self.ingest_page(page);
                self.update_thread_names().await;
                let completed_token = pending.search_token.or(search_token);
                self.continue_search_if_token_matches(completed_token);
            }
        }
        Ok(())
    }

    fn reset_pagination(&mut self) {
        self.pagination.next_cursor = None;
        self.pagination.num_scanned_files = 0;
        self.pagination.reached_scan_cap = false;
        self.pagination.loading = LoadingState::Idle;
    }

    fn ingest_page(&mut self, page: ThreadsPage) {
        if let Some(cursor) = page.next_cursor.clone() {
            self.pagination.next_cursor = Some(cursor);
        } else {
            self.pagination.next_cursor = None;
        }
        self.pagination.num_scanned_files = self
            .pagination
            .num_scanned_files
            .saturating_add(page.num_scanned_files);
        if page.reached_scan_cap {
            self.pagination.reached_scan_cap = true;
        }

        let rows = rows_from_items(page.items);
        for row in rows {
            if self.seen_paths.insert(row.path.clone()) {
                self.all_rows.push(row);
            }
        }

        self.apply_filter();
    }

    async fn update_thread_names(&mut self) {
        let mut missing_ids = HashSet::new();
        for row in &self.all_rows {
            let Some(thread_id) = row.thread_id else {
                continue;
            };
            if self.thread_name_cache.contains_key(&thread_id) {
                continue;
            }
            missing_ids.insert(thread_id);
        }

        if missing_ids.is_empty() {
            return;
        }

        let names = find_thread_names_by_ids(&self.codex_home, &missing_ids)
            .await
            .unwrap_or_default();
        for thread_id in missing_ids {
            let thread_name = names.get(&thread_id).cloned();
            self.thread_name_cache.insert(thread_id, thread_name);
        }

        let mut updated = false;
        for row in self.all_rows.iter_mut() {
            let Some(thread_id) = row.thread_id else {
                continue;
            };
            let thread_name = self.thread_name_cache.get(&thread_id).cloned().flatten();
            if row.thread_name == thread_name {
                continue;
            }
            row.thread_name = thread_name;
            updated = true;
        }

        if updated {
            self.apply_filter();
        }
    }

    fn apply_filter(&mut self) {
        let base_iter = self
            .all_rows
            .iter()
            .filter(|row| self.row_matches_filter(row));
        if self.query.is_empty() {
            self.filtered_rows = base_iter.cloned().collect();
        } else {
            let q = self.query.to_lowercase();
            self.filtered_rows = base_iter.filter(|r| r.matches_query(&q)).cloned().collect();
        }
        if self.selected >= self.filtered_rows.len() {
            self.selected = self.filtered_rows.len().saturating_sub(1);
        }
        if self.filtered_rows.is_empty() {
            self.scroll_top = 0;
        }
        self.ensure_selected_visible();
        self.request_frame();
    }

    fn row_matches_filter(&self, row: &Row) -> bool {
        if self.show_all {
            return true;
        }
        let Some(filter_cwd) = self.filter_cwd.as_ref() else {
            return true;
        };
        let Some(row_cwd) = row.cwd.as_ref() else {
            return false;
        };
        paths_match(row_cwd, filter_cwd)
    }

    fn set_query(&mut self, new_query: String) {
        if self.query == new_query {
            return;
        }
        self.query = new_query;
        self.selected = 0;
        self.apply_filter();
        if self.query.is_empty() {
            self.search_state = SearchState::Idle;
            return;
        }
        if !self.filtered_rows.is_empty() {
            self.search_state = SearchState::Idle;
            return;
        }
        if self.pagination.reached_scan_cap || self.pagination.next_cursor.is_none() {
            self.search_state = SearchState::Idle;
            return;
        }
        let token = self.allocate_search_token();
        self.search_state = SearchState::Active { token };
        self.load_more_if_needed(LoadTrigger::Search { token });
    }

    fn continue_search_if_needed(&mut self) {
        let Some(token) = self.search_state.active_token() else {
            return;
        };
        if !self.filtered_rows.is_empty() {
            self.search_state = SearchState::Idle;
            return;
        }
        if self.pagination.reached_scan_cap || self.pagination.next_cursor.is_none() {
            self.search_state = SearchState::Idle;
            return;
        }
        self.load_more_if_needed(LoadTrigger::Search { token });
    }

    fn continue_search_if_token_matches(&mut self, completed_token: Option<usize>) {
        let Some(active) = self.search_state.active_token() else {
            return;
        };
        if let Some(token) = completed_token
            && token != active
        {
            return;
        }
        self.continue_search_if_needed();
    }

    fn ensure_selected_visible(&mut self) {
        if self.filtered_rows.is_empty() {
            self.scroll_top = 0;
            return;
        }
        let capacity = self.view_rows.unwrap_or(self.filtered_rows.len()).max(1);

        if self.selected < self.scroll_top {
            self.scroll_top = self.selected;
        } else {
            let last_visible = self.scroll_top.saturating_add(capacity - 1);
            if self.selected > last_visible {
                self.scroll_top = self.selected.saturating_sub(capacity - 1);
            }
        }

        let max_start = self.filtered_rows.len().saturating_sub(capacity);
        if self.scroll_top > max_start {
            self.scroll_top = max_start;
        }
    }

    fn ensure_minimum_rows_for_view(&mut self, minimum_rows: usize) {
        if minimum_rows == 0 {
            return;
        }
        if self.filtered_rows.len() >= minimum_rows {
            return;
        }
        if self.pagination.loading.is_pending() || self.pagination.next_cursor.is_none() {
            return;
        }
        if let Some(token) = self.search_state.active_token() {
            self.load_more_if_needed(LoadTrigger::Search { token });
        } else {
            self.load_more_if_needed(LoadTrigger::Scroll);
        }
    }

    fn update_view_rows(&mut self, rows: usize) {
        self.view_rows = if rows == 0 { None } else { Some(rows) };
        self.ensure_selected_visible();
    }

    fn maybe_load_more_for_scroll(&mut self) {
        if self.pagination.loading.is_pending() {
            return;
        }
        if self.pagination.next_cursor.is_none() {
            return;
        }
        if self.filtered_rows.is_empty() {
            return;
        }
        let remaining = self.filtered_rows.len().saturating_sub(self.selected + 1);
        if remaining <= LOAD_NEAR_THRESHOLD {
            self.load_more_if_needed(LoadTrigger::Scroll);
        }
    }

    fn load_more_if_needed(&mut self, trigger: LoadTrigger) {
        if self.pagination.loading.is_pending() {
            return;
        }
        let Some(cursor) = self.pagination.next_cursor.clone() else {
            return;
        };
        let request_token = self.allocate_request_token();
        let search_token = match trigger {
            LoadTrigger::Scroll => None,
            LoadTrigger::Search { token } => Some(token),
        };
        self.pagination.loading = LoadingState::Pending(PendingLoad {
            request_token,
            search_token,
        });
        self.request_frame();

        (self.page_loader)(PageLoadRequest {
            cursor: Some(cursor),
            request_token,
            search_token,
            default_provider: self.default_provider.clone(),
            sort_key: self.sort_key,
        });
    }

    fn allocate_request_token(&mut self) -> usize {
        let token = self.next_request_token;
        self.next_request_token = self.next_request_token.wrapping_add(1);
        token
    }

    fn allocate_search_token(&mut self) -> usize {
        let token = self.next_search_token;
        self.next_search_token = self.next_search_token.wrapping_add(1);
        token
    }

    /// Cycles the sort order between creation time and last-updated time.
    ///
    /// Triggers a full reload because the backend must re-sort all sessions.
    /// The existing `all_rows` are cleared and pagination restarts from the
    /// beginning with the new sort key.
    fn toggle_sort_key(&mut self) {
        self.sort_key = match self.sort_key {
            ThreadSortKey::CreatedAt => ThreadSortKey::UpdatedAt,
            ThreadSortKey::UpdatedAt => ThreadSortKey::CreatedAt,
        };
        self.start_initial_load();
    }
}

fn rows_from_items(items: Vec<ThreadItem>) -> Vec<Row> {
    items.into_iter().map(|item| head_to_row(&item)).collect()
}

fn head_to_row(item: &ThreadItem) -> Row {
    let created_at = item.created_at.as_deref().and_then(parse_timestamp_str);
    let updated_at = item
        .updated_at
        .as_deref()
        .and_then(parse_timestamp_str)
        .or(created_at);

    let preview = item
        .first_user_message
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| String::from("(no message yet)"));

    Row {
        path: item.path.clone(),
        preview,
        thread_id: item.thread_id,
        thread_name: None,
        created_at,
        updated_at,
        cwd: item.cwd.clone(),
        git_branch: item.git_branch.clone(),
    }
}

fn paths_match(a: &Path, b: &Path) -> bool {
    if let (Ok(ca), Ok(cb)) = (
        path_utils::normalize_for_path_comparison(a),
        path_utils::normalize_for_path_comparison(b),
    ) {
        return ca == cb;
    }
    a == b
}

fn parse_timestamp_str(ts: &str) -> Option<DateTime<Utc>> {
    chrono::DateTime::parse_from_rfc3339(ts)
        .map(|dt| dt.with_timezone(&Utc))
        .ok()
}

fn draw_picker(tui: &mut Tui, state: &PickerState) -> std::io::Result<()> {
    // Render full-screen overlay
    let height = tui.terminal.size()?.height;
    tui.draw(height, |frame| {
        let area = frame.area();
        let [header, search, columns, list, hint] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(area.height.saturating_sub(4)),
            Constraint::Length(1),
        ])
        .areas(area);

        // Header
        let header_line: Line = vec![
            state.action.title().bold().cyan(),
            "  ".into(),
            "Sort:".dim(),
            " ".into(),
            sort_key_label(state.sort_key).magenta(),
        ]
        .into();
        frame.render_widget_ref(header_line, header);

        // Search line
        let q = if state.query.is_empty() {
            "Type to search".dim().to_string()
        } else {
            format!("Search: {}", state.query)
        };
        frame.render_widget_ref(Line::from(q), search);

        let metrics = calculate_column_metrics(&state.filtered_rows, state.show_all);

        // Column headers and list
        render_column_headers(frame, columns, &metrics, state.sort_key);
        render_list(frame, list, state, &metrics);

        // Hint line
        let action_label = state.action.action_label();
        let hint_line: Line = vec![
            key_hint::plain(KeyCode::Enter).into(),
            format!(" to {action_label} ").dim(),
            "    ".dim(),
            key_hint::plain(KeyCode::Esc).into(),
            " to start new ".dim(),
            "    ".dim(),
            key_hint::ctrl(KeyCode::Char('c')).into(),
            " to quit ".dim(),
            "    ".dim(),
            key_hint::plain(KeyCode::Tab).into(),
            " to toggle sort ".dim(),
            "    ".dim(),
            key_hint::plain(KeyCode::Up).into(),
            "/".dim(),
            key_hint::plain(KeyCode::Down).into(),
            " to browse".dim(),
        ]
        .into();
        frame.render_widget_ref(hint_line, hint);
    })
}

fn render_list(
    frame: &mut crate::custom_terminal::Frame,
    area: Rect,
    state: &PickerState,
    metrics: &ColumnMetrics,
) {
    if area.height == 0 {
        return;
    }

    let rows = &state.filtered_rows;
    if rows.is_empty() {
        let message = render_empty_state_line(state);
        frame.render_widget_ref(message, area);
        return;
    }

    let capacity = area.height as usize;
    let start = state.scroll_top.min(rows.len().saturating_sub(1));
    let end = rows.len().min(start + capacity);
    let labels = &metrics.labels;
    let mut y = area.y;

    let visibility = column_visibility(area.width, metrics, state.sort_key);
    let max_created_width = metrics.max_created_width;
    let max_updated_width = metrics.max_updated_width;
    let max_branch_width = metrics.max_branch_width;
    let max_cwd_width = metrics.max_cwd_width;

    for (idx, (row, (created_label, updated_label, branch_label, cwd_label))) in rows[start..end]
        .iter()
        .zip(labels[start..end].iter())
        .enumerate()
    {
        let is_sel = start + idx == state.selected;
        let marker = if is_sel { "> ".bold() } else { "  ".into() };
        let marker_width = 2usize;
        let created_span = if visibility.show_created {
            Some(Span::from(format!("{created_label:<max_created_width$}")).dim())
        } else {
            None
        };
        let updated_span = if visibility.show_updated {
            Some(Span::from(format!("{updated_label:<max_updated_width$}")).dim())
        } else {
            None
        };
        let branch_span = if !visibility.show_branch {
            None
        } else if branch_label.is_empty() {
            Some(
                Span::from(format!(
                    "{empty:<width$}",
                    empty = "-",
                    width = max_branch_width
                ))
                .dim(),
            )
        } else {
            Some(Span::from(format!("{branch_label:<max_branch_width$}")).cyan())
        };
        let cwd_span = if !visibility.show_cwd {
            None
        } else if cwd_label.is_empty() {
            Some(
                Span::from(format!(
                    "{empty:<width$}",
                    empty = "-",
                    width = max_cwd_width
                ))
                .dim(),
            )
        } else {
            Some(Span::from(format!("{cwd_label:<max_cwd_width$}")).dim())
        };

        let mut preview_width = area.width as usize;
        preview_width = preview_width.saturating_sub(marker_width);
        if visibility.show_created {
            preview_width = preview_width.saturating_sub(max_created_width + 2);
        }
        if visibility.show_updated {
            preview_width = preview_width.saturating_sub(max_updated_width + 2);
        }
        if visibility.show_branch {
            preview_width = preview_width.saturating_sub(max_branch_width + 2);
        }
        if visibility.show_cwd {
            preview_width = preview_width.saturating_sub(max_cwd_width + 2);
        }
        let add_leading_gap = !visibility.show_created
            && !visibility.show_updated
            && !visibility.show_branch
            && !visibility.show_cwd;
        if add_leading_gap {
            preview_width = preview_width.saturating_sub(2);
        }
        let preview = truncate_text(row.display_preview(), preview_width);
        let mut spans: Vec<Span> = vec![marker];
        if let Some(created) = created_span {
            spans.push(created);
            spans.push("  ".into());
        }
        if let Some(updated) = updated_span {
            spans.push(updated);
            spans.push("  ".into());
        }
        if let Some(branch) = branch_span {
            spans.push(branch);
            spans.push("  ".into());
        }
        if let Some(cwd) = cwd_span {
            spans.push(cwd);
            spans.push("  ".into());
        }
        if add_leading_gap {
            spans.push("  ".into());
        }
        spans.push(preview.into());

        let line: Line = spans.into();
        let rect = Rect::new(area.x, y, area.width, 1);
        frame.render_widget_ref(line, rect);
        y = y.saturating_add(1);
    }

    if state.pagination.loading.is_pending() && y < area.y.saturating_add(area.height) {
        let loading_line: Line = vec!["  ".into(), "Loading older sessions…".italic().dim()].into();
        let rect = Rect::new(area.x, y, area.width, 1);
        frame.render_widget_ref(loading_line, rect);
    }
}

fn render_empty_state_line(state: &PickerState) -> Line<'static> {
    if !state.query.is_empty() {
        if state.search_state.is_active()
            || (state.pagination.loading.is_pending() && state.pagination.next_cursor.is_some())
        {
            return vec!["Searching…".italic().dim()].into();
        }
        if state.pagination.reached_scan_cap {
            let msg = format!(
                "Search scanned first {} sessions; more may exist",
                state.pagination.num_scanned_files
            );
            return vec![Span::from(msg).italic().dim()].into();
        }
        return vec!["No results for your search".italic().dim()].into();
    }

    if state.all_rows.is_empty() && state.pagination.num_scanned_files == 0 {
        return vec!["No sessions yet".italic().dim()].into();
    }

    if state.pagination.loading.is_pending() {
        return vec!["Loading older sessions…".italic().dim()].into();
    }

    vec!["No sessions yet".italic().dim()].into()
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

fn format_updated_label(row: &Row) -> String {
    match (row.updated_at, row.created_at) {
        (Some(updated), _) => human_time_ago(updated),
        (None, Some(created)) => human_time_ago(created),
        (None, None) => "-".to_string(),
    }
}

fn format_created_label(row: &Row) -> String {
    match row.created_at {
        Some(created) => human_time_ago(created),
        None => "-".to_string(),
    }
}

fn render_column_headers(
    frame: &mut crate::custom_terminal::Frame,
    area: Rect,
    metrics: &ColumnMetrics,
    sort_key: ThreadSortKey,
) {
    if area.height == 0 {
        return;
    }

    let mut spans: Vec<Span> = vec!["  ".into()];
    let visibility = column_visibility(area.width, metrics, sort_key);
    if visibility.show_created {
        let label = format!(
            "{text:<width$}",
            text = "Created at",
            width = metrics.max_created_width
        );
        spans.push(Span::from(label).bold());
        spans.push("  ".into());
    }
    if visibility.show_updated {
        let label = format!(
            "{text:<width$}",
            text = "Updated at",
            width = metrics.max_updated_width
        );
        spans.push(Span::from(label).bold());
        spans.push("  ".into());
    }
    if visibility.show_branch {
        let label = format!(
            "{text:<width$}",
            text = "Branch",
            width = metrics.max_branch_width
        );
        spans.push(Span::from(label).bold());
        spans.push("  ".into());
    }
    if visibility.show_cwd {
        let label = format!(
            "{text:<width$}",
            text = "CWD",
            width = metrics.max_cwd_width
        );
        spans.push(Span::from(label).bold());
        spans.push("  ".into());
    }
    spans.push("Conversation".bold());
    frame.render_widget_ref(Line::from(spans), area);
}

/// Pre-computed column widths and formatted labels for all visible rows.
///
/// Widths are measured in Unicode display width (not byte length) so columns
/// align correctly when labels contain non-ASCII characters.
struct ColumnMetrics {
    max_created_width: usize,
    max_updated_width: usize,
    max_branch_width: usize,
    max_cwd_width: usize,
    /// (created_label, updated_label, branch_label, cwd_label) per row.
    labels: Vec<(String, String, String, String)>,
}

/// Determines which columns to render given available terminal width.
///
/// When the terminal is narrow, only one timestamp column is shown (whichever
/// matches the current sort key). Branch and CWD are hidden if their max
/// widths are zero (no data to show).
#[derive(Debug, PartialEq, Eq)]
struct ColumnVisibility {
    show_created: bool,
    show_updated: bool,
    show_branch: bool,
    show_cwd: bool,
}

fn calculate_column_metrics(rows: &[Row], include_cwd: bool) -> ColumnMetrics {
    fn right_elide(s: &str, max: usize) -> String {
        if s.chars().count() <= max {
            return s.to_string();
        }
        if max <= 1 {
            return "…".to_string();
        }
        let tail_len = max - 1;
        let tail: String = s
            .chars()
            .rev()
            .take(tail_len)
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        format!("…{tail}")
    }

    let mut labels: Vec<(String, String, String, String)> = Vec::with_capacity(rows.len());
    let mut max_created_width = UnicodeWidthStr::width("Created at");
    let mut max_updated_width = UnicodeWidthStr::width("Updated at");
    let mut max_branch_width = UnicodeWidthStr::width("Branch");
    let mut max_cwd_width = if include_cwd {
        UnicodeWidthStr::width("CWD")
    } else {
        0
    };

    for row in rows {
        let created = format_created_label(row);
        let updated = format_updated_label(row);
        let branch_raw = row.git_branch.clone().unwrap_or_default();
        let branch = right_elide(&branch_raw, 24);
        let cwd = if include_cwd {
            let cwd_raw = row
                .cwd
                .as_ref()
                .map(|p| display_path_for(p, std::path::Path::new("/")))
                .unwrap_or_default();
            right_elide(&cwd_raw, 24)
        } else {
            String::new()
        };
        max_created_width = max_created_width.max(UnicodeWidthStr::width(created.as_str()));
        max_updated_width = max_updated_width.max(UnicodeWidthStr::width(updated.as_str()));
        max_branch_width = max_branch_width.max(UnicodeWidthStr::width(branch.as_str()));
        max_cwd_width = max_cwd_width.max(UnicodeWidthStr::width(cwd.as_str()));
        labels.push((created, updated, branch, cwd));
    }

    ColumnMetrics {
        max_created_width,
        max_updated_width,
        max_branch_width,
        max_cwd_width,
        labels,
    }
}

/// Computes which columns fit in the available width.
///
/// The algorithm reserves at least `MIN_PREVIEW_WIDTH` characters for the
/// conversation preview. If both timestamp columns don't fit, only the one
/// matching the current sort key is shown.
fn column_visibility(
    area_width: u16,
    metrics: &ColumnMetrics,
    sort_key: ThreadSortKey,
) -> ColumnVisibility {
    const MIN_PREVIEW_WIDTH: usize = 10;

    let show_branch = metrics.max_branch_width > 0;
    let show_cwd = metrics.max_cwd_width > 0;

    // Calculate remaining width after all optional columns.
    let mut preview_width = area_width as usize;
    preview_width = preview_width.saturating_sub(2); // marker
    if metrics.max_created_width > 0 {
        preview_width = preview_width.saturating_sub(metrics.max_created_width + 2);
    }
    if metrics.max_updated_width > 0 {
        preview_width = preview_width.saturating_sub(metrics.max_updated_width + 2);
    }
    if show_branch {
        preview_width = preview_width.saturating_sub(metrics.max_branch_width + 2);
    }
    if show_cwd {
        preview_width = preview_width.saturating_sub(metrics.max_cwd_width + 2);
    }

    // If preview would be too narrow, hide the non-active timestamp column.
    let show_both = preview_width >= MIN_PREVIEW_WIDTH;
    let show_created = if show_both {
        metrics.max_created_width > 0
    } else {
        sort_key == ThreadSortKey::CreatedAt
    };
    let show_updated = if show_both {
        metrics.max_updated_width > 0
    } else {
        sort_key == ThreadSortKey::UpdatedAt
    };

    ColumnVisibility {
        show_created,
        show_updated,
        show_branch,
        show_cwd,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use codex_protocol::ThreadId;

    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;
    use crossterm::event::KeyModifiers;
    use insta::assert_snapshot;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::fs::FileTimes;
    use std::fs::OpenOptions;
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::Mutex;

    fn make_item(path: &str, ts: &str, preview: &str) -> ThreadItem {
        ThreadItem {
            path: PathBuf::from(path),
            thread_id: None,
            first_user_message: Some(preview.to_string()),
            cwd: None,
            git_branch: None,
            git_sha: None,
            git_origin_url: None,
            source: None,
            model_provider: None,
            cli_version: None,
            created_at: Some(ts.to_string()),
            updated_at: Some(ts.to_string()),
        }
    }

    fn cursor_from_str(repr: &str) -> Cursor {
        serde_json::from_str::<Cursor>(&format!("\"{repr}\""))
            .expect("cursor format should deserialize")
    }

    fn page(
        items: Vec<ThreadItem>,
        next_cursor: Option<Cursor>,
        num_scanned_files: usize,
        reached_scan_cap: bool,
    ) -> ThreadsPage {
        ThreadsPage {
            items,
            next_cursor,
            num_scanned_files,
            reached_scan_cap,
        }
    }

    #[allow(dead_code)]
    fn set_rollout_mtime(path: &Path, updated_at: DateTime<Utc>) {
        let times = FileTimes::new().set_modified(updated_at.into());
        OpenOptions::new()
            .append(true)
            .open(path)
            .expect("open rollout")
            .set_times(times)
            .expect("set times");
    }

    // TODO(jif) fix
    // #[tokio::test]
    // async fn resume_picker_orders_by_updated_at() {
    //     use uuid::Uuid;
    //
    //     let tempdir = tempfile::tempdir().expect("tempdir");
    //     let sessions_root = tempdir.path().join("sessions");
    //     std::fs::create_dir_all(&sessions_root).expect("mkdir sessions root");
    //
    //     let now = Utc::now();
    //
    //     let write_rollout = |ts: DateTime<Utc>, preview: &str| -> PathBuf {
    //         let dir = sessions_root
    //             .join(ts.format("%Y").to_string())
    //             .join(ts.format("%m").to_string())
    //             .join(ts.format("%d").to_string());
    //         std::fs::create_dir_all(&dir).expect("mkdir date dirs");
    //         let filename = format!(
    //             "rollout-{}-{}.jsonl",
    //             ts.format("%Y-%m-%dT%H-%M-%S"),
    //             Uuid::new_v4()
    //         );
    //         let path = dir.join(filename);
    //         let meta = SessionMeta {
    //             id: ThreadId::new(),
    //             forked_from_id: None,
    //             timestamp: ts.to_rfc3339(),
    //             cwd: PathBuf::from("/tmp"),
    //             originator: String::from("user"),
    //             cli_version: String::from("0.0.0"),
    //             source: SessionSource::Cli,
    //             model_provider: Some(String::from("openai")),
    //             base_instructions: None,
    //             dynamic_tools: None,
    //         };
    //         let meta_line = RolloutLine {
    //             timestamp: ts.to_rfc3339(),
    //             item: RolloutItem::SessionMeta(SessionMetaLine { meta, git: None }),
    //         };
    //         let user_line = RolloutLine {
    //             timestamp: ts.to_rfc3339(),
    //             item: RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
    //                 message: preview.to_string(),
    //                 images: None,
    //                 text_elements: Vec::new(),
    //                 local_images: Vec::new(),
    //             })),
    //         };
    //         let meta_json = serde_json::to_string(&meta_line).expect("serialize meta");
    //         let user_json = serde_json::to_string(&user_line).expect("serialize user");
    //         std::fs::write(&path, format!("{meta_json}\n{user_json}\n")).expect("write rollout");
    //         path
    //     };
    //
    //     let created_a = now - Duration::minutes(1);
    //     let created_b = now - Duration::minutes(2);
    //
    //     let path_a = write_rollout(created_a, "A (created newer)");
    //     let path_b = write_rollout(created_b, "B (created older)");
    //
    //     set_rollout_mtime(&path_a, now - Duration::minutes(10));
    //     set_rollout_mtime(&path_b, now - Duration::seconds(10));
    //
    //     let page = RolloutRecorder::list_threads(
    //         tempdir.path(),
    //         PAGE_SIZE,
    //         None,
    //         ThreadSortKey::UpdatedAt,
    //         INTERACTIVE_SESSION_SOURCES,
    //         Some(&[String::from("openai")]),
    //         "openai",
    //     )
    //     .await
    //     .expect("list threads");
    //
    //     let rows = rows_from_items(page.items);
    //     let previews: Vec<String> = rows.iter().map(|row| row.preview.clone()).collect();
    //
    //     assert_eq!(
    //         previews,
    //         vec![
    //             "B (created older)".to_string(),
    //             "A (created newer)".to_string()
    //         ]
    //     );
    // }

    #[test]
    fn head_to_row_uses_first_user_message() {
        let item = ThreadItem {
            path: PathBuf::from("/tmp/a.jsonl"),
            thread_id: None,
            first_user_message: Some("real question".to_string()),
            cwd: None,
            git_branch: None,
            git_sha: None,
            git_origin_url: None,
            source: None,
            model_provider: None,
            cli_version: None,
            created_at: Some("2025-01-01T00:00:00Z".into()),
            updated_at: Some("2025-01-01T00:00:00Z".into()),
        };
        let row = head_to_row(&item);
        assert_eq!(row.preview, "real question");
    }

    #[test]
    fn rows_from_items_preserves_backend_order() {
        // Construct two items with different timestamps and real user text.
        let a = ThreadItem {
            path: PathBuf::from("/tmp/a.jsonl"),
            thread_id: None,
            first_user_message: Some("A".to_string()),
            cwd: None,
            git_branch: None,
            git_sha: None,
            git_origin_url: None,
            source: None,
            model_provider: None,
            cli_version: None,
            created_at: Some("2025-01-01T00:00:00Z".into()),
            updated_at: Some("2025-01-01T00:00:00Z".into()),
        };
        let b = ThreadItem {
            path: PathBuf::from("/tmp/b.jsonl"),
            thread_id: None,
            first_user_message: Some("B".to_string()),
            cwd: None,
            git_branch: None,
            git_sha: None,
            git_origin_url: None,
            source: None,
            model_provider: None,
            cli_version: None,
            created_at: Some("2025-01-02T00:00:00Z".into()),
            updated_at: Some("2025-01-02T00:00:00Z".into()),
        };
        let rows = rows_from_items(vec![a, b]);
        assert_eq!(rows.len(), 2);
        // Preserve the given order even if timestamps differ; backend already provides newest-first.
        assert!(rows[0].preview.contains('A'));
        assert!(rows[1].preview.contains('B'));
    }

    #[test]
    fn row_uses_tail_timestamp_for_updated_at() {
        let item = ThreadItem {
            path: PathBuf::from("/tmp/a.jsonl"),
            thread_id: None,
            first_user_message: Some("Hello".to_string()),
            cwd: None,
            git_branch: None,
            git_sha: None,
            git_origin_url: None,
            source: None,
            model_provider: None,
            cli_version: None,
            created_at: Some("2025-01-01T00:00:00Z".into()),
            updated_at: Some("2025-01-01T01:00:00Z".into()),
        };

        let row = head_to_row(&item);
        let expected_created = chrono::DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let expected_updated = chrono::DateTime::parse_from_rfc3339("2025-01-01T01:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        assert_eq!(row.created_at, Some(expected_created));
        assert_eq!(row.updated_at, Some(expected_updated));
    }

    #[test]
    fn row_display_preview_prefers_thread_name() {
        let row = Row {
            path: PathBuf::from("/tmp/a.jsonl"),
            preview: String::from("first message"),
            thread_id: None,
            thread_name: Some(String::from("My session")),
            created_at: None,
            updated_at: None,
            cwd: None,
            git_branch: None,
        };

        assert_eq!(row.display_preview(), "My session");
    }

    #[test]
    fn resume_table_snapshot() {
        use crate::custom_terminal::Terminal;
        use crate::test_backend::VT100Backend;
        use ratatui::layout::Constraint;
        use ratatui::layout::Layout;

        let loader: PageLoader = Arc::new(|_| {});
        let mut state = PickerState::new(
            PathBuf::from("/tmp"),
            FrameRequester::test_dummy(),
            loader,
            String::from("openai"),
            true,
            None,
            SessionPickerAction::Resume,
        );

        let now = Utc::now();
        let rows = vec![
            Row {
                path: PathBuf::from("/tmp/a.jsonl"),
                preview: String::from("Fix resume picker timestamps"),
                thread_id: None,
                thread_name: None,
                created_at: Some(now - Duration::minutes(16)),
                updated_at: Some(now - Duration::seconds(42)),
                cwd: None,
                git_branch: None,
            },
            Row {
                path: PathBuf::from("/tmp/b.jsonl"),
                preview: String::from("Investigate lazy pagination cap"),
                thread_id: None,
                thread_name: None,
                created_at: Some(now - Duration::hours(1)),
                updated_at: Some(now - Duration::minutes(35)),
                cwd: None,
                git_branch: None,
            },
            Row {
                path: PathBuf::from("/tmp/c.jsonl"),
                preview: String::from("Explain the codebase"),
                thread_id: None,
                thread_name: None,
                created_at: Some(now - Duration::hours(2)),
                updated_at: Some(now - Duration::hours(2)),
                cwd: None,
                git_branch: None,
            },
        ];
        state.all_rows = rows.clone();
        state.filtered_rows = rows;
        state.view_rows = Some(3);
        state.selected = 1;
        state.scroll_top = 0;
        state.update_view_rows(3);

        let metrics = calculate_column_metrics(&state.filtered_rows, state.show_all);

        let width: u16 = 80;
        let height: u16 = 6;
        let backend = VT100Backend::new(width, height);
        let mut terminal = Terminal::with_options(backend).expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 0, width, height));

        {
            let mut frame = terminal.get_frame();
            let area = frame.area();
            let segments =
                Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).split(area);
            render_column_headers(&mut frame, segments[0], &metrics, state.sort_key);
            render_list(&mut frame, segments[1], &state, &metrics);
        }
        terminal.flush().expect("flush");

        let snapshot = terminal.backend().to_string();
        assert_snapshot!("resume_picker_table", snapshot);
    }

    // TODO(jif) fix
    // #[tokio::test]
    // async fn resume_picker_screen_snapshot() {
    //     use crate::custom_terminal::Terminal;
    //     use crate::test_backend::VT100Backend;
    //     use uuid::Uuid;
    //
    //     // Create real rollout files so the snapshot uses the actual listing pipeline.
    //     let tempdir = tempfile::tempdir().expect("tempdir");
    //     let sessions_root = tempdir.path().join("sessions");
    //     std::fs::create_dir_all(&sessions_root).expect("mkdir sessions root");
    //
    //     let now = Utc::now();
    //
    //     // Helper to write a rollout file with minimal meta + one user message.
    //     let write_rollout = |ts: DateTime<Utc>, cwd: &str, branch: &str, preview: &str| {
    //         let dir = sessions_root
    //             .join(ts.format("%Y").to_string())
    //             .join(ts.format("%m").to_string())
    //             .join(ts.format("%d").to_string());
    //         std::fs::create_dir_all(&dir).expect("mkdir date dirs");
    //         let filename = format!(
    //             "rollout-{}-{}.jsonl",
    //             ts.format("%Y-%m-%dT%H-%M-%S"),
    //             Uuid::new_v4()
    //         );
    //         let path = dir.join(filename);
    //         let meta = serde_json::json!({
    //             "timestamp": ts.to_rfc3339(),
    //             "item": {
    //                 "SessionMeta": {
    //                     "meta": {
    //                         "id": Uuid::new_v4(),
    //                         "timestamp": ts.to_rfc3339(),
    //                         "cwd": cwd,
    //                         "originator": "user",
    //                         "cli_version": "0.0.0",
    //                         "source": "Cli",
    //                         "model_provider": "openai",
    //                     }
    //                 }
    //             }
    //         });
    //         let user = serde_json::json!({
    //             "timestamp": ts.to_rfc3339(),
    //             "item": {
    //                 "EventMsg": {
    //                     "UserMessage": {
    //                         "message": preview,
    //                         "images": null
    //                     }
    //                 }
    //             }
    //         });
    //         let branch_meta = serde_json::json!({
    //             "timestamp": ts.to_rfc3339(),
    //             "item": {
    //                 "EventMsg": {
    //                     "SessionMeta": {
    //                         "meta": {
    //                             "git_branch": branch
    //                         }
    //                     }
    //                 }
    //             }
    //         });
    //         std::fs::write(&path, format!("{meta}\n{user}\n{branch_meta}\n"))
    //             .expect("write rollout");
    //     };
    //
    //     write_rollout(
    //         now - Duration::seconds(42),
    //         "/tmp/project",
    //         "feature/resume",
    //         "Fix resume picker timestamps",
    //     );
    //     write_rollout(
    //         now - Duration::minutes(35),
    //         "/tmp/other",
    //         "main",
    //         "Investigate lazy pagination cap",
    //     );
    //
    //     let loader: PageLoader = Arc::new(|_| {});
    //     let mut state = PickerState::new(
    //         PathBuf::from("/tmp"),
    //         FrameRequester::test_dummy(),
    //         loader,
    //         String::from("openai"),
    //         true,
    //         None,
    //         SessionPickerAction::Resume,
    //     );
    //
    //     let page = RolloutRecorder::list_threads(
    //         &state.codex_home,
    //         PAGE_SIZE,
    //         None,
    //         ThreadSortKey::CreatedAt,
    //         INTERACTIVE_SESSION_SOURCES,
    //         Some(&[String::from("openai")]),
    //         "openai",
    //     )
    //     .await
    //     .expect("list conversations");
    //
    //     let rows = rows_from_items(page.items);
    //     state.all_rows = rows.clone();
    //     state.filtered_rows = rows;
    //     state.view_rows = Some(4);
    //     state.selected = 0;
    //     state.scroll_top = 0;
    //     state.update_view_rows(4);
    //
    //     let metrics = calculate_column_metrics(&state.filtered_rows, state.show_all);
    //
    //     let width: u16 = 80;
    //     let height: u16 = 9;
    //     let backend = VT100Backend::new(width, height);
    //     let mut terminal = Terminal::with_options(backend).expect("terminal");
    //     terminal.set_viewport_area(Rect::new(0, 0, width, height));
    //
    //     {
    //         let mut frame = terminal.get_frame();
    //         let area = frame.area();
    //         let [header, search, columns, list, hint] = Layout::vertical([
    //             Constraint::Length(1),
    //             Constraint::Length(1),
    //             Constraint::Length(1),
    //             Constraint::Min(area.height.saturating_sub(4)),
    //             Constraint::Length(1),
    //         ])
    //         .areas(area);
    //
    //         frame.render_widget_ref(
    //             Line::from(vec![
    //                 "Resume a previous session".bold().cyan(),
    //                 "  ".into(),
    //                 "Sort:".dim(),
    //                 " ".into(),
    //                 "Created at".magenta(),
    //             ]),
    //             header,
    //         );
    //
    //         frame.render_widget_ref(Line::from("Type to search".dim()), search);
    //
    //         render_column_headers(&mut frame, columns, &metrics, state.sort_key);
    //         render_list(&mut frame, list, &state, &metrics);
    //
    //         let hint_line: Line = vec![
    //             key_hint::plain(KeyCode::Enter).into(),
    //             " to resume ".dim(),
    //             "    ".dim(),
    //             key_hint::plain(KeyCode::Esc).into(),
    //             " to start new ".dim(),
    //             "    ".dim(),
    //             key_hint::ctrl(KeyCode::Char('c')).into(),
    //             " to quit ".dim(),
    //             "    ".dim(),
    //             key_hint::plain(KeyCode::Tab).into(),
    //             " to toggle sort ".dim(),
    //         ]
    //         .into();
    //         frame.render_widget_ref(hint_line, hint);
    //     }
    //     terminal.flush().expect("flush");
    //
    //     let snapshot = terminal.backend().to_string();
    //     assert_snapshot!("resume_picker_screen", snapshot);
    // }

    #[tokio::test]
    async fn resume_picker_thread_names_snapshot() {
        use crate::custom_terminal::Terminal;
        use crate::test_backend::VT100Backend;
        use ratatui::layout::Constraint;
        use ratatui::layout::Layout;

        let tempdir = tempfile::tempdir().expect("tempdir");
        let session_index_path = tempdir.path().join("session_index.jsonl");

        let id1 =
            ThreadId::from_string("11111111-1111-1111-1111-111111111111").expect("thread id 1");
        let id2 =
            ThreadId::from_string("22222222-2222-2222-2222-222222222222").expect("thread id 2");
        let entries = vec![
            json!({
                "id": id1,
                "thread_name": "Keep this for now",
                "updated_at": "2025-01-01T00:00:00Z",
            }),
            json!({
                "id": id2,
                "thread_name": "Named thread",
                "updated_at": "2025-01-01T00:00:00Z",
            }),
        ];
        let mut out = String::new();
        for entry in entries {
            out.push_str(&serde_json::to_string(&entry).expect("session index entry"));
            out.push('\n');
        }
        std::fs::write(&session_index_path, out).expect("write session index");

        let loader: PageLoader = Arc::new(|_| {});
        let mut state = PickerState::new(
            tempdir.path().to_path_buf(),
            FrameRequester::test_dummy(),
            loader,
            String::from("openai"),
            true,
            None,
            SessionPickerAction::Resume,
        );

        let now = Utc::now();
        let rows = vec![
            Row {
                path: PathBuf::from("/tmp/a.jsonl"),
                preview: String::from("First message preview"),
                thread_id: Some(id1),
                thread_name: None,
                created_at: None,
                updated_at: Some(now - Duration::days(2)),
                cwd: None,
                git_branch: None,
            },
            Row {
                path: PathBuf::from("/tmp/b.jsonl"),
                preview: String::from("Second message preview"),
                thread_id: Some(id2),
                thread_name: None,
                created_at: None,
                updated_at: Some(now - Duration::days(3)),
                cwd: None,
                git_branch: None,
            },
        ];
        state.all_rows = rows.clone();
        state.filtered_rows = rows;
        state.view_rows = Some(2);
        state.selected = 0;
        state.scroll_top = 0;
        state.update_view_rows(2);

        state.update_thread_names().await;

        let metrics = calculate_column_metrics(&state.filtered_rows, state.show_all);

        let width: u16 = 80;
        let height: u16 = 5;
        let backend = VT100Backend::new(width, height);
        let mut terminal = Terminal::with_options(backend).expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 0, width, height));

        {
            let mut frame = terminal.get_frame();
            let area = frame.area();
            let segments =
                Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).split(area);
            render_column_headers(&mut frame, segments[0], &metrics, state.sort_key);
            render_list(&mut frame, segments[1], &state, &metrics);
        }
        terminal.flush().expect("flush");

        let snapshot = terminal.backend().to_string();
        assert_snapshot!("resume_picker_thread_names", snapshot);
    }

    #[test]
    fn pageless_scrolling_deduplicates_and_keeps_order() {
        let loader: PageLoader = Arc::new(|_| {});
        let mut state = PickerState::new(
            PathBuf::from("/tmp"),
            FrameRequester::test_dummy(),
            loader,
            String::from("openai"),
            true,
            None,
            SessionPickerAction::Resume,
        );

        state.reset_pagination();
        state.ingest_page(page(
            vec![
                make_item("/tmp/a.jsonl", "2025-01-03T00:00:00Z", "third"),
                make_item("/tmp/b.jsonl", "2025-01-02T00:00:00Z", "second"),
            ],
            Some(cursor_from_str(
                "2025-01-02T00-00-00|00000000-0000-0000-0000-000000000000",
            )),
            2,
            false,
        ));

        state.ingest_page(page(
            vec![
                make_item("/tmp/a.jsonl", "2025-01-03T00:00:00Z", "duplicate"),
                make_item("/tmp/c.jsonl", "2025-01-01T00:00:00Z", "first"),
            ],
            Some(cursor_from_str(
                "2025-01-01T00-00-00|00000000-0000-0000-0000-000000000001",
            )),
            2,
            false,
        ));

        state.ingest_page(page(
            vec![make_item(
                "/tmp/d.jsonl",
                "2024-12-31T23:00:00Z",
                "very old",
            )],
            None,
            1,
            false,
        ));

        let previews: Vec<_> = state
            .filtered_rows
            .iter()
            .map(|row| row.preview.as_str())
            .collect();
        assert_eq!(previews, vec!["third", "second", "first", "very old"]);

        let unique_paths = state
            .filtered_rows
            .iter()
            .map(|row| row.path.clone())
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(unique_paths.len(), 4);
    }

    #[test]
    fn ensure_minimum_rows_prefetches_when_underfilled() {
        let recorded_requests: Arc<Mutex<Vec<PageLoadRequest>>> = Arc::new(Mutex::new(Vec::new()));
        let request_sink = recorded_requests.clone();
        let loader: PageLoader = Arc::new(move |req: PageLoadRequest| {
            request_sink.lock().unwrap().push(req);
        });

        let mut state = PickerState::new(
            PathBuf::from("/tmp"),
            FrameRequester::test_dummy(),
            loader,
            String::from("openai"),
            true,
            None,
            SessionPickerAction::Resume,
        );
        state.reset_pagination();
        state.ingest_page(page(
            vec![
                make_item("/tmp/a.jsonl", "2025-01-01T00:00:00Z", "one"),
                make_item("/tmp/b.jsonl", "2025-01-02T00:00:00Z", "two"),
            ],
            Some(cursor_from_str(
                "2025-01-03T00-00-00|00000000-0000-0000-0000-000000000000",
            )),
            2,
            false,
        ));

        assert!(recorded_requests.lock().unwrap().is_empty());
        state.ensure_minimum_rows_for_view(10);
        let guard = recorded_requests.lock().unwrap();
        assert_eq!(guard.len(), 1);
        assert!(guard[0].search_token.is_none());
    }

    #[test]
    fn column_visibility_hides_extra_date_column_when_narrow() {
        let metrics = ColumnMetrics {
            max_created_width: 8,
            max_updated_width: 12,
            max_branch_width: 0,
            max_cwd_width: 0,
            labels: Vec::new(),
        };

        let created = column_visibility(30, &metrics, ThreadSortKey::CreatedAt);
        assert_eq!(
            created,
            ColumnVisibility {
                show_created: true,
                show_updated: false,
                show_branch: false,
                show_cwd: false,
            }
        );

        let updated = column_visibility(30, &metrics, ThreadSortKey::UpdatedAt);
        assert_eq!(
            updated,
            ColumnVisibility {
                show_created: false,
                show_updated: true,
                show_branch: false,
                show_cwd: false,
            }
        );

        let wide = column_visibility(40, &metrics, ThreadSortKey::CreatedAt);
        assert_eq!(
            wide,
            ColumnVisibility {
                show_created: true,
                show_updated: true,
                show_branch: false,
                show_cwd: false,
            }
        );
    }

    #[tokio::test]
    async fn toggle_sort_key_reloads_with_new_sort() {
        let recorded_requests: Arc<Mutex<Vec<PageLoadRequest>>> = Arc::new(Mutex::new(Vec::new()));
        let request_sink = recorded_requests.clone();
        let loader: PageLoader = Arc::new(move |req: PageLoadRequest| {
            request_sink.lock().unwrap().push(req);
        });

        let mut state = PickerState::new(
            PathBuf::from("/tmp"),
            FrameRequester::test_dummy(),
            loader,
            String::from("openai"),
            true,
            None,
            SessionPickerAction::Resume,
        );

        state.start_initial_load();
        {
            let guard = recorded_requests.lock().unwrap();
            assert_eq!(guard.len(), 1);
            assert_eq!(guard[0].sort_key, ThreadSortKey::CreatedAt);
        }

        state
            .handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE))
            .await
            .unwrap();

        let guard = recorded_requests.lock().unwrap();
        assert_eq!(guard.len(), 2);
        assert_eq!(guard[1].sort_key, ThreadSortKey::UpdatedAt);
    }

    #[tokio::test]
    async fn page_navigation_uses_view_rows() {
        let loader: PageLoader = Arc::new(|_| {});
        let mut state = PickerState::new(
            PathBuf::from("/tmp"),
            FrameRequester::test_dummy(),
            loader,
            String::from("openai"),
            true,
            None,
            SessionPickerAction::Resume,
        );

        let mut items = Vec::new();
        for idx in 0..20 {
            let ts = format!("2025-01-{:02}T00:00:00Z", idx + 1);
            let preview = format!("item-{idx}");
            let path = format!("/tmp/item-{idx}.jsonl");
            items.push(make_item(&path, &ts, &preview));
        }

        state.reset_pagination();
        state.ingest_page(page(items, None, 20, false));
        state.update_view_rows(5);

        assert_eq!(state.selected, 0);
        state
            .handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE))
            .await
            .unwrap();
        assert_eq!(state.selected, 5);

        state
            .handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE))
            .await
            .unwrap();
        assert_eq!(state.selected, 10);

        state
            .handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE))
            .await
            .unwrap();
        assert_eq!(state.selected, 5);
    }

    #[tokio::test]
    async fn up_at_bottom_does_not_scroll_when_visible() {
        let loader: PageLoader = Arc::new(|_| {});
        let mut state = PickerState::new(
            PathBuf::from("/tmp"),
            FrameRequester::test_dummy(),
            loader,
            String::from("openai"),
            true,
            None,
            SessionPickerAction::Resume,
        );

        let mut items = Vec::new();
        for idx in 0..10 {
            let ts = format!("2025-02-{:02}T00:00:00Z", idx + 1);
            let preview = format!("item-{idx}");
            let path = format!("/tmp/item-{idx}.jsonl");
            items.push(make_item(&path, &ts, &preview));
        }

        state.reset_pagination();
        state.ingest_page(page(items, None, 10, false));
        state.update_view_rows(5);

        state.selected = state.filtered_rows.len().saturating_sub(1);
        state.ensure_selected_visible();

        let initial_top = state.scroll_top;
        assert_eq!(initial_top, state.filtered_rows.len().saturating_sub(5));

        state
            .handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE))
            .await
            .unwrap();

        assert_eq!(state.scroll_top, initial_top);
        assert_eq!(state.selected, state.filtered_rows.len().saturating_sub(2));
    }

    #[tokio::test]
    async fn set_query_loads_until_match_and_respects_scan_cap() {
        let recorded_requests: Arc<Mutex<Vec<PageLoadRequest>>> = Arc::new(Mutex::new(Vec::new()));
        let request_sink = recorded_requests.clone();
        let loader: PageLoader = Arc::new(move |req: PageLoadRequest| {
            request_sink.lock().unwrap().push(req);
        });

        let mut state = PickerState::new(
            PathBuf::from("/tmp"),
            FrameRequester::test_dummy(),
            loader,
            String::from("openai"),
            true,
            None,
            SessionPickerAction::Resume,
        );
        state.reset_pagination();
        state.ingest_page(page(
            vec![make_item(
                "/tmp/start.jsonl",
                "2025-01-01T00:00:00Z",
                "alpha",
            )],
            Some(cursor_from_str(
                "2025-01-02T00-00-00|00000000-0000-0000-0000-000000000000",
            )),
            1,
            false,
        ));
        recorded_requests.lock().unwrap().clear();

        state.set_query("target".to_string());
        let first_request = {
            let guard = recorded_requests.lock().unwrap();
            assert_eq!(guard.len(), 1);
            guard[0].clone()
        };

        state
            .handle_background_event(BackgroundEvent::PageLoaded {
                request_token: first_request.request_token,
                search_token: first_request.search_token,
                page: Ok(page(
                    vec![make_item("/tmp/beta.jsonl", "2025-01-02T00:00:00Z", "beta")],
                    Some(cursor_from_str(
                        "2025-01-03T00-00-00|00000000-0000-0000-0000-000000000001",
                    )),
                    5,
                    false,
                )),
            })
            .await
            .unwrap();

        let second_request = {
            let guard = recorded_requests.lock().unwrap();
            assert_eq!(guard.len(), 2);
            guard[1].clone()
        };
        assert!(state.search_state.is_active());
        assert!(state.filtered_rows.is_empty());

        state
            .handle_background_event(BackgroundEvent::PageLoaded {
                request_token: second_request.request_token,
                search_token: second_request.search_token,
                page: Ok(page(
                    vec![make_item(
                        "/tmp/match.jsonl",
                        "2025-01-03T00:00:00Z",
                        "target log",
                    )],
                    Some(cursor_from_str(
                        "2025-01-04T00-00-00|00000000-0000-0000-0000-000000000002",
                    )),
                    7,
                    false,
                )),
            })
            .await
            .unwrap();

        assert!(!state.filtered_rows.is_empty());
        assert!(!state.search_state.is_active());

        recorded_requests.lock().unwrap().clear();
        state.set_query("missing".to_string());
        let active_request = {
            let guard = recorded_requests.lock().unwrap();
            assert_eq!(guard.len(), 1);
            guard[0].clone()
        };

        state
            .handle_background_event(BackgroundEvent::PageLoaded {
                request_token: second_request.request_token,
                search_token: second_request.search_token,
                page: Ok(page(Vec::new(), None, 0, false)),
            })
            .await
            .unwrap();
        assert_eq!(recorded_requests.lock().unwrap().len(), 1);

        state
            .handle_background_event(BackgroundEvent::PageLoaded {
                request_token: active_request.request_token,
                search_token: active_request.search_token,
                page: Ok(page(Vec::new(), None, 3, true)),
            })
            .await
            .unwrap();

        assert!(state.filtered_rows.is_empty());
        assert!(!state.search_state.is_active());
        assert!(state.pagination.reached_scan_cap);
    }
}
