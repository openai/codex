use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use codex_app_server_protocol::FindFilesStreamChunkNotification;
use codex_app_server_protocol::FuzzyFileSearchResult;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_file_search as file_search;
use tokio::sync::mpsc;
use tracing::warn;

use crate::outgoing_message::OutgoingMessageSender;

const DEFAULT_LIMIT: usize = 50;
const DEFAULT_THREADS: usize = 4;
const DEFAULT_CHUNK_SIZE: usize = 100;

pub(crate) async fn run_find_files_stream(
    request_id: RequestId,
    query: String,
    roots: Vec<String>,
    exclude: Vec<String>,
    cancellation_flag: Arc<AtomicBool>,
    outgoing: Arc<OutgoingMessageSender>,
) {
    if query.is_empty() || roots.is_empty() {
        send_chunks(&outgoing, request_id, query, Vec::new(), 0, false).await;
        return;
    }

    let (tx, mut rx) = mpsc::unbounded_channel::<StreamEvent>();
    let mut sessions = Vec::new();
    let mut started_roots = Vec::new();

    for root in roots.iter() {
        let reporter = Arc::new(StreamReporter {
            root: root.clone(),
            tx: tx.clone(),
            cancellation_flag: cancellation_flag.clone(),
        });
        let session = file_search::create_session(
            root.as_ref(),
            file_search::SessionOptions {
                limit: std::num::NonZeroUsize::new(DEFAULT_LIMIT)
                    .unwrap_or(std::num::NonZeroUsize::MIN),
                exclude: exclude.clone(),
                threads: std::num::NonZeroUsize::new(DEFAULT_THREADS)
                    .unwrap_or(std::num::NonZeroUsize::MIN),
                compute_indices: true,
                respect_gitignore: true,
            },
            reporter,
        );
        match session {
            Ok(session) => {
                session.update_query(&query);
                sessions.push(session);
                started_roots.push(root.clone());
            }
            Err(err) => {
                warn!("find-files-stream failed to start for root '{root}': {err}");
                let _ = tx.send(StreamEvent::Complete { root: root.clone() });
            }
        }
    }

    drop(tx);

    let mut snapshots: HashMap<String, file_search::FileSearchSnapshot> = HashMap::new();
    let mut completed: HashMap<String, bool> = HashMap::new();
    for root in started_roots.iter() {
        completed.insert(root.clone(), false);
    }

    while let Some(event) = rx.recv().await {
        if cancellation_flag.load(Ordering::Relaxed) {
            break;
        }

        match event {
            StreamEvent::Update { root, snapshot } => {
                snapshots.insert(root.clone(), snapshot);
                send_aggregate_chunks(&outgoing, request_id.clone(), &query, &snapshots, true)
                    .await;
            }
            StreamEvent::Complete { root } => {
                if let Some(entry) = completed.get_mut(&root) {
                    *entry = true;
                }
                if completed.values().all(|done| *done) {
                    send_aggregate_chunks(&outgoing, request_id, &query, &snapshots, false).await;
                    break;
                }
            }
        }
    }

    drop(sessions);
}

fn aggregate_results(
    snapshots: &HashMap<String, file_search::FileSearchSnapshot>,
) -> (Vec<FuzzyFileSearchResult>, usize) {
    let mut results = Vec::new();
    let mut total_match_count: usize = 0;

    for (root, snapshot) in snapshots {
        total_match_count = total_match_count.saturating_add(snapshot.total_match_count);
        for matched in snapshot.matches.iter() {
            results.push(FuzzyFileSearchResult {
                root: root.clone(),
                path: matched.path.clone(),
                file_name: file_search::file_name_from_path(&matched.path),
                score: matched.score,
                indices: matched.indices.clone(),
            });
        }
    }

    results.sort_by(|a, b| {
        use std::cmp::Ordering;
        match b.score.cmp(&a.score) {
            Ordering::Equal => match a.path.cmp(&b.path) {
                Ordering::Equal => a.root.cmp(&b.root),
                other => other,
            },
            other => other,
        }
    });

    (results, total_match_count)
}

async fn send_aggregate_chunks(
    outgoing: &OutgoingMessageSender,
    request_id: RequestId,
    query: &str,
    snapshots: &HashMap<String, file_search::FileSearchSnapshot>,
    running: bool,
) {
    let (results, total_match_count) = aggregate_results(snapshots);
    send_chunks(
        outgoing,
        request_id,
        query.to_string(),
        results,
        total_match_count,
        running,
    )
    .await;
}

async fn send_chunks(
    outgoing: &OutgoingMessageSender,
    request_id: RequestId,
    query: String,
    files: Vec<FuzzyFileSearchResult>,
    total_match_count: usize,
    running: bool,
) {
    let chunk_count = files.len().max(1).div_ceil(DEFAULT_CHUNK_SIZE);
    for chunk_index in 0..chunk_count {
        let start = chunk_index * DEFAULT_CHUNK_SIZE;
        let end = (start + DEFAULT_CHUNK_SIZE).min(files.len());
        let chunk = files.get(start..end).unwrap_or_default().to_vec();
        let notification = FindFilesStreamChunkNotification {
            request_id: request_id.clone(),
            query: query.clone(),
            files: chunk,
            total_match_count,
            chunk_index,
            chunk_count,
            running,
        };
        outgoing
            .send_server_notification(ServerNotification::FindFilesStreamChunk(notification))
            .await;
    }
}

enum StreamEvent {
    Update {
        root: String,
        snapshot: file_search::FileSearchSnapshot,
    },
    Complete {
        root: String,
    },
}

struct StreamReporter {
    root: String,
    tx: mpsc::UnboundedSender<StreamEvent>,
    cancellation_flag: Arc<AtomicBool>,
}

impl file_search::SessionReporter for StreamReporter {
    fn on_update(&self, snapshot: &file_search::FileSearchSnapshot) {
        if self.cancellation_flag.load(Ordering::Relaxed) {
            return;
        }
        let _ = self.tx.send(StreamEvent::Update {
            root: self.root.clone(),
            snapshot: snapshot.clone(),
        });
    }

    fn on_complete(&self) {
        let _ = self.tx.send(StreamEvent::Complete {
            root: self.root.clone(),
        });
    }
}
