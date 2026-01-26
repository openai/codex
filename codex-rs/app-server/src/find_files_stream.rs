use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use codex_app_server_protocol::FindFilesStreamChunkNotification;
use codex_app_server_protocol::FuzzyFileSearchResult;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_file_search as file_search;
use codex_file_search::FileSearchResults;
use codex_file_search::search_manager::DebounceConfig;
use codex_file_search::search_manager::DebouncedSearchManager;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tracing::warn;

use crate::outgoing_message::OutgoingMessageSender;

const LIMIT_PER_ROOT: usize = 50;
const MAX_THREADS: usize = 12;
const CHUNK_SIZE: usize = 100;
const COMPUTE_INDICES: bool = true;

type SearchCallback = dyn Fn(String, FileSearchResults, bool) + Send + Sync + 'static;

pub(crate) enum FindFilesStreamUpdate {
    Cancel,
    Results {
        root: String,
        query: String,
        results: FileSearchResults,
        running: bool,
    },
}

struct RootSearchState {
    results: FileSearchResults,
    running: bool,
    seen: bool,
}

pub(crate) struct FindFilesStreamSession {
    roots: Vec<String>,
    managers: Vec<DebouncedSearchManager<Box<SearchCallback>>>,
    request_id: Arc<Mutex<RequestId>>,
    cancel_flag: Arc<AtomicBool>,
    update_tx: mpsc::UnboundedSender<FindFilesStreamUpdate>,
}

impl FindFilesStreamSession {
    pub(crate) fn new(
        roots: Vec<String>,
        request_id: RequestId,
        outgoing: Arc<OutgoingMessageSender>,
    ) -> (Self, oneshot::Receiver<()>) {
        let limit_per_root = NonZeroUsize::new(LIMIT_PER_ROOT).unwrap_or(NonZeroUsize::MIN);
        let threads_per_root = threads_per_root(roots.len());
        let request_id = Arc::new(Mutex::new(request_id));
        let cancel_flag = Arc::new(AtomicBool::new(false));

        let (update_tx, update_rx) = mpsc::unbounded_channel();
        let (done_tx, done_rx) = oneshot::channel();

        let mut managers = Vec::with_capacity(roots.len());
        for root in &roots {
            let root_path = PathBuf::from(root);
            let root_name = root.clone();
            let update_tx = update_tx.clone();
            let cancel_flag = Arc::clone(&cancel_flag);
            let callback: Box<SearchCallback> = Box::new(move |query, results, running| {
                if cancel_flag.load(Ordering::Relaxed) {
                    return;
                }
                if update_tx
                    .send(FindFilesStreamUpdate::Results {
                        root: root_name.clone(),
                        query,
                        results,
                        running,
                    })
                    .is_err()
                {
                    warn!("find-files-stream update channel closed");
                }
            });

            let manager = DebouncedSearchManager::new(
                root_path,
                limit_per_root,
                threads_per_root,
                COMPUTE_INDICES,
                Vec::new(),
                Arc::new(callback),
                DebounceConfig::default(),
            );
            managers.push(manager);
        }

        let cancel_flag_for_task = Arc::clone(&cancel_flag);
        let request_id_for_task = Arc::clone(&request_id);
        let roots_for_task = roots.clone();
        tokio::spawn(async move {
            run_stream_task(
                roots_for_task,
                update_rx,
                request_id_for_task,
                outgoing,
                cancel_flag_for_task,
            )
            .await;
            let _ = done_tx.send(());
        });

        (
            Self {
                roots,
                managers,
                request_id,
                cancel_flag,
                update_tx,
            },
            done_rx,
        )
    }

    pub(crate) fn roots(&self) -> &[String] {
        &self.roots
    }

    pub(crate) fn update_request_id(&self, request_id: RequestId) {
        #[expect(clippy::unwrap_used)]
        let mut locked = self.request_id.lock().unwrap();
        *locked = request_id;
    }

    pub(crate) fn on_query(&self, query: String) {
        for manager in &self.managers {
            manager.on_query(query.clone());
        }
    }

    pub(crate) fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
        let _ = self.update_tx.send(FindFilesStreamUpdate::Cancel);
    }
}

async fn run_stream_task(
    roots: Vec<String>,
    mut update_rx: mpsc::UnboundedReceiver<FindFilesStreamUpdate>,
    request_id: Arc<Mutex<RequestId>>,
    outgoing: Arc<OutgoingMessageSender>,
    cancel_flag: Arc<AtomicBool>,
) {
    let mut root_states: HashMap<String, RootSearchState> = roots
        .iter()
        .map(|root| {
            (
                root.clone(),
                RootSearchState {
                    results: FileSearchResults {
                        matches: Vec::new(),
                        total_match_count: 0,
                    },
                    running: false,
                    seen: false,
                },
            )
        })
        .collect();
    let mut current_query = String::new();

    while let Some(update) = update_rx.recv().await {
        match update {
            FindFilesStreamUpdate::Cancel => break,
            FindFilesStreamUpdate::Results {
                root,
                query,
                results,
                running,
            } => {
                if cancel_flag.load(Ordering::Relaxed) {
                    break;
                }

                if query != current_query {
                    current_query.clear();
                    current_query.push_str(&query);
                    for state in root_states.values_mut() {
                        state.results = FileSearchResults {
                            matches: Vec::new(),
                            total_match_count: 0,
                        };
                        state.running = false;
                        state.seen = false;
                    }
                }

                if let Some(state) = root_states.get_mut(&root) {
                    state.results = results;
                    state.running = running;
                    state.seen = true;
                } else {
                    warn!("find-files-stream received update for unexpected root: {root}");
                    continue;
                }

                let mut files = Vec::new();
                let mut total_match_count = 0usize;
                let mut any_running = false;
                let mut all_seen = true;
                for (root, state) in &root_states {
                    if !state.seen {
                        all_seen = false;
                    }
                    any_running |= state.running;
                    total_match_count += state.results.total_match_count;
                    for entry in &state.results.matches {
                        files.push(FuzzyFileSearchResult {
                            root: root.clone(),
                            path: entry.path.clone(),
                            file_name: file_search::file_name_from_path(&entry.path),
                            score: entry.score,
                            indices: entry.indices.clone(),
                        });
                    }
                }

                files.sort_by(file_search::cmp_by_score_desc_then_path_asc::<
                    FuzzyFileSearchResult,
                    _,
                    _,
                >(|f| f.score, |f| f.path.as_str()));

                let chunk_count = if files.is_empty() {
                    1
                } else {
                    (files.len() + CHUNK_SIZE - 1) / CHUNK_SIZE
                };

                #[expect(clippy::unwrap_used)]
                let current_request_id = request_id.lock().unwrap().clone();
                if files.is_empty() {
                    let notification = FindFilesStreamChunkNotification {
                        request_id: current_request_id,
                        query: current_query.clone(),
                        files: Vec::new(),
                        total_match_count,
                        chunk_index: 0,
                        chunk_count,
                        running: any_running,
                    };
                    outgoing
                        .send_server_notification(ServerNotification::FindFilesStreamChunk(
                            notification,
                        ))
                        .await;
                } else {
                    for (index, chunk) in files.chunks(CHUNK_SIZE).enumerate() {
                        let notification = FindFilesStreamChunkNotification {
                            request_id: current_request_id.clone(),
                            query: current_query.clone(),
                            files: chunk.to_vec(),
                            total_match_count,
                            chunk_index: index,
                            chunk_count,
                            running: any_running,
                        };
                        outgoing
                            .send_server_notification(ServerNotification::FindFilesStreamChunk(
                                notification,
                            ))
                            .await;
                    }
                }

                if all_seen && !any_running {
                    cancel_flag.store(true, Ordering::Relaxed);
                    break;
                }
            }
        }
    }
}

fn threads_per_root(roots_len: usize) -> NonZeroUsize {
    let cores = std::thread::available_parallelism()
        .map(std::num::NonZero::get)
        .unwrap_or(1);
    let threads = cores.min(MAX_THREADS);
    let threads_per_root = (threads / roots_len.max(1)).max(1);
    NonZeroUsize::new(threads_per_root).unwrap_or(NonZeroUsize::MIN)
}
