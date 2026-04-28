use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use crate::app_server_session::AppServerSession;
use crate::color::is_light;
use crate::key_hint;
use crate::legacy_core::config::Config;
use crate::markdown::append_markdown;
use crate::session_resume::resolve_session_thread_id;
use crate::status::format_directory_display;
use crate::terminal_palette::default_bg;
use crate::text_formatting::truncate_text;
use crate::tui::FrameRequester;
use crate::tui::Tui;
use crate::tui::TuiEvent;
use chrono::DateTime;
use chrono::Utc;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadListCwdFilter;
use codex_app_server_protocol::ThreadListParams;
use codex_app_server_protocol::ThreadSortKey;
use codex_app_server_protocol::ThreadSourceKind;
use codex_protocol::ThreadId;
use codex_utils_path as path_utils;
use color_eyre::eyre::Result;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Stylize as _;
use ratatui::text::Line;
use ratatui::text::Span;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tracing::warn;
use unicode_width::UnicodeWidthStr;

const PAGE_SIZE: usize = 25;
const LOAD_NEAR_THRESHOLD: usize = 5;
const SESSION_META_INDENT_WIDTH: usize = 2;
const SESSION_META_DATE_WIDTH: usize = 12;
const SESSION_META_FIELD_GAP_WIDTH: usize = 2;
const SESSION_META_MIN_CWD_WIDTH: usize = 30;
const SESSION_META_MAX_CWD_WIDTH: usize = 72;
const SESSION_META_BRANCH_ICON: &str = "";
const SESSION_META_CWD_ICON: &str = "⌁";

#[derive(Debug, Clone)]
pub struct SessionTarget {
    pub path: Option<PathBuf>,
    pub thread_id: ThreadId,
}

impl SessionTarget {
    pub fn display_label(&self) -> String {
        self.path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| format!("thread {}", self.thread_id))
    }
}

#[derive(Debug, Clone)]
pub enum SessionSelection {
    StartFresh,
    Resume(SessionTarget),
    Fork(SessionTarget),
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

    fn selection(self, path: Option<PathBuf>, thread_id: ThreadId) -> SessionSelection {
        let target_session = SessionTarget { path, thread_id };
        match self {
            SessionPickerAction::Resume => SessionSelection::Resume(target_session),
            SessionPickerAction::Fork => SessionSelection::Fork(target_session),
        }
    }
}

#[derive(Clone)]
struct PageLoadRequest {
    cursor: Option<PageCursor>,
    request_token: usize,
    search_token: Option<usize>,
    provider_filter: ProviderFilter,
    sort_key: ThreadSortKey,
}

enum PickerLoadRequest {
    Page(PageLoadRequest),
    Preview { thread_id: ThreadId },
}

#[derive(Clone)]
enum ProviderFilter {
    Any,
    MatchDefault(String),
}

type PickerLoader = Arc<dyn Fn(PickerLoadRequest) + Send + Sync>;

enum BackgroundEvent {
    PageLoaded {
        request_token: usize,
        search_token: Option<usize>,
        page: std::io::Result<PickerPage>,
    },
    PreviewLoaded {
        thread_id: ThreadId,
        preview: std::io::Result<Vec<TranscriptPreviewLine>>,
    },
}

#[derive(Clone)]
enum PageCursor {
    AppServer(String),
}

struct PickerPage {
    rows: Vec<Row>,
    next_cursor: Option<PageCursor>,
    num_scanned_files: usize,
    reached_scan_cap: bool,
}

/// Interactive session picker that lists app-server threads with simple search,
/// lazy transcript previews, and pagination.
///
/// Sessions render as compact multi-line records with stable metadata first and
/// the conversation preview last. Users can toggle between sorting by creation
/// time and last-updated time using Tab, and can expand the selected session with
/// Space to load recent transcript context on demand.
///
/// Sessions are loaded on-demand via cursor-based pagination. The backend
/// `thread/list` API returns pages ordered by the selected sort key, and the
/// picker deduplicates across pages to handle overlapping windows when new
/// sessions appear during pagination.
///
/// Filtering happens in two layers:
/// 1. Provider, source, and eligible working-directory filtering at the backend.
/// 2. Typed search filtering over loaded rows in the picker.
pub async fn run_resume_picker_with_app_server(
    tui: &mut Tui,
    config: &Config,
    show_all: bool,
    include_non_interactive: bool,
    app_server: AppServerSession,
) -> Result<SessionSelection> {
    let (bg_tx, bg_rx) = mpsc::unbounded_channel();
    let is_remote = app_server.is_remote();
    let cwd_filter = picker_cwd_filter(
        config.cwd.as_path(),
        show_all,
        is_remote,
        app_server.remote_cwd_override(),
    );
    run_session_picker_with_loader(
        tui,
        config,
        show_all,
        SessionPickerAction::Resume,
        is_remote,
        spawn_app_server_page_loader(app_server, cwd_filter, include_non_interactive, bg_tx),
        bg_rx,
    )
    .await
}

pub async fn run_fork_picker_with_app_server(
    tui: &mut Tui,
    config: &Config,
    show_all: bool,
    app_server: AppServerSession,
) -> Result<SessionSelection> {
    let (bg_tx, bg_rx) = mpsc::unbounded_channel();
    let is_remote = app_server.is_remote();
    let cwd_filter = picker_cwd_filter(
        config.cwd.as_path(),
        show_all,
        is_remote,
        app_server.remote_cwd_override(),
    );
    run_session_picker_with_loader(
        tui,
        config,
        show_all,
        SessionPickerAction::Fork,
        is_remote,
        spawn_app_server_page_loader(
            app_server, cwd_filter, /*include_non_interactive*/ false, bg_tx,
        ),
        bg_rx,
    )
    .await
}

async fn run_session_picker_with_loader(
    tui: &mut Tui,
    config: &Config,
    show_all: bool,
    action: SessionPickerAction,
    is_remote: bool,
    picker_loader: PickerLoader,
    bg_rx: mpsc::UnboundedReceiver<BackgroundEvent>,
) -> Result<SessionSelection> {
    let alt = AltScreenGuard::enter(tui);
    let provider_filter = if is_remote {
        ProviderFilter::Any
    } else {
        ProviderFilter::MatchDefault(config.model_provider_id.to_string())
    };
    // Remote sessions live in the server's filesystem namespace, so the client
    // process cwd is not a meaningful row filter. Local cwd filtering and explicit
    // remote --cd filtering are handled server-side in thread/list.
    let filter_cwd = None;

    let mut state = PickerState::new(
        alt.tui.frame_requester(),
        picker_loader,
        provider_filter,
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
                    TuiEvent::Draw | TuiEvent::Resize => {
                        if let Ok(size) = alt.tui.terminal.size() {
                            let list_height = size.height.saturating_sub(6) as usize;
                            state.update_viewport(list_height, size.width);
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

fn picker_cwd_filter(
    config_cwd: &Path,
    show_all: bool,
    is_remote: bool,
    remote_cwd_override: Option<&Path>,
) -> Option<PathBuf> {
    if show_all {
        None
    } else if is_remote {
        remote_cwd_override.map(Path::to_path_buf)
    } else {
        Some(config_cwd.to_path_buf())
    }
}

fn spawn_app_server_page_loader(
    app_server: AppServerSession,
    cwd_filter: Option<PathBuf>,
    include_non_interactive: bool,
    bg_tx: mpsc::UnboundedSender<BackgroundEvent>,
) -> PickerLoader {
    let (request_tx, mut request_rx) = mpsc::unbounded_channel::<PickerLoadRequest>();

    tokio::spawn(async move {
        let mut app_server = app_server;
        while let Some(request) = request_rx.recv().await {
            match request {
                PickerLoadRequest::Page(request) => {
                    let cursor = request.cursor.map(|PageCursor::AppServer(cursor)| cursor);
                    let page = load_app_server_page(
                        &mut app_server,
                        cursor,
                        cwd_filter.as_deref(),
                        request.provider_filter,
                        request.sort_key,
                        include_non_interactive,
                    )
                    .await;
                    let _ = bg_tx.send(BackgroundEvent::PageLoaded {
                        request_token: request.request_token,
                        search_token: request.search_token,
                        page,
                    });
                }
                PickerLoadRequest::Preview { thread_id } => {
                    let preview = load_transcript_preview(&mut app_server, thread_id).await;
                    let _ = bg_tx.send(BackgroundEvent::PreviewLoaded { thread_id, preview });
                }
            }
        }
        if let Err(err) = app_server.shutdown().await {
            warn!(%err, "Failed to shut down app-server picker session");
        }
    });

    Arc::new(move |request: PickerLoadRequest| {
        let _ = request_tx.send(request);
    })
}

/// Returns the human-readable column header for the given sort key.
fn sort_key_label(sort_key: ThreadSortKey) -> &'static str {
    match sort_key {
        ThreadSortKey::CreatedAt => "Created",
        ThreadSortKey::UpdatedAt => "Updated",
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
    requester: FrameRequester,
    relative_time_reference: Option<DateTime<Utc>>,
    pagination: PaginationState,
    all_rows: Vec<Row>,
    filtered_rows: Vec<Row>,
    seen_rows: HashSet<SeenRowKey>,
    selected: usize,
    scroll_top: usize,
    query: String,
    search_state: SearchState,
    next_request_token: usize,
    next_search_token: usize,
    picker_loader: PickerLoader,
    view_rows: Option<usize>,
    view_width: Option<u16>,
    provider_filter: ProviderFilter,
    show_all: bool,
    filter_cwd: Option<PathBuf>,
    action: SessionPickerAction,
    sort_key: ThreadSortKey,
    inline_error: Option<String>,
    expanded_thread_id: Option<ThreadId>,
    transcript_previews: HashMap<ThreadId, TranscriptPreviewState>,
}

struct PaginationState {
    next_cursor: Option<PageCursor>,
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

#[derive(Clone)]
enum TranscriptPreviewState {
    Loading,
    Loaded(Vec<TranscriptPreviewLine>),
    Failed,
}

#[derive(Clone)]
struct TranscriptPreviewLine {
    speaker: TranscriptPreviewSpeaker,
    text: String,
}

#[derive(Clone, Copy)]
enum TranscriptPreviewSpeaker {
    User,
    Assistant,
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

async fn load_app_server_page(
    app_server: &mut AppServerSession,
    cursor: Option<String>,
    cwd_filter: Option<&Path>,
    provider_filter: ProviderFilter,
    sort_key: ThreadSortKey,
    include_non_interactive: bool,
) -> std::io::Result<PickerPage> {
    let response = app_server
        .thread_list(thread_list_params(
            cursor,
            cwd_filter,
            provider_filter,
            sort_key,
            include_non_interactive,
        ))
        .await
        .map_err(std::io::Error::other)?;
    let num_scanned_files = response.data.len();

    Ok(PickerPage {
        rows: response
            .data
            .into_iter()
            .filter_map(row_from_app_server_thread)
            .collect(),
        next_cursor: response.next_cursor.map(PageCursor::AppServer),
        num_scanned_files,
        reached_scan_cap: false,
    })
}

async fn load_transcript_preview(
    app_server: &mut AppServerSession,
    thread_id: ThreadId,
) -> std::io::Result<Vec<TranscriptPreviewLine>> {
    const MAX_PREVIEW_LINES: usize = 6;

    let thread = app_server
        .thread_read(thread_id, /*include_turns*/ true)
        .await
        .map_err(std::io::Error::other)?;
    let mut lines = thread
        .turns
        .iter()
        .flat_map(|turn| turn.items.iter())
        .filter_map(|item| match item {
            ThreadItem::UserMessage { content, .. } => Some(TranscriptPreviewLine {
                speaker: TranscriptPreviewSpeaker::User,
                text: content
                    .iter()
                    .filter_map(|input| match input {
                        codex_app_server_protocol::UserInput::Text { text, .. } => {
                            Some(text.as_str())
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join(" "),
            }),
            ThreadItem::AgentMessage { text, .. } => Some(TranscriptPreviewLine {
                speaker: TranscriptPreviewSpeaker::Assistant,
                text: text.clone(),
            }),
            _ => None,
        })
        .flat_map(|line| {
            line.text
                .lines()
                .filter(|text| !text.trim().is_empty())
                .map(move |text| TranscriptPreviewLine {
                    speaker: line.speaker,
                    text: text.trim().to_string(),
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    if lines.len() > MAX_PREVIEW_LINES {
        lines.drain(..lines.len() - MAX_PREVIEW_LINES);
    }
    Ok(lines)
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
    path: Option<PathBuf>,
    preview: String,
    thread_id: Option<ThreadId>,
    thread_name: Option<String>,
    created_at: Option<DateTime<Utc>>,
    updated_at: Option<DateTime<Utc>>,
    cwd: Option<PathBuf>,
    git_branch: Option<String>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum SeenRowKey {
    Path(PathBuf),
    Thread(ThreadId),
}

impl Row {
    fn seen_key(&self) -> Option<SeenRowKey> {
        if let Some(path) = self.path.clone() {
            return Some(SeenRowKey::Path(path));
        }
        self.thread_id.map(SeenRowKey::Thread)
    }

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
        if self
            .thread_id
            .is_some_and(|thread_id| thread_id.to_string().to_lowercase().contains(query))
        {
            return true;
        }
        if self
            .git_branch
            .as_ref()
            .is_some_and(|branch| branch.to_lowercase().contains(query))
        {
            return true;
        }
        if self
            .cwd
            .as_ref()
            .is_some_and(|cwd| cwd.to_string_lossy().to_lowercase().contains(query))
        {
            return true;
        }
        false
    }
}

impl PickerState {
    fn new(
        requester: FrameRequester,
        picker_loader: PickerLoader,
        provider_filter: ProviderFilter,
        show_all: bool,
        filter_cwd: Option<PathBuf>,
        action: SessionPickerAction,
    ) -> Self {
        Self {
            requester,
            relative_time_reference: None,
            pagination: PaginationState {
                next_cursor: None,
                num_scanned_files: 0,
                reached_scan_cap: false,
                loading: LoadingState::Idle,
            },
            all_rows: Vec::new(),
            filtered_rows: Vec::new(),
            seen_rows: HashSet::new(),
            selected: 0,
            scroll_top: 0,
            query: String::new(),
            search_state: SearchState::Idle,
            next_request_token: 0,
            next_search_token: 0,
            picker_loader,
            view_rows: None,
            view_width: None,
            provider_filter,
            show_all,
            filter_cwd,
            action,
            sort_key: ThreadSortKey::UpdatedAt,
            inline_error: None,
            expanded_thread_id: None,
            transcript_previews: HashMap::new(),
        }
    }

    fn request_frame(&self) {
        self.requester.schedule_frame();
    }

    async fn handle_key(&mut self, key: KeyEvent) -> Result<Option<SessionSelection>> {
        self.inline_error = None;
        match key {
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                if self.query.is_empty() {
                    return Ok(Some(SessionSelection::StartFresh));
                }
                self.clear_query_preserving_selection();
            }
            KeyEvent {
                code: KeyCode::Char('c'),
                modifiers,
                ..
            } if modifiers.contains(KeyModifiers::CONTROL) => {
                return Ok(Some(SessionSelection::Exit));
            }
            KeyEvent {
                code: KeyCode::Enter,
                ..
            } => {
                if let Some(row) = self.filtered_rows.get(self.selected) {
                    let path = row.path.clone();
                    let thread_id = match row.thread_id {
                        Some(thread_id) => Some(thread_id),
                        None => match path.as_ref() {
                            Some(path) => {
                                resolve_session_thread_id(path.as_path(), /*id_str_if_uuid*/ None)
                                    .await
                            }
                            None => None,
                        },
                    };
                    if let Some(thread_id) = thread_id {
                        return Ok(Some(self.action.selection(path, thread_id)));
                    }
                    self.inline_error = Some(match path {
                        Some(path) => {
                            format!("Failed to read session metadata from {}", path.display())
                        }
                        None => {
                            String::from("Failed to read session metadata from selected session")
                        }
                    });
                    self.request_frame();
                }
            }
            KeyEvent {
                code: KeyCode::Up, ..
            }
            | KeyEvent {
                code: KeyCode::Char('p'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('\u{0010}'),
                modifiers: KeyModifiers::NONE,
                ..
            } /* ^P */ => {
                if self.selected > 0 {
                    self.selected -= 1;
                    self.ensure_selected_visible();
                }
                self.request_frame();
            }
            KeyEvent {
                code: KeyCode::Down,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('n'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('\u{000e}'),
                modifiers: KeyModifiers::NONE,
                ..
            } /* ^N */ => {
                if self.selected + 1 < self.filtered_rows.len() {
                    self.selected += 1;
                    self.ensure_selected_visible();
                }
                self.maybe_load_more_for_scroll();
                self.request_frame();
            }
            KeyEvent {
                code: KeyCode::PageUp,
                ..
            } => {
                let step = self.view_rows.unwrap_or(10).max(1);
                if self.selected > 0 {
                    self.selected = self.selected.saturating_sub(step);
                    self.ensure_selected_visible();
                    self.request_frame();
                }
            }
            KeyEvent {
                code: KeyCode::PageDown,
                ..
            } => {
                if !self.filtered_rows.is_empty() {
                    let step = self.view_rows.unwrap_or(10).max(1);
                    let max_index = self.filtered_rows.len().saturating_sub(1);
                    self.selected = (self.selected + step).min(max_index);
                    self.ensure_selected_visible();
                    self.maybe_load_more_for_scroll();
                    self.request_frame();
                }
            }
            KeyEvent {
                code: KeyCode::Tab, ..
            } => {
                self.toggle_sort_key();
                self.request_frame();
            }
            KeyEvent {
                code: KeyCode::Char(' '),
                ..
            } => {
                self.toggle_selected_expansion();
            }
            KeyEvent {
                code: KeyCode::Backspace,
                ..
            } => {
                let mut new_query = self.query.clone();
                new_query.pop();
                self.set_query(new_query);
            }
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers,
                ..
            } => {
                // basic text input for search
                if !modifiers.contains(KeyModifiers::CONTROL)
                    && !modifiers.contains(KeyModifiers::ALT)
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
        self.relative_time_reference = Some(Utc::now());
        self.reset_pagination();
        self.all_rows.clear();
        self.filtered_rows.clear();
        self.seen_rows.clear();
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

        (self.picker_loader)(PickerLoadRequest::Page(PageLoadRequest {
            cursor: None,
            request_token,
            search_token,
            provider_filter: self.provider_filter.clone(),
            sort_key: self.sort_key,
        }));
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
                let completed_token = pending.search_token.or(search_token);
                self.continue_search_if_token_matches(completed_token);
            }
            BackgroundEvent::PreviewLoaded { thread_id, preview } => {
                self.transcript_previews.insert(
                    thread_id,
                    match preview {
                        Ok(lines) => TranscriptPreviewState::Loaded(lines),
                        Err(_) => TranscriptPreviewState::Failed,
                    },
                );
                self.request_frame();
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

    fn ingest_page(&mut self, page: PickerPage) {
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

        for row in page.rows {
            if let Some(seen_key) = row.seen_key() {
                if self.seen_rows.insert(seen_key) {
                    self.all_rows.push(row);
                }
            } else {
                self.all_rows.push(row);
            }
        }

        self.apply_filter();
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

    fn clear_query_preserving_selection(&mut self) {
        let selected_key = self
            .filtered_rows
            .get(self.selected)
            .and_then(Row::seen_key);
        self.query.clear();
        self.search_state = SearchState::Idle;
        self.apply_filter();
        if let Some(selected_key) = selected_key
            && let Some(index) = self
                .filtered_rows
                .iter()
                .position(|row| row.seen_key().as_ref() == Some(&selected_key))
        {
            self.selected = index;
            self.ensure_selected_visible();
            self.request_frame();
        }
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
        let viewport_rows = self.view_rows.unwrap_or(usize::MAX).max(1);
        if self.selected < self.scroll_top {
            self.scroll_top = self.selected;
        }
        while self.rendered_height_between(self.scroll_top, self.selected)
            > self.available_content_rows(viewport_rows)
            && self.scroll_top < self.selected
        {
            self.scroll_top += 1;
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

    fn update_viewport(&mut self, rows: usize, width: u16) {
        self.view_rows = if rows == 0 { None } else { Some(rows) };
        self.view_width = Some(width);
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

        (self.picker_loader)(PickerLoadRequest::Page(PageLoadRequest {
            cursor: Some(cursor),
            request_token,
            search_token,
            provider_filter: self.provider_filter.clone(),
            sort_key: self.sort_key,
        }));
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

    fn toggle_selected_expansion(&mut self) {
        let Some(row) = self.filtered_rows.get(self.selected) else {
            return;
        };
        let Some(thread_id) = row.thread_id else {
            return;
        };
        if self.expanded_thread_id == Some(thread_id) {
            self.expanded_thread_id = None;
            self.request_frame();
            return;
        }
        self.expanded_thread_id = Some(thread_id);
        if let std::collections::hash_map::Entry::Vacant(e) =
            self.transcript_previews.entry(thread_id)
        {
            e.insert(TranscriptPreviewState::Loading);
            (self.picker_loader)(PickerLoadRequest::Preview { thread_id });
        }
        self.request_frame();
    }

    fn rendered_height_between(&self, start: usize, end_inclusive: usize) -> usize {
        self.filtered_rows
            .get(start..=end_inclusive)
            .unwrap_or_default()
            .iter()
            .enumerate()
            .map(|(offset, row)| {
                let row_idx = start + offset;
                let is_selected = row_idx == self.selected;
                let is_expanded = is_selected
                    && row.thread_id.is_some()
                    && self.expanded_thread_id == row.thread_id;
                render_session_lines(
                    row,
                    self,
                    is_selected,
                    is_expanded,
                    self.view_width.unwrap_or(u16::MAX),
                )
                .len()
            })
            .sum::<usize>()
            + end_inclusive.saturating_sub(start)
    }

    fn has_more_above(&self) -> bool {
        self.scroll_top > 0
    }

    fn has_more_below(&self, viewport_height: usize) -> bool {
        if self.filtered_rows.is_empty() {
            return false;
        }
        if self.pagination.next_cursor.is_some() {
            return true;
        }
        let capacity = self.available_content_rows(viewport_height);
        let mut used = 0usize;
        for (offset, row) in self.filtered_rows[self.scroll_top..].iter().enumerate() {
            let row_idx = self.scroll_top + offset;
            let is_selected = row_idx == self.selected;
            let is_expanded =
                is_selected && row.thread_id.is_some() && self.expanded_thread_id == row.thread_id;
            let row_height = render_session_lines(
                row,
                self,
                is_selected,
                is_expanded,
                self.view_width.unwrap_or(u16::MAX),
            )
            .len();
            let separator_height = usize::from(offset > 0);
            if used + separator_height + row_height > capacity {
                return true;
            }
            used += separator_height + row_height;
        }
        false
    }

    fn available_content_rows(&self, viewport_height: usize) -> usize {
        viewport_height
            .saturating_sub(usize::from(self.has_more_above()))
            .saturating_sub(usize::from(
                self.pagination.next_cursor.is_some()
                    || self.selected + 1 < self.filtered_rows.len(),
            ))
            .max(1)
    }
}

fn row_from_app_server_thread(thread: Thread) -> Option<Row> {
    let thread_id = match ThreadId::from_string(&thread.id) {
        Ok(thread_id) => thread_id,
        Err(err) => {
            warn!(thread_id = thread.id, %err, "Skipping app-server picker row with invalid id");
            return None;
        }
    };
    let preview = thread.preview.trim();
    Some(Row {
        path: thread.path,
        preview: if preview.is_empty() {
            String::from("(no message yet)")
        } else {
            preview.to_string()
        },
        thread_id: Some(thread_id),
        thread_name: thread.name,
        created_at: chrono::DateTime::from_timestamp(thread.created_at, 0)
            .map(|dt| dt.with_timezone(&Utc)),
        updated_at: chrono::DateTime::from_timestamp(thread.updated_at, 0)
            .map(|dt| dt.with_timezone(&Utc)),
        cwd: Some(thread.cwd.to_path_buf()),
        git_branch: thread.git_info.and_then(|git_info| git_info.branch),
    })
}

fn thread_list_params(
    cursor: Option<String>,
    cwd_filter: Option<&Path>,
    provider_filter: ProviderFilter,
    sort_key: ThreadSortKey,
    include_non_interactive: bool,
) -> ThreadListParams {
    ThreadListParams {
        cursor,
        limit: Some(PAGE_SIZE as u32),
        sort_key: Some(sort_key),
        sort_direction: None,
        model_providers: match provider_filter {
            ProviderFilter::Any => None,
            ProviderFilter::MatchDefault(default_provider) => Some(vec![default_provider]),
        },
        source_kinds: (!include_non_interactive)
            .then_some(vec![ThreadSourceKind::Cli, ThreadSourceKind::VsCode]),
        archived: Some(false),
        cwd: cwd_filter.map(|cwd| ThreadListCwdFilter::One(cwd.to_string_lossy().into_owned())),
        use_state_db_only: false,
        search_term: None,
    }
}

fn paths_match(a: &Path, b: &Path) -> bool {
    path_utils::paths_match_after_normalization(a, b)
}

#[cfg_attr(not(test), allow(dead_code))]
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
        let [
            header,
            _header_gap,
            search,
            _search_gap,
            list,
            _footer_gap,
            hint,
        ] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(area.height.saturating_sub(6)),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(area);

        let chrome = |area: Rect| {
            Rect::new(
                area.x.saturating_add(1),
                area.y,
                area.width.saturating_sub(2),
                area.height,
            )
        };

        // Header
        let header_title = if default_bg().is_some_and(is_light) {
            state.action.title().bold().fg(Color::Indexed(22))
        } else {
            state.action.title().bold().cyan()
        };
        let header_line: Line = vec![header_title].into();
        frame.render_widget_ref(header_line, chrome(header));

        // Search line
        let search = chrome(search);
        frame.render_widget_ref(search_line(state, search.width), search);

        let list = Rect::new(
            list.x.saturating_add(2),
            list.y,
            list.width.saturating_sub(4),
            list.height,
        );
        render_list(frame, list, state);

        // Hint line
        frame.render_widget_ref(hint_line(state), hint);
    })
}

fn search_line(state: &PickerState, width: u16) -> Line<'_> {
    if let Some(error) = state.inline_error.as_deref() {
        return Line::from(error.red());
    }
    let search = if state.query.is_empty() {
        "Type to search".dim()
    } else {
        format!("Search: {}", state.query).into()
    };
    let sort_prefix = "Sort: ";
    let sort_value = sort_key_label(state.sort_key);
    let search_width = search.content.chars().count();
    let sort_width = sort_prefix.chars().count() + sort_value.chars().count();
    let spacer_width = width
        .saturating_sub((search_width + sort_width) as u16)
        .max(2) as usize;
    vec![
        search,
        " ".repeat(spacer_width).into(),
        sort_prefix.dim(),
        sort_value.magenta(),
    ]
    .into()
}

fn hint_line(state: &PickerState) -> Line<'_> {
    let action_label = state.action.action_label();
    let esc_label = if state.query.is_empty() {
        " to start new "
    } else {
        " to clear search "
    };
    vec![
        key_hint::plain(KeyCode::Enter).into(),
        format!(" to {action_label} ").dim(),
        "    ".dim(),
        key_hint::plain(KeyCode::Esc).into(),
        esc_label.dim(),
        "    ".dim(),
        key_hint::ctrl(KeyCode::Char('c')).into(),
        " to quit ".dim(),
        "    ".dim(),
        key_hint::plain(KeyCode::Tab).into(),
        " to toggle sort ".dim(),
        "    ".dim(),
        key_hint::plain(KeyCode::Char(' ')).into(),
        " to expand ".dim(),
        "    ".dim(),
        key_hint::plain(KeyCode::Up).into(),
        "/".dim(),
        key_hint::plain(KeyCode::Down).into(),
        " to browse".dim(),
    ]
    .into()
}

fn render_list(frame: &mut crate::custom_terminal::Frame, area: Rect, state: &PickerState) {
    if area.height == 0 {
        return;
    }

    let rows = &state.filtered_rows;
    if rows.is_empty() {
        let message = render_empty_state_line(state);
        frame.render_widget_ref(message, area);
        return;
    }

    let show_more_above = state.has_more_above();
    let show_more_below = state.has_more_below(area.height as usize);
    let content_area = Rect::new(
        area.x,
        area.y.saturating_add(u16::from(show_more_above)),
        area.width,
        area.height
            .saturating_sub(u16::from(show_more_above))
            .saturating_sub(u16::from(show_more_below)),
    );
    if show_more_above {
        frame.render_widget_ref(
            more_line("↑ more"),
            Rect::new(area.x, area.y, area.width, 1),
        );
    }

    let start = state.scroll_top.min(rows.len().saturating_sub(1));
    let mut y = content_area.y;
    for (idx, row) in rows[start..].iter().enumerate() {
        if y >= content_area.y.saturating_add(content_area.height) {
            break;
        }
        let row_idx = start + idx;
        let is_selected = row_idx == state.selected;
        let is_expanded =
            is_selected && row.thread_id.is_some() && state.expanded_thread_id == row.thread_id;
        for line in render_session_lines(row, state, is_selected, is_expanded, area.width) {
            if y >= content_area.y.saturating_add(content_area.height) {
                break;
            }
            frame.render_widget_ref(line, Rect::new(area.x, y, area.width, 1));
            y = y.saturating_add(1);
        }
        if y < content_area.y.saturating_add(content_area.height) && start + idx + 1 < rows.len() {
            y = y.saturating_add(1);
        }
    }

    if state.pagination.loading.is_pending()
        && y < content_area.y.saturating_add(content_area.height)
    {
        let loading_line: Line = vec!["  ".into(), "Loading older sessions…".italic().dim()].into();
        let rect = Rect::new(area.x, y, area.width, 1);
        frame.render_widget_ref(loading_line, rect);
    }
    if show_more_below {
        let label = if state.pagination.loading.is_pending() {
            "↓ loading more"
        } else {
            "↓ more"
        };
        frame.render_widget_ref(
            more_line(label),
            Rect::new(
                area.x,
                area.y.saturating_add(area.height.saturating_sub(1)),
                area.width,
                1,
            ),
        );
    }
}

fn more_line(label: &'static str) -> Line<'static> {
    vec![label.dim()].into()
}

fn render_session_lines(
    row: &Row,
    state: &PickerState,
    is_selected: bool,
    is_expanded: bool,
    width: u16,
) -> Vec<Line<'static>> {
    let marker = match (is_selected, is_expanded) {
        (true, true) => "▾ ".bold().cyan(),
        (true, false) => "▸ ".bold().cyan(),
        (false, _) => "  ".into(),
    };
    let reference = state.relative_time_reference.unwrap_or_else(Utc::now);
    let created = format_relative_time(reference, row.created_at);
    let updated = format_relative_time(reference, row.updated_at.or(row.created_at));
    let branch = row.git_branch.as_deref();
    let cwd = row
        .cwd
        .as_ref()
        .map(|path| format_directory_display(path, /*max_width*/ None));
    let title = truncate_text(row.display_preview(), width.saturating_sub(2) as usize);
    let title = if is_selected {
        if default_bg().is_some_and(is_light) {
            title.magenta()
        } else {
            title.yellow()
        }
    } else {
        title.into()
    };
    let mut lines = vec![vec![marker, title].into()];
    if is_expanded {
        lines.extend(render_transcript_preview_lines(row, state, width));
    }
    lines.extend(render_footer_lines(
        state.sort_key,
        &created,
        &updated,
        branch,
        cwd.as_deref(),
        width,
    ));
    lines
}

fn render_footer_lines(
    sort_key: ThreadSortKey,
    created: &str,
    updated: &str,
    branch: Option<&str>,
    cwd: Option<&str>,
    width: u16,
) -> Vec<Line<'static>> {
    let date = match sort_key {
        ThreadSortKey::CreatedAt => created,
        ThreadSortKey::UpdatedAt => updated,
    };
    let parts = vec![
        FooterPart::Date(date.to_string()),
        FooterPart::Cwd(cwd.map(str::to_string)),
        FooterPart::Branch(branch.map(str::to_string)),
    ];
    pack_footer_parts(parts, width)
}

enum FooterPart {
    Date(String),
    Branch(Option<String>),
    Cwd(Option<String>),
}

impl FooterPart {
    fn text(&self) -> &str {
        match self {
            FooterPart::Date(text) => text,
            FooterPart::Branch(Some(text)) | FooterPart::Cwd(Some(text)) => text,
            FooterPart::Branch(None) => "no branch",
            FooterPart::Cwd(None) => "no cwd",
        }
    }

    fn prefix(&self) -> Option<&'static str> {
        match self {
            FooterPart::Date(_) => None,
            FooterPart::Branch(_) => Some(SESSION_META_BRANCH_ICON),
            FooterPart::Cwd(_) => Some(SESSION_META_CWD_ICON),
        }
    }
}

fn pack_footer_parts(parts: Vec<FooterPart>, width: u16) -> Vec<Line<'static>> {
    let available_width = width as usize;
    if available_width <= SESSION_META_INDENT_WIDTH {
        return Vec::new();
    }
    let cwd_width = cwd_column_width(available_width);
    let all_parts_width = footer_parts_width(&parts, cwd_width);
    if all_parts_width <= available_width {
        return vec![footer_line(parts, available_width, cwd_width)];
    }

    let mut lines = Vec::with_capacity(parts.len());
    let mut current_parts = Vec::new();
    for part in parts {
        let mut candidate_parts = std::mem::take(&mut current_parts);
        candidate_parts.push(part);
        if candidate_parts.len() > 1
            && footer_parts_width(&candidate_parts, cwd_width) > available_width
        {
            let previous_parts = candidate_parts
                .drain(..candidate_parts.len().saturating_sub(1))
                .collect();
            lines.push(footer_line(previous_parts, available_width, cwd_width));
        }
        current_parts = candidate_parts;
    }
    if !current_parts.is_empty() {
        lines.push(footer_line(current_parts, available_width, cwd_width));
    }
    lines
}

fn cwd_column_width(width: usize) -> usize {
    let available = width.saturating_sub(
        SESSION_META_INDENT_WIDTH + SESSION_META_DATE_WIDTH + 2 * SESSION_META_FIELD_GAP_WIDTH,
    );
    (available / 2).clamp(SESSION_META_MIN_CWD_WIDTH, SESSION_META_MAX_CWD_WIDTH)
}

fn footer_parts_width(parts: &[FooterPart], cwd_width: usize) -> usize {
    let content_width: usize = parts
        .iter()
        .enumerate()
        .map(|(idx, part)| footer_part_width(part, idx + 1 < parts.len(), cwd_width))
        .sum();
    SESSION_META_INDENT_WIDTH + content_width
}

fn footer_part_width(part: &FooterPart, padded: bool, cwd_width: usize) -> usize {
    let prefix_width = part.prefix().map_or(0, UnicodeWidthStr::width);
    let prefix_gap_width = usize::from(part.prefix().is_some() && !part.text().is_empty());
    let text_width = UnicodeWidthStr::width(part.text());
    let actual_width = prefix_width + prefix_gap_width + text_width;
    match part {
        FooterPart::Date(_) if padded => SESSION_META_DATE_WIDTH.max(actual_width),
        FooterPart::Cwd(_) if padded => cwd_width,
        _ => actual_width,
    }
}

fn footer_line(parts: Vec<FooterPart>, width: usize, cwd_width: usize) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = vec!["  ".into()];
    let mut remaining_width = width.saturating_sub(SESSION_META_INDENT_WIDTH);
    let part_count = parts.len();
    for (idx, part) in parts.into_iter().enumerate() {
        if idx > 0 {
            let gap_width = SESSION_META_FIELD_GAP_WIDTH.min(remaining_width);
            if gap_width > 0 {
                spans.push(" ".repeat(gap_width).dim());
                remaining_width = remaining_width.saturating_sub(gap_width);
            }
        }
        let padded = idx + 1 < part_count;
        let target_width = match part {
            FooterPart::Date(_) if padded => Some(SESSION_META_DATE_WIDTH),
            FooterPart::Cwd(_) if padded => Some(cwd_width),
            FooterPart::Date(_) | FooterPart::Branch(_) | FooterPart::Cwd(_) => None,
        };
        let used_width = push_footer_part(&mut spans, part, target_width, remaining_width);
        remaining_width = remaining_width.saturating_sub(used_width);
        if let Some(target_width) = target_width {
            let padding = target_width.saturating_sub(used_width);
            if padding > 0 {
                spans.push(" ".repeat(padding).dim());
                remaining_width = remaining_width.saturating_sub(padding);
            }
        }
    }
    spans.into()
}

fn push_footer_part(
    spans: &mut Vec<Span<'static>>,
    part: FooterPart,
    target_width: Option<usize>,
    available_width: usize,
) -> usize {
    let text = part.text().to_string();
    let Some(prefix) = part.prefix() else {
        let text = truncate_text(&text, available_width);
        let width = UnicodeWidthStr::width(text.as_str());
        spans.push(text.dim());
        return width;
    };

    let prefix_width = UnicodeWidthStr::width(prefix);
    if available_width <= prefix_width {
        let prefix = truncate_text(prefix, available_width);
        let width = UnicodeWidthStr::width(prefix.as_str());
        spans.push(prefix.dim());
        return width;
    }

    spans.push(prefix.dim());
    let mut used_width = prefix_width;
    if !text.is_empty() && used_width < available_width {
        spans.push(" ".dim());
        used_width += 1;
    }
    let text_width = target_width
        .unwrap_or(available_width)
        .saturating_sub(used_width)
        .min(available_width.saturating_sub(used_width));
    let text = truncate_text(&text, text_width);
    let rendered_text_width = UnicodeWidthStr::width(text.as_str());
    match part {
        FooterPart::Branch(None) | FooterPart::Cwd(None) => spans.push(text.dim().italic()),
        _ => spans.push(text.dim()),
    }
    used_width + rendered_text_width
}

fn render_transcript_preview_lines(
    row: &Row,
    state: &PickerState,
    width: u16,
) -> Vec<Line<'static>> {
    let Some(thread_id) = row.thread_id else {
        return Vec::new();
    };
    match state.transcript_previews.get(&thread_id) {
        Some(TranscriptPreviewState::Loading) => {
            vec![vec!["  │ ".dim(), "Loading recent transcript...".italic().dim()].into()]
        }
        Some(TranscriptPreviewState::Failed) => vec![
            vec![
                "  │ ".dim(),
                "Could not load transcript preview".italic().red(),
            ]
            .into(),
        ],
        Some(TranscriptPreviewState::Loaded(lines)) if lines.is_empty() => vec![
            vec![
                "  └ ".dim(),
                "No transcript preview available".italic().dim(),
            ]
            .into(),
        ],
        Some(TranscriptPreviewState::Loaded(lines)) => {
            let mut rendered = Vec::new();
            for line in lines {
                rendered.extend(render_transcript_content_lines(line, width));
            }
            let rendered_len = rendered.len();
            rendered
                .into_iter()
                .enumerate()
                .map(|(idx, line)| {
                    let prefix = if idx + 1 == rendered_len {
                        "  └ "
                    } else {
                        "  │ "
                    };
                    prefix_transcript_line(prefix, line)
                })
                .collect()
        }
        None => Vec::new(),
    }
}

fn render_transcript_content_lines(line: &TranscriptPreviewLine, width: u16) -> Vec<Line<'static>> {
    let content_width = width.saturating_sub(4) as usize;
    match line.speaker {
        TranscriptPreviewSpeaker::User => vec![
            Line::from(truncate_text(&line.text, content_width))
                .cyan()
                .dim()
                .italic(),
        ],
        TranscriptPreviewSpeaker::Assistant => {
            let mut lines = Vec::new();
            append_markdown(
                &line.text,
                Some(content_width),
                /*cwd*/ None,
                &mut lines,
            );
            for line in &mut lines {
                *line = line.clone().dim();
            }
            lines
        }
    }
}

fn prefix_transcript_line(prefix: &'static str, line: Line<'static>) -> Line<'static> {
    let mut spans = vec![prefix.dim()];
    spans.extend(line.spans);
    Line::from(spans).style(line.style)
}

fn format_relative_time(reference: DateTime<Utc>, ts: Option<DateTime<Utc>>) -> String {
    let Some(ts) = ts else {
        return "-".to_string();
    };
    let seconds = (reference - ts).num_seconds().max(0);
    if seconds < 60 {
        return format!("{seconds}s ago");
    }
    let minutes = seconds / 60;
    if minutes < 60 {
        return format!("{minutes}m ago");
    }
    let hours = minutes / 60;
    if hours < 24 {
        return format!("{hours}h ago");
    }
    let days = hours / 24;
    format!("{days}d ago")
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

    if state.pagination.loading.is_pending() {
        if state.all_rows.is_empty() && state.pagination.num_scanned_files == 0 {
            return vec!["Loading sessions…".italic().dim()].into();
        }
        return vec!["Loading older sessions…".italic().dim()].into();
    }

    vec!["No sessions yet".italic().dim()].into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use codex_protocol::ThreadId;
    use codex_utils_absolute_path::test_support::PathBufExt;
    use codex_utils_absolute_path::test_support::test_path_buf;

    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;
    use crossterm::event::KeyModifiers;
    use insta::assert_snapshot;
    use pretty_assertions::assert_eq;
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::Mutex;

    fn page(
        rows: Vec<Row>,
        next_cursor: Option<&str>,
        num_scanned_files: usize,
        reached_scan_cap: bool,
    ) -> PickerPage {
        PickerPage {
            rows,
            next_cursor: next_cursor.map(|cursor| PageCursor::AppServer(cursor.to_string())),
            num_scanned_files,
            reached_scan_cap,
        }
    }

    fn page_only_loader(loader: impl Fn(PageLoadRequest) + Send + Sync + 'static) -> PickerLoader {
        Arc::new(move |request| {
            if let PickerLoadRequest::Page(request) = request {
                loader(request);
            }
        })
    }

    fn make_row(path: &str, ts: &str, preview: &str) -> Row {
        let timestamp = parse_timestamp_str(ts).expect("timestamp should parse");
        Row {
            path: Some(PathBuf::from(path)),
            preview: preview.to_string(),
            thread_id: None,
            thread_name: None,
            created_at: Some(timestamp),
            updated_at: Some(timestamp),
            cwd: None,
            git_branch: None,
        }
    }

    #[test]
    fn row_display_preview_prefers_thread_name() {
        let row = Row {
            path: Some(PathBuf::from("/tmp/a.jsonl")),
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
    fn local_picker_thread_list_params_include_cwd_filter() {
        let cwd_filter = picker_cwd_filter(
            Path::new("/tmp/project"),
            /*show_all*/ false,
            /*is_remote*/ false,
            /*remote_cwd_override*/ None,
        );
        let params = thread_list_params(
            Some(String::from("cursor-1")),
            cwd_filter.as_deref(),
            ProviderFilter::MatchDefault(String::from("openai")),
            ThreadSortKey::UpdatedAt,
            /*include_non_interactive*/ false,
        );

        assert_eq!(
            params.cwd,
            Some(ThreadListCwdFilter::One(String::from("/tmp/project")))
        );
    }

    #[test]
    fn row_search_matches_metadata_fields() {
        let thread_id =
            ThreadId::from_string("019dabc1-0ef5-7431-b81c-03037f51f62c").expect("thread id");
        let row = Row {
            path: Some(PathBuf::from("/tmp/a.jsonl")),
            preview: String::from("first message"),
            thread_id: Some(thread_id),
            thread_name: Some(String::from("My session")),
            created_at: None,
            updated_at: None,
            cwd: Some(PathBuf::from("/tmp/codex-session-picker")),
            git_branch: Some(String::from("fcoury/session-picker")),
        };

        assert!(row.matches_query("session-picker"));
        assert!(row.matches_query("fcoury"));
        assert!(row.matches_query(&thread_id.to_string()[..8]));
    }

    #[test]
    fn footer_prioritizes_active_sort_timestamp() {
        let updated = render_footer_lines(
            ThreadSortKey::UpdatedAt,
            "5h ago",
            "3h ago",
            Some("main"),
            Some("tmp/codex"),
            /*width*/ 80,
        );
        let created = render_footer_lines(
            ThreadSortKey::CreatedAt,
            "5h ago",
            "3h ago",
            Some("main"),
            Some("tmp/codex"),
            /*width*/ 80,
        );

        assert_eq!(updated.len(), 1);
        assert_eq!(created.len(), 1);
        assert!(updated[0].to_string().starts_with("  3h ago"));
        assert!(created[0].to_string().starts_with("  5h ago"));
        assert!(!updated[0].to_string().contains("created 5h ago"));
        assert!(!created[0].to_string().contains("updated 3h ago"));
        assert_metadata_order(&updated[0], "⌁ tmp/codex", " main");
        assert_metadata_order(&created[0], "⌁ tmp/codex", " main");
    }

    #[test]
    fn footer_marks_missing_branch() {
        let footer = render_footer_lines(
            ThreadSortKey::UpdatedAt,
            "5h ago",
            "3h ago",
            /*branch*/ None,
            Some("/tmp/codex"),
            /*width*/ 80,
        );

        assert_eq!(footer.len(), 1);
        let rendered = footer[0].to_string();
        assert!(rendered.contains("⌁ /tmp/codex"));
        assert!(rendered.contains(" no branch"));
        assert_metadata_order(&footer[0], "⌁ /tmp/codex", " no branch");
    }

    #[test]
    fn footer_branch_expands_when_line_has_room() {
        let branch = "etraut/animations-false-improvements";
        let footer = render_footer_lines(
            ThreadSortKey::UpdatedAt,
            "5h ago",
            "4h ago",
            Some(branch),
            Some("~/code/codex.etraut-animations-false-improvements/codex-rs"),
            /*width*/ 140,
        );

        assert_eq!(footer.len(), 1);
        assert!(footer[0].to_string().contains(branch));
    }

    #[test]
    fn footer_cwd_truncates_to_responsive_column() {
        let cwd = "~/code/codex.owner-extremely-long-worktree-name-that-needs-truncating/codex-rs";
        let branch = "owner/branch";
        let footer = render_footer_lines(
            ThreadSortKey::UpdatedAt,
            "5h ago",
            "4h ago",
            Some(branch),
            Some(cwd),
            /*width*/ 80,
        );

        assert_eq!(footer.len(), 1);
        let footer = footer[0].to_string();
        assert!(!footer.contains(cwd));
        assert!(footer.contains("⌁ ~/code/codex."));
        assert!(footer.contains("..."));
        assert!(footer.contains(" owner/branch"));
    }

    fn assert_metadata_order(line: &Line<'_>, first: &str, second: &str) {
        let rendered = line.to_string();
        let first_index = rendered.find(first).expect("first metadata item");
        let second_index = rendered.find(second).expect("second metadata item");
        assert!(first_index < second_index);
    }

    #[test]
    fn remote_thread_list_params_omit_provider_filter() {
        let params = thread_list_params(
            Some(String::from("cursor-1")),
            Some(Path::new("repo/on/server")),
            ProviderFilter::Any,
            ThreadSortKey::UpdatedAt,
            /*include_non_interactive*/ false,
        );

        assert_eq!(params.cursor, Some(String::from("cursor-1")));
        assert_eq!(params.model_providers, None);
        assert_eq!(
            params.source_kinds,
            Some(vec![ThreadSourceKind::Cli, ThreadSourceKind::VsCode])
        );
        assert_eq!(
            params.cwd,
            Some(ThreadListCwdFilter::One(String::from("repo/on/server")))
        );
    }

    #[test]
    fn remote_thread_list_params_can_include_non_interactive_sources() {
        let params = thread_list_params(
            Some(String::from("cursor-1")),
            /*cwd_filter*/ None,
            ProviderFilter::Any,
            ThreadSortKey::UpdatedAt,
            /*include_non_interactive*/ true,
        );

        assert_eq!(params.cursor, Some(String::from("cursor-1")));
        assert_eq!(params.model_providers, None);
        assert_eq!(params.source_kinds, None);
    }

    #[test]
    fn remote_picker_does_not_filter_rows_by_local_cwd() {
        let loader = page_only_loader(|_| {});
        let state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::Any,
            /*show_all*/ false,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );
        let row = Row {
            path: None,
            preview: String::from("remote session"),
            thread_id: Some(ThreadId::new()),
            thread_name: None,
            created_at: None,
            updated_at: None,
            cwd: Some(PathBuf::from("/srv/remote-project")),
            git_branch: None,
        };

        assert!(state.row_matches_filter(&row));
    }

    #[test]
    fn resume_table_snapshot() {
        use crate::custom_terminal::Terminal;
        use crate::test_backend::VT100Backend;

        let loader = page_only_loader(|_| {});
        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );

        let now = parse_timestamp_str("2026-04-28T16:30:00Z").expect("timestamp");
        let rows = vec![
            Row {
                path: Some(PathBuf::from("/tmp/a.jsonl")),
                preview: String::from("Fix resume picker timestamps"),
                thread_id: None,
                thread_name: None,
                created_at: Some(now - Duration::minutes(16)),
                updated_at: Some(now - Duration::seconds(42)),
                cwd: None,
                git_branch: None,
            },
            Row {
                path: Some(PathBuf::from("/tmp/b.jsonl")),
                preview: String::from("Investigate lazy pagination cap"),
                thread_id: None,
                thread_name: None,
                created_at: Some(now - Duration::hours(1)),
                updated_at: Some(now - Duration::minutes(35)),
                cwd: None,
                git_branch: None,
            },
            Row {
                path: Some(PathBuf::from("/tmp/c.jsonl")),
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
        state.relative_time_reference = Some(now);
        state.selected = 1;
        state.scroll_top = 0;
        state.update_viewport(/*rows*/ 12, /*width*/ 80);

        let width: u16 = 80;
        let height: u16 = 12;
        let backend = VT100Backend::new(width, height);
        let mut terminal = Terminal::with_options(backend).expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 0, width, height));

        {
            let mut frame = terminal.get_frame();
            let area = frame.area();
            render_list(&mut frame, area, &state);
        }
        terminal.flush().expect("flush");

        let snapshot = terminal.backend().to_string();
        assert_snapshot!("resume_picker_table", snapshot);
    }

    #[test]
    fn resume_search_error_snapshot() {
        use crate::custom_terminal::Terminal;
        use crate::test_backend::VT100Backend;

        let loader = page_only_loader(|_| {});
        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );
        state.inline_error = Some(String::from(
            "Failed to read session metadata from /tmp/missing.jsonl",
        ));

        let width: u16 = 80;
        let height: u16 = 1;
        let backend = VT100Backend::new(width, height);
        let mut terminal = Terminal::with_options(backend).expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 0, width, height));

        {
            let mut frame = terminal.get_frame();
            let line = search_line(&state, frame.area().width);
            frame.render_widget_ref(line, frame.area());
        }
        terminal.flush().expect("flush");

        let snapshot = terminal.backend().to_string();
        assert_snapshot!("resume_picker_search_error", snapshot);
    }

    #[test]
    fn hint_line_switches_esc_label_for_search_mode() {
        let loader = page_only_loader(|_| {});
        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );

        assert!(hint_line(&state).to_string().contains("esc to start new"));

        state.query = String::from("picker");

        assert!(
            hint_line(&state)
                .to_string()
                .contains("esc to clear search")
        );
    }

    #[test]
    fn expanded_session_snapshot() {
        use crate::custom_terminal::Terminal;
        use crate::test_backend::VT100Backend;

        let loader = page_only_loader(|_| {});
        let thread_id =
            ThreadId::from_string("019dabc1-0ef5-7431-b81c-03037f51f62c").expect("thread id");
        let row = Row {
            path: Some(PathBuf::from("/tmp/a.jsonl")),
            preview: String::from("Investigate picker expansion"),
            thread_id: Some(thread_id),
            thread_name: None,
            created_at: parse_timestamp_str("2026-04-28T16:30:00Z"),
            updated_at: parse_timestamp_str("2026-04-28T17:45:00Z"),
            cwd: Some(PathBuf::from("/tmp/codex")),
            git_branch: Some(String::from("fcoury/session-picker")),
        };
        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );
        state.all_rows = vec![row.clone()];
        state.filtered_rows = vec![row];
        state.relative_time_reference =
            Some(parse_timestamp_str("2026-04-28T18:00:00Z").expect("timestamp"));
        state.expanded_thread_id = Some(thread_id);
        state.transcript_previews.insert(
            thread_id,
            TranscriptPreviewState::Loaded(vec![
                TranscriptPreviewLine {
                    speaker: TranscriptPreviewSpeaker::User,
                    text: String::from("Show me the recent transcript"),
                },
                TranscriptPreviewLine {
                    speaker: TranscriptPreviewSpeaker::Assistant,
                    text: String::from("Here are the *last* few lines."),
                },
            ]),
        );

        let width: u16 = 90;
        let height: u16 = 6;
        let backend = VT100Backend::new(width, height);
        let mut terminal = Terminal::with_options(backend).expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 0, width, height));

        {
            let mut frame = terminal.get_frame();
            let area = frame.area();
            render_list(&mut frame, area, &state);
        }
        terminal.flush().expect("flush");

        assert_snapshot!(
            "resume_picker_expanded_session",
            terminal.backend().to_string()
        );
    }

    #[test]
    fn narrow_session_snapshot() {
        use crate::custom_terminal::Terminal;
        use crate::test_backend::VT100Backend;

        let loader = page_only_loader(|_| {});
        let row = Row {
            path: Some(PathBuf::from("/tmp/a.jsonl")),
            preview: String::from("Investigate picker expansion"),
            thread_id: Some(
                ThreadId::from_string("019dabc1-0ef5-7431-b81c-03037f51f62c").expect("thread id"),
            ),
            thread_name: None,
            created_at: parse_timestamp_str("2026-04-28T16:30:00Z"),
            updated_at: parse_timestamp_str("2026-04-28T17:45:00Z"),
            cwd: Some(PathBuf::from("/tmp/codex")),
            git_branch: Some(String::from("fcoury/session-picker")),
        };
        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );
        state.all_rows = vec![row.clone()];
        state.filtered_rows = vec![row];
        state.relative_time_reference =
            Some(parse_timestamp_str("2026-04-28T18:00:00Z").expect("timestamp"));

        let width: u16 = 58;
        let height: u16 = 6;
        let backend = VT100Backend::new(width, height);
        let mut terminal = Terminal::with_options(backend).expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 0, width, height));

        {
            let mut frame = terminal.get_frame();
            let area = frame.area();
            render_list(&mut frame, area, &state);
        }
        terminal.flush().expect("flush");

        assert_snapshot!(
            "resume_picker_narrow_session",
            terminal.backend().to_string()
        );
    }

    #[test]
    fn session_list_more_indicators_snapshot() {
        use crate::custom_terminal::Terminal;
        use crate::test_backend::VT100Backend;

        let loader = page_only_loader(|_| {});
        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );
        let now = parse_timestamp_str("2026-04-28T16:30:00Z").expect("timestamp");
        state.all_rows = (0..5)
            .map(|idx| Row {
                path: Some(PathBuf::from(format!("/tmp/{idx}.jsonl"))),
                preview: format!("item-{idx}"),
                thread_id: None,
                thread_name: None,
                created_at: Some(now - Duration::hours(idx)),
                updated_at: Some(now - Duration::minutes(idx * 5)),
                cwd: None,
                git_branch: None,
            })
            .collect();
        state.filtered_rows = state.all_rows.clone();
        state.relative_time_reference = Some(now);
        state.selected = 2;
        state.scroll_top = 1;
        state.update_viewport(/*rows*/ 6, /*width*/ 80);

        let width: u16 = 80;
        let height: u16 = 6;
        let backend = VT100Backend::new(width, height);
        let mut terminal = Terminal::with_options(backend).expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 0, width, height));

        {
            let mut frame = terminal.get_frame();
            let area = frame.area();
            render_list(&mut frame, area, &state);
        }
        terminal.flush().expect("flush");

        assert_snapshot!(
            "resume_picker_more_indicators",
            terminal.backend().to_string()
        );
    }

    #[test]
    fn pageless_scrolling_deduplicates_and_keeps_order() {
        let loader = page_only_loader(|_| {});
        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );

        state.reset_pagination();
        state.ingest_page(page(
            vec![
                make_row("/tmp/a.jsonl", "2025-01-03T00:00:00Z", "third"),
                make_row("/tmp/b.jsonl", "2025-01-02T00:00:00Z", "second"),
            ],
            Some("2025-01-02T00:00:00Z"),
            /*num_scanned_files*/ 2,
            /*reached_scan_cap*/ false,
        ));

        state.ingest_page(page(
            vec![
                make_row("/tmp/a.jsonl", "2025-01-03T00:00:00Z", "duplicate"),
                make_row("/tmp/c.jsonl", "2025-01-01T00:00:00Z", "first"),
            ],
            Some("2025-01-01T00:00:00Z"),
            /*num_scanned_files*/ 2,
            /*reached_scan_cap*/ false,
        ));

        state.ingest_page(page(
            vec![make_row("/tmp/d.jsonl", "2024-12-31T23:00:00Z", "very old")],
            /*next_cursor*/ None,
            /*num_scanned_files*/ 1,
            /*reached_scan_cap*/ false,
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
        let loader = page_only_loader(move |req: PageLoadRequest| {
            request_sink.lock().unwrap().push(req);
        });

        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );
        state.reset_pagination();
        state.ingest_page(page(
            vec![
                make_row("/tmp/a.jsonl", "2025-01-01T00:00:00Z", "one"),
                make_row("/tmp/b.jsonl", "2025-01-02T00:00:00Z", "two"),
            ],
            Some("2025-01-03T00:00:00Z"),
            /*num_scanned_files*/ 2,
            /*reached_scan_cap*/ false,
        ));

        assert!(recorded_requests.lock().unwrap().is_empty());
        state.ensure_minimum_rows_for_view(/*minimum_rows*/ 10);
        let guard = recorded_requests.lock().unwrap();
        assert_eq!(guard.len(), 1);
        assert!(guard[0].search_token.is_none());
    }

    #[tokio::test]
    async fn toggle_sort_key_reloads_with_new_sort() {
        let recorded_requests: Arc<Mutex<Vec<PageLoadRequest>>> = Arc::new(Mutex::new(Vec::new()));
        let request_sink = recorded_requests.clone();
        let loader = page_only_loader(move |req: PageLoadRequest| {
            request_sink.lock().unwrap().push(req);
        });

        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );

        state.start_initial_load();
        {
            let guard = recorded_requests.lock().unwrap();
            assert_eq!(guard.len(), 1);
            assert_eq!(guard[0].sort_key, ThreadSortKey::UpdatedAt);
        }

        state
            .handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE))
            .await
            .unwrap();

        let guard = recorded_requests.lock().unwrap();
        assert_eq!(guard.len(), 2);
        assert_eq!(guard[1].sort_key, ThreadSortKey::CreatedAt);
    }

    #[tokio::test]
    async fn page_navigation_uses_view_rows() {
        let loader = page_only_loader(|_| {});
        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );

        let mut items = Vec::new();
        for idx in 0..20 {
            let ts = format!("2025-01-{:02}T00:00:00Z", idx + 1);
            let preview = format!("item-{idx}");
            let path = format!("/tmp/item-{idx}.jsonl");
            items.push(make_row(&path, &ts, &preview));
        }

        state.reset_pagination();
        state.ingest_page(page(
            items, /*next_cursor*/ None, /*num_scanned_files*/ 20,
            /*reached_scan_cap*/ false,
        ));
        state.update_viewport(/*rows*/ 5, /*width*/ 80);

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
    async fn enter_on_row_without_resolvable_thread_id_shows_inline_error() {
        let loader = page_only_loader(|_| {});
        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );

        let row = Row {
            path: Some(PathBuf::from("/tmp/missing.jsonl")),
            preview: String::from("missing metadata"),
            thread_id: None,
            thread_name: None,
            created_at: None,
            updated_at: None,
            cwd: None,
            git_branch: None,
        };
        state.all_rows = vec![row.clone()];
        state.filtered_rows = vec![row];

        let selection = state
            .handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .await
            .expect("enter should not abort the picker");

        assert!(selection.is_none());
        assert_eq!(
            state.inline_error,
            Some(String::from(
                "Failed to read session metadata from /tmp/missing.jsonl"
            ))
        );
    }

    #[tokio::test]
    async fn enter_on_pathless_thread_uses_thread_id() {
        let loader = page_only_loader(|_| {});
        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );
        let thread_id = ThreadId::new();
        let row = Row {
            path: None,
            preview: String::from("pathless thread"),
            thread_id: Some(thread_id),
            thread_name: None,
            created_at: None,
            updated_at: None,
            cwd: None,
            git_branch: None,
        };
        state.all_rows = vec![row.clone()];
        state.filtered_rows = vec![row];

        let selection = state
            .handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .await
            .expect("enter should not abort the picker");

        match selection {
            Some(SessionSelection::Resume(SessionTarget {
                path: None,
                thread_id: selected_thread_id,
            })) => assert_eq!(selected_thread_id, thread_id),
            other => panic!("unexpected selection: {other:?}"),
        }
    }

    #[test]
    fn app_server_row_keeps_pathless_threads() {
        let thread_id = ThreadId::new();
        let thread = Thread {
            id: thread_id.to_string(),
            forked_from_id: None,
            preview: String::from("remote thread"),
            ephemeral: false,
            model_provider: String::from("openai"),
            created_at: 1,
            updated_at: 2,
            status: codex_app_server_protocol::ThreadStatus::Idle,
            path: None,
            cwd: test_path_buf("/tmp").abs(),
            cli_version: String::from("0.0.0"),
            source: codex_app_server_protocol::SessionSource::Cli,
            agent_nickname: None,
            agent_role: None,
            git_info: None,
            name: Some(String::from("Named thread")),
            turns: Vec::new(),
        };

        let row = row_from_app_server_thread(thread).expect("row should be preserved");

        assert_eq!(row.path, None);
        assert_eq!(row.thread_id, Some(thread_id));
        assert_eq!(row.thread_name, Some(String::from("Named thread")));
    }

    #[tokio::test]
    async fn moving_to_last_card_scrolls_when_cards_exceed_viewport() {
        let loader = page_only_loader(|_| {});
        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );

        let mut items = Vec::new();
        for idx in 0..3 {
            let ts = format!("2025-02-{:02}T00:00:00Z", idx + 1);
            let preview = format!("item-{idx}");
            let path = format!("/tmp/item-{idx}.jsonl");
            items.push(make_row(&path, &ts, &preview));
        }

        state.reset_pagination();
        state.ingest_page(page(
            items, /*next_cursor*/ None, /*num_scanned_files*/ 3,
            /*reached_scan_cap*/ false,
        ));
        state.update_viewport(/*rows*/ 5, /*width*/ 80);

        state
            .handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))
            .await
            .unwrap();
        assert_eq!(state.scroll_top, 1);

        state
            .handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))
            .await
            .unwrap();

        assert_eq!(state.selected, 2);
        assert_eq!(state.scroll_top, 2);
    }

    #[tokio::test]
    async fn up_from_bottom_keeps_viewport_stable_when_card_remains_visible() {
        let loader = page_only_loader(|_| {});
        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );

        let mut items = Vec::new();
        for idx in 0..10 {
            let ts = format!("2025-02-{:02}T00:00:00Z", idx + 1);
            let preview = format!("item-{idx}");
            let path = format!("/tmp/item-{idx}.jsonl");
            items.push(make_row(&path, &ts, &preview));
        }

        state.reset_pagination();
        state.ingest_page(page(
            items, /*next_cursor*/ None, /*num_scanned_files*/ 10,
            /*reached_scan_cap*/ false,
        ));
        state.update_viewport(/*rows*/ 5, /*width*/ 80);

        state.selected = state.filtered_rows.len().saturating_sub(1);
        state.ensure_selected_visible();

        let initial_top = state.scroll_top;
        assert_eq!(initial_top, state.filtered_rows.len().saturating_sub(1));

        state
            .handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE))
            .await
            .unwrap();

        assert_eq!(state.scroll_top, initial_top.saturating_sub(1));
        assert_eq!(state.selected, state.filtered_rows.len().saturating_sub(2));
    }

    #[tokio::test]
    async fn up_scrolls_only_after_crossing_top_edge() {
        let loader = page_only_loader(|_| {});
        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );

        let mut items = Vec::new();
        for idx in 0..10 {
            let ts = format!("2025-02-{:02}T00:00:00Z", idx + 1);
            let preview = format!("item-{idx}");
            let path = format!("/tmp/item-{idx}.jsonl");
            items.push(make_row(&path, &ts, &preview));
        }

        state.reset_pagination();
        state.ingest_page(page(
            items, /*next_cursor*/ None, /*num_scanned_files*/ 10,
            /*reached_scan_cap*/ false,
        ));
        state.update_viewport(/*rows*/ 5, /*width*/ 80);
        state.selected = 8;
        state.scroll_top = 8;

        state
            .handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE))
            .await
            .unwrap();

        assert_eq!(state.selected, 7);
        assert_eq!(state.scroll_top, 7);
    }

    #[test]
    fn list_reports_more_rows_above_and_below() {
        let loader = page_only_loader(|_| {});
        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );

        let mut items = Vec::new();
        for idx in 0..5 {
            let ts = format!("2025-02-{:02}T00:00:00Z", idx + 1);
            let preview = format!("item-{idx}");
            let path = format!("/tmp/item-{idx}.jsonl");
            items.push(make_row(&path, &ts, &preview));
        }

        state.reset_pagination();
        state.ingest_page(page(
            items, /*next_cursor*/ None, /*num_scanned_files*/ 5,
            /*reached_scan_cap*/ false,
        ));
        state.update_viewport(/*rows*/ 5, /*width*/ 80);

        assert!(!state.has_more_above());
        assert!(state.has_more_below(/*viewport_height*/ 5));

        state.scroll_top = 2;

        assert!(state.has_more_above());
        assert!(state.has_more_below(/*viewport_height*/ 5));
    }

    #[tokio::test]
    async fn set_query_loads_until_match_and_respects_scan_cap() {
        let recorded_requests: Arc<Mutex<Vec<PageLoadRequest>>> = Arc::new(Mutex::new(Vec::new()));
        let request_sink = recorded_requests.clone();
        let loader = page_only_loader(move |req: PageLoadRequest| {
            request_sink.lock().unwrap().push(req);
        });

        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );
        state.reset_pagination();
        state.ingest_page(page(
            vec![make_row(
                "/tmp/start.jsonl",
                "2025-01-01T00:00:00Z",
                "alpha",
            )],
            Some("2025-01-02T00:00:00Z"),
            /*num_scanned_files*/ 1,
            /*reached_scan_cap*/ false,
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
                    vec![make_row("/tmp/beta.jsonl", "2025-01-02T00:00:00Z", "beta")],
                    Some("2025-01-03T00:00:00Z"),
                    /*num_scanned_files*/ 5,
                    /*reached_scan_cap*/ false,
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
                    vec![make_row(
                        "/tmp/match.jsonl",
                        "2025-01-03T00:00:00Z",
                        "target log",
                    )],
                    Some("2025-01-04T00:00:00Z"),
                    /*num_scanned_files*/ 7,
                    /*reached_scan_cap*/ false,
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
                page: Ok(page(
                    Vec::new(),
                    /*next_cursor*/ None,
                    /*num_scanned_files*/ 0,
                    /*reached_scan_cap*/ false,
                )),
            })
            .await
            .unwrap();
        assert_eq!(recorded_requests.lock().unwrap().len(), 1);

        state
            .handle_background_event(BackgroundEvent::PageLoaded {
                request_token: active_request.request_token,
                search_token: active_request.search_token,
                page: Ok(page(
                    Vec::new(),
                    /*next_cursor*/ None,
                    /*num_scanned_files*/ 3,
                    /*reached_scan_cap*/ true,
                )),
            })
            .await
            .unwrap();

        assert!(state.filtered_rows.is_empty());
        assert!(!state.search_state.is_active());
        assert!(state.pagination.reached_scan_cap);
    }

    #[tokio::test]
    async fn esc_with_empty_query_starts_fresh() {
        let loader = page_only_loader(|_| {});
        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );

        let selection = state
            .handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
            .await
            .expect("handle key");

        assert!(matches!(selection, Some(SessionSelection::StartFresh)));
    }

    #[tokio::test]
    async fn esc_with_query_clears_search_and_preserves_selected_result() {
        let loader = page_only_loader(|_| {});
        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );
        state.reset_pagination();
        state.ingest_page(page(
            vec![
                make_row("/tmp/alpha.jsonl", "2025-01-03T00:00:00Z", "alpha"),
                make_row("/tmp/beta.jsonl", "2025-01-02T00:00:00Z", "beta"),
            ],
            /*next_cursor*/ None,
            /*num_scanned_files*/ 2,
            /*reached_scan_cap*/ false,
        ));
        state.set_query(String::from("beta"));

        let selection = state
            .handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
            .await
            .expect("handle key");

        assert!(selection.is_none());
        assert!(state.query.is_empty());
        assert_eq!(state.filtered_rows.len(), 2);
        assert_eq!(
            state.filtered_rows[state.selected].path.as_deref(),
            Some(Path::new("/tmp/beta.jsonl"))
        );
    }
}
