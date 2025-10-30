use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::DateTime;
use chrono::Duration;
use chrono::Local;
use chrono::TimeZone;
use chrono::Utc;
use codex_core::ConversationItem;
use codex_core::ConversationsPage;
use codex_core::Cursor;
use codex_core::INTERACTIVE_SESSION_SOURCES;
use codex_core::RolloutRecorder;
use codex_protocol::items::TurnItem;
use color_eyre::eyre::Result;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use dunce::simplified;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Stylize as _;
use ratatui::text::Line;
use ratatui::text::Span;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::UnboundedReceiverStream;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::key_hint;
use crate::tui::FrameRequester;
use crate::tui::Tui;
use crate::tui::TuiEvent;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::SessionMetaLine;

const PAGE_SIZE: usize = 25;
const LOAD_NEAR_THRESHOLD: usize = 5;
const UPDATED_COL_MAX_WIDTH: usize = 16;
const CONVERSATION_COL_MAX_WIDTH: usize = 60;
const MIN_UPDATED_WIDTH: usize = 7;
const MIN_CONVERSATION_WIDTH: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TimeFilter {
    Today,
    Recent,
    Last30Days,
    All,
}

impl TimeFilter {
    const ORDERED: [TimeFilter; 4] = [
        TimeFilter::Today,
        TimeFilter::Recent,
        TimeFilter::Last30Days,
        TimeFilter::All,
    ];

    fn label(self) -> &'static str {
        match self {
            TimeFilter::Today => "Today",
            TimeFilter::Recent => "Recent",
            TimeFilter::Last30Days => "Last 30 days",
            TimeFilter::All => "All",
        }
    }

    fn next(self) -> Self {
        match self {
            TimeFilter::Today => TimeFilter::Recent,
            TimeFilter::Recent => TimeFilter::Last30Days,
            TimeFilter::Last30Days => TimeFilter::All,
            TimeFilter::All => TimeFilter::Today,
        }
    }

    fn previous(self) -> Self {
        match self {
            TimeFilter::Today => TimeFilter::All,
            TimeFilter::Recent => TimeFilter::Today,
            TimeFilter::Last30Days => TimeFilter::Recent,
            TimeFilter::All => TimeFilter::Last30Days,
        }
    }

    fn matches(self, timestamp: Option<DateTime<Utc>>) -> bool {
        if matches!(self, TimeFilter::All) {
            return true;
        }
        let Some(ts) = timestamp else {
            return false;
        };
        let now = Utc::now();
        match self {
            TimeFilter::Today => ts >= start_of_today_utc(),
            TimeFilter::Recent => ts >= now - Duration::days(7),
            TimeFilter::Last30Days => ts >= now - Duration::days(30),
            TimeFilter::All => true,
        }
    }
}

#[derive(Clone, Copy)]
enum FilterDirection {
    Next,
    Prev,
}

#[derive(Debug, Clone)]
pub enum ResumeSelection {
    StartFresh,
    Resume(PathBuf),
    Exit,
}

#[derive(Clone)]
struct PageLoadRequest {
    codex_home: PathBuf,
    cursor: Option<Cursor>,
    request_token: usize,
    search_token: Option<usize>,
    default_provider: String,
}

type PageLoader = Arc<dyn Fn(PageLoadRequest) + Send + Sync>;

enum BackgroundEvent {
    PageLoaded {
        request_token: usize,
        search_token: Option<usize>,
        page: std::io::Result<ConversationsPage>,
    },
}

/// Interactive session picker that lists recorded rollout files with simple
/// search and pagination. Shows the first user input as the preview, relative
/// time (e.g., "5 seconds ago"), and the absolute path.
pub async fn run_resume_picker(
    tui: &mut Tui,
    codex_home: &Path,
    default_provider: &str,
    target_cwd: Option<&Path>,
) -> Result<ResumeSelection> {
    let alt = AltScreenGuard::enter(tui);
    let (bg_tx, bg_rx) = mpsc::unbounded_channel();

    let default_provider = default_provider.to_string();

    let loader_tx = bg_tx.clone();
    let page_loader: PageLoader = Arc::new(move |request: PageLoadRequest| {
        let tx = loader_tx.clone();
        tokio::spawn(async move {
            let provider_filter = vec![request.default_provider.clone()];
            let page = RolloutRecorder::list_conversations(
                &request.codex_home,
                PAGE_SIZE,
                request.cursor.as_ref(),
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
        target_cwd.map(Path::to_path_buf),
    );
    state.load_initial_page().await?;
    let mut tui_events = alt.tui.event_stream().fuse();
    let mut background_events = UnboundedReceiverStream::new(bg_rx).fuse();
    state.request_frame();

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
                state.handle_background_event(event)?;
            }
            else => break,
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
    target_cwd: Option<PathBuf>,
    time_filter: TimeFilter,
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
    cwd: Option<PathBuf>,
    created_at: Option<DateTime<Utc>>,
    updated_at: Option<DateTime<Utc>>,
}

impl PickerState {
    fn new(
        codex_home: PathBuf,
        requester: FrameRequester,
        page_loader: PageLoader,
        default_provider: String,
        target_cwd: Option<PathBuf>,
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
            target_cwd,
            time_filter: TimeFilter::Today,
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
            KeyCode::Left => {
                self.cycle_time_filter(FilterDirection::Prev);
            }
            KeyCode::Right => {
                self.cycle_time_filter(FilterDirection::Next);
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

    async fn load_initial_page(&mut self) -> Result<()> {
        let provider_filter = vec![self.default_provider.clone()];
        let page = RolloutRecorder::list_conversations(
            &self.codex_home,
            PAGE_SIZE,
            None,
            INTERACTIVE_SESSION_SOURCES,
            Some(provider_filter.as_slice()),
            self.default_provider.as_str(),
        )
        .await?;
        self.reset_pagination();
        self.all_rows.clear();
        self.filtered_rows.clear();
        self.seen_paths.clear();
        self.search_state = SearchState::Idle;
        self.selected = 0;
        self.ingest_page(page);
        Ok(())
    }

    fn handle_background_event(&mut self, event: BackgroundEvent) -> Result<()> {
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

    fn ingest_page(&mut self, page: ConversationsPage) {
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
            if !self.matches_target_cwd(&row) {
                continue;
            }
            if self.seen_paths.insert(row.path.clone()) {
                self.all_rows.push(row);
            }
        }

        self.apply_filter();
    }

    fn apply_filter(&mut self) {
        let query = if self.query.is_empty() {
            None
        } else {
            Some(self.query.to_lowercase())
        };

        let mut filtered = self.filter_rows_with(self.time_filter, query.as_deref());

        if filtered.is_empty()
            && query.is_none()
            && !self.all_rows.is_empty()
            && let Some(start_idx) = TimeFilter::ORDERED
                .iter()
                .position(|f| *f == self.time_filter)
        {
            for &candidate in &TimeFilter::ORDERED[start_idx + 1..] {
                let candidate_rows = self.filter_rows_with(candidate, None);
                if !candidate_rows.is_empty() {
                    self.time_filter = candidate;
                    filtered = candidate_rows;
                    break;
                }
            }
        }

        self.filtered_rows = filtered;
        if self.selected >= self.filtered_rows.len() {
            self.selected = self.filtered_rows.len().saturating_sub(1);
        }
        if self.filtered_rows.is_empty() {
            self.scroll_top = 0;
        }
        self.ensure_selected_visible();
        self.request_frame();
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

    fn matches_target_cwd(&self, row: &Row) -> bool {
        match &self.target_cwd {
            Some(target) => row
                .cwd
                .as_ref()
                .map(|cwd| simplified(cwd.as_path()) == simplified(target.as_path()))
                .unwrap_or(false),
            None => true,
        }
    }

    fn matches_time_filter(&self, filter: TimeFilter, row: &Row) -> bool {
        let timestamp = row.updated_at.or(row.created_at);
        filter.matches(timestamp)
    }

    fn filter_rows_with(&self, filter: TimeFilter, query: Option<&str>) -> Vec<Row> {
        self.all_rows
            .iter()
            .filter(|row| {
                self.matches_target_cwd(row)
                    && self.matches_time_filter(filter, row)
                    && query.map(|q| row_matches_query(row, q)).unwrap_or(true)
            })
            .cloned()
            .collect()
    }

    fn cycle_time_filter(&mut self, direction: FilterDirection) {
        let next = match direction {
            FilterDirection::Next => self.time_filter.next(),
            FilterDirection::Prev => self.time_filter.previous(),
        };
        if next == self.time_filter {
            return;
        }
        self.time_filter = next;
        self.apply_filter();
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
            codex_home: self.codex_home.clone(),
            cursor: Some(cursor),
            request_token,
            search_token,
            default_provider: self.default_provider.clone(),
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
}

fn rows_from_items(items: Vec<ConversationItem>) -> Vec<Row> {
    items.into_iter().map(|item| head_to_row(&item)).collect()
}

fn head_to_row(item: &ConversationItem) -> Row {
    let created_at = item
        .created_at
        .as_deref()
        .and_then(parse_timestamp_str)
        .or_else(|| item.head.first().and_then(extract_timestamp));
    let updated_at = item
        .updated_at
        .as_deref()
        .and_then(parse_timestamp_str)
        .or(created_at);

    let meta = find_session_meta(&item.head);
    let cwd = meta
        .as_ref()
        .map(|meta_line| meta_line.meta.cwd.clone())
        .or_else(|| extract_cwd_from_tail(&item.tail));

    let preview = conversation_preview(item);

    Row {
        path: item.path.clone(),
        preview,
        cwd,
        created_at,
        updated_at,
    }
}

fn conversation_preview(item: &ConversationItem) -> String {
    if let Some(text) = first_user_message(&item.head) {
        return text;
    }
    if let Some(text) = first_user_message(&item.tail) {
        return text;
    }
    "(no user messages yet)".to_string()
}

fn pad_or_truncate(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    const ELLIPSIS: &str = "...";
    const ELLIPSIS_WIDTH: usize = 3;

    let graphemes: Vec<&str> = UnicodeSegmentation::graphemes(text, true).collect();
    let mut visible: Vec<&str> = Vec::new();
    let mut current_width = 0;
    for grapheme in &graphemes {
        let grapheme_width = UnicodeWidthStr::width(*grapheme);
        if current_width + grapheme_width > width {
            break;
        }
        visible.push(grapheme);
        current_width += grapheme_width;
    }

    let mut result = visible.concat();
    let was_truncated = visible.len() < graphemes.len();

    if was_truncated && width >= ELLIPSIS_WIDTH {
        while current_width + ELLIPSIS_WIDTH > width && !visible.is_empty() {
            if let Some(removed) = visible.pop() {
                current_width -= UnicodeWidthStr::width(removed);
            }
        }
        result = visible.concat();
        if current_width + ELLIPSIS_WIDTH <= width {
            result.push_str(ELLIPSIS);
        }
    }

    let display_width = UnicodeWidthStr::width(result.as_str());
    if display_width < width {
        result.extend(std::iter::repeat_n(' ', width - display_width));
    }

    result
}

fn start_of_today_utc() -> DateTime<Utc> {
    let local_now = Local::now();
    let date = local_now.date_naive();
    let naive = date
        .and_hms_opt(0, 0, 0)
        .expect("midnight should be representable");
    let start_local = Local
        .from_local_datetime(&naive)
        .single()
        .unwrap_or_else(|| {
            Local
                .from_local_datetime(&naive)
                .earliest()
                .expect("midnight should map to a local datetime")
        });
    start_local.with_timezone(&Utc)
}

fn find_session_meta(values: &[serde_json::Value]) -> Option<SessionMetaLine> {
    values
        .iter()
        .filter_map(|value| serde_json::from_value::<SessionMetaLine>(value.clone()).ok())
        .next()
}

fn extract_cwd_from_tail(values: &[serde_json::Value]) -> Option<PathBuf> {
    values
        .iter()
        .filter_map(|value| serde_json::from_value::<SessionMetaLine>(value.clone()).ok())
        .map(|meta| meta.meta.cwd)
        .next()
}

fn first_user_message(values: &[serde_json::Value]) -> Option<String> {
    values
        .iter()
        .filter_map(|value| serde_json::from_value::<ResponseItem>(value.clone()).ok())
        .find_map(|item| match codex_core::parse_turn_item(&item) {
            Some(TurnItem::UserMessage(user)) => {
                let message = clean_preview_text(user.message());
                if message.is_empty() {
                    None
                } else {
                    Some(message)
                }
            }
            _ => None,
        })
}

fn clean_preview_text(text: String) -> String {
    let mut prev_was_space = false;
    let mut cleaned = String::new();
    for ch in text.trim().chars() {
        if ch.is_whitespace() {
            if !prev_was_space {
                cleaned.push(' ');
                prev_was_space = true;
            }
        } else {
            cleaned.push(ch);
            prev_was_space = false;
        }
    }
    cleaned.trim().to_string()
}

fn row_matches_query(row: &Row, query: &str) -> bool {
    if row.preview.to_lowercase().contains(query) {
        return true;
    }
    if row.path.to_string_lossy().to_lowercase().contains(query) {
        return true;
    }
    if let Some(cwd) = &row.cwd
        && cwd.to_string_lossy().to_lowercase().contains(query)
    {
        return true;
    }
    false
}

fn parse_timestamp_str(ts: &str) -> Option<DateTime<Utc>> {
    chrono::DateTime::parse_from_rfc3339(ts)
        .map(|dt| dt.with_timezone(&Utc))
        .ok()
}

fn extract_timestamp(value: &serde_json::Value) -> Option<DateTime<Utc>> {
    value
        .get("timestamp")
        .and_then(|v| v.as_str())
        .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
        .map(|dt| dt.with_timezone(&Utc))
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
        frame.render_widget_ref(
            Line::from(vec!["Resume a previous session".bold().cyan()]),
            header,
        );

        // Search + filter line
        let mut search_spans: Vec<Span> = Vec::new();
        search_spans.push("Filter: ".dim());
        search_spans.push(state.time_filter.label().bold());
        search_spans.push("    ".into());
        if state.query.is_empty() {
            search_spans.push("Type to search".dim());
        } else {
            search_spans.push(format!("Search: {}", state.query).into());
        }
        frame.render_widget_ref(Line::from(search_spans), search);

        let metrics = calculate_column_metrics(&state.filtered_rows);
        let widths = compute_column_widths(list.width as usize, &metrics);

        // Column headers and list
        render_column_headers(frame, columns, &metrics, &widths);
        render_list(frame, list, state, &metrics, &widths);

        // Hint line
        let hint_line: Line = vec![
            key_hint::plain(KeyCode::Enter).into(),
            " to resume ".dim(),
            "    ".dim(),
            key_hint::plain(KeyCode::Esc).into(),
            " to start new ".dim(),
            "    ".dim(),
            key_hint::ctrl(KeyCode::Char('c')).into(),
            " to quit ".dim(),
            "    ".dim(),
            key_hint::plain(KeyCode::Left).into(),
            "/".dim(),
            key_hint::plain(KeyCode::Right).into(),
            " change time range ".dim(),
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
    widths: &ColumnWidths,
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

    for (idx, (row, updated_label)) in rows[start..end]
        .iter()
        .zip(labels[start..end].iter())
        .enumerate()
    {
        let is_sel = start + idx == state.selected;
        let marker = if is_sel { "> ".bold() } else { "  ".into() };
        let mut spans: Vec<Span> = vec![marker];
        if widths.conversation > 0 {
            spans.push(Span::from(pad_or_truncate(
                &row.preview,
                widths.conversation,
            )));
        }
        if widths.updated > 0 {
            spans.push("  ".into());
            spans.push(Span::from(pad_or_truncate(updated_label, widths.updated)).dim());
        }

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

    if state.filtered_rows.is_empty() && state.query.is_empty() && !state.all_rows.is_empty() {
        let msg = format!("No sessions in {} range", state.time_filter.label());
        return vec![Span::from(msg).italic().dim()].into();
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

fn render_column_headers(
    frame: &mut crate::custom_terminal::Frame,
    area: Rect,
    _metrics: &ColumnMetrics,
    widths: &ColumnWidths,
) {
    if area.height == 0 {
        return;
    }

    let mut spans: Vec<Span> = vec!["  ".into()];
    if widths.conversation > 0 {
        spans.push(Span::from(pad_or_truncate("Conversation", widths.conversation)).bold());
    }
    if widths.updated > 0 {
        spans.push("  ".into());
        spans.push(Span::from(pad_or_truncate("Updated", widths.updated)).bold());
    }
    frame.render_widget_ref(Line::from(spans), area);
}

struct ColumnMetrics {
    max_updated_width: usize,
    labels: Vec<String>,
}

struct ColumnWidths {
    conversation: usize,
    updated: usize,
}

fn calculate_column_metrics(rows: &[Row]) -> ColumnMetrics {
    let mut labels: Vec<String> = Vec::with_capacity(rows.len());
    let mut max_updated_width = UnicodeWidthStr::width("Updated");

    for row in rows {
        let updated = format_updated_label(row);
        max_updated_width = max_updated_width.max(UnicodeWidthStr::width(updated.as_str()));
        labels.push(updated);
    }

    ColumnMetrics {
        max_updated_width,
        labels,
    }
}

fn compute_column_widths(area_width: usize, metrics: &ColumnMetrics) -> ColumnWidths {
    const MARKER_WIDTH: usize = 2;
    const GAP_WIDTH: usize = 2;

    if area_width <= MARKER_WIDTH {
        return ColumnWidths {
            conversation: 0,
            updated: 0,
        };
    }

    let mut updated = if metrics.max_updated_width == 0 {
        0
    } else {
        metrics
            .max_updated_width
            .clamp(MIN_UPDATED_WIDTH, UPDATED_COL_MAX_WIDTH)
    };

    let mut conversation = area_width.saturating_sub(MARKER_WIDTH);
    if updated > 0 {
        conversation = conversation.saturating_sub(GAP_WIDTH + updated);
    }
    conversation = conversation.min(CONVERSATION_COL_MAX_WIDTH);

    let mut total = total_width(MARKER_WIDTH, GAP_WIDTH, conversation, updated);

    while total > area_width {
        let overflow = total - area_width;

        if conversation > MIN_CONVERSATION_WIDTH {
            let reducible = conversation.saturating_sub(MIN_CONVERSATION_WIDTH);
            if reducible > 0 {
                let reduce = reducible.min(overflow);
                conversation -= reduce;
                total -= reduce;
                continue;
            }
        }

        if updated > MIN_UPDATED_WIDTH {
            let reducible = updated.saturating_sub(MIN_UPDATED_WIDTH);
            if reducible > 0 {
                let reduce = reducible.min(overflow);
                updated -= reduce;
                total -= reduce;
                continue;
            }
        }

        if conversation > 0 {
            let reduce = conversation.min(overflow);
            conversation -= reduce;
            total -= reduce;
            continue;
        }

        if updated > 0 {
            let reducible = updated.saturating_sub(1);
            if reducible > 0 {
                let reduce = reducible.min(overflow);
                updated -= reduce;
                total -= reduce;
                continue;
            }
        }

        if updated > 0 {
            total -= updated + GAP_WIDTH;
            updated = 0;
            continue;
        }

        break;
    }

    let mut max_conversation = area_width.saturating_sub(MARKER_WIDTH);
    if updated > 0 {
        max_conversation = max_conversation.saturating_sub(GAP_WIDTH + updated);
    }
    conversation = max_conversation
        .min(CONVERSATION_COL_MAX_WIDTH)
        .min(conversation);

    if conversation == 0 {
        if updated > 0 {
            updated = 0;
        }
        conversation = area_width.saturating_sub(MARKER_WIDTH);
    }

    if conversation == 0 && area_width > MARKER_WIDTH {
        conversation = area_width - MARKER_WIDTH;
    }

    ColumnWidths {
        conversation,
        updated,
    }
}

fn total_width(marker: usize, gap: usize, conversation: usize, updated: usize) -> usize {
    let mut total = marker + conversation;
    if updated > 0 {
        total += gap + updated;
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use codex_protocol::ConversationId;
    use codex_protocol::protocol::SessionMeta;
    use codex_protocol::protocol::SessionMetaLine;
    use codex_protocol::protocol::SessionSource;
    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;
    use crossterm::event::KeyModifiers;
    use insta::assert_snapshot;
    use serde_json::json;
    use serde_json::to_value;
    use std::future::Future;
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::Mutex;

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

    fn make_item(path: &str, ts: &str, preview: &str) -> ConversationItem {
        ConversationItem {
            path: PathBuf::from(path),
            head: head_with_ts_and_user_text(ts, &[preview]),
            tail: Vec::new(),
            created_at: Some(ts.to_string()),
            updated_at: Some(ts.to_string()),
        }
    }

    fn set_time_filter(state: &mut PickerState, filter: TimeFilter) {
        state.time_filter = filter;
        state.apply_filter();
    }

    fn row_with_timestamp(path: &str, preview: &str, timestamp: DateTime<Utc>) -> Row {
        Row {
            path: PathBuf::from(path),
            preview: preview.to_string(),
            cwd: None,
            created_at: Some(timestamp),
            updated_at: Some(timestamp),
        }
    }

    fn make_item_with_meta(path: &str, ts: &str, preview: &str, cwd: &str) -> ConversationItem {
        let meta = SessionMetaLine {
            meta: SessionMeta {
                id: ConversationId::from_string("00000000-0000-4000-8000-000000000001")
                    .expect("valid conversation id"),
                timestamp: ts.to_string(),
                cwd: PathBuf::from(cwd),
                originator: "codex".to_string(),
                cli_version: "0.0.0".to_string(),
                instructions: None,
                source: SessionSource::Cli,
                model_provider: Some("openai".to_string()),
            },
            git: None,
        };

        let mut head = vec![to_value(meta).expect("meta to value")];
        head.extend(head_with_ts_and_user_text(ts, &[preview]));

        ConversationItem {
            path: PathBuf::from(path),
            head,
            tail: Vec::new(),
            created_at: Some(ts.to_string()),
            updated_at: Some(ts.to_string()),
        }
    }

    fn cursor_from_str(repr: &str) -> Cursor {
        serde_json::from_str::<Cursor>(&format!("\"{repr}\""))
            .expect("cursor format should deserialize")
    }

    fn page(
        items: Vec<ConversationItem>,
        next_cursor: Option<Cursor>,
        num_scanned_files: usize,
        reached_scan_cap: bool,
    ) -> ConversationsPage {
        ConversationsPage {
            items,
            next_cursor,
            num_scanned_files,
            reached_scan_cap,
        }
    }

    fn block_on_future<F: Future<Output = T>, T>(future: F) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(future)
    }

    #[test]
    fn preview_uses_first_real_user_message() {
        let head = vec![
            json!({ "timestamp": "2025-01-01T00:00:00Z" }),
            json!({
                "type": "message",
                "role": "user",
                "content": [
                    { "type": "input_text", "text": "<user_instructions>hi</user_instructions>" },
                ]
            }),
            json!({
                "type": "message",
                "role": "user",
                "content": [
                    { "type": "input_text", "text": "<environment_context>...</environment_context>" },
                ]
            }),
            json!({
                "type": "message",
                "role": "user",
                "content": [
                    { "type": "input_text", "text": " real   question \nwith spaces" },
                    { "type": "input_image", "image_url": "ignored" }
                ]
            }),
            json!({
                "type": "message",
                "role": "user",
                "content": [ { "type": "input_text", "text": "later text" } ]
            }),
        ];
        let item = ConversationItem {
            path: PathBuf::from("/tmp/test.jsonl"),
            head,
            tail: Vec::new(),
            created_at: None,
            updated_at: None,
        };
        assert_eq!(conversation_preview(&item), "real question with spaces");
    }

    #[test]
    fn preview_falls_back_to_tail_user_message() {
        let tail = vec![
            json!({
                "type": "message",
                "role": "assistant",
                "content": [ { "type": "output_text", "text": "assistant" } ],
            }),
            json!({
                "type": "message",
                "role": "user",
                "content": [ { "type": "input_text", "text": "tail question" } ],
            }),
        ];
        let item = ConversationItem {
            path: PathBuf::from("/tmp/test.jsonl"),
            head: Vec::new(),
            tail,
            created_at: None,
            updated_at: None,
        };
        assert_eq!(conversation_preview(&item), "tail question");
    }

    #[test]
    fn rows_from_items_preserves_backend_order() {
        // Construct two items with different timestamps and real user text.
        let a = ConversationItem {
            path: PathBuf::from("/tmp/a.jsonl"),
            head: head_with_ts_and_user_text("2025-01-01T00:00:00Z", &["A"]),
            tail: Vec::new(),
            created_at: Some("2025-01-01T00:00:00Z".into()),
            updated_at: Some("2025-01-01T00:00:00Z".into()),
        };
        let b = ConversationItem {
            path: PathBuf::from("/tmp/b.jsonl"),
            head: head_with_ts_and_user_text("2025-01-02T00:00:00Z", &["B"]),
            tail: Vec::new(),
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
    fn head_to_row_extracts_cwd_from_meta() {
        let item = make_item_with_meta(
            "/tmp/a.jsonl",
            "2025-01-01T00:00:00Z",
            "Initial message",
            "/workspace/project-alpha",
        );
        let row = head_to_row(&item);
        assert_eq!(
            row.cwd.as_deref(),
            Some(Path::new("/workspace/project-alpha"))
        );
    }

    #[test]
    fn query_matches_directory_path() {
        let loader: PageLoader = Arc::new(|_| {});
        let mut state = PickerState::new(
            PathBuf::from("/tmp"),
            FrameRequester::test_dummy(),
            loader,
            String::from("openai"),
            None,
        );
        state.reset_pagination();
        state.ingest_page(page(
            vec![
                make_item_with_meta(
                    "/tmp/a.jsonl",
                    "2025-01-01T00:00:00Z",
                    "first",
                    "/workspace/project-alpha",
                ),
                make_item_with_meta(
                    "/tmp/b.jsonl",
                    "2025-01-01T00:00:00Z",
                    "second",
                    "/workspace/project-beta",
                ),
            ],
            None,
            2,
            false,
        ));

        set_time_filter(&mut state, TimeFilter::All);

        state.set_query("beta".to_string());
        assert_eq!(state.filtered_rows.len(), 1);
        let row = &state.filtered_rows[0];
        assert_eq!(
            row.cwd.as_deref(),
            Some(Path::new("/workspace/project-beta"))
        );
    }

    #[test]
    fn filters_out_sessions_not_matching_target_directory() {
        let loader: PageLoader = Arc::new(|_| {});
        let mut state = PickerState::new(
            PathBuf::from("/tmp"),
            FrameRequester::test_dummy(),
            loader,
            String::from("openai"),
            Some(PathBuf::from("/workspace/project-alpha")),
        );
        state.reset_pagination();
        state.ingest_page(page(
            vec![
                make_item_with_meta(
                    "/tmp/a.jsonl",
                    "2025-01-01T00:00:00Z",
                    "first",
                    "/workspace/project-alpha",
                ),
                make_item_with_meta(
                    "/tmp/b.jsonl",
                    "2025-01-01T00:00:00Z",
                    "second",
                    "/workspace/project-beta",
                ),
            ],
            None,
            2,
            false,
        ));

        set_time_filter(&mut state, TimeFilter::All);

        assert_eq!(state.all_rows.len(), 1);
        let row = &state.all_rows[0];
        assert_eq!(
            row.cwd.as_deref(),
            Some(Path::new("/workspace/project-alpha"))
        );
        assert_eq!(state.filtered_rows.len(), 1);
    }

    #[test]
    fn narrow_width_preserves_updated_column() {
        let metrics = ColumnMetrics {
            max_updated_width: 12,
            labels: Vec::new(),
        };
        for width in [20usize, 18usize] {
            let widths = compute_column_widths(width, &metrics);
            assert!(
                widths.updated > 0,
                "expected updated column for width {width}"
            );
            let total = total_width(2, 2, widths.conversation, widths.updated);
            assert!(total <= width, "layout exceeded width {width}");
        }
    }

    #[test]
    fn time_filter_today_limits_results() {
        let loader: PageLoader = Arc::new(|_| {});
        let mut state = PickerState::new(
            PathBuf::from("/tmp"),
            FrameRequester::test_dummy(),
            loader,
            String::from("openai"),
            None,
        );

        let now = Utc::now();
        state.all_rows = vec![
            row_with_timestamp("/tmp/today.jsonl", "today", now),
            row_with_timestamp("/tmp/old.jsonl", "old", now - Duration::days(2)),
        ];
        state.apply_filter();

        assert_eq!(state.filtered_rows.len(), 1);
        assert_eq!(
            state.filtered_rows[0].path,
            PathBuf::from("/tmp/today.jsonl")
        );
    }

    #[test]
    fn time_filter_recent_and_last30_days_include_expected_sessions() {
        let loader: PageLoader = Arc::new(|_| {});
        let mut state = PickerState::new(
            PathBuf::from("/tmp"),
            FrameRequester::test_dummy(),
            loader,
            String::from("openai"),
            None,
        );

        let now = Utc::now();
        let recent = now - Duration::days(5);
        let last_month = now - Duration::days(20);
        let old = now - Duration::days(60);

        state.all_rows = vec![
            row_with_timestamp("/tmp/now.jsonl", "now", now),
            row_with_timestamp("/tmp/recent.jsonl", "recent", recent),
            row_with_timestamp("/tmp/month.jsonl", "month", last_month),
            row_with_timestamp("/tmp/old.jsonl", "old", old),
        ];

        set_time_filter(&mut state, TimeFilter::Recent);
        let recent_paths: Vec<_> = state
            .filtered_rows
            .iter()
            .map(|row| row.path.as_path())
            .collect();
        assert_eq!(
            recent_paths,
            vec![Path::new("/tmp/now.jsonl"), Path::new("/tmp/recent.jsonl")]
        );

        set_time_filter(&mut state, TimeFilter::Last30Days);
        let month_paths: Vec<_> = state
            .filtered_rows
            .iter()
            .map(|row| row.path.as_path())
            .collect();
        assert_eq!(
            month_paths,
            vec![
                Path::new("/tmp/now.jsonl"),
                Path::new("/tmp/recent.jsonl"),
                Path::new("/tmp/month.jsonl"),
            ]
        );

        set_time_filter(&mut state, TimeFilter::All);
        assert_eq!(state.filtered_rows.len(), 4);
    }

    #[test]
    fn auto_expands_time_filter_when_today_empty() {
        let loader: PageLoader = Arc::new(|_| {});
        let mut state = PickerState::new(
            PathBuf::from("/tmp"),
            FrameRequester::test_dummy(),
            loader,
            String::from("openai"),
            None,
        );

        let now = Utc::now();
        state.all_rows = vec![
            row_with_timestamp("/tmp/recent.jsonl", "recent", now - Duration::days(5)),
            row_with_timestamp("/tmp/month.jsonl", "month", now - Duration::days(20)),
        ];

        state.apply_filter();

        assert_eq!(state.time_filter, TimeFilter::Recent);
        assert_eq!(state.filtered_rows.len(), 1);
        assert_eq!(
            state.filtered_rows[0].path,
            PathBuf::from("/tmp/recent.jsonl")
        );
    }

    #[test]
    fn row_uses_tail_timestamp_for_updated_at() {
        let head = head_with_ts_and_user_text("2025-01-01T00:00:00Z", &["Hello"]);
        let tail = vec![json!({
            "timestamp": "2025-01-01T01:00:00Z",
            "type": "message",
            "role": "assistant",
            "content": [
                {
                    "type": "output_text",
                    "text": "hi",
                }
            ],
        })];
        let item = ConversationItem {
            path: PathBuf::from("/tmp/a.jsonl"),
            head,
            tail,
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
            None,
        );

        let now = Utc::now();
        let rows = vec![
            Row {
                path: PathBuf::from("/tmp/a.jsonl"),
                preview: String::from("Fix resume picker timestamps"),
                cwd: None,
                created_at: Some(now - Duration::minutes(16)),
                updated_at: Some(now - Duration::seconds(42)),
            },
            Row {
                path: PathBuf::from("/tmp/b.jsonl"),
                preview: String::from("Investigate lazy pagination cap"),
                cwd: None,
                created_at: Some(now - Duration::hours(1)),
                updated_at: Some(now - Duration::minutes(35)),
            },
            Row {
                path: PathBuf::from("/tmp/c.jsonl"),
                preview: String::from("Explain the codebase"),
                cwd: None,
                created_at: Some(now - Duration::hours(2)),
                updated_at: Some(now - Duration::hours(2)),
            },
        ];
        state.all_rows = rows.clone();
        state.filtered_rows = rows;
        state.view_rows = Some(3);
        state.selected = 1;
        state.scroll_top = 0;
        state.update_view_rows(3);

        let width: u16 = 80;
        let height: u16 = 6;
        let metrics = calculate_column_metrics(&state.filtered_rows);
        let widths = compute_column_widths(width as usize, &metrics);
        let backend = VT100Backend::new(width, height);
        let mut terminal = Terminal::with_options(backend).expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 0, width, height));

        {
            let mut frame = terminal.get_frame();
            let area = frame.area();
            let segments =
                Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).split(area);
            render_column_headers(&mut frame, segments[0], &metrics, &widths);
            render_list(&mut frame, segments[1], &state, &metrics, &widths);
        }
        terminal.flush().expect("flush");

        let snapshot = terminal.backend().to_string();
        assert_snapshot!("resume_picker_table", snapshot);
    }

    #[test]
    fn pageless_scrolling_deduplicates_and_keeps_order() {
        let loader: PageLoader = Arc::new(|_| {});
        let mut state = PickerState::new(
            PathBuf::from("/tmp"),
            FrameRequester::test_dummy(),
            loader,
            String::from("openai"),
            None,
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

        set_time_filter(&mut state, TimeFilter::All);

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
            None,
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

        set_time_filter(&mut state, TimeFilter::All);

        assert!(recorded_requests.lock().unwrap().is_empty());
        state.ensure_minimum_rows_for_view(10);
        let guard = recorded_requests.lock().unwrap();
        assert_eq!(guard.len(), 1);
        assert!(guard[0].search_token.is_none());
    }

    #[test]
    fn page_navigation_uses_view_rows() {
        let loader: PageLoader = Arc::new(|_| {});
        let mut state = PickerState::new(
            PathBuf::from("/tmp"),
            FrameRequester::test_dummy(),
            loader,
            String::from("openai"),
            None,
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
        set_time_filter(&mut state, TimeFilter::All);
        state.update_view_rows(5);

        assert_eq!(state.selected, 0);
        block_on_future(async {
            state
                .handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE))
                .await
                .unwrap();
        });
        assert_eq!(state.selected, 5);

        block_on_future(async {
            state
                .handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE))
                .await
                .unwrap();
        });
        assert_eq!(state.selected, 10);

        block_on_future(async {
            state
                .handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE))
                .await
                .unwrap();
        });
        assert_eq!(state.selected, 5);
    }

    #[test]
    fn up_at_bottom_does_not_scroll_when_visible() {
        let loader: PageLoader = Arc::new(|_| {});
        let mut state = PickerState::new(
            PathBuf::from("/tmp"),
            FrameRequester::test_dummy(),
            loader,
            String::from("openai"),
            None,
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
        set_time_filter(&mut state, TimeFilter::All);
        state.update_view_rows(5);

        state.selected = state.filtered_rows.len().saturating_sub(1);
        state.ensure_selected_visible();

        let initial_top = state.scroll_top;
        assert_eq!(initial_top, state.filtered_rows.len().saturating_sub(5));

        block_on_future(async {
            state
                .handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE))
                .await
                .unwrap();
        });

        assert_eq!(state.scroll_top, initial_top);
        assert_eq!(state.selected, state.filtered_rows.len().saturating_sub(2));
    }

    #[test]
    fn set_query_loads_until_match_and_respects_scan_cap() {
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
            None,
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
            .unwrap();
        assert_eq!(recorded_requests.lock().unwrap().len(), 1);

        state
            .handle_background_event(BackgroundEvent::PageLoaded {
                request_token: active_request.request_token,
                search_token: active_request.search_token,
                page: Ok(page(Vec::new(), None, 3, true)),
            })
            .unwrap();

        assert!(state.filtered_rows.is_empty());
        assert!(!state.search_state.is_active());
        assert!(state.pagination.reached_scan_cap);
    }
}
