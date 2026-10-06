//! A wake invalidated during context construction must not enter task startup.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use codex_core::TurnInputRequest;
use codex_core::config::Config;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::TurnLifecycleContributor;
use codex_extension_api::TurnStartInput;
use codex_extension_api::TurnStopInput;
use codex_extension_items::sleep::SleepItem;
use codex_http_client::HttpClientFactory;
use codex_login::AuthManager;
use codex_models_manager::ModelsManagerConfig;
use codex_models_manager::bundled_models_response;
use codex_models_manager::manager::ModelsManager;
use codex_models_manager::manager::ModelsManagerFuture;
use codex_models_manager::manager::RefreshStrategy;
use codex_models_manager::manager::StaticModelsManager;
use codex_protocol::AgentPath;
use codex_protocol::config_types::CollaborationModeMask;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::InterAgentCommunication;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use tokio::sync::Notify;
use tokio::sync::TryLockError;

#[derive(Debug, Default)]
struct Gate {
    entered: Notify,
    release: Notify,
}

#[derive(Debug)]
struct GatedModelsManager {
    inner: StaticModelsManager,
    gate_next_lookup: AtomicBool,
    context_build: Gate,
}

impl ModelsManager for GatedModelsManager {
    fn raw_model_catalog(
        &self,
        strategy: RefreshStrategy,
        factory: HttpClientFactory,
    ) -> ModelsManagerFuture<'_, ModelsResponse> {
        self.inner.raw_model_catalog(strategy, factory)
    }

    fn get_remote_models(&self) -> ModelsManagerFuture<'_, Vec<ModelInfo>> {
        self.inner.get_remote_models()
    }

    fn try_get_remote_models(&self) -> Result<Vec<ModelInfo>, TryLockError> {
        self.inner.try_get_remote_models()
    }

    fn auth_manager(&self) -> Option<&AuthManager> {
        self.inner.auth_manager()
    }

    fn list_collaboration_modes(&self) -> Vec<CollaborationModeMask> {
        self.inner.list_collaboration_modes()
    }

    fn refresh_if_new_etag(
        &self,
        etag: String,
        factory: HttpClientFactory,
    ) -> ModelsManagerFuture<'_, ()> {
        self.inner.refresh_if_new_etag(etag, factory)
    }

    fn get_model_info<'a>(
        &'a self,
        model: &'a str,
        config: &'a ModelsManagerConfig,
    ) -> ModelsManagerFuture<'a, ModelInfo> {
        Box::pin(async move {
            if self.gate_next_lookup.swap(/*val*/ false, Ordering::SeqCst) {
                self.context_build.entered.notify_one();
                self.context_build.release.notified().await;
            }
            self.inner.get_model_info(model, config).await
        })
    }
}

struct CompletionSignal(Arc<Notify>);

impl Drop for CompletionSignal {
    fn drop(&mut self) {
        self.0.notify_one();
    }
}

#[derive(Default)]
struct SleepingTurn {
    started: AtomicUsize,
    has_stopped: AtomicBool,
    stop: Gate,
    finished: Arc<Notify>,
}

impl TurnLifecycleContributor for SleepingTurn {
    fn on_turn_start<'a>(&'a self, _input: TurnStartInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            self.started.fetch_add(/*val*/ 1, Ordering::SeqCst);
        })
    }

    fn on_turn_stop<'a>(&'a self, input: TurnStopInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            if self.has_stopped.swap(/*val*/ true, Ordering::SeqCst) {
                return;
            }
            input.thread_store.insert(SleepItem {
                id: "sleep-before-mail".to_string(),
                duration_ms: 60_000,
            });
            // This turn's context survives through the completion-triggered wake.
            // Its drop is a barrier after the entire wake attempt has returned.
            input
                .turn_store
                .insert(CompletionSignal(Arc::clone(&self.finished)));
            self.stop.entered.notify_one();
            self.stop.release.notified().await;
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interrupt_during_wake_context_construction_does_not_start_turn() -> anyhow::Result<()> {
    let server = responses::start_mock_server().await;
    let requests = responses::mount_sse_once(&server, responses::sse_completed("initial")).await;
    let models = Arc::new(GatedModelsManager {
        inner: StaticModelsManager::new(/*auth_manager*/ None, bundled_models_response()?),
        gate_next_lookup: AtomicBool::new(/*v*/ false),
        context_build: Gate::default(),
    });
    let sleeping_turn = Arc::new(SleepingTurn::default());
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions.turn_lifecycle_contributor(sleeping_turn.clone());
    let test = test_codex()
        .with_models_manager(models.clone())
        .with_extensions(Arc::new(extensions.build()))
        .build_with_auto_env(&server)
        .await?;
    let codex = &test.codex;
    codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "wait for the worker".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    sleeping_turn.stop.entered.notified().await;

    // Queue mail while the completed turn still owns the reservation, so its
    // completion starts the wake independently of the submission loop.
    codex
        .submit(Op::InterAgentCommunication {
            communication: InterAgentCommunication::new(
                AgentPath::try_from("/root/worker").expect("valid worker path"),
                AgentPath::root(),
                Vec::new(),
                "worker completed".to_string(),
                /*trigger_turn*/ false,
            ),
            start_options: Default::default(),
        })
        .await?;
    codex.submit(Op::RealtimeConversationListVoices).await?;
    wait_for_event(codex, |event| {
        matches!(event, EventMsg::RealtimeConversationListVoicesResponse(_))
    })
    .await;
    models
        .gate_next_lookup
        .store(/*val*/ true, Ordering::SeqCst);
    sleeping_turn.stop.release.notify_one();

    // Context construction happens after the discovery-time reservation check.
    // Process the interrupt before allowing that context construction to finish.
    models.context_build.entered.notified().await;
    codex.submit(Op::Interrupt).await?;
    codex.submit(Op::RealtimeConversationListVoices).await?;
    wait_for_event(codex, |event| {
        matches!(event, EventMsg::RealtimeConversationListVoicesResponse(_))
    })
    .await;
    models.context_build.release.notify_one();
    sleeping_turn.finished.notified().await;

    assert_eq!(sleeping_turn.started.load(Ordering::SeqCst), 1);
    requests.single_request();
    codex.shutdown_and_wait().await?;
    Ok(())
}
