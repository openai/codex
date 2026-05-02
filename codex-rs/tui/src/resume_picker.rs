use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use crate::app_server_session::AppServerSession;
use crate::color::is_light;
use crate::legacy_core::config::Config;
use crate::legacy_core::config::edit::ConfigEditsBuilder;
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
use codex_config::types::SessionPickerViewMode;
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
use ratatui::style::Style;
use ratatui::style::Styled as _;
use ratatui::style::Stylize as _;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Clear;
use ratatui::widgets::Widget;
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
const FOOTER_COMPACT_BREAKPOINT: u16 = 120;
const FOOTER_WIDE_MIN_GAP: usize = 4;
const FOOTER_COMPACT_MIN_GAP: usize = 2;
const DENSE_PATH_AUTO_HIDE_BREAKPOINT: u16 = 120;

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
    cwd_filter: Option<PathBuf>,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SessionFilterMode {
    Cwd,
    All,
}

impl SessionFilterMode {
    fn from_show_all(show_all: bool, filter_cwd: Option<&Path>) -> Self {
        if show_all || filter_cwd.is_none() {
            Self::All
        } else {
            Self::Cwd
        }
    }

    fn toggle(self, filter_cwd: Option<&Path>) -> Self {
        match self {
            Self::Cwd => Self::All,
            Self::All if filter_cwd.is_some() => Self::Cwd,
            Self::All => Self::All,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ToolbarControl {
    Filter,
    Sort,
}

impl ToolbarControl {
    fn previous(self) -> Self {
        match self {
            Self::Filter => Self::Sort,
            Self::Sort => Self::Filter,
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Filter => Self::Sort,
            Self::Sort => Self::Filter,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SessionListDensity {
    Comfortable,
    Dense,
}

impl SessionListDensity {
    fn toggle(self) -> Self {
        match self {
            Self::Comfortable => Self::Dense,
            Self::Dense => Self::Comfortable,
        }
    }
}

impl From<SessionPickerViewMode> for SessionListDensity {
    fn from(mode: SessionPickerViewMode) -> Self {
        match mode {
            SessionPickerViewMode::Comfortable => Self::Comfortable,
            SessionPickerViewMode::Dense => Self::Dense,
        }
    }
}

impl From<SessionListDensity> for SessionPickerViewMode {
    fn from(density: SessionListDensity) -> Self {
        match density {
            SessionListDensity::Comfortable => Self::Comfortable,
            SessionListDensity::Dense => Self::Dense,
        }
    }
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

#[derive(Clone)]
struct SessionPickerViewPersistence {
    codex_home: PathBuf,
    active_profile: Option<String>,
}

struct SessionPickerRunOptions {
    show_all: bool,
    filter_cwd: Option<PathBuf>,
    action: SessionPickerAction,
    provider_filter: ProviderFilter,
    initial_density: SessionListDensity,
    view_persistence: Option<SessionPickerViewPersistence>,
}

/// Interactive session picker that lists app-server threads with simple search,
/// lazy transcript previews, and pagination.
///
/// Sessions render as compact multi-line records with stable metadata first and
/// the conversation preview last. Users can focus Sort/Filter toolbar controls
/// with Tab, change the focused control with the arrow keys, and expand the
/// selected session with Space to load recent transcript context on demand.
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
        /*show_all*/ false,
        is_remote,
        app_server.remote_cwd_override(),
    );
    let provider_filter = picker_provider_filter(config, is_remote);
    let options = SessionPickerRunOptions {
        show_all,
        filter_cwd: cwd_filter,
        action: SessionPickerAction::Resume,
        provider_filter,
        initial_density: SessionListDensity::from(config.tui_session_picker_view),
        view_persistence: Some(SessionPickerViewPersistence {
            codex_home: config.codex_home.to_path_buf(),
            active_profile: config.active_profile.clone(),
        }),
    };
    run_session_picker_with_loader(
        tui,
        options,
        spawn_app_server_page_loader(app_server, include_non_interactive, bg_tx),
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
        /*show_all*/ false,
        is_remote,
        app_server.remote_cwd_override(),
    );
    let provider_filter = picker_provider_filter(config, is_remote);
    let options = SessionPickerRunOptions {
        show_all,
        filter_cwd: cwd_filter,
        action: SessionPickerAction::Fork,
        provider_filter,
        initial_density: SessionListDensity::from(config.tui_session_picker_view),
        view_persistence: Some(SessionPickerViewPersistence {
            codex_home: config.codex_home.to_path_buf(),
            active_profile: config.active_profile.clone(),
        }),
    };
    run_session_picker_with_loader(
        tui,
        options,
        spawn_app_server_page_loader(app_server, /*include_non_interactive*/ false, bg_tx),
        bg_rx,
    )
    .await
}

async fn run_session_picker_with_loader(
    tui: &mut Tui,
    options: SessionPickerRunOptions,
    picker_loader: PickerLoader,
    bg_rx: mpsc::UnboundedReceiver<BackgroundEvent>,
) -> Result<SessionSelection> {
    let alt = AltScreenGuard::enter(tui);
    let mut state = PickerState::new(
        alt.tui.frame_requester(),
        picker_loader,
        options.provider_filter,
        options.show_all,
        options.filter_cwd,
        options.action,
    );
    state.density = options.initial_density;
    state.view_persistence = options.view_persistence;
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

fn picker_provider_filter(config: &Config, is_remote: bool) -> ProviderFilter {
    if is_remote {
        ProviderFilter::Any
    } else {
        ProviderFilter::MatchDefault(config.model_provider_id.to_string())
    }
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
                        request.cwd_filter.as_deref(),
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
    filter_mode: SessionFilterMode,
    filter_cwd: Option<PathBuf>,
    toolbar_focus: ToolbarControl,
    density: SessionListDensity,
    view_persistence: Option<SessionPickerViewPersistence>,
    dense_path_column_override: Option<bool>,
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
            filter_mode: SessionFilterMode::from_show_all(show_all, filter_cwd.as_deref()),
            filter_cwd,
            toolbar_focus: ToolbarControl::Filter,
            density: SessionListDensity::Comfortable,
            view_persistence: None,
            dense_path_column_override: None,
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
                code: KeyCode::Char('t'),
                modifiers,
                ..
            } if modifiers.contains(KeyModifiers::CONTROL) => {
                self.toggle_density().await;
            }
            KeyEvent {
                code: KeyCode::Char('\u{0014}'),
                modifiers: KeyModifiers::NONE,
                ..
            } /* ^T */ => {
                self.toggle_density().await;
            }
            KeyEvent {
                code: KeyCode::Char('o'),
                modifiers,
                ..
            } if modifiers.contains(KeyModifiers::CONTROL) => {
                self.toggle_dense_path_column();
            }
            KeyEvent {
                code: KeyCode::Char('\u{000f}'),
                modifiers: KeyModifiers::NONE,
                ..
            } /* ^O */ => {
                self.toggle_dense_path_column();
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
                self.focus_next_toolbar_control();
                self.request_frame();
            }
            KeyEvent {
                code: KeyCode::BackTab,
                ..
            } => {
                self.focus_previous_toolbar_control();
                self.request_frame();
            }
            KeyEvent {
                code: KeyCode::Left,
                ..
            }
            | KeyEvent {
                code: KeyCode::Right,
                ..
            } => {
                self.change_focused_toolbar_value();
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
            cwd_filter: self.active_cwd_filter(),
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
        if self.filter_mode == SessionFilterMode::All {
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
            cwd_filter: self.active_cwd_filter(),
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

    fn toggle_filter_mode(&mut self) {
        let next_filter_mode = self.filter_mode.toggle(self.filter_cwd.as_deref());
        if self.filter_mode == next_filter_mode {
            return;
        }
        self.filter_mode = next_filter_mode;
        self.start_initial_load();
    }

    fn active_cwd_filter(&self) -> Option<PathBuf> {
        match self.filter_mode {
            SessionFilterMode::Cwd => self.filter_cwd.clone(),
            SessionFilterMode::All => None,
        }
    }

    fn focus_previous_toolbar_control(&mut self) {
        self.toolbar_focus = self.toolbar_focus.previous();
    }

    fn focus_next_toolbar_control(&mut self) {
        self.toolbar_focus = self.toolbar_focus.next();
    }

    fn change_focused_toolbar_value(&mut self) {
        match self.toolbar_focus {
            ToolbarControl::Sort => self.toggle_sort_key(),
            ToolbarControl::Filter => self.toggle_filter_mode(),
        }
    }

    async fn toggle_density(&mut self) {
        self.density = self.density.toggle();
        self.ensure_selected_visible();
        if let Err(err) = self.persist_density().await {
            warn!(error = %err, "failed to persist session picker view mode");
            self.inline_error = Some(format!("Failed to save view mode: {err}"));
        }
        self.request_frame();
    }

    async fn persist_density(&self) -> Result<()> {
        let Some(persistence) = &self.view_persistence else {
            return Ok(());
        };

        ConfigEditsBuilder::new(&persistence.codex_home)
            .with_profile(persistence.active_profile.as_deref())
            .set_session_picker_view(SessionPickerViewMode::from(self.density))
            .apply()
            .await
            .map_err(|err| color_eyre::eyre::eyre!("failed to write config.toml: {err}"))?;

        Ok(())
    }

    fn toggle_dense_path_column(&mut self) {
        if self.density != SessionListDensity::Dense || self.filter_mode != SessionFilterMode::All {
            return;
        }
        let width = self.view_width.unwrap_or(u16::MAX);
        let automatic = width >= DENSE_PATH_AUTO_HIDE_BREAKPOINT;
        let next = !self.dense_path_column_visible(width);
        self.dense_path_column_override = (next != automatic).then_some(next);
        self.ensure_selected_visible();
        self.request_frame();
    }

    fn dense_path_column_visible(&self, width: u16) -> bool {
        if self.density != SessionListDensity::Dense || self.filter_mode != SessionFilterMode::All {
            return false;
        }
        self.dense_path_column_override
            .unwrap_or(width >= DENSE_PATH_AUTO_HIDE_BREAKPOINT)
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
            + self.row_separator_height() * end_inclusive.saturating_sub(start)
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
            let separator_height = usize::from(offset > 0) * self.row_separator_height();
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

    fn row_separator_height(&self) -> usize {
        match self.density {
            SessionListDensity::Comfortable => 1,
            SessionListDensity::Dense => 0,
        }
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
        frame.render_widget_ref(hint_line(state, hint.width), hint);
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
    let mut toolbar = toolbar_line(state, /*compact*/ false);
    if toolbar.width() as u16 > width.saturating_sub(2) {
        toolbar = toolbar_line(state, /*compact*/ true);
    }
    let search_width = UnicodeWidthStr::width(search.content.as_ref());
    let toolbar_width = toolbar.width();
    let spacer_width = width
        .saturating_sub((search_width + toolbar_width) as u16)
        .max(2) as usize;
    let available_search_width = width
        .saturating_sub(toolbar_width as u16)
        .saturating_sub(spacer_width as u16) as usize;
    let search = if search_width > available_search_width {
        let truncated = truncate_text(search.content.as_ref(), available_search_width);
        if state.query.is_empty() {
            truncated.dim()
        } else {
            truncated.into()
        }
    } else {
        search
    };

    let mut spans = vec![search, " ".repeat(spacer_width).into()];
    spans.extend(toolbar.spans);
    spans.into()
}

fn toolbar_line(state: &PickerState, compact: bool) -> Line<'static> {
    let mut spans = Vec::new();
    spans.extend(filter_control_spans(state, compact));
    spans.push("   ".dim());
    spans.extend(sort_control_spans(state, compact));
    spans.into()
}

fn sort_control_spans(state: &PickerState, compact: bool) -> Vec<Span<'static>> {
    let sort_focused = state.toolbar_focus == ToolbarControl::Sort;
    if compact {
        return vec![
            "Sort:".dim(),
            toolbar_value(
                sort_key_label(state.sort_key),
                /*active*/ true,
                sort_focused,
            ),
        ];
    }
    vec![
        "Sort: ".dim(),
        toolbar_value(
            sort_key_label(ThreadSortKey::UpdatedAt),
            state.sort_key == ThreadSortKey::UpdatedAt,
            sort_focused,
        ),
        toolbar_value(
            sort_key_label(ThreadSortKey::CreatedAt),
            state.sort_key == ThreadSortKey::CreatedAt,
            sort_focused,
        ),
    ]
}

fn filter_control_spans(state: &PickerState, compact: bool) -> Vec<Span<'static>> {
    let filter_focused = state.toolbar_focus == ToolbarControl::Filter;
    if compact || state.filter_cwd.is_none() {
        return vec![
            "Filter:".dim(),
            toolbar_value(
                filter_mode_label(state.filter_mode),
                /*active*/ true,
                filter_focused,
            ),
        ];
    }
    vec![
        "Filter: ".dim(),
        toolbar_value(
            filter_mode_label(SessionFilterMode::Cwd),
            state.filter_mode == SessionFilterMode::Cwd,
            filter_focused,
        ),
        toolbar_value(
            filter_mode_label(SessionFilterMode::All),
            state.filter_mode == SessionFilterMode::All,
            filter_focused,
        ),
    ]
}

fn toolbar_value(label: &'static str, active: bool, focused: bool) -> Span<'static> {
    if active {
        let value = format!("[{label}]");
        if focused {
            value.magenta()
        } else {
            value.into()
        }
    } else {
        format!(" {label} ").dim()
    }
}

fn filter_mode_label(filter_mode: SessionFilterMode) -> &'static str {
    match filter_mode {
        SessionFilterMode::Cwd => "Cwd",
        SessionFilterMode::All => "All",
    }
}

struct PickerFooterHint {
    key: &'static str,
    wide_label: String,
    compact_label: String,
    priority: u8,
}

fn hint_line(state: &PickerState, width: u16) -> Line<'static> {
    let action_label = state.action.action_label();
    let esc_label = if state.query.is_empty() {
        "start new"
    } else {
        "clear search"
    };
    let density_label = match state.density {
        SessionListDensity::Comfortable => "dense view",
        SessionListDensity::Dense => "comfortable view",
    };
    let density_compact_label = match state.density {
        SessionListDensity::Comfortable => "dense",
        SessionListDensity::Dense => "comfy",
    };
    let hints = vec![
        PickerFooterHint {
            key: "enter",
            wide_label: action_label.to_string(),
            compact_label: action_label.to_string(),
            priority: 0,
        },
        PickerFooterHint {
            key: "esc",
            wide_label: esc_label.to_string(),
            compact_label: if state.query.is_empty() {
                String::from("new")
            } else {
                String::from("clear")
            },
            priority: 1,
        },
        PickerFooterHint {
            key: "ctrl+c",
            wide_label: String::from("quit"),
            compact_label: String::from("quit"),
            priority: 2,
        },
        PickerFooterHint {
            key: "tab",
            wide_label: String::from("focus sort/filter"),
            compact_label: String::from("focus"),
            priority: 7,
        },
        PickerFooterHint {
            key: "←/→",
            wide_label: String::from("change option"),
            compact_label: String::from("option"),
            priority: 8,
        },
        PickerFooterHint {
            key: "ctrl+t",
            wide_label: density_label.to_string(),
            compact_label: density_compact_label.to_string(),
            priority: 3,
        },
        PickerFooterHint {
            key: "ctrl+o",
            wide_label: if state.dense_path_column_visible(width) {
                String::from("hide path")
            } else {
                String::from("show path")
            },
            compact_label: String::from("path"),
            priority: 4,
        },
        PickerFooterHint {
            key: "space",
            wide_label: String::from("expand"),
            compact_label: String::from("exp"),
            priority: 5,
        },
        PickerFooterHint {
            key: "↑/↓",
            wide_label: String::from("browse"),
            compact_label: String::from("browse"),
            priority: 6,
        },
    ]
    .into_iter()
    .filter(|hint| hint.key != "ctrl+o" || state.dense_path_toggle_available())
    .collect::<Vec<_>>();
    if width >= FOOTER_COMPACT_BREAKPOINT
        && let Some(line) = fit_footer_hints(&hints, FooterHintLabelMode::Wide, width)
    {
        return line;
    }
    if let Some(line) = fit_footer_hints(&hints, FooterHintLabelMode::Compact, width) {
        return line;
    }
    if let Some(line) = fit_footer_hints(&hints, FooterHintLabelMode::KeyOnly, width) {
        return line;
    }

    let mut retained = (0..hints.len()).collect::<Vec<_>>();
    retained.sort_by_key(|idx| hints[*idx].priority);
    for retain_count in (1..=retained.len()).rev() {
        let mut candidate_indices = retained[..retain_count].to_vec();
        candidate_indices.sort_unstable();
        let candidate = candidate_indices
            .iter()
            .map(|idx| &hints[*idx])
            .collect::<Vec<_>>();
        if let Some(line) = fit_footer_hint_refs(&candidate, FooterHintLabelMode::KeyOnly, width) {
            return line;
        }
    }
    Line::default()
}

impl PickerState {
    fn dense_path_toggle_available(&self) -> bool {
        self.density == SessionListDensity::Dense && self.filter_mode == SessionFilterMode::All
    }
}

#[derive(Clone, Copy)]
enum FooterHintLabelMode {
    Wide,
    Compact,
    KeyOnly,
}

fn fit_footer_hints(
    hints: &[PickerFooterHint],
    mode: FooterHintLabelMode,
    width: u16,
) -> Option<Line<'static>> {
    let hint_refs = hints.iter().collect::<Vec<_>>();
    fit_footer_hint_refs(&hint_refs, mode, width)
}

fn fit_footer_hint_refs(
    hints: &[&PickerFooterHint],
    mode: FooterHintLabelMode,
    width: u16,
) -> Option<Line<'static>> {
    let min_gap = match mode {
        FooterHintLabelMode::Wide => FOOTER_WIDE_MIN_GAP,
        FooterHintLabelMode::Compact | FooterHintLabelMode::KeyOnly => FOOTER_COMPACT_MIN_GAP,
    };
    let total_hint_width = footer_hints_width(hints, mode, /*gap_width*/ 0);
    let gap_count = hints.len().saturating_sub(1);
    let gap_width = if gap_count == 0 {
        0
    } else {
        let remaining_width = width.saturating_sub(total_hint_width as u16) as usize;
        if remaining_width >= min_gap * gap_count {
            remaining_width / gap_count
        } else {
            min_gap
        }
    };
    if footer_hints_width(hints, mode, gap_width) > width as usize {
        return None;
    }

    let mut spans = Vec::new();
    for (idx, hint) in hints.iter().enumerate() {
        if idx > 0 {
            spans.push(" ".repeat(gap_width).dim());
        }
        spans.push(hint.key.into());
        let label = match mode {
            FooterHintLabelMode::Wide => Some(hint.wide_label.as_str()),
            FooterHintLabelMode::Compact => Some(hint.compact_label.as_str()),
            FooterHintLabelMode::KeyOnly => None,
        };
        if let Some(label) = label {
            spans.push(" ".dim());
            spans.push(label.to_string().dim());
        }
    }
    Some(spans.into())
}

fn footer_hints_width(
    hints: &[&PickerFooterHint],
    mode: FooterHintLabelMode,
    gap_width: usize,
) -> usize {
    hints
        .iter()
        .enumerate()
        .map(|(idx, hint)| {
            let label_width = match mode {
                FooterHintLabelMode::Wide => 1 + UnicodeWidthStr::width(hint.wide_label.as_str()),
                FooterHintLabelMode::Compact => {
                    1 + UnicodeWidthStr::width(hint.compact_label.as_str())
                }
                FooterHintLabelMode::KeyOnly => 0,
            };
            let hint_width = UnicodeWidthStr::width(hint.key) + label_width;
            if idx == 0 {
                hint_width
            } else {
                hint_width + gap_width
            }
        })
        .sum()
}

fn render_list(frame: &mut crate::custom_terminal::Frame, area: Rect, state: &PickerState) {
    if area.height == 0 {
        return;
    }
    Clear.render(area, frame.buffer);

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
        if state.density == SessionListDensity::Comfortable
            && y < content_area.y.saturating_add(content_area.height)
            && start + idx + 1 < rows.len()
        {
            frame.render_widget_ref(
                session_separator_line(area.width),
                Rect::new(area.x, y, area.width, /*height*/ 1),
            );
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

fn session_separator_line(width: u16) -> Line<'static> {
    let line_width = width.saturating_sub(2) as usize;
    vec!["  ".into(), "─".repeat(line_width).dark_gray()].into()
}

fn render_session_lines(
    row: &Row,
    state: &PickerState,
    is_selected: bool,
    is_expanded: bool,
    width: u16,
) -> Vec<Line<'static>> {
    match state.density {
        SessionListDensity::Comfortable => {
            render_comfortable_session_lines(row, state, is_selected, is_expanded, width)
        }
        SessionListDensity::Dense => {
            render_dense_session_lines(row, state, is_selected, is_expanded, width)
        }
    }
}

fn render_comfortable_session_lines(
    row: &Row,
    state: &PickerState,
    is_selected: bool,
    is_expanded: bool,
    width: u16,
) -> Vec<Line<'static>> {
    let marker = selection_marker(is_selected, is_expanded);
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
        selected_session_title_span(title)
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
        state.filter_mode == SessionFilterMode::All,
        width,
    ));
    lines
}

fn render_dense_session_lines(
    row: &Row,
    state: &PickerState,
    is_selected: bool,
    is_expanded: bool,
    width: u16,
) -> Vec<Line<'static>> {
    let marker = selection_marker(is_selected, is_expanded);
    let reference = state.relative_time_reference.unwrap_or_else(Utc::now);
    let created = format_relative_time(reference, row.created_at);
    let updated = format_relative_time(reference, row.updated_at.or(row.created_at));
    let date = match state.sort_key {
        ThreadSortKey::CreatedAt => created,
        ThreadSortKey::UpdatedAt => updated,
    };
    let cwd = row
        .cwd
        .as_ref()
        .map(|path| format_directory_display(path, /*max_width*/ None));
    let show_cwd = state.dense_path_column_visible(width);
    let mut lines = vec![dense_summary_line(DenseSummaryInput {
        marker,
        date: &date,
        branch: row.git_branch.as_deref(),
        cwd: cwd.as_deref(),
        show_cwd,
        preserve_cwd: state.dense_path_column_override == Some(true) && show_cwd,
        title: row.display_preview(),
        is_selected,
        width,
    })];
    if is_expanded {
        lines.extend(render_transcript_preview_lines(row, state, width));
    }
    lines
}

struct DenseSummaryInput<'a> {
    marker: Span<'static>,
    date: &'a str,
    branch: Option<&'a str>,
    cwd: Option<&'a str>,
    show_cwd: bool,
    preserve_cwd: bool,
    title: &'a str,
    is_selected: bool,
    width: u16,
}

fn dense_summary_line(input: DenseSummaryInput<'_>) -> Line<'static> {
    let branch = Some(format!(
        "{SESSION_META_BRANCH_ICON} {}",
        input.branch.unwrap_or("no branch")
    ));
    let cwd = input
        .show_cwd
        .then(|| format!("{SESSION_META_CWD_ICON} {}", input.cwd.unwrap_or("no cwd")));

    let marker_width = input.marker.width();
    let available = (input.width as usize).saturating_sub(marker_width);
    let columns = dense_columns(available, input.show_cwd, input.preserve_cwd);
    let branch = columns.branch_width.and_then(|width| {
        branch
            .as_deref()
            .map(|branch| dense_column_text(branch, width))
    });
    let cwd = columns
        .cwd_width
        .and_then(|width| cwd.as_deref().map(|cwd| dense_column_text(cwd, width)));
    let title_width = columns.title_width;
    let title = truncate_text(input.title, title_width);
    let title = if input.is_selected {
        selected_session_title_span(title)
    } else {
        title.into()
    };

    let spans = vec![
        input.marker,
        dense_column_text(input.date, columns.date_width).dim(),
    ];
    let mut spans = spans;
    if let Some(branch) = branch {
        spans.push(branch.dim());
    }
    if let Some(cwd) = cwd {
        spans.push(cwd.dim());
    }
    spans.push(title);
    let mut line = Line::from(spans);
    if input.is_selected {
        let padding = (input.width as usize).saturating_sub(line.width());
        if padding > 0 {
            line.spans
                .push(" ".repeat(padding).set_style(selected_session_style()));
        }
        line = line.style(selected_session_style());
    }
    line
}

struct DenseColumns {
    date_width: usize,
    branch_width: Option<usize>,
    cwd_width: Option<usize>,
    title_width: usize,
}

fn dense_columns(width: usize, show_cwd: bool, preserve_cwd: bool) -> DenseColumns {
    let date_width = SESSION_META_DATE_WIDTH;
    let minimum_title_width = 12;
    let mut branch_width = Some((width / 4).clamp(14, 32));
    let mut cwd_width = show_cwd.then(|| (width / 3).clamp(18, 48));

    loop {
        let metadata_width = date_width + branch_width.unwrap_or(0) + cwd_width.unwrap_or(0);
        if width.saturating_sub(metadata_width) >= minimum_title_width {
            break;
        }
        if preserve_cwd && branch_width.is_some() {
            branch_width = None;
            continue;
        }
        if cwd_width.is_some() {
            cwd_width = None;
            continue;
        }
        if branch_width.is_some() {
            branch_width = None;
            continue;
        }
        break;
    }

    let metadata_width = date_width + branch_width.unwrap_or(0) + cwd_width.unwrap_or(0);
    DenseColumns {
        date_width,
        branch_width,
        cwd_width,
        title_width: width.saturating_sub(metadata_width),
    }
}

fn dense_column_text(text: &str, width: usize) -> String {
    let text = truncate_text(text, width.saturating_sub(1));
    let padding = width.saturating_sub(UnicodeWidthStr::width(text.as_str()));
    format!("{text}{}", " ".repeat(padding))
}

fn selection_marker(is_selected: bool, is_expanded: bool) -> Span<'static> {
    match (is_selected, is_expanded) {
        (true, true) => "⌄ ".set_style(selected_session_style().bold()),
        (true, false) => "❯ ".set_style(selected_session_style().bold()),
        (false, _) => "  ".into(),
    }
}

fn selected_session_style() -> Style {
    if default_bg().is_some_and(is_light) {
        Style::default().fg(Color::Magenta)
    } else {
        Style::default().fg(Color::Yellow)
    }
}

fn selected_session_title_span(title: String) -> Span<'static> {
    title.set_style(selected_session_style())
}

fn render_footer_lines(
    sort_key: ThreadSortKey,
    created: &str,
    updated: &str,
    branch: Option<&str>,
    cwd: Option<&str>,
    show_cwd: bool,
    width: u16,
) -> Vec<Line<'static>> {
    let date = match sort_key {
        ThreadSortKey::CreatedAt => created,
        ThreadSortKey::UpdatedAt => updated,
    };
    let mut parts = vec![FooterPart::Date(date.to_string())];
    if show_cwd {
        parts.push(FooterPart::Cwd(cwd.map(str::to_string)));
    }
    parts.push(FooterPart::Branch(branch.map(str::to_string)));
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
    if seconds == 0 {
        return "now".to_string();
    }
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
    use codex_config::CONFIG_TOML_FILE;
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
    use tempfile::tempdir;

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
    fn relative_time_formats_zero_seconds_as_now() {
        let reference = DateTime::parse_from_rfc3339("2026-05-02T12:00:00Z")
            .expect("valid timestamp")
            .with_timezone(&Utc);

        assert_eq!(format_relative_time(reference, Some(reference)), "now");
        assert_eq!(
            format_relative_time(reference, Some(reference - Duration::seconds(1))),
            "1s ago"
        );
    }

    #[test]
    fn footer_prioritizes_active_sort_timestamp() {
        let updated = render_footer_lines(
            ThreadSortKey::UpdatedAt,
            "5h ago",
            "3h ago",
            Some("main"),
            Some("tmp/codex"),
            /*show_cwd*/ true,
            /*width*/ 80,
        );
        let created = render_footer_lines(
            ThreadSortKey::CreatedAt,
            "5h ago",
            "3h ago",
            Some("main"),
            Some("tmp/codex"),
            /*show_cwd*/ true,
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
            /*show_cwd*/ true,
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
            /*show_cwd*/ true,
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
            /*show_cwd*/ true,
            /*width*/ 80,
        );

        assert_eq!(footer.len(), 1);
        let footer = footer[0].to_string();
        assert!(!footer.contains(cwd));
        assert!(footer.contains("⌁ ~/code/codex."));
        assert!(footer.contains("..."));
        assert!(footer.contains(" owner/branch"));
    }

    #[test]
    fn footer_omits_cwd_when_hidden() {
        let footer = render_footer_lines(
            ThreadSortKey::UpdatedAt,
            "5h ago",
            "4h ago",
            Some("owner/branch"),
            Some("~/code/codex.owner-worktree/codex-rs"),
            /*show_cwd*/ false,
            /*width*/ 80,
        );

        assert_eq!(footer.len(), 1);
        let footer = footer[0].to_string();
        assert!(footer.contains("4h ago"));
        assert!(footer.contains(" owner/branch"));
        assert!(!footer.contains("⌁"));
        assert!(!footer.contains("~/code"));
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

        assert!(
            hint_line(&state, /*width*/ 160)
                .to_string()
                .contains("esc start new")
        );

        state.query = String::from("picker");

        assert!(
            hint_line(&state, /*width*/ 160)
                .to_string()
                .contains("esc clear search")
        );
    }

    #[test]
    fn hint_line_switches_density_label() {
        let loader = page_only_loader(|_| {});
        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );

        assert!(
            hint_line(&state, /*width*/ 160)
                .to_string()
                .contains("ctrl+t dense view")
        );

        state.density = SessionListDensity::Dense;

        assert!(
            hint_line(&state, /*width*/ 220)
                .to_string()
                .contains("ctrl+t comfortable view")
        );
    }

    #[test]
    fn hint_line_compacts_on_narrow_width() {
        let loader = page_only_loader(|_| {});
        let state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );

        let rendered = hint_line(&state, /*width*/ 96).to_string();

        assert!(rendered.contains("esc new"));
        assert!(rendered.contains("tab focus"));
        assert!(rendered.contains("←/→ option"));
        assert!(rendered.contains("ctrl+t dense"));
        assert!(!rendered.contains("focus sort/filter"));
    }

    #[test]
    fn hint_line_snapshot_uses_distributed_wide_footer() {
        let loader = page_only_loader(|_| {});
        let state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );

        assert_snapshot!(
            "resume_picker_footer_wide",
            hint_line(&state, /*width*/ 160).to_string()
        );
    }

    #[test]
    fn hint_line_snapshot_uses_compact_footer() {
        let loader = page_only_loader(|_| {});
        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );
        state.query = String::from("picker");
        state.density = SessionListDensity::Dense;

        assert_snapshot!(
            "resume_picker_footer_compact",
            hint_line(&state, /*width*/ 96).to_string()
        );
    }

    #[test]
    fn hint_line_prioritizes_keybinds_when_very_narrow() {
        let loader = page_only_loader(|_| {});
        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );
        state.density = SessionListDensity::Dense;

        let width = 38;
        let line = hint_line(&state, width);
        let rendered = line.to_string();

        assert!(line.width() <= width as usize);
        assert!(rendered.contains("enter"));
        assert!(rendered.contains("esc"));
        assert!(rendered.contains("ctrl+c"));
        assert!(rendered.contains("ctrl+t"));
        assert!(rendered.contains("ctrl+o"));
        assert!(!rendered.contains("space"));
        assert!(!rendered.contains("↑/↓"));
    }

    #[tokio::test]
    async fn ctrl_t_toggles_density_without_typing_into_search() {
        let loader = page_only_loader(|_| {});
        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );
        state.query = String::from("pick");

        state
            .handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL))
            .await
            .unwrap();

        assert_eq!(state.density, SessionListDensity::Dense);
        assert_eq!(state.query, "pick");
    }

    #[tokio::test]
    async fn ctrl_t_persists_density_preference() {
        let tmp = tempdir().expect("tmpdir");
        let loader = page_only_loader(|_| {});
        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );
        state.view_persistence = Some(SessionPickerViewPersistence {
            codex_home: tmp.path().to_path_buf(),
            active_profile: None,
        });

        state
            .handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL))
            .await
            .unwrap();

        assert_eq!(state.density, SessionListDensity::Dense);
        let contents =
            std::fs::read_to_string(tmp.path().join(CONFIG_TOML_FILE)).expect("read config");
        assert_eq!(
            contents,
            r#"[tui]
session_picker_view = "dense"
"#
        );
    }

    #[tokio::test]
    async fn ctrl_t_persists_density_preference_for_active_profile() {
        let tmp = tempdir().expect("tmpdir");
        let loader = page_only_loader(|_| {});
        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );
        state.view_persistence = Some(SessionPickerViewPersistence {
            codex_home: tmp.path().to_path_buf(),
            active_profile: Some(String::from("work")),
        });

        state
            .handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL))
            .await
            .unwrap();

        assert_eq!(state.density, SessionListDensity::Dense);
        let contents =
            std::fs::read_to_string(tmp.path().join(CONFIG_TOML_FILE)).expect("read config");
        assert_eq!(
            contents,
            r#"[profiles.work.tui]
session_picker_view = "dense"
"#
        );
    }

    #[tokio::test]
    async fn ctrl_t_keeps_toggled_density_when_persistence_fails() {
        let tmp = tempdir().expect("tmpdir");
        let codex_home_file = tmp.path().join("codex-home-file");
        std::fs::write(&codex_home_file, "not a directory").expect("write codex home file");
        let loader = page_only_loader(|_| {});
        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );
        state.view_persistence = Some(SessionPickerViewPersistence {
            codex_home: codex_home_file,
            active_profile: None,
        });

        state
            .handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL))
            .await
            .unwrap();

        assert_eq!(state.density, SessionListDensity::Dense);
        assert!(
            state
                .inline_error
                .as_deref()
                .is_some_and(|error| error.contains("Failed to save view mode")),
            "expected persistence error, got {:?}",
            state.inline_error
        );
    }

    #[tokio::test]
    async fn ctrl_o_toggles_dense_path_column_override() {
        let loader = page_only_loader(|_| {});
        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );
        state.density = SessionListDensity::Dense;
        state.update_viewport(/*rows*/ 10, /*width*/ 80);

        assert!(!state.dense_path_column_visible(/*width*/ 80));

        state
            .handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL))
            .await
            .unwrap();

        assert_eq!(state.dense_path_column_override, Some(true));
        assert!(state.dense_path_column_visible(/*width*/ 80));

        state
            .handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL))
            .await
            .unwrap();

        assert_eq!(state.dense_path_column_override, None);
        assert!(!state.dense_path_column_visible(/*width*/ 80));
    }

    #[tokio::test]
    async fn ctrl_o_is_ignored_outside_dense_all_mode() {
        let loader = page_only_loader(|_| {});
        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ false,
            Some(PathBuf::from("/tmp/project")),
            SessionPickerAction::Resume,
        );
        state.density = SessionListDensity::Dense;
        state.update_viewport(/*rows*/ 10, /*width*/ 80);

        state
            .handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL))
            .await
            .unwrap();

        assert_eq!(state.dense_path_column_override, None);
        assert!(!state.dense_path_column_visible(/*width*/ 80));

        state.filter_mode = SessionFilterMode::All;
        state.density = SessionListDensity::Comfortable;

        state
            .handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL))
            .await
            .unwrap();

        assert_eq!(state.dense_path_column_override, None);
    }

    #[test]
    fn search_line_renders_sort_and_filter_tabs() {
        use crate::custom_terminal::Terminal;
        use crate::test_backend::VT100Backend;

        let loader = page_only_loader(|_| {});
        let state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ false,
            Some(PathBuf::from("/tmp/project")),
            SessionPickerAction::Resume,
        );

        let width: u16 = 100;
        let backend = VT100Backend::new(width, /*height*/ 1);
        let mut terminal = Terminal::with_options(backend).expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 0, width, 1));

        {
            let mut frame = terminal.get_frame();
            let line = search_line(&state, frame.area().width);
            frame.render_widget_ref(line, frame.area());
        }
        terminal.flush().expect("flush");

        assert_snapshot!(
            "resume_picker_search_line_sort_filter_tabs",
            terminal.backend().to_string()
        );
    }

    #[test]
    fn search_line_compacts_toolbar_on_narrow_width() {
        let loader = page_only_loader(|_| {});
        let state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ false,
            Some(PathBuf::from("/tmp/project")),
            SessionPickerAction::Resume,
        );

        let line = search_line(&state, /*width*/ 40).to_string();

        assert!(line.contains("Filter:[Cwd]"));
        assert!(line.contains("Sort:[Updated]"));
        assert!(line.find("Filter:[Cwd]") < line.find("Sort:[Updated]"));
    }

    fn dense_snapshot_row() -> Row {
        Row {
            path: Some(PathBuf::from("/tmp/a.jsonl")),
            preview: String::from(
                "Propose session picker redesign with enough title text to exercise truncation",
            ),
            thread_id: Some(
                ThreadId::from_string("019dabc1-0ef5-7431-b81c-03037f51f62c").expect("thread id"),
            ),
            thread_name: None,
            created_at: parse_timestamp_str("2026-04-28T16:30:00Z"),
            updated_at: parse_timestamp_str("2026-04-28T17:45:00Z"),
            cwd: Some(PathBuf::from(
                "/Users/felipe.coury/code/codex.fcoury-session-picker/codex-rs",
            )),
            git_branch: Some(String::from("fcoury/session-picker")),
        }
    }

    fn render_dense_row_snapshot(
        show_all: bool,
        filter_cwd: Option<PathBuf>,
        width: u16,
        dense_path_column_override: Option<bool>,
    ) -> String {
        use crate::custom_terminal::Terminal;
        use crate::test_backend::VT100Backend;

        let loader = page_only_loader(|_| {});
        let row = dense_snapshot_row();
        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            show_all,
            filter_cwd,
            SessionPickerAction::Resume,
        );
        state.density = SessionListDensity::Dense;
        state.dense_path_column_override = dense_path_column_override;
        state.all_rows = vec![row.clone()];
        state.filtered_rows = vec![row];
        state.relative_time_reference =
            Some(parse_timestamp_str("2026-04-28T18:00:00Z").expect("timestamp"));

        let backend = VT100Backend::new(width, /*height*/ 3);
        let mut terminal = Terminal::with_options(backend).expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 0, width, 3));

        {
            let mut frame = terminal.get_frame();
            let area = frame.area();
            render_list(&mut frame, area, &state);
        }
        terminal.flush().expect("flush");

        terminal.backend().to_string()
    }

    #[test]
    fn dense_session_snapshot_omits_cwd_in_cwd_filter() {
        assert_snapshot!(
            "resume_picker_dense_cwd",
            render_dense_row_snapshot(
                /*show_all*/ false,
                Some(PathBuf::from(
                    "/Users/felipe.coury/code/codex.fcoury-session-picker/codex-rs"
                )),
                /*width*/ 100,
                /*dense_path_column_override*/ None,
            )
        );
    }

    #[test]
    fn dense_session_snapshot_includes_cwd_in_all_filter() {
        assert_snapshot!(
            "resume_picker_dense_all",
            render_dense_row_snapshot(
                /*show_all*/ true, /*filter_cwd*/ None, /*width*/ 120,
                /*dense_path_column_override*/ None,
            )
        );
    }

    #[test]
    fn dense_session_snapshot_auto_hides_cwd_when_narrow() {
        assert_snapshot!(
            "resume_picker_dense_all_auto_hidden_cwd",
            render_dense_row_snapshot(
                /*show_all*/ true, /*filter_cwd*/ None, /*width*/ 100,
                /*dense_path_column_override*/ None,
            )
        );
    }

    #[test]
    fn dense_session_snapshot_forces_cwd_when_narrow() {
        assert_snapshot!(
            "resume_picker_dense_all_forced_cwd",
            render_dense_row_snapshot(
                /*show_all*/ true,
                /*filter_cwd*/ None,
                /*width*/ 48,
                /*dense_path_column_override*/ Some(true),
            )
        );
    }

    #[test]
    fn dense_session_snapshot_drops_metadata_when_narrow() {
        assert_snapshot!(
            "resume_picker_dense_narrow",
            render_dense_row_snapshot(
                /*show_all*/ true, /*filter_cwd*/ None, /*width*/ 48,
                /*dense_path_column_override*/ None,
            )
        );
    }

    #[test]
    fn dense_selected_summary_line_uses_full_width_selection_style() {
        let line = dense_summary_line(DenseSummaryInput {
            marker: selection_marker(/*is_selected*/ true, /*is_expanded*/ false),
            date: "15m ago",
            branch: Some("fcoury/session-picker"),
            cwd: Some("~/code/codex"),
            show_cwd: true,
            preserve_cwd: false,
            title: "Selected dense row",
            is_selected: true,
            width: 80,
        });

        assert_eq!(line.width(), 80);
        assert_eq!(line.style.fg, selected_session_style().fg);
        assert_eq!(line.spans[0].content, "❯ ");
    }

    #[test]
    fn dense_session_snapshot_uses_no_blank_lines_between_rows() {
        use crate::custom_terminal::Terminal;
        use crate::test_backend::VT100Backend;

        let loader = page_only_loader(|_| {});
        let mut first = dense_snapshot_row();
        first.preview = String::from("First dense row");
        let mut second = dense_snapshot_row();
        second.preview = String::from("Second dense row");
        second.git_branch = Some(String::from("fcoury/other-branch"));
        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ false,
            Some(PathBuf::from(
                "/Users/felipe.coury/code/codex.fcoury-session-picker/codex-rs",
            )),
            SessionPickerAction::Resume,
        );
        state.density = SessionListDensity::Dense;
        state.all_rows = vec![first.clone(), second.clone()];
        state.filtered_rows = vec![first, second];
        state.selected = 1;
        state.relative_time_reference =
            Some(parse_timestamp_str("2026-04-28T18:00:00Z").expect("timestamp"));

        let backend = VT100Backend::new(/*width*/ 80, /*height*/ 2);
        let mut terminal = Terminal::with_options(backend).expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 0, 80, 2));

        {
            let mut frame = terminal.get_frame();
            let area = frame.area();
            render_list(&mut frame, area, &state);
        }
        terminal.flush().expect("flush");

        assert_snapshot!(
            "resume_picker_dense_no_blank_lines",
            terminal.backend().to_string()
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
    fn density_toggle_clears_stale_more_indicator() {
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
        state.all_rows = (0..4)
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

        let width: u16 = 80;
        let height: u16 = 6;
        let backend = VT100Backend::new(width, height);
        let mut terminal = Terminal::with_options(backend).expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 0, width, height));

        state.update_viewport(height as usize, width);
        {
            let mut frame = terminal.get_frame();
            let area = frame.area();
            render_list(&mut frame, area, &state);
        }
        terminal.flush().expect("flush");
        assert!(terminal.backend().to_string().contains("↓ more"));

        state.density = SessionListDensity::Dense;
        state.update_viewport(height as usize, width);
        {
            let mut frame = terminal.get_frame();
            let area = frame.area();
            render_list(&mut frame, area, &state);
        }
        terminal.flush().expect("flush");

        assert!(!terminal.backend().to_string().contains("↓ more"));
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
        state
            .handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE))
            .await
            .unwrap();

        let guard = recorded_requests.lock().unwrap();
        assert_eq!(guard.len(), 2);
        assert_eq!(guard[1].sort_key, ThreadSortKey::CreatedAt);
    }

    #[tokio::test]
    async fn default_filter_focus_arrows_reload_with_new_filter() {
        let recorded_requests: Arc<Mutex<Vec<PageLoadRequest>>> = Arc::new(Mutex::new(Vec::new()));
        let request_sink = recorded_requests.clone();
        let loader = page_only_loader(move |req: PageLoadRequest| {
            request_sink.lock().unwrap().push(req);
        });

        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ false,
            Some(PathBuf::from("/tmp/project")),
            SessionPickerAction::Resume,
        );

        state.start_initial_load();
        {
            let guard = recorded_requests.lock().unwrap();
            assert_eq!(guard.len(), 1);
            assert_eq!(guard[0].cwd_filter, Some(PathBuf::from("/tmp/project")));
        }

        state
            .handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE))
            .await
            .unwrap();

        let guard = recorded_requests.lock().unwrap();
        assert_eq!(guard.len(), 2);
        assert_eq!(guard[1].cwd_filter, None);
    }

    #[tokio::test]
    async fn all_filter_can_switch_back_to_cwd_when_cwd_candidate_exists() {
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
            Some(PathBuf::from("/tmp/project")),
            SessionPickerAction::Resume,
        );

        state.start_initial_load();
        {
            let guard = recorded_requests.lock().unwrap();
            assert_eq!(guard.len(), 1);
            assert_eq!(guard[0].cwd_filter, None);
        }

        state
            .handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE))
            .await
            .unwrap();

        let guard = recorded_requests.lock().unwrap();
        assert_eq!(guard.len(), 2);
        assert_eq!(guard[1].cwd_filter, Some(PathBuf::from("/tmp/project")));
    }

    #[tokio::test]
    async fn filter_stays_all_when_no_cwd_candidate_exists() {
        let recorded_requests: Arc<Mutex<Vec<PageLoadRequest>>> = Arc::new(Mutex::new(Vec::new()));
        let request_sink = recorded_requests.clone();
        let loader = page_only_loader(move |req: PageLoadRequest| {
            request_sink.lock().unwrap().push(req);
        });

        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::Any,
            /*show_all*/ false,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );

        assert_eq!(
            search_line(&state, /*width*/ 80)
                .to_string()
                .matches("Cwd")
                .count(),
            0
        );

        state.start_initial_load();
        state
            .handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE))
            .await
            .unwrap();

        let guard = recorded_requests.lock().unwrap();
        assert_eq!(guard.len(), 1);
        assert_eq!(guard[0].cwd_filter, None);
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
