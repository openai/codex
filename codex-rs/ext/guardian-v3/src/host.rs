//! A single worker owns a root's cursor and routes durable originals in order.
//! User-message and turn-end wakes coalesce. Root shutdown joins the worker before history closes.
use std::sync::Arc;
use std::sync::Weak;

use anyhow::Context;
use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadReadyInput;
use codex_extension_api::ThreadStopInput;
use codex_extension_api::TurnAbortInput;
use codex_extension_api::TurnLifecycleContributor;
use codex_extension_api::TurnStopInput;
use codex_guardian_reviewer::ReviewerTasks;
use codex_history::HistoryInitialization;
use codex_message_board::MessageBoardHost;
use codex_protocol::ThreadId;
use codex_protocol::items::TurnItem;
use codex_state::SqliteConfig;
use codex_thread_store::ThreadStoreError;
use tokio::sync::mpsc;

use crate::EvidenceCursor;
use crate::HOUSEKEEPER_INSTRUCTIONS;
use crate::Housekeeper;
use crate::local_board;
use crate::private_thread::PrivateThread;

pub fn install(
    registry: &mut ExtensionRegistryBuilder<Config>,
    manager: Weak<ThreadManager>,
    sqlite: SqliteConfig,
    board_host: impl Fn(ThreadId) -> Arc<dyn MessageBoardHost> + Send + Sync + 'static,
) {
    let host = Arc::new(Host {
        manager,
        sqlite,
        board_host: Box::new(board_host),
    });
    registry.thread_lifecycle_contributor(host.clone());
    registry.turn_lifecycle_contributor(host);
}

struct Host {
    manager: Weak<ThreadManager>,
    sqlite: SqliteConfig,
    board_host: Box<dyn Fn(ThreadId) -> Arc<dyn MessageBoardHost> + Send + Sync>,
}

struct RootWorker {
    wake: mpsc::Sender<()>,
    lifetime: Arc<ReviewerTasks>,
}

impl Host {
    async fn activate(
        &self,
        root: ThreadId,
        input: &ThreadReadyInput<'_, Config>,
    ) -> anyhow::Result<RootWorker> {
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
        let storage = local_board(self.sqlite.clone(), root.into());
        let board = storage.read_write().open((self.board_host)(root)).await?;
        let lifetime = Arc::new(ReviewerTasks::default());
        let (wake, mut requests) = mpsc::channel(1);
        let scope = lifetime.clone();
        let manager = self.manager.clone();
        let mut awaiting_first_write = matches!(
            input.history_initialization,
            HistoryInitialization::New | HistoryInitialization::Cleared
        );
        lifetime.tasks.spawn(async move {
            let cancellation = &scope.cancellation;
            let work = async {
                let startup = async {
                    requests.recv().await.context("Guardian worker closed")?;
                    let manager = manager.upgrade().context("Guardian host stopped")?;
                    let parent = manager.get_thread(root).await?;
                    PrivateThread::start(&manager, &parent, &scope, HOUSEKEEPER_INSTRUCTIONS).await
                };
                let Some(model) = cancellation.run_until_cancelled(startup).await else {
                    return Ok(());
                };
                let model = model?;
                let mut housekeeper = Housekeeper::new(Arc::new(board), cursor, model.policy)?;
                loop {
                    let Some(flushed) = cancellation.run_until_cancelled(history.flush_thread(root)).await else {
                        break;
                    };
                    flushed?;
                    let capture = housekeeper.cursor().capture(history.as_ref());
                    let Some(captured) = cancellation.run_until_cancelled(capture).await else {
                        break;
                    };
                    match captured {
                        Ok(snapshot) => {
                            awaiting_first_write = false;
                            while housekeeper
                                .route_next(&snapshot, &model.runtime, cancellation)
                                .await
                                .result? != 0
                            {}
                        }
                        // A wake can precede the first durable record. Keep the cursor until the next wake.
                        Err(error) if awaiting_first_write && matches!(error.downcast_ref::<ThreadStoreError>(),
                            Some(ThreadStoreError::ThreadNotFound { thread_id }) if *thread_id == root) => {}
                        Err(error) => return Err(error),
                    }
                    if cancellation.run_until_cancelled(requests.recv()).await.flatten().is_none() {
                        break;
                    }
                }
                anyhow::Ok(())
            };
            if work.await.is_err() && !cancellation.is_cancelled() {
                // Provider error messages can quote the evidence being routed.
                tracing::warn!(%root, "Guardian housekeeping stopped");
            }
        });
        Ok(RootWorker { wake, lifetime })
    }
}

impl ThreadLifecycleContributor<Config> for Host {
    fn on_thread_ready<'a>(
        &'a self,
        input: ThreadReadyInput<'a, Config>,
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            if input.session_source.is_internal()
                || input.config.ephemeral
                || input.thread_store.level_id() != input.session_store.level_id()
            {
                return;
            }
            let Ok(root) = ThreadId::from_string(input.thread_store.level_id()) else {
                return;
            };
            match self.activate(root, &input).await {
                Ok(worker) => {
                    input.thread_store.insert(worker);
                }
                Err(error) => tracing::warn!(%root, error = %error, "Guardian intake unavailable"),
            }
        })
    }
    fn on_thread_stop<'a>(&'a self, input: ThreadStopInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
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
