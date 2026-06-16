//! Session picker loading and background event handling.

use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadListCwdFilter;
use codex_app_server_protocol::ThreadListParams;
use codex_app_server_protocol::ThreadSortKey;
use codex_protocol::ThreadId;
use tokio::sync::mpsc;
use tracing::warn;

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
use super::load_session_transcript;
use super::load_transcript_preview;

const PAGE_SIZE: usize = 25;

#[derive(Clone)]
pub(super) struct PageLoadRequest {
    pub(super) cursor: Option<PageCursor>,
    pub(super) request_token: usize,
    pub(super) search_token: Option<usize>,
    pub(super) cwd_filter: Option<PathBuf>,
    pub(super) provider_filter: ProviderFilter,
    pub(super) sort_key: ThreadSortKey,
}

pub(super) enum PickerLoadRequest {
    Page(PageLoadRequest),
    Preview { thread_id: ThreadId },
    Transcript { thread_id: ThreadId },
}

pub(super) type PickerLoader = Arc<dyn Fn(PickerLoadRequest) + Send + Sync>;

pub(super) enum BackgroundEvent {
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

pub(super) fn spawn_app_server_page_loader(
    app_server: AppServerSession,
    include_non_interactive: bool,
    raw_reasoning_visibility: RawReasoningVisibility,
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
                    let _ = bg_tx.send(BackgroundEvent::Page {
                        request_token: request.request_token,
                        search_token: request.search_token,
                        page,
                    });
                }
                PickerLoadRequest::Preview { thread_id } => {
                    let preview = load_transcript_preview(&mut app_server, thread_id).await;
                    let _ = bg_tx.send(BackgroundEvent::Preview { thread_id, preview });
                }
                PickerLoadRequest::Transcript { thread_id } => {
                    let transcript = load_session_transcript(
                        &mut app_server,
                        thread_id,
                        raw_reasoning_visibility,
                    )
                    .await;
                    let _ = bg_tx.send(BackgroundEvent::Transcript {
                        thread_id,
                        transcript,
                    });
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

async fn load_app_server_page(
    app_server: &mut AppServerSession,
    cursor: Option<String>,
    cwd_filter: Option<&Path>,
    provider_filter: ProviderFilter,
    sort_key: ThreadSortKey,
    include_non_interactive: bool,
) -> io::Result<PickerPage> {
    let response = app_server
        .thread_list(thread_list_params(
            cursor,
            cwd_filter,
            provider_filter,
            sort_key,
            include_non_interactive,
        ))
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
        source_kinds: Some(crate::resume_source_kinds(include_non_interactive)),
        archived: Some(false),
        parent_thread_id: None,
        cwd: cwd_filter.map(|cwd| ThreadListCwdFilter::One(cwd.to_string_lossy().into_owned())),
        use_state_db_only: false,
        search_term: None,
    }
}

impl PickerState {
    pub(super) async fn handle_background_event(
        &mut self,
        event: BackgroundEvent,
    ) -> color_eyre::eyre::Result<()> {
        match event {
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
                let page = page.map_err(color_eyre::Report::from)?;
                self.ingest_page(page);
                self.complete_pending_page_down();
                let completed_token = pending.search_token.or(search_token);
                self.continue_search_if_token_matches(completed_token);
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
}

#[cfg(test)]
#[path = "loading_tests.rs"]
mod tests;
