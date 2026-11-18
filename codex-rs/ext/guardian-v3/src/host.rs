//! A single worker owns a root's cursor and routes durable originals in order.
//! User-message and turn-end wakes coalesce. Root shutdown joins the worker before history closes.
use std::sync::Arc;
use std::sync::Weak;

use anyhow::Context;
use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_core::guardian_review::ReviewRetrieval;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadReadyInput;
use codex_extension_api::ThreadStopInput;
use codex_extension_api::TurnAbortInput;
use codex_extension_api::TurnLifecycleContributor;
use codex_extension_api::TurnStopInput;
use codex_features::GuardianV3HousekeeperConfigToml;
use codex_guardian_reviewer::ReviewerTasks;
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
use crate::cutoff::RootEvidence;
use crate::local_board;
use crate::local_board_options;

pub fn install(
    registry: &mut ExtensionRegistryBuilder<Config>,
    manager: Weak<ThreadManager>,
    config: &Config,
    board_host: impl Fn(ThreadId) -> Arc<dyn MessageBoardHost> + Send + Sync + 'static,
) {
    let settings = config.guardian_v3.as_deref();
    let host = Arc::new(Host {
        manager,
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

struct Host {
    manager: Weak<ThreadManager>,
    storage: BoardStorage,
    housekeeper: Arc<GuardianV3HousekeeperConfigToml>,
    retrieval: ReviewRetrieval,
    board_host: Box<dyn Fn(ThreadId) -> Arc<dyn MessageBoardHost> + Send + Sync>,
}

pub(super) struct RootWorker {
    pub(super) reader: Arc<dyn AgentMessageBoard>,
    wake: mpsc::Sender<()>,
    lifetime: Arc<ReviewerTasks>,
}

impl Host {
    async fn activate(
        &self,
        root: ThreadId,
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
        let lifetime = Arc::new(ReviewerTasks::default());
        let (wake, wakes) = mpsc::channel(1);
        let (queue, mut captures) = RootEvidence::new(lifetime.clone(), history, cursor.clone());
        lifetime
            .tasks
            .spawn(queue.clone().capture_on_activity(wakes));
        let worker = RootWorker {
            wake,
            lifetime: Arc::clone(&lifetime),
            reader,
        };
        let delivery = Arc::clone(&queue.delivery);
        let scope = lifetime.clone();
        let manager = self.manager.clone();
        let settings = Arc::clone(&self.housekeeper);
        lifetime.tasks.spawn(async move {
            let cancellation = &scope.cancellation;
            let work = async {
                let Some(mut capture) = cancellation
                    .run_until_cancelled(captures.recv())
                    .await
                    .flatten()
                else {
                    return Ok(());
                };
                let startup = async {
                    let manager = manager.upgrade().context("Guardian host stopped")?;
                    let parent = manager.get_thread(root).await?;
                    Housekeeper::new(
                        Arc::new(board),
                        cursor,
                        delivery,
                        &parent,
                        &manager,
                        &settings,
                    )
                    .await
                };
                let Some(housekeeper) = cancellation.run_until_cancelled(startup).await else {
                    return Ok(());
                };
                let mut housekeeper = housekeeper?;
                loop {
                    while housekeeper
                        .route_next(&capture.snapshot, cancellation)
                        .await?
                        != 0
                    {}
                    capture.finish();
                    let Some(next) = cancellation
                        .run_until_cancelled(captures.recv())
                        .await
                        .flatten()
                    else {
                        break;
                    };
                    capture = next;
                }
                anyhow::Ok(())
            };
            if work.await.is_err() && !cancellation.is_cancelled() {
                // Provider error messages can quote the evidence being routed.
                tracing::warn!(%root, "Guardian housekeeping stopped");
            }
        });
        input.thread_store.insert_arc(queue);
        input.thread_store.insert(worker);
        Ok(())
    }
}

impl ThreadLifecycleContributor<Config> for Host {
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
            let actor_reviewer = Arc::new(ActorReviewer::new(
                self.manager.clone(),
                root,
                actor,
                path,
                self.retrieval.clone(),
            ));
            input.thread_store.insert_arc(Arc::clone(&actor_reviewer));
            let attached: Arc<dyn codex_core::guardian_review::AttachedReviewer> = actor_reviewer;
            input.thread_store.insert(attached);
            if actor != root {
                return;
            }
            if let Err(error) = self.activate(root, &input).await {
                tracing::warn!(%root, error = %error, "Guardian intake unavailable");
            }
        })
    }
    fn on_thread_stop<'a>(&'a self, input: ThreadStopInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            if let Some(actor) = input.thread_store.remove::<ActorReviewer>() {
                actor.shutdown().await;
            }
            if let Some(worker) = input.thread_store.remove::<RootWorker>() {
                input.thread_store.remove::<RootEvidence>();
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
