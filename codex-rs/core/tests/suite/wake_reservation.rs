//! A wake invalidated during context construction must not enter task startup.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use codex_config::CloudConfigBundleLoader;
use codex_config::LoaderOverrides;
use codex_config::loader::load_guardian_policy_snapshot;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_core::ConfigRefreshOutcome;
use codex_core::TurnInputRequest;
use codex_core::config::Config;
use codex_exec_server::LOCAL_FS;
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
use codex_protocol::models::PermissionProfile;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::InterAgentCommunication;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_successful_turn;
use pretty_assertions::assert_eq;
use tokio::sync::Notify;
use tokio::sync::TryLockError;

#[derive(Debug, Default)]
pub(super) struct Gate {
    pub(super) entered: Notify,
    pub(super) release: Notify,
}

#[derive(Debug)]
pub(super) struct GatedModelsManager {
    inner: StaticModelsManager,
    gate_next_lookup: AtomicBool,
    context_build: Gate,
}

impl GatedModelsManager {
    pub(super) fn new() -> anyhow::Result<Self> {
        Ok(Self::with_catalog(bundled_models_response()?))
    }

    pub(super) fn with_catalog(catalog: ModelsResponse) -> Self {
        Self {
            inner: StaticModelsManager::new(
                /*auth_manager*/ None,
                catalog,
                codex_models_manager::ModelOrigin::ExplicitConfig,
            ),
            gate_next_lookup: AtomicBool::new(false),
            context_build: Gate::default(),
        }
    }

    pub(super) fn gate_next_lookup(&self) -> &Gate {
        self.gate_next_lookup.store(true, Ordering::SeqCst);
        &self.context_build
    }
}

impl ModelsManager for GatedModelsManager {
    fn ensure_initialized(&self, factory: HttpClientFactory) -> ModelsManagerFuture<'_, ()> {
        self.inner.ensure_initialized(factory)
    }

    fn model_catalog_with_origins(
        &self,
        strategy: RefreshStrategy,
        factory: HttpClientFactory,
    ) -> ModelsManagerFuture<'_, Vec<codex_models_manager::CatalogModel>> {
        self.inner.model_catalog_with_origins(strategy, factory)
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
    ) -> ModelsManagerFuture<'a, codex_models_manager::CatalogModel> {
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
    let models = Arc::new(GatedModelsManager::new()?);
    let sleeping_turn = Arc::new(SleepingTurn::default());
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions.turn_lifecycle_contributor(sleeping_turn.clone());
    let test = test_codex()
        .with_models_manager(models.clone())
        .with_extensions(Arc::new(extensions.build()))
        .build_with_auto_env(&server)
        .await?;
    #[cfg(unix)]
    let git_root = test
        .codex
        .environment_selections()
        .await
        .iter()
        .any(|selection| selection.environment_id == codex_exec_server::LOCAL_ENVIRONMENT_ID)
        .then(|| test.config.cwd.as_path().to_path_buf());
    #[cfg(unix)]
    if let Some(root) = &git_root {
        super::git_enrichment::run_git(root, &["init", "-q", "-b", "main"])?;
        std::fs::write(root.join("tracked"), "initial\n")?;
        super::git_enrichment::run_git(root, &["add", "tracked"])?;
        super::git_enrichment::run_git(
            root,
            &[
                "-c",
                "user.name=Codex",
                "-c",
                "user.email=codex@example.invalid",
                "commit",
                "-qm",
                "initial",
            ],
        )?;
    }
    let codex = &test.codex;
    codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "wait for the worker".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    sleeping_turn.stop.entered.notified().await;
    #[cfg(unix)]
    if let Some(root) = &git_root {
        codex_git_utils::get_worktree_status_in_repo(root, root)
            .await
            .ok_or_else(|| anyhow::anyhow!("initial status should complete"))?;
        std::fs::create_dir_all(root.join(".git/info"))?;
        std::fs::write(root.join(".git/info/attributes"), "tracked filter=wake\n")?;
        super::git_enrichment::run_git(
            root,
            &[
                "config",
                "filter.wake.clean",
                "printf ran > .git/wake-status-ran; cat",
            ],
        )?;
        let tracked = root.join("tracked");
        std::fs::write(&tracked, "changed\n")?;
        std::fs::File::open(tracked)?
            .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1))?;
    }

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
    let context_build = models.gate_next_lookup();
    sleeping_turn.stop.release.notify_one();

    // Context construction happens after the discovery-time reservation check.
    // Process the interrupt before allowing that context construction to finish.
    context_build.entered.notified().await;
    codex.submit(Op::Interrupt).await?;
    codex.submit(Op::RealtimeConversationListVoices).await?;
    wait_for_event(codex, |event| {
        matches!(event, EventMsg::RealtimeConversationListVoicesResponse(_))
    })
    .await;
    context_build.release.notify_one();
    sleeping_turn.finished.notified().await;

    assert_eq!(sleeping_turn.started.load(Ordering::SeqCst), 1);
    #[cfg(unix)]
    if let Some(root) = &git_root {
        assert!(!root.join(".git/wake-status-ran").exists());
    }
    requests.single_request();
    codex.shutdown_and_wait().await?;
    Ok(())
}

#[tokio::test]
async fn guardian_policy_refresh_blocks_turns_and_resumes_queued_work() -> anyhow::Result<()> {
    let server = responses::start_mock_server().await;
    let test = test_codex().build_with_auto_env(&server).await?;
    let codex = &test.codex;
    let model = &test.session_configured.model;
    let conflict = CloudConfigBundleFixture::loader_with_enterprise_requirement(format!(
        "allowed_approvals_reviewers = ['user']\n[auto_review]\nrequired_on_models = [{model:?}]"
    ));
    let blocked = load_guardian_policy_snapshot(
        LOCAL_FS.as_ref(),
        test.config.codex_home.as_path(),
        &LoaderOverrides::without_managed_config_for_tests(),
        &conflict,
        /*revision*/ 1,
    )
    .await?;
    assert_eq!(
        codex.refresh_guardian_policy(&blocked).await,
        ConfigRefreshOutcome::Rejected
    );

    let requests =
        responses::mount_sse_once(&server, responses::sse_completed("resumed-turn")).await;
    codex
        .set_app_server_client_info(
            Some("test-app-server".to_owned()),
            /*app_server_client_version*/ None,
        )
        .await?;
    let error = test
        .submit_turn_with_approval_and_permission_profile(
            "Try a manual turn with the blocked model.",
            AskForApproval::OnRequest,
            PermissionProfile::workspace_write(),
        )
        .await
        .expect_err("a model that requires Guardian cannot run with a user-only reviewer");
    assert!(error.to_string().contains("you need to use auto review"));

    let queued_result = "Work queued under the blocked policy";
    codex
        .submit(Op::InterAgentCommunication {
            communication: InterAgentCommunication::new(
                AgentPath::try_from("/root/worker").expect("valid worker path"),
                AgentPath::root(),
                Vec::new(),
                queued_result.to_owned(),
                /*trigger_turn*/ true,
            ),
            start_options: Default::default(),
        })
        .await?;
    // This acknowledgement is processed after the blocked wake attempt.
    codex.submit(Op::RealtimeConversationListVoices).await?;
    wait_for_event(codex, |event| {
        matches!(event, EventMsg::RealtimeConversationListVoicesResponse(_))
    })
    .await;
    assert!(!codex.has_active_turn().await);
    assert!(requests.requests().is_empty());

    let allowed = load_guardian_policy_snapshot(
        LOCAL_FS.as_ref(),
        test.config.codex_home.as_path(),
        &LoaderOverrides::without_managed_config_for_tests(),
        &CloudConfigBundleLoader::default(),
        blocked.revision + 1,
    )
    .await?;
    assert_eq!(
        codex.refresh_guardian_policy(&allowed).await,
        ConfigRefreshOutcome::Published
    );
    wait_for_successful_turn(codex).await;
    assert!(
        requests
            .single_request()
            .inputs_of_type("agent_message")
            .iter()
            .any(|item| item["content"][0]["text"] == queued_result)
    );
    Ok(())
}
