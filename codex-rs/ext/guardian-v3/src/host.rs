//! A single worker routes durable originals from the live tree's last published position.
//! User-message and turn-end wakes coalesce. Root shutdown joins the worker before history closes.
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::PoisonError;
use std::sync::Weak;

use anyhow::Context;
use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_core::guardian_review::PendingReviewWrites;
use codex_core::guardian_review::ReviewRetrieval;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadReadyInput;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::ThreadStopInput;
use codex_extension_api::TurnAbortInput;
use codex_extension_api::TurnLifecycleContributor;
use codex_extension_api::TurnStopInput;
use codex_features::GuardianV3HousekeeperConfigToml;
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

use crate::ActorReviewer;
use crate::EvidenceCursor;
use crate::Housekeeper;
use crate::LiveDelivery;
use crate::local_board;
use crate::local_board_options;

#[cfg(test)]
#[path = "host_tests.rs"]
mod tests;

pub fn install(
    registry: &mut ExtensionRegistryBuilder<Config>,
    manager: Weak<ThreadManager>,
    config: &Config,
    board_host: impl Fn(ThreadId) -> Arc<dyn MessageBoardHost> + Send + Sync + 'static,
) {
    let settings = config.guardian_v3.as_deref();
    let host = Arc::new(Host {
        manager,
        deliveries: Mutex::default(),
        storage: if settings
            .and_then(|settings| settings.message_board_in_memory)
            .unwrap_or(false)
        {
            BoardStorage::Memory
        } else {
            BoardStorage::File(config.sqlite_config().clone())
        },
        housekeeper: Arc::new(
            settings
                .and_then(|settings| settings.housekeeper.clone())
                .unwrap_or_default(),
        ),
        retrieval: crate::board_retrieval(
            settings
                .and_then(|settings| settings.reviewer.as_ref())
                .and_then(|reviewer| reviewer.retrieval_instructions.as_deref()),
        ),
        board_host: Box::new(board_host),
    });
    registry.thread_lifecycle_contributor(host.clone());
    registry.turn_lifecycle_contributor(host);
}

enum BoardStorage {
    File(SqliteConfig),
    Memory,
}

// Shared by loaded actors, even before their first review.
// Only the root can initialize the cursor, using its own history.
pub(super) type TreeDelivery = OnceLock<Arc<LiveDelivery>>;

struct Host {
    manager: Weak<ThreadManager>,
    deliveries: Mutex<HashMap<ThreadId, Weak<TreeDelivery>>>,
    storage: BoardStorage,
    housekeeper: Arc<GuardianV3HousekeeperConfigToml>,
    retrieval: ReviewRetrieval,
    board_host: Box<dyn Fn(ThreadId) -> Arc<dyn MessageBoardHost> + Send + Sync>,
}

pub(super) struct RootWorker {
    pub(super) reader: Arc<dyn AgentMessageBoard>,
    wake: mpsc::Sender<()>,
    lifetime: ReviewerTasks,
}

impl Host {
    fn tree_delivery(&self, root: ThreadId) -> Arc<TreeDelivery> {
        let mut deliveries = self
            .deliveries
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(tree) = deliveries.get(&root).and_then(Weak::upgrade) {
            return tree;
        }
        deliveries.retain(|_, tree| tree.strong_count() > 0);
        let tree = Arc::new(TreeDelivery::new());
        deliveries.insert(root, Arc::downgrade(&tree));
        tree
    }

    async fn activate(
        &self,
        root: ThreadId,
        tree: &TreeDelivery,
        input: &ThreadReadyInput<'_, Config>,
    ) -> anyhow::Result<()> {
        // On resume the intake cursor starts at the tail. A new in-memory board
        // cannot serve earlier evidence, so leave its attached reviewer unavailable.
        if matches!(self.storage, BoardStorage::Memory) {
            anyhow::ensure!(
                matches!(
                    input.history_initialization,
                    HistoryInitialization::New | HistoryInitialization::Cleared
                ),
                "in-memory Guardian evidence requires a fresh root"
            );
        }
        let history = self
            .manager
            .upgrade()
            .context("Guardian host stopped")?
            .thread_store();
        let cursor = EvidenceCursor::at_thread_ready(
            history.as_ref(),
            root,
            input.history_mode,
            input.history_initialization,
        )
        .await?;
        let board_host = (self.board_host)(root);
        let board = match &self.storage {
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
        let reader: Arc<dyn AgentMessageBoard> = Arc::new(board.read_only_handle());
        let lifetime = ReviewerTasks::default();
        let (wake, mut wakes) = mpsc::channel(1);
        let initial_position = cursor.position();
        let delivery = Arc::clone(tree.get_or_init(|| Arc::new(LiveDelivery::from_cursor(cursor))));
        let saved = delivery.cursor();
        // A surviving child can keep a cursor behind the resumed root's history tail.
        if saved.position().thread_id == initial_position.thread_id
            && saved.position().end_ordinal_exclusive < initial_position.end_ordinal_exclusive
        {
            let _ = wake.try_send(());
        }
        let manager = self.manager.clone();
        let settings = Arc::clone(&self.housekeeper);
        let cancellation = lifetime.cancellation.clone();
        lifetime.tasks.spawn(async move {
            let work = async {
                let board = Arc::new(board);
                let mut housekeeper = None;
                while let Some(Some(())) = cancellation.run_until_cancelled(wakes.recv()).await {
                    let cursor = delivery.cursor();
                    let read = async {
                        history.flush_thread(root).await?;
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
                        tracing::warn!(root_thread_id = %root, %error, "Guardian intake rejected stored evidence");
                    })?;
                    if !snapshot.source_advanced() {
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
                        continue;
                    }
                    let housekeeper = match &mut housekeeper {
                        Some(worker) => worker,
                        None => {
                            let startup = async {
                                let manager = manager.upgrade().context("Guardian host stopped")?;
                                let parent = manager.get_thread(root).await?;
                                Housekeeper::new(
                                    board.clone(),
                                    Arc::clone(&delivery),
                                    &parent,
                                    &manager,
                                    &settings,
                                ).await
                            };
                            let Some(worker) = cancellation.run_until_cancelled(startup).await else {
                                break;
                            };
                            housekeeper.insert(worker?)
                        }
                    };
                    // route_next drains accepted appends before honoring cancellation.
                    while housekeeper.route_next(&snapshot, &cancellation).await? != 0 {}
                    tracing::debug!(
                        name: "guardian_v3.housekeeping_routed",
                        root_thread_id = %root,
                        end_ordinal_exclusive = snapshot.cutoff().end_ordinal_exclusive,
                        "Guardian housekeeper routed evidence"
                    );
                }
                anyhow::Ok(())
            };
            if work.await.is_err() && !cancellation.is_cancelled() {
                // Provider error messages can quote the evidence being routed.
                tracing::warn!(%root, "Guardian housekeeping stopped");
            }
        });
        input.thread_store.insert(RootWorker {
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
        // Track writes before the manager publishes the root to concurrent callers.
        if !input.config.ephemeral
            && !input.session_source.is_internal()
            && input.thread_store.level_id() == input.session_store.level_id()
        {
            input.thread_store.get_or_init(PendingReviewWrites::default);
        }
        Box::pin(std::future::ready(()))
    }

    fn on_thread_ready<'a>(
        &'a self,
        input: ThreadReadyInput<'a, Config>,
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            if input.session_source.is_internal() || input.config.ephemeral {
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
            let tree = self.tree_delivery(root);
            let actor_reviewer = Arc::new(ActorReviewer::new(
                self.manager.clone(),
                root,
                actor,
                path,
                self.retrieval.clone(),
                Arc::clone(&tree),
            ));
            input.thread_store.insert_arc(Arc::clone(&actor_reviewer));
            let attached: Arc<dyn codex_core::guardian_review::AttachedReviewer> =
                actor_reviewer.clone();
            input.thread_store.insert(attached);
            if actor != root {
                return;
            }
            if let Err(error) = self.activate(root, &tree, &input).await {
                tracing::warn!(%root, error = %error, "Guardian intake unavailable");
                return;
            }
            if routes_approval_policy_to_guardian(
                input.config.permissions.approval_policy.value(),
                input.config.approvals_reviewer,
            ) {
                actor_reviewer.schedule_root_prewarm();
            }
        })
    }
    fn on_thread_stop<'a>(&'a self, input: ThreadStopInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
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
        let _ = worker.wake.try_send(());
    }
}
