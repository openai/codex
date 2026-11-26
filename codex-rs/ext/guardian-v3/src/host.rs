//! A single worker routes durable originals from the live tree's last published position.
//! User-message and turn-end wakes coalesce. Root shutdown joins the worker before history closes.
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::PoisonError;
use std::sync::Weak;

use anyhow::Context;
use codex_core::CodexThread;
use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_core::guardian_review::PreserveReviewWrites;
use codex_core::guardian_review::ReviewPreparationError;
use codex_core::guardian_review::ReviewRetrieval;
use codex_core::guardian_review::ShadowReviewer;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadReadyInput;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::ThreadStopInput;
use codex_extension_api::TurnAbortInput;
use codex_extension_api::TurnLifecycleContributor;
use codex_extension_api::TurnStartInput;
use codex_extension_api::TurnStopInput;
use codex_features::GuardianV3ConfigToml;
use codex_guardian_reviewer::ReviewerTasks;
use codex_guardian_reviewer::routes_approval_policy_to_guardian;
use codex_history::HistoryInitialization;
use codex_message_board::AgentMessageBoard;
use codex_message_board::LocalAgentMessageBoard;
use codex_message_board::MessageBoardHost;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_protocol::items::TurnItem;
use codex_state::SqliteConfig;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use crate::ActorReviewer;
use crate::EvidenceCursor;
use crate::Housekeeper;
use crate::LiveDelivery;
use crate::local_board;
use crate::local_board_options;
use crate::shadow_metrics::BoardSize;
use crate::shadow_metrics::PayloadLimit;
use crate::shadow_metrics::ShadowMetrics;

#[path = "shadow.rs"]
mod shadow;

#[cfg(test)]
#[path = "host_tests.rs"]
mod tests;

pub fn install(
    registry: &mut ExtensionRegistryBuilder<Config>,
    manager: Weak<ThreadManager>,
    config: &Config,
    board_host: impl Fn(ThreadId) -> Arc<dyn MessageBoardHost> + Send + Sync + 'static,
) {
    let host = Arc::new(Host {
        live: Some(Arc::new(BoardSettings::live(config))),
        manager,
        deliveries: Arc::default(),
        board_host: Arc::new(board_host),
    });
    registry.thread_lifecycle_contributor(host.clone());
    registry.turn_lifecycle_contributor(host);
}

/// Desktop supplies rollout overrides at thread start, after registration.
pub fn install_shadow(
    registry: &mut ExtensionRegistryBuilder<Config>,
    manager: Weak<ThreadManager>,
    board_host: impl Fn(ThreadId) -> Arc<dyn MessageBoardHost> + Send + Sync + 'static,
) {
    let host = Arc::new(Host {
        live: None,
        manager,
        deliveries: Arc::default(),
        board_host: Arc::new(board_host),
    });
    registry.thread_lifecycle_contributor(host.clone());
    registry.config_contributor(host.clone());
    registry.turn_lifecycle_contributor(host);
}

impl Host {
    fn enabled(&self, config: &Config) -> bool {
        self.live.is_some()
            || (config
                .features
                .enabled(codex_features::Feature::GuardianV3Shadow)
                && config.features.enabled(codex_features::Feature::GuardianV2)
                // A shadow must never open the durable board used by live GV3.
                && message_board_in_memory(config).unwrap_or(true)
                && routes_approval_policy_to_guardian(
                    config.permissions.approval_policy.value(),
                    config.approvals_reviewer,
                ))
    }
}

#[derive(Clone, Copy)]
enum IntakeStart {
    Initial(
        codex_protocol::protocol::ThreadHistoryMode,
        HistoryInitialization,
    ),
    Restart,
}

fn message_board_in_memory(config: &Config) -> Option<bool> {
    config.guardian_v3.as_ref()?.message_board_in_memory
}

enum BoardStorage {
    File(SqliteConfig),
    Memory,
}

struct BoardSettings {
    storage: BoardStorage,
    config: Arc<GuardianV3ConfigToml>,
    retrieval: ReviewRetrieval,
}

impl BoardSettings {
    fn live(config: &Config) -> Self {
        let storage = if message_board_in_memory(config).unwrap_or(false) {
            BoardStorage::Memory
        } else {
            BoardStorage::File(config.sqlite_config().clone())
        };
        Self::new(config, storage)
    }

    fn new(config: &Config, storage: BoardStorage) -> Self {
        let config = config.guardian_v3.clone().unwrap_or_default();
        let retrieval = crate::board_retrieval(
            config
                .reviewer
                .as_ref()
                .and_then(|reviewer| reviewer.retrieval_instructions.as_deref()),
        );
        Self {
            storage,
            config,
            retrieval,
        }
    }
}

pub(super) struct PublishedBoard {
    pub(super) delivery: Arc<LiveDelivery>,
    retrieval: ReviewRetrieval,
}

// Shared by loaded actors, even before their first review.
// Only the root can initialize the cursor, using its own history.
#[derive(Default)]
pub(super) struct TreeDelivery {
    pub(super) board: OnceLock<PublishedBoard>,
    stop: CancellationToken,
    shadow_intake_stopped: CancellationToken,
    actors: Mutex<Vec<Weak<ActorReviewer>>>,
}

#[derive(Clone)]
struct Host {
    // Live settings come from server startup. Shadow settings come from each root.
    live: Option<Arc<BoardSettings>>,
    manager: Weak<ThreadManager>,
    deliveries: Arc<Mutex<HashMap<ThreadId, Weak<TreeDelivery>>>>,
    board_host: Arc<dyn Fn(ThreadId) -> Arc<dyn MessageBoardHost> + Send + Sync>,
}

pub(super) struct RootWorker {
    pub(super) shadow: bool,
    pub(super) reader: Arc<dyn AgentMessageBoard>,
    // Live wakes coalesce. Callers that need to wait supply a reply sender.
    wake: mpsc::Sender<Option<oneshot::Sender<()>>>,
    lifetime: ReviewerTasks,
}

/// Wake the root worker and wait for it to route pending history to the board.
pub async fn route_pending_history(root: Arc<CodexThread>) -> Result<(), ReviewPreparationError> {
    use ReviewPreparationError::EvidenceUnavailable;
    let worker = root
        .thread_extension_data()
        .get::<RootWorker>()
        .ok_or(EvidenceUnavailable)?;
    let (done, routed) = oneshot::channel();
    worker
        .wake
        .send(Some(done))
        .await
        .map_err(|_| EvidenceUnavailable)?;
    routed.await.map_err(|_| EvidenceUnavailable)
}

impl Host {
    fn tree_delivery(&self, root: ThreadId) -> Arc<TreeDelivery> {
        let mut deliveries = self
            .deliveries
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(tree) = deliveries.get(&root).and_then(Weak::upgrade)
            && !tree.stop.is_cancelled()
        {
            return tree;
        }
        deliveries.retain(|_, tree| tree.strong_count() > 0);
        let tree = Arc::new(TreeDelivery::default());
        deliveries.insert(root, Arc::downgrade(&tree));
        tree
    }

    fn reviewer(
        &self,
        identity: &shadow::Actor,
        tree: &Arc<TreeDelivery>,
        retrieval: &ReviewRetrieval,
        cancellation: CancellationToken,
    ) -> Arc<ActorReviewer> {
        Arc::new(ActorReviewer::new(
            self.manager.clone(),
            identity.root,
            identity.actor,
            identity.path.clone(),
            retrieval.clone(),
            Arc::clone(tree),
            cancellation,
        ))
    }

    async fn activate(
        &self,
        root: ThreadId,
        tree: &TreeDelivery,
        thread: &ExtensionData,
        start: IntakeStart,
        settings: &BoardSettings,
    ) -> anyhow::Result<()> {
        let history = self
            .manager
            .upgrade()
            .context("Guardian host stopped")?
            .thread_store();
        let cursor = match start {
            IntakeStart::Initial(mode, initialization) => {
                if matches!(settings.storage, BoardStorage::Memory) {
                    anyhow::ensure!(
                        matches!(
                            initialization,
                            HistoryInitialization::New | HistoryInitialization::Cleared
                        ),
                        "in-memory Guardian evidence requires a fresh root"
                    );
                }
                EvidenceCursor::at_thread_ready(history.as_ref(), root, mode, initialization)
                    .await?
            }
            IntakeStart::Restart => {
                history.flush_thread(root).await?;
                EvidenceCursor::from_now(history.as_ref(), root).await?
            }
        };
        let board_host = (self.board_host)(root);
        let board = match &settings.storage {
            BoardStorage::File(sqlite) => {
                local_board(sqlite.clone(), root.into())
                    .read_write()
                    .open(board_host)
                    .await?
            }
            BoardStorage::Memory => {
                LocalAgentMessageBoard::open_in_memory(
                    &local_board_options(),
                    root.into(),
                    board_host,
                )
                .await?
            }
        };
        // No awaits after this check. A concurrent stop must either cancel
        // startup or find the published worker to drain.
        let _publication = tree.actors.lock().unwrap_or_else(PoisonError::into_inner);
        if tree.stop.is_cancelled() {
            return Ok(());
        }
        let reader: Arc<dyn AgentMessageBoard> = Arc::new(board.read_only_handle());
        let lifetime = ReviewerTasks {
            cancellation: tree.stop.child_token(),
            ..Default::default()
        };
        let (wake, mut wakes) = mpsc::channel::<Option<oneshot::Sender<()>>>(1);
        let initial_position = cursor.position();
        let published = tree.board.get_or_init(|| PublishedBoard {
            delivery: Arc::new(LiveDelivery::from_cursor(cursor)),
            retrieval: settings.retrieval.clone(),
        });
        let delivery = Arc::clone(&published.delivery);
        let saved = delivery.cursor();
        // A surviving child can keep a cursor behind the resumed root's history tail.
        if saved.position().thread_id == initial_position.thread_id
            && saved.position().end_ordinal_exclusive < initial_position.end_ordinal_exclusive
        {
            let _ = wake.try_send(None);
        }
        let shadow = self.live.is_none();
        let intake_guard = shadow.then(|| tree.shadow_intake_stopped.clone().drop_guard());
        let metrics = thread.get::<ShadowMetrics>();
        let manager = self.manager.clone();
        let config = Arc::clone(&settings.config);
        let cancellation = lifetime.cancellation.clone();
        lifetime.tasks.spawn(async move {
            let mut failure_stage = "housekeeper";
            let work = async {
                let board = Arc::new(board);
                let mut housekeeper = None;
                while let Some(Some(done)) = cancellation.run_until_cancelled(wakes.recv()).await {
                    // On failure, dropping the sender wakes the caller with an error.
                    let finish = || {
                        if let Some(done) = done {
                            let _ = done.send(());
                        }
                    };
                    let cursor = delivery.cursor();
                    let read = async {
                        // Shadow reads can catch up on the next wake without
                        // contending with live appends for the history writer.
                        if !shadow {
                            history.flush_thread(root).await?;
                        }
                        cursor.read_history(history.as_ref()).await
                    };
                    let Some(records) = cancellation.run_until_cancelled(read).await else {
                        break;
                    };
                    let Ok(records) = records else {
                        tracing::warn!(%root, "Guardian history unavailable until the next wake");
                        continue;
                    };
                    let snapshot = cursor.snapshot(records).inspect_err(|error| {
                        failure_stage = "capture";
                        tracing::warn!(root_thread_id = %root, %error, "Guardian intake rejected stored evidence");
                    })?;
                    if !snapshot.source_advanced() {
                        finish();
                        continue;
                    }
                    for &source_ordinal in &snapshot.skipped_incomplete_deliveries {
                        tracing::warn!(
                            name: "guardian_v3.evidence_skipped",
                            root_thread_id = %root,
                            source_ordinal,
                            reason = "incomplete_delivered_message",
                            "Skipped incomplete Guardian evidence"
                        );
                    }
                    if snapshot.records().is_empty() {
                        delivery.acknowledge(&snapshot, /*count*/ 0)?;
                        finish();
                        continue;
                    }
                    let housekeeper = match &mut housekeeper {
                        Some(worker) => worker,
                        None => {
                            let startup = async {
                                let manager = manager.upgrade().context("Guardian host stopped")?;
                                let parent = manager.get_thread(root).await?;
                                let mut worker = Housekeeper::new(
                                    board.clone(),
                                    Arc::clone(&delivery),
                                    &parent,
                                    &manager,
                                    config.housekeeper.as_ref().unwrap_or(&Default::default()),
                                ).await?;
                                if shadow {
                                    worker.shadow_size = Some(BoardSize::new(config.shadow_board_limit_mib));
                                }
                                anyhow::Ok(worker)
                            };
                            let Some(worker) = cancellation.run_until_cancelled(startup).await else {
                                break;
                            };
                            housekeeper.insert(worker?)
                        }
                    };
                    // route_next drains accepted appends before honoring cancellation.
                    while housekeeper.route_next(&snapshot, &cancellation).await? != 0 {
                        if let (Some(size), Some(metrics)) = (housekeeper.shadow_size, &metrics) {
                            size.record(metrics);
                        }
                    }
                    finish();
                    tracing::debug!(
                        name: "guardian_v3.housekeeping_routed",
                        root_thread_id = %root,
                        end_ordinal_exclusive = snapshot.cutoff().end_ordinal_exclusive,
                        "Guardian housekeeper routed evidence"
                    );
                }
                anyhow::Ok(())
            };
            let result = work.await;
            drop(intake_guard);
            if let Err(error) = result
                && !cancellation.is_cancelled()
            {
                if let Some(metrics) = metrics {
                    metrics.stopped(if error.is::<PayloadLimit>() {
                        "payload_limit"
                    } else {
                        failure_stage
                    });
                }
                // Provider error messages can quote the evidence being routed.
                tracing::warn!(%root, "Guardian housekeeping stopped");
            }
        });
        thread.insert(RootWorker {
            shadow,
            wake,
            lifetime,
            reader,
        });
        Ok(())
    }
}

impl ThreadLifecycleContributor<Config> for Host {
    fn on_thread_start<'a>(
        &'a self,
        input: ThreadStartInput<'a, Config>,
    ) -> ExtensionFuture<'a, ()> {
        if !self.enabled(input.config) {
            return Box::pin(std::future::ready(()));
        }
        if !input.config.ephemeral
            && !input.session_source.is_internal()
            && input.thread_store.level_id() == input.session_store.level_id()
        {
            if self.live.is_none() {
                // Observe settings changes even before the root is ready.
                input.thread_store.insert(shadow::Enrollment::default());
                if let Some(metrics) = input.extension_metrics {
                    input.thread_store.insert(ShadowMetrics(metrics));
                }
            } else {
                // Only authoritative GV3 needs evidence writes to survive interruption.
                input.thread_store.insert(PreserveReviewWrites);
            }
        }
        Box::pin(std::future::ready(()))
    }

    fn on_thread_ready<'a>(
        &'a self,
        input: ThreadReadyInput<'a, Config>,
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            if input.session_source.is_internal()
                || input.config.ephemeral
                || !self.enabled(input.config)
            {
                return;
            }
            let (Ok(actor), Ok(root)) = (
                ThreadId::from_string(input.thread_store.level_id()),
                ThreadId::from_string(input.session_store.level_id()),
            ) else {
                return;
            };
            // Legacy spawned agents have no board identity and keep their native reviewer.
            let Some(path) = input
                .session_source
                .get_agent_path()
                .or_else(|| (actor == root).then(AgentPath::root))
            else {
                return;
            };
            let identity = shadow::Actor { root, actor, path };
            let Some(settings) = &self.live else {
                // Only the root supplies history. Children can inherit context and share its board.
                if actor == root
                    && (input.history_mode
                        != codex_protocol::protocol::ThreadHistoryMode::Paginated
                        || !matches!(input.history_initialization, HistoryInitialization::New))
                {
                    return;
                }
                input.thread_store.insert(identity);
                self.start_shadow_if_needed(input.thread_store);
                return;
            };
            let tree = self.tree_delivery(root);
            // The authoritative reviewer must stay attached even if intake fails.
            let reviewer = self.reviewer(
                &identity,
                &tree,
                &settings.retrieval,
                tree.stop.child_token(),
            );
            let attached: Arc<dyn codex_core::guardian_review::AttachedReviewer> = reviewer.clone();
            input.thread_store.insert_arc(Arc::clone(&reviewer));
            input.thread_store.insert(attached);
            if actor != root {
                return;
            }
            let start = IntakeStart::Initial(input.history_mode, input.history_initialization);
            if let Err(error) = self
                .activate(root, &tree, input.thread_store, start, settings)
                .await
            {
                tracing::warn!(%root, error = %error, "Guardian intake unavailable");
                return;
            }
            if routes_approval_policy_to_guardian(
                input.config.permissions.approval_policy.value(),
                input.config.approvals_reviewer,
            ) {
                reviewer.schedule_root_prewarm();
            }
        })
    }
    fn on_thread_stop<'a>(&'a self, input: ThreadStopInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            input.thread_store.remove::<shadow::Actor>();
            if let Some(enrollment) = input.thread_store.remove::<shadow::Enrollment>() {
                enrollment.stop(input.thread_store);
                enrollment.drain().await;
            }
            input.thread_store.remove::<ShadowReviewer>();
            if let Some(actor) = input.thread_store.remove::<ActorReviewer>() {
                actor.shutdown().await;
            }
            if let Some(worker) = input.thread_store.remove::<RootWorker>() {
                worker.lifetime.cancellation.cancel();
                worker.lifetime.tasks.close();
                worker.lifetime.tasks.wait().await;
            }
        })
    }
}

impl TurnLifecycleContributor for Host {
    fn on_turn_start<'a>(&'a self, input: TurnStartInput<'a>) -> ExtensionFuture<'a, ()> {
        self.start_shadow_if_needed(input.thread_store);
        Box::pin(std::future::ready(()))
    }

    fn on_item_completed<'a>(
        &'a self,
        thread: &'a ExtensionData,
        _turn: &'a ExtensionData,
        item: &'a TurnItem,
    ) -> ExtensionFuture<'a, ()> {
        if matches!(item, TurnItem::UserMessage(_)) {
            wake_worker(thread);
        }
        Box::pin(std::future::ready(()))
    }

    fn on_turn_stop<'a>(&'a self, input: TurnStopInput<'a>) -> ExtensionFuture<'a, ()> {
        wake_worker(input.thread_store);
        Box::pin(std::future::ready(()))
    }

    fn on_turn_abort<'a>(&'a self, input: TurnAbortInput<'a>) -> ExtensionFuture<'a, ()> {
        wake_worker(input.thread_store);
        Box::pin(std::future::ready(()))
    }
}

fn wake_worker(thread: &ExtensionData) {
    if let Some(worker) = thread.get::<RootWorker>() {
        let _ = worker.wake.try_send(None);
    }
}
