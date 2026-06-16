//! Session picker loading, State DB seeding, and authoritative reconciliation.

use std::future::Future;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadListCwdFilter;
use codex_app_server_protocol::ThreadListParams;
use codex_app_server_protocol::ThreadListResponse;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadReadResponse;
use codex_app_server_protocol::ThreadSortKey;
use codex_protocol::ThreadId;
use tokio::sync::mpsc;
use tokio::task::JoinError;
use tokio::task::JoinSet;
use tracing::warn;
use uuid::Uuid;

use super::AppServerSession;
use super::LoadingState;
use super::PickerState;
use super::ProviderFilter;
use super::RawReasoningVisibility;
use super::Row;
use super::SessionTranscriptState;
use super::TranscriptCells;
use super::TranscriptPreviewLine;
use super::TranscriptPreviewState;
#[cfg(test)]
use super::parse_timestamp_str;
use super::transcript_preview_lines;
use crate::thread_transcript::thread_to_transcript_cells;

const PAGE_SIZE: usize = 25;
// Expanded rows read full transcripts, so keep preview I/O narrowly bounded.
const MAX_CONCURRENT_PREVIEW_READS: usize = 2;

#[derive(Clone)]
pub(super) struct PageLoadRequest {
    pub(super) cursor: Option<PageCursor>,
    pub(super) request_token: usize,
    pub(super) search_token: Option<usize>,
    pub(super) cwd_filter: Option<PathBuf>,
    pub(super) provider_filter: ProviderFilter,
    pub(super) sort_key: ThreadSortKey,
    pub(super) seed_from_state_db: bool,
}

pub(super) enum PickerLoadRequest {
    Page(PageLoadRequest),
    Preview { thread_id: ThreadId },
    Transcript { thread_id: ThreadId },
}

pub(super) type PickerLoader = Arc<dyn Fn(PickerLoadRequest) + Send + Sync>;

pub(super) enum BackgroundEvent {
    SeedPage {
        request_token: usize,
        page: PickerPage,
    },
    Page {
        request_token: usize,
        search_token: Option<usize>,
        page: io::Result<PickerPage>,
    },
    Preview {
        thread_id: ThreadId,
        preview: io::Result<Vec<TranscriptPreviewLine>>,
    },
    Transcript {
        thread_id: ThreadId,
        transcript: io::Result<TranscriptCells>,
    },
}

#[derive(Clone)]
pub(super) enum PageCursor {
    AppServer(String),
}

pub(super) struct PickerPage {
    pub(super) rows: Vec<Row>,
    pub(super) next_cursor: Option<PageCursor>,
    pub(super) num_scanned_files: usize,
    pub(super) reached_scan_cap: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum InitialPageLoad {
    #[default]
    Authoritative,
    SeedPending,
    Provisional,
}

impl InitialPageLoad {
    pub(super) fn state_db_first() -> Self {
        Self::SeedPending
    }

    pub(super) fn begin_load(&mut self) -> bool {
        let seed_from_state_db = *self == Self::SeedPending;
        *self = Self::Authoritative;
        seed_from_state_db
    }

    fn mark_seeded(&mut self) {
        *self = Self::Provisional;
    }

    fn finish_reconciliation(&mut self) -> bool {
        let was_provisional = *self == Self::Provisional;
        *self = Self::Authoritative;
        was_provisional
    }

    pub(super) fn is_provisional(self) -> bool {
        self == Self::Provisional
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ThreadListLookupMode {
    StateDbOnly,
    ScanAndRepair,
}

pub(super) fn spawn_app_server_page_loader(
    app_server: AppServerSession,
    include_non_interactive: bool,
    raw_reasoning_visibility: RawReasoningVisibility,
    bg_tx: mpsc::UnboundedSender<BackgroundEvent>,
) -> PickerLoader {
    let (request_tx, request_rx) = mpsc::unbounded_channel::<PickerLoadRequest>();
    let request_handle = app_server.request_handle();

    tokio::spawn(async move {
        run_picker_loader(request_rx, move |request| {
            handle_picker_load_request(
                request,
                request_handle.clone(),
                include_non_interactive,
                raw_reasoning_visibility,
                bg_tx.clone(),
            )
        })
        .await;
        if let Err(err) = app_server.shutdown().await {
            warn!(%err, "Failed to shut down app-server picker session");
        }
    });

    Arc::new(move |request: PickerLoadRequest| {
        let _ = request_tx.send(request);
    })
}

async fn run_picker_loader<F, Fut>(
    mut request_rx: mpsc::UnboundedReceiver<PickerLoadRequest>,
    load_request: F,
) where
    F: Fn(PickerLoadRequest) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let (page_tx, page_rx) = mpsc::unbounded_channel();
    let page_load_request = load_request.clone();
    let page_task = tokio::spawn(run_page_loader(page_rx, move |request| {
        page_load_request(PickerLoadRequest::Page(request))
    }));
    let (preview_tx, preview_rx) = mpsc::unbounded_channel();
    let preview_load_request = load_request.clone();
    let preview_task = tokio::spawn(run_preview_loader(preview_rx, move |thread_id| {
        preview_load_request(PickerLoadRequest::Preview { thread_id })
    }));
    let mut tasks = JoinSet::new();
    loop {
        tokio::select! {
            request = request_rx.recv() => {
                let Some(request) = request else {
                    break;
                };
                match request {
                    PickerLoadRequest::Page(request) => {
                        let _ = page_tx.send(request);
                    }
                    PickerLoadRequest::Preview { thread_id } => {
                        let _ = preview_tx.send(thread_id);
                    }
                    request @ PickerLoadRequest::Transcript { .. } => {
                        tasks.spawn(load_request(request));
                    }
                }
            }
            result = tasks.join_next(), if !tasks.is_empty() => {
                if let Some(result) = result {
                    log_loader_task_result(result);
                }
            }
        }
    }

    drop(page_tx);
    drop(preview_tx);
    page_task.abort();
    log_loader_task_result(page_task.await);
    preview_task.abort();
    log_loader_task_result(preview_task.await);
    tasks.abort_all();
    while let Some(result) = tasks.join_next().await {
        log_loader_task_result(result);
    }
}

async fn run_preview_loader<F, Fut>(
    mut request_rx: mpsc::UnboundedReceiver<ThreadId>,
    mut load_preview: F,
) where
    F: FnMut(ThreadId) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let mut tasks = JoinSet::new();
    let mut request_channel_open = true;
    while request_channel_open || !tasks.is_empty() {
        tokio::select! {
            request = request_rx.recv(), if request_channel_open && tasks.len() < MAX_CONCURRENT_PREVIEW_READS => {
                match request {
                    Some(thread_id) => {
                        tasks.spawn(load_preview(thread_id));
                    }
                    None => request_channel_open = false,
                }
            }
            result = tasks.join_next(), if !tasks.is_empty() => {
                if let Some(result) = result {
                    log_loader_task_result(result);
                }
            }
        }
    }
}

async fn run_page_loader<F, Fut>(
    mut request_rx: mpsc::UnboundedReceiver<PageLoadRequest>,
    load_page: F,
) where
    F: Fn(PageLoadRequest) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let Some(mut request) = request_rx.recv().await else {
        return;
    };
    loop {
        let load = load_page(request);
        tokio::pin!(load);
        tokio::select! {
            () = &mut load => {
                let Some(next_request) = request_rx.recv().await else {
                    break;
                };
                request = next_request;
            }
            next_request = request_rx.recv() => {
                let Some(next_request) = next_request else {
                    load.await;
                    break;
                };
                request = next_request;
            }
        }
        while let Ok(next_request) = request_rx.try_recv() {
            request = next_request;
        }
    }
}

fn log_loader_task_result(result: Result<(), JoinError>) {
    if let Err(err) = result
        && !err.is_cancelled()
    {
        warn!(%err, "Session picker loader task failed");
    }
}

async fn handle_picker_load_request(
    request: PickerLoadRequest,
    request_handle: AppServerRequestHandle,
    include_non_interactive: bool,
    raw_reasoning_visibility: RawReasoningVisibility,
    bg_tx: mpsc::UnboundedSender<BackgroundEvent>,
) {
    match request {
        PickerLoadRequest::Page(request) => {
            if request.seed_from_state_db {
                match load_app_server_page(
                    &request_handle,
                    /*cursor*/ None,
                    request.cwd_filter.as_deref(),
                    request.provider_filter.clone(),
                    request.sort_key,
                    include_non_interactive,
                    ThreadListLookupMode::StateDbOnly,
                )
                .await
                {
                    Ok(page) => {
                        let _ = bg_tx.send(BackgroundEvent::SeedPage {
                            request_token: request.request_token,
                            page,
                        });
                    }
                    Err(err) => {
                        warn!(
                            %err,
                            "State DB picker lookup failed; falling back to scan-and-repair"
                        );
                    }
                }
            }

            let cursor = request.cursor.map(|PageCursor::AppServer(cursor)| cursor);
            let page = load_app_server_page(
                &request_handle,
                cursor,
                request.cwd_filter.as_deref(),
                request.provider_filter,
                request.sort_key,
                include_non_interactive,
                ThreadListLookupMode::ScanAndRepair,
            )
            .await;
            let _ = bg_tx.send(BackgroundEvent::Page {
                request_token: request.request_token,
                search_token: request.search_token,
                page,
            });
        }
        PickerLoadRequest::Preview { thread_id } => {
            let preview = read_app_server_thread(&request_handle, thread_id)
                .await
                .map(|thread| transcript_preview_lines(&thread));
            let _ = bg_tx.send(BackgroundEvent::Preview { thread_id, preview });
        }
        PickerLoadRequest::Transcript { thread_id } => {
            let transcript = read_app_server_thread(&request_handle, thread_id)
                .await
                .map(|thread| thread_to_transcript_cells(&thread, raw_reasoning_visibility));
            let _ = bg_tx.send(BackgroundEvent::Transcript {
                thread_id,
                transcript,
            });
        }
    }
}

async fn load_app_server_page(
    request_handle: &AppServerRequestHandle,
    cursor: Option<String>,
    cwd_filter: Option<&Path>,
    provider_filter: ProviderFilter,
    sort_key: ThreadSortKey,
    include_non_interactive: bool,
    lookup_mode: ThreadListLookupMode,
) -> io::Result<PickerPage> {
    let response: ThreadListResponse = request_handle
        .request_typed(ClientRequest::ThreadList {
            request_id: RequestId::String(format!("resume-picker-thread-list-{}", Uuid::new_v4())),
            params: thread_list_params(
                cursor,
                cwd_filter,
                provider_filter,
                sort_key,
                include_non_interactive,
                lookup_mode,
            ),
        })
        .await
        .map_err(io::Error::other)?;
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

async fn read_app_server_thread(
    request_handle: &AppServerRequestHandle,
    thread_id: ThreadId,
) -> io::Result<Thread> {
    let response: ThreadReadResponse = request_handle
        .request_typed(ClientRequest::ThreadRead {
            request_id: RequestId::String(format!("resume-picker-thread-read-{}", Uuid::new_v4())),
            params: ThreadReadParams {
                thread_id: thread_id.to_string(),
                include_turns: true,
            },
        })
        .await
        .map_err(io::Error::other)?;
    Ok(response.thread)
}

fn row_from_app_server_thread(thread: Thread) -> Option<Row> {
    let thread_id = match ThreadId::from_string(&thread.id) {
        Ok(thread_id) => thread_id,
        Err(err) => {
            warn!(thread_id = thread.id, %err, "Skipping app-server picker row with invalid id");
            return None;
        }
    };
    Some(Row::from_app_server_thread(&thread, thread_id))
}

impl Row {
    fn from_app_server_thread(thread: &Thread, thread_id: ThreadId) -> Self {
        let preview = thread.preview.trim();
        Self {
            path: thread.path.clone(),
            preview: if preview.is_empty() {
                String::from("(no message yet)")
            } else {
                preview.to_string()
            },
            thread_id: Some(thread_id),
            thread_name: thread.name.clone(),
            created_at: chrono::DateTime::from_timestamp(thread.created_at, 0)
                .map(|dt| dt.with_timezone(&chrono::Utc)),
            updated_at: chrono::DateTime::from_timestamp(thread.updated_at, 0)
                .map(|dt| dt.with_timezone(&chrono::Utc)),
            cwd: Some(thread.cwd.to_path_buf()),
            git_branch: thread
                .git_info
                .as_ref()
                .and_then(|git_info| git_info.branch.clone()),
        }
    }
}

fn thread_list_params(
    cursor: Option<String>,
    cwd_filter: Option<&Path>,
    provider_filter: ProviderFilter,
    sort_key: ThreadSortKey,
    include_non_interactive: bool,
    lookup_mode: ThreadListLookupMode,
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
        source_kinds: Some(crate::resume_source_kinds(include_non_interactive)),
        archived: Some(false),
        parent_thread_id: None,
        cwd: cwd_filter.map(|cwd| ThreadListCwdFilter::One(cwd.to_string_lossy().into_owned())),
        use_state_db_only: lookup_mode == ThreadListLookupMode::StateDbOnly,
        search_term: None,
    }
}

impl PickerState {
    pub(super) async fn handle_background_event(
        &mut self,
        event: BackgroundEvent,
    ) -> color_eyre::eyre::Result<()> {
        match event {
            BackgroundEvent::SeedPage {
                request_token,
                page,
            } => {
                let LoadingState::Pending(pending) = self.pagination.loading else {
                    return Ok(());
                };
                if pending.request_token != request_token {
                    return Ok(());
                }
                self.initial_page_load.mark_seeded();
                self.replace_with_page(page);
            }
            BackgroundEvent::Page {
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
                match page {
                    Ok(page) if self.initial_page_load.finish_reconciliation() => {
                        self.replace_with_page(page);
                        self.complete_pending_page_down();
                        self.reevaluate_search();
                    }
                    Ok(page) => {
                        self.ingest_page(page);
                        self.complete_pending_page_down();
                        let completed_token = pending.search_token.or(search_token);
                        self.continue_search_if_token_matches(completed_token);
                    }
                    Err(err) if self.initial_page_load.is_provisional() => {
                        warn!(
                            %err,
                            "Session picker reconciliation failed; keeping State DB results"
                        );
                        let cached_results_are_truncated = self.pagination.next_cursor.is_some();
                        self.pagination.next_cursor = None;
                        self.inline_error = Some(if cached_results_are_truncated {
                            String::from(
                                "Could not refresh sessions; showing the first page of indexed results",
                            )
                        } else {
                            String::from("Could not refresh sessions; showing indexed results")
                        });
                        self.complete_pending_page_down();
                        self.reevaluate_search();
                        self.request_frame();
                    }
                    Err(err) => return Err(color_eyre::Report::from(err)),
                }
            }
            BackgroundEvent::Preview { thread_id, preview } => {
                self.transcript_previews.insert(
                    thread_id,
                    match preview {
                        Ok(lines) => TranscriptPreviewState::Loaded(lines),
                        Err(_) => TranscriptPreviewState::Failed,
                    },
                );
                self.request_frame();
            }
            BackgroundEvent::Transcript {
                thread_id,
                transcript,
            } => match transcript {
                Ok(cells) => {
                    let should_open = self.pending_transcript_open == Some(thread_id);
                    self.transcript_cells
                        .insert(thread_id, SessionTranscriptState::Loaded(cells.clone()));
                    if should_open {
                        self.open_pending_transcript_if_ready();
                    }
                    self.request_frame();
                }
                Err(_) => {
                    self.transcript_cells
                        .insert(thread_id, SessionTranscriptState::Failed);
                    if self.pending_transcript_open == Some(thread_id) {
                        self.pending_transcript_open = None;
                        self.transcript_loading_frame_shown = false;
                        self.inline_error = Some("Could not load transcript preview".to_string());
                    }
                    self.request_frame();
                }
            },
        }
        Ok(())
    }

    /// Replaces the current result set with a new first page while preserving
    /// the selected thread when it is still present.
    fn replace_with_page(&mut self, page: PickerPage) {
        let selected_row = self.filtered_rows.get(self.selected);
        let selected_thread_id = selected_row.and_then(|row| row.thread_id);
        let selected_key = selected_row.and_then(Row::seen_key);
        let selected_index = self.selected;

        self.pagination.next_cursor = page.next_cursor;
        self.pagination.num_scanned_files = page.num_scanned_files;
        self.pagination.reached_scan_cap = page.reached_scan_cap;
        self.frozen_footer_percent = None;
        self.all_rows.clear();
        self.filtered_rows.clear();
        self.seen_rows.clear();

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
        self.selected = selected_thread_id
            .and_then(|selected_thread_id| {
                self.filtered_rows
                    .iter()
                    .position(|row| row.thread_id == Some(selected_thread_id))
            })
            .or_else(|| {
                selected_key.and_then(|selected_key| {
                    self.filtered_rows
                        .iter()
                        .position(|row| row.seen_key().as_ref() == Some(&selected_key))
                })
            })
            .unwrap_or_else(|| selected_index.min(self.filtered_rows.len().saturating_sub(1)));
        self.ensure_selected_visible();
        self.request_frame();
    }
}

#[cfg(test)]
#[path = "loading_tests.rs"]
mod tests;
