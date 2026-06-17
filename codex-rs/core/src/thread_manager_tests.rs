use super::*;
use crate::config::test_config;
use crate::init_state_db;
use crate::installation_id::INSTALLATION_ID_FILENAME;
use crate::rollout::RolloutRecorder;
use crate::session::session::SessionSettingsUpdate;
use crate::session::tests::make_session_and_context;
use crate::tasks::InterruptedTurnHistoryMarker;
use crate::tasks::interrupted_turn_history_marker;
use codex_extension_api::empty_extension_registry;
use codex_models_manager::manager::RefreshStrategy;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ReasoningItemReasoningSummary;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::protocol::AgentMessageEvent;
use codex_protocol::protocol::InitialHistory;
use codex_protocol::protocol::InternalSessionSource;
use codex_protocol::protocol::ResumedHistory;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::ThreadSource;
use codex_protocol::protocol::TurnStartedEvent;
use codex_protocol::protocol::UserMessageEvent;
use codex_utils_path_uri::PathUri;
use core_test_support::PathBufExt;
use core_test_support::PathExt;
use core_test_support::responses::mount_models_once;
use pretty_assertions::assert_eq;
use std::time::Duration;
use tempfile::tempdir;
use wiremock::MockServer;

const TEST_INSTALLATION_ID: &str = "11111111-1111-4111-8111-111111111111";

fn user_msg(text: &str) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::OutputText {
            text: text.to_string(),
        }],
        phase: None,
        metadata: None,
    }
}
fn assistant_msg(text: &str) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: "assistant".to_string(),
        content: vec![ContentItem::OutputText {
            text: text.to_string(),
        }],
        phase: None,
        metadata: None,
    }
}

fn contextual_user_interrupted_marker() -> ResponseItem {
    interrupted_turn_history_marker(InterruptedTurnHistoryMarker::ContextualUser)
        .expect("contextual-user interrupted marker should be enabled")
}

fn developer_interrupted_marker() -> ResponseItem {
    interrupted_turn_history_marker(InterruptedTurnHistoryMarker::Developer)
        .expect("developer interrupted marker should be enabled")
}

#[test]
fn truncates_before_requested_user_message() {
    let items = [
        user_msg("u1"),
        assistant_msg("a1"),
        assistant_msg("a2"),
        user_msg("u2"),
        assistant_msg("a3"),
        ResponseItem::Reasoning {
            id: "r1".to_string(),
            summary: vec![ReasoningItemReasoningSummary::SummaryText {
                text: "s".to_string(),
            }],
            content: None,
            encrypted_content: None,
            metadata: None,
        },
        ResponseItem::FunctionCall {
            id: None,
            call_id: "c1".to_string(),
            name: "tool".to_string(),
            namespace: None,
            arguments: "{}".to_string(),
            metadata: None,
        },
        assistant_msg("a4"),
    ];

    let initial: Vec<RolloutItem> = items
        .iter()
        .cloned()
        .map(RolloutItem::ResponseItem)
        .collect();
    let truncated = truncate_before_nth_user_message(
        InitialHistory::Forked(initial),
        /*n*/ 1,
        &SnapshotTurnState {
            ends_mid_turn: false,
            active_turn_id: None,
            active_turn_start_index: None,
        },
    );
    let got_items = truncated.get_rollout_items();
    let expected_items = vec![
        RolloutItem::ResponseItem(items[0].clone()),
        RolloutItem::ResponseItem(items[1].clone()),
        RolloutItem::ResponseItem(items[2].clone()),
    ];
    assert_eq!(
        serde_json::to_value(&got_items).unwrap(),
        serde_json::to_value(&expected_items).unwrap()
    );

    let initial2: Vec<RolloutItem> = items
        .iter()
        .cloned()
        .map(RolloutItem::ResponseItem)
        .collect();
    let truncated2 = truncate_before_nth_user_message(
        InitialHistory::Forked(initial2.clone()),
        /*n*/ 2,
        &SnapshotTurnState {
            ends_mid_turn: false,
            active_turn_id: None,
            active_turn_start_index: None,
        },
    );
    assert_eq!(
        serde_json::to_value(truncated2.get_rollout_items()).unwrap(),
        serde_json::to_value(initial2).unwrap()
    );
}

#[test]
fn out_of_range_truncation_drops_only_unfinished_suffix_mid_turn() {
    let items = vec![
        RolloutItem::ResponseItem(user_msg("u1")),
        RolloutItem::ResponseItem(assistant_msg("a1")),
        RolloutItem::ResponseItem(user_msg("u2")),
        RolloutItem::ResponseItem(assistant_msg("partial")),
    ];

    let truncated = truncate_before_nth_user_message(
        InitialHistory::Forked(items.clone()),
        usize::MAX,
        &SnapshotTurnState {
            ends_mid_turn: true,
            active_turn_id: None,
            active_turn_start_index: None,
        },
    );

    assert_eq!(
        serde_json::to_value(truncated.get_rollout_items()).unwrap(),
        serde_json::to_value(items[..2].to_vec()).unwrap()
    );
}

#[test]
fn fork_thread_accepts_legacy_usize_snapshot_argument() {
    fn assert_legacy_snapshot_callsite(
        manager: &ThreadManager,
        config: Config,
        path: std::path::PathBuf,
    ) {
        let _future = manager.fork_thread(
            usize::MAX,
            config,
            path,
            /*thread_source*/ None,
            /*parent_trace*/ None,
        );
    }

    let _: fn(&ThreadManager, Config, std::path::PathBuf) = assert_legacy_snapshot_callsite;
}

#[test]
fn out_of_range_truncation_drops_pre_user_active_turn_prefix() {
    let items = vec![
        RolloutItem::ResponseItem(user_msg("u1")),
        RolloutItem::ResponseItem(assistant_msg("a1")),
        RolloutItem::EventMsg(EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: "turn-2".to_string(),
            trace_id: None,
            started_at: None,
            model_context_window: None,
            collaboration_mode_kind: Default::default(),
        })),
        RolloutItem::ResponseItem(user_msg("u2")),
        RolloutItem::ResponseItem(assistant_msg("partial")),
    ];

    let snapshot_state = snapshot_turn_state(&InitialHistory::Forked(items.clone()));
    assert_eq!(
        snapshot_state,
        SnapshotTurnState {
            ends_mid_turn: true,
            active_turn_id: Some("turn-2".to_string()),
            active_turn_start_index: Some(2),
        },
    );

    let truncated = truncate_before_nth_user_message(
        InitialHistory::Forked(items.clone()),
        usize::MAX,
        &snapshot_state,
    );

    assert_eq!(
        serde_json::to_value(truncated.get_rollout_items()).unwrap(),
        serde_json::to_value(items[..2].to_vec()).unwrap()
    );
}

#[tokio::test]
async fn ignores_session_prefix_messages_when_truncating() {
    let (session, turn_context) = make_session_and_context().await;
    let mut items = session.build_initial_context(&turn_context).await;
    items.push(user_msg("feature request"));
    items.push(assistant_msg("ack"));
    items.push(user_msg("second question"));
    items.push(assistant_msg("answer"));

    let rollout_items: Vec<RolloutItem> = items
        .iter()
        .cloned()
        .map(RolloutItem::ResponseItem)
        .collect();

    let truncated = truncate_before_nth_user_message(
        InitialHistory::Forked(rollout_items),
        /*n*/ 1,
        &SnapshotTurnState {
            ends_mid_turn: false,
            active_turn_id: None,
            active_turn_start_index: None,
        },
    );
    let got_items = truncated.get_rollout_items();

    let expected: Vec<RolloutItem> = vec![
        RolloutItem::ResponseItem(items[0].clone()),
        RolloutItem::ResponseItem(items[1].clone()),
        RolloutItem::ResponseItem(items[2].clone()),
        RolloutItem::ResponseItem(items[3].clone()),
    ];

    assert_eq!(
        serde_json::to_value(&got_items).unwrap(),
        serde_json::to_value(&expected).unwrap()
    );
}

#[tokio::test]
async fn shutdown_all_threads_bounded_submits_shutdown_to_every_thread() {
    let temp_dir = tempdir().expect("tempdir");
    let mut config = test_config().await;
    config.codex_home = temp_dir.path().join("codex-home").abs();
    config.cwd = config.codex_home.abs();
    std::fs::create_dir_all(&config.codex_home).expect("create codex home");

    let manager = ThreadManager::with_models_provider_and_home_for_tests(
        CodexAuth::from_api_key("dummy"),
        config.model_provider.clone(),
        config.codex_home.to_path_buf(),
        Arc::new(codex_exec_server::EnvironmentManager::default_for_tests()),
    );
    let thread_1 = manager
        .start_thread(config.clone())
        .await
        .expect("start first thread")
        .thread_id;
    let thread_2 = manager
        .start_thread(config.clone())
        .await
        .expect("start second thread")
        .thread_id;

    let report = manager
        .shutdown_all_threads_bounded(Duration::from_secs(10))
        .await;

    let mut expected_completed = vec![thread_1, thread_2];
    expected_completed.sort_by_key(std::string::ToString::to_string);
    assert_eq!(report.completed, expected_completed);
    assert!(report.submit_failed.is_empty());
    assert!(report.timed_out.is_empty());
    assert!(manager.list_thread_ids().await.is_empty());
}

#[tokio::test]
async fn start_thread_keeps_internal_threads_hidden_from_normal_lookups() {
    let temp_dir = tempdir().expect("tempdir");
    let mut config = test_config().await;
    config.codex_home = temp_dir.path().join("codex-home").abs();
    config.cwd = config.codex_home.abs();
    std::fs::create_dir_all(&config.codex_home).expect("create codex home");

    let manager = ThreadManager::with_models_provider_and_home_for_tests(
        CodexAuth::from_api_key("dummy"),
        config.model_provider.clone(),
        config.codex_home.to_path_buf(),
        Arc::new(codex_exec_server::EnvironmentManager::default_for_tests()),
    );
    let thread = manager
        .start_thread_with_options(StartThreadOptions {
            config,
            initial_history: InitialHistory::New,
            session_source: Some(SessionSource::Internal(
                InternalSessionSource::MemoryConsolidation,
            )),
            thread_source: None,
            dynamic_tools: Vec::new(),
            metrics_service_name: None,
            parent_trace: None,
            environments: Vec::new(),
            thread_extension_init: Default::default(),
        })
        .await
        .expect("internal thread should start");

    assert_eq!(manager.list_thread_ids().await, Vec::new());
    assert!(manager.get_thread(thread.thread_id).await.is_err());

    let report = manager
        .shutdown_all_threads_bounded(Duration::from_secs(10))
        .await;
    assert_eq!(report.completed, vec![thread.thread_id]);
    assert!(report.submit_failed.is_empty());
    assert!(report.timed_out.is_empty());
    assert!(manager.list_thread_ids().await.is_empty());
}

#[tokio::test]
async fn start_thread_seeds_extension_data_for_mcp_and_lifecycle_contributors() {
    struct InitialDataRecorder {
        lifecycle_observed: Arc<std::sync::Mutex<Vec<(String, String)>>>,
        mcp_observed: Arc<std::sync::Mutex<Vec<String>>>,
    }

    impl codex_extension_api::ThreadLifecycleContributor<Config> for InitialDataRecorder {
        fn on_thread_start<'a>(
            &'a self,
            input: codex_extension_api::ThreadStartInput<'a, Config>,
        ) -> codex_extension_api::ExtensionFuture<'a, ()> {
            Box::pin(async move {
                let selected_root = input
                    .thread_store
                    .get::<Vec<SelectedCapabilityRoot>>()
                    .and_then(|roots| roots.first().cloned())
                    .expect("selected root should be available");
                self.lifecycle_observed
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push((input.thread_store.level_id().to_string(), selected_root.id));
                input
                    .thread_store
                    .insert(Vec::<SelectedCapabilityRoot>::new());
            })
        }
    }

    impl codex_extension_api::McpServerContributor<Config> for InitialDataRecorder {
        fn id(&self) -> &'static str {
            "selected_root_test"
        }

        fn contribute<'a>(
            &'a self,
            context: codex_extension_api::McpServerContributionContext<'a, Config>,
        ) -> codex_extension_api::ExtensionFuture<'a, Vec<codex_extension_api::McpServerContribution>>
        {
            Box::pin(async move {
                let thread_init = context
                    .thread_init()
                    .expect("initial MCP resolution should be thread-scoped");
                let selected_root = thread_init
                    .get::<Vec<SelectedCapabilityRoot>>()
                    .and_then(|roots| roots.first().cloned())
                    .expect("selected root should be available");
                self.mcp_observed
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push(selected_root.id.clone());
                let mut server = codex_mcp::codex_apps_mcp_server_config(
                    "https://selected.invalid",
                    /*apps_mcp_product_sku*/ None,
                );
                let CapabilityRootLocation::Environment { environment_id, .. } =
                    &selected_root.location;
                server.environment_id = environment_id.clone();
                server.enabled = false;
                let plugin_id = selected_root.id;
                vec![codex_extension_api::McpServerContribution::SelectedPlugin {
                    name: plugin_id.clone(),
                    plugin_display_name: plugin_id.clone(),
                    plugin_id,
                    selection_order: 0,
                    config: Box::new(server),
                }]
            })
        }
    }

    let temp_dir = tempdir().expect("tempdir");
    let mut config = test_config().await;
    config.codex_home = temp_dir.path().join("codex-home").abs();
    config.cwd = config.codex_home.abs();
    std::fs::create_dir_all(&config.codex_home).expect("create codex home");

    let lifecycle_observed = Arc::new(std::sync::Mutex::new(Vec::new()));
    let mcp_observed = Arc::new(std::sync::Mutex::new(Vec::new()));
    let recorder = Arc::new(InitialDataRecorder {
        lifecycle_observed: Arc::clone(&lifecycle_observed),
        mcp_observed: Arc::clone(&mcp_observed),
    });
    let mut extensions = codex_extension_api::ExtensionRegistryBuilder::new();
    extensions.thread_lifecycle_contributor(recorder.clone());
    extensions.mcp_server_contributor(recorder);
    let manager = ThreadManager::new(
        &config,
        AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing()),
        SessionSource::Exec,
        Arc::new(codex_exec_server::EnvironmentManager::default_for_tests()),
        Arc::new(extensions.build()),
        Arc::new(crate::test_support::EmptyUserInstructionsProvider),
        /*analytics_events_client*/ None,
        thread_store_from_config(&config, /*state_db*/ None),
        /*state_db*/ None,
        TEST_INSTALLATION_ID.to_string(),
        /*attestation_provider*/ None,
    );
    let selected_root_init = |id: &str, environment_id: &str| {
        let mut init = codex_extension_api::ExtensionDataInit::new();
        init.insert(vec![SelectedCapabilityRoot {
            id: id.to_string(),
            location: CapabilityRootLocation::Environment {
                environment_id: environment_id.to_string(),
                path: format!("/plugins/{id}"),
            },
        }]);
        init
    };

    let first_thread = manager
        .start_thread_with_options(StartThreadOptions {
            config: config.clone(),
            initial_history: InitialHistory::New,
            session_source: None,
            thread_source: None,
            dynamic_tools: Vec::new(),
            metrics_service_name: None,
            parent_trace: None,
            environments: Vec::new(),
            thread_extension_init: selected_root_init("selected-a", "env-a"),
        })
        .await
        .expect("start first thread");
    let second_thread = manager
        .start_thread_with_options(StartThreadOptions {
            config: config.clone(),
            initial_history: InitialHistory::New,
            session_source: None,
            thread_source: None,
            dynamic_tools: Vec::new(),
            metrics_service_name: None,
            parent_trace: None,
            environments: Vec::new(),
            thread_extension_init: selected_root_init("selected-b", "env-b"),
        })
        .await
        .expect("start second thread");
    let first_resolved = first_thread.thread.runtime_mcp_config(&config).await;
    let second_resolved = second_thread.thread.runtime_mcp_config(&config).await;

    assert_eq!(
        *lifecycle_observed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        vec![
            (first_thread.thread_id.to_string(), "selected-a".to_string()),
            (
                second_thread.thread_id.to_string(),
                "selected-b".to_string()
            ),
        ]
    );
    assert_eq!(
        *mcp_observed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        vec![
            "selected-a".to_string(),
            "selected-b".to_string(),
            "selected-a".to_string(),
            "selected-b".to_string(),
        ]
    );
    let selected_servers = |config: &codex_mcp::McpConfig| {
        codex_mcp::configured_mcp_servers(config)
            .into_iter()
            .filter(|(name, _)| name.starts_with("selected-"))
            .map(|(name, server)| (name, server.environment_id))
            .collect::<std::collections::BTreeMap<_, _>>()
    };
    assert_eq!(
        selected_servers(&first_resolved),
        std::collections::BTreeMap::from([("selected-a".to_string(), "env-a".to_string())])
    );
    assert_eq!(
        selected_servers(&second_resolved),
        std::collections::BTreeMap::from([("selected-b".to_string(), "env-b".to_string())])
    );
}

#[tokio::test]
async fn resume_and_fork_do_not_restore_thread_environments_from_rollout() {
    let temp_dir = tempdir().expect("tempdir");
    let mut config = test_config().await;
    config.codex_home = temp_dir.path().join("codex-home").abs();
    config.cwd = config.codex_home.abs();
    std::fs::create_dir_all(&config.codex_home).expect("create codex home");

    let auth_manager =
        AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    let manager = ThreadManager::new(
        &config,
        auth_manager.clone(),
        SessionSource::Exec,
        Arc::new(codex_exec_server::EnvironmentManager::default_for_tests()),
        empty_extension_registry(),
        Arc::new(crate::test_support::EmptyUserInstructionsProvider),
        /*analytics_events_client*/ None,
        thread_store_from_config(&config, /*state_db*/ None),
        /*state_db*/ None,
        TEST_INSTALLATION_ID.to_string(),
        /*attestation_provider*/ None,
    );
    let selected_cwd =
        AbsolutePathBuf::try_from(config.cwd.as_path().join("selected")).expect("absolute path");
    std::fs::create_dir_all(&selected_cwd).expect("create selected cwd");
    let environments = vec![TurnEnvironmentSelection {
        environment_id: "local".to_string(),
        cwd: PathUri::from_abs_path(&selected_cwd),
    }];
    let default_cwd = config.cwd.clone();
    let mut source_config = config.clone();
    source_config.cwd = selected_cwd.clone();
    let source = manager
        .start_thread_with_options(StartThreadOptions {
            config: source_config,
            initial_history: InitialHistory::New,
            session_source: None,
            thread_source: None,
            dynamic_tools: Vec::new(),
            metrics_service_name: None,
            parent_trace: None,
            environments: environments.clone(),
            thread_extension_init: Default::default(),
        })
        .await
        .expect("start source thread");
    source.thread.ensure_rollout_materialized().await;
    source
        .thread
        .flush_rollout()
        .await
        .expect("flush source rollout");
    let rollout_path = source
        .thread
        .rollout_path()
        .expect("source rollout path should exist");
    source
        .thread
        .shutdown_and_wait()
        .await
        .expect("shutdown source thread before resume");
    let _ = manager.remove_thread(&source.thread_id).await;

    let resumed = manager
        .resume_thread_from_rollout(
            config.clone(),
            rollout_path.clone(),
            auth_manager,
            /*parent_trace*/ None,
        )
        .await
        .expect("resume source thread");
    let resumed_turn = resumed
        .thread
        .codex
        .session
        .new_turn_with_sub_id("resume-turn".to_string(), SessionSettingsUpdate::default())
        .await
        .expect("build resumed turn context");
    assert_eq!(resumed_turn.environments.turn_environments.len(), 1);
    assert_eq!(
        resumed_turn.environments.turn_environments[0].cwd(),
        &PathUri::from_abs_path(&default_cwd)
    );
    assert_ne!(
        resumed_turn.environments.turn_environments[0].cwd(),
        &PathUri::from_abs_path(&selected_cwd)
    );

    let forked = manager
        .fork_thread(
            ForkSnapshot::Interrupted,
            config,
            rollout_path,
            /*thread_source*/ None,
            /*parent_trace*/ None,
        )
        .await
        .expect("fork source thread");
    let forked_turn = forked
        .thread
        .codex
        .session
        .new_turn_with_sub_id("fork-turn".to_string(), SessionSettingsUpdate::default())
        .await
        .expect("build forked turn context");
    assert_eq!(forked_turn.environments.turn_environments.len(), 1);
    assert_eq!(
        forked_turn.environments.turn_environments[0].cwd(),
        &PathUri::from_abs_path(&default_cwd)
    );
    assert_ne!(
        forked_turn.environments.turn_environments[0].cwd(),
        &PathUri::from_abs_path(&selected_cwd)
    );
}

#[tokio::test]
async fn explicit_installation_id_skips_codex_home_file() {
    let temp_dir = tempdir().expect("tempdir");
    let mut config = test_config().await;
    config.codex_home = temp_dir.path().join("codex-home").abs();
    config.cwd = config.codex_home.abs();
    std::fs::create_dir_all(&config.codex_home).expect("create codex home");

    let auth_manager =
        AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    let installation_id = uuid::Uuid::new_v4().to_string();
    let state_db = init_state_db(&config).await;
    let thread_store = thread_store_from_config(&config, state_db.clone());
    let manager = ThreadManager::new(
        &config,
        auth_manager,
        SessionSource::Exec,
        Arc::new(codex_exec_server::EnvironmentManager::default_for_tests()),
        empty_extension_registry(),
        Arc::new(crate::test_support::EmptyUserInstructionsProvider),
        /*analytics_events_client*/ None,
        thread_store,
        state_db.clone(),
        installation_id.clone(),
        /*attestation_provider*/ None,
    );

    let thread = manager
        .start_thread(config.clone())
        .await
        .expect("start thread with explicit installation id");

    assert!(!config.codex_home.join(INSTALLATION_ID_FILENAME).exists());
    assert_eq!(thread.thread.codex.session.installation_id, installation_id);

    thread
        .thread
        .shutdown_and_wait()
        .await
        .expect("shutdown thread");
    let _ = manager.remove_thread(&thread.thread_id).await;
}

#[tokio::test]
async fn resume_active_thread_from_rollout_returns_running_thread() {
    let temp_dir = tempdir().expect("tempdir");
    let mut config = test_config().await;
    config.codex_home = temp_dir.path().join("codex-home").abs();
    config.cwd = config.codex_home.abs();
    std::fs::create_dir_all(&config.codex_home).expect("create codex home");

    let auth_manager =
        AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    let manager = ThreadManager::new(
        &config,
        auth_manager.clone(),
        SessionSource::Exec,
        Arc::new(codex_exec_server::EnvironmentManager::default_for_tests()),
        empty_extension_registry(),
        Arc::new(crate::test_support::EmptyUserInstructionsProvider),
        /*analytics_events_client*/ None,
        thread_store_from_config(&config, /*state_db*/ None),
        /*state_db*/ None,
        TEST_INSTALLATION_ID.to_string(),
        /*attestation_provider*/ None,
    );

    let source = manager
        .start_thread(config.clone())
        .await
        .expect("start source thread");
    source.thread.ensure_rollout_materialized().await;
    source
        .thread
        .flush_rollout()
        .await
        .expect("flush source rollout");
    let rollout_path = source
        .thread
        .rollout_path()
        .expect("source rollout path should exist");

    let resumed = manager
        .resume_thread_from_rollout(
            config,
            rollout_path,
            auth_manager,
            /*parent_trace*/ None,
        )
        .await
        .expect("resume active source thread");
    assert_eq!(resumed.thread_id, source.thread_id);
    assert!(Arc::ptr_eq(&resumed.thread, &source.thread));

    source
        .thread
        .shutdown_and_wait()
        .await
        .expect("shutdown source thread");
}

#[tokio::test]
async fn resume_stopped_thread_from_rollout_spawns_new_thread() {
    let temp_dir = tempdir().expect("tempdir");
    let mut config = test_config().await;
    config.codex_home = temp_dir.path().join("codex-home").abs();
    config.cwd = config.codex_home.abs();
    std::fs::create_dir_all(&config.codex_home).expect("create codex home");

    let auth_manager =
        AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    let manager = ThreadManager::new(
        &config,
        auth_manager.clone(),
        SessionSource::Exec,
        Arc::new(codex_exec_server::EnvironmentManager::default_for_tests()),
        empty_extension_registry(),
        Arc::new(crate::test_support::EmptyUserInstructionsProvider),
        /*analytics_events_client*/ None,
        thread_store_from_config(&config, /*state_db*/ None),
        /*state_db*/ None,
        TEST_INSTALLATION_ID.to_string(),
        /*attestation_provider*/ None,
    );

    let source = manager
        .start_thread(config.clone())
        .await
        .expect("start source thread");
    source.thread.ensure_rollout_materialized().await;
    source
        .thread
        .flush_rollout()
        .await
        .expect("flush source rollout");
    let rollout_path = source
        .thread
        .rollout_path()
        .expect("source rollout path should exist");
    source
        .thread
        .shutdown_and_wait()
        .await
        .expect("shutdown source thread");

    let resumed = manager
        .resume_thread_from_rollout(
            config,
            rollout_path,
            auth_manager,
            /*parent_trace*/ None,
        )
        .await
        .expect("resume stopped source thread");
    assert_eq!(resumed.thread_id, source.thread_id);
    assert!(!Arc::ptr_eq(&resumed.thread, &source.thread));

    resumed
        .thread
        .shutdown_and_wait()
        .await
        .expect("shutdown resumed thread");
}

#[tokio::test]
async fn resume_stopped_thread_from_rollout_preserves_thread_source() {
    let temp_dir = tempdir().expect("tempdir");
    let mut config = test_config().await;
    config.codex_home = temp_dir.path().join("codex-home").abs();
    config.cwd = config.codex_home.abs();
    std::fs::create_dir_all(&config.codex_home).expect("create codex home");

    let auth_manager =
        AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    let state_db = init_state_db(&config).await;
    let thread_store = thread_store_from_config(&config, state_db.clone());
    let manager = ThreadManager::new(
        &config,
        auth_manager.clone(),
        SessionSource::Exec,
        Arc::new(codex_exec_server::EnvironmentManager::default_for_tests()),
        empty_extension_registry(),
        Arc::new(crate::test_support::EmptyUserInstructionsProvider),
        /*analytics_events_client*/ None,
        thread_store,
        state_db.clone(),
        TEST_INSTALLATION_ID.to_string(),
        /*attestation_provider*/ None,
    );

    let source = manager
        .start_thread_with_options(StartThreadOptions {
            config: config.clone(),
            initial_history: InitialHistory::New,
            session_source: None,
            thread_source: Some(ThreadSource::User),
            dynamic_tools: Vec::new(),
            metrics_service_name: None,
            parent_trace: None,
            environments: Vec::new(),
            thread_extension_init: Default::default(),
        })
        .await
        .expect("start source thread");
    source.thread.ensure_rollout_materialized().await;
    source
        .thread
        .flush_rollout()
        .await
        .expect("flush source rollout");
    let rollout_path = source
        .thread
        .rollout_path()
        .expect("source rollout path should exist");
    source
        .thread
        .shutdown_and_wait()
        .await
        .expect("shutdown source thread before resume");
    let _ = manager.remove_thread(&source.thread_id).await;

    let resumed = manager
        .resume_thread_from_rollout(
            config,
            rollout_path,
            auth_manager,
            /*parent_trace*/ None,
        )
        .await
        .expect("resume source thread");

    assert_eq!(
        resumed
            .thread
            .config_snapshot()
            .await
            .thread_source
            .as_ref(),
        Some(&ThreadSource::User)
    );

    resumed
        .thread
        .shutdown_and_wait()
        .await
        .expect("shutdown resumed thread");
}

#[tokio::test]
async fn rollout_path_resume_and_fork_read_history_through_thread_store() {
    let temp_dir = tempdir().expect("tempdir");
    let mut config = test_config().await;
    config.codex_home = temp_dir.path().join("codex-home").abs();
    config.cwd = config.codex_home.abs();
    config.experimental_thread_store = ThreadStoreConfig::InMemory {
        id: format!("thread-manager-{}", uuid::Uuid::new_v4()),
    };
    std::fs::create_dir_all(&config.codex_home).expect("create codex home");

    let auth_manager =
        AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    let state_db = init_state_db(&config).await;
    let thread_store = thread_store_from_config(&config, state_db.clone());
    let in_memory_store = thread_store
        .as_any()
        .downcast_ref::<InMemoryThreadStore>()
        .expect("configured in-memory store");
    let manager = ThreadManager::new(
        &config,
        auth_manager.clone(),
        SessionSource::Exec,
        Arc::new(codex_exec_server::EnvironmentManager::default_for_tests()),
        empty_extension_registry(),
        Arc::new(crate::test_support::EmptyUserInstructionsProvider),
        /*analytics_events_client*/ None,
        thread_store.clone(),
        state_db,
        TEST_INSTALLATION_ID.to_string(),
        /*attestation_provider*/ None,
    );

    let source = manager
        .start_thread(config.clone())
        .await
        .expect("start source thread");
    source
        .thread
        .shutdown_and_wait()
        .await
        .expect("shutdown source thread");
    let _ = manager.remove_thread(&source.thread_id).await;

    let rollout_path = config
        .codex_home
        .join("rollouts/source.jsonl")
        .to_path_buf();
    let resumed = manager
        .resume_thread_with_history(
            config.clone(),
            InitialHistory::Resumed(ResumedHistory {
                conversation_id: source.thread_id,
                history: vec![RolloutItem::ResponseItem(user_msg("hello"))],
                rollout_path: Some(rollout_path.clone()),
            }),
            auth_manager.clone(),
            /*parent_trace*/ None,
        )
        .await
        .expect("seed rollout path in store");
    resumed
        .thread
        .shutdown_and_wait()
        .await
        .expect("shutdown seeded resumed thread");
    let _ = manager.remove_thread(&resumed.thread_id).await;

    let resumed_from_path = manager
        .resume_thread_from_rollout(
            config.clone(),
            rollout_path.clone(),
            auth_manager,
            /*parent_trace*/ None,
        )
        .await
        .expect("resume from rollout path");
    assert_eq!(resumed_from_path.thread_id, resumed.thread_id);

    let forked = manager
        .fork_thread(
            ForkSnapshot::Interrupted,
            config,
            rollout_path,
            /*thread_source*/ None,
            /*parent_trace*/ None,
        )
        .await
        .expect("fork from rollout path");
    assert_ne!(forked.thread_id, resumed.thread_id);

    let calls = in_memory_store.calls().await;
    assert_eq!(calls.read_thread_by_rollout_path, 2);

    resumed_from_path
        .thread
        .shutdown_and_wait()
        .await
        .expect("shutdown path-resumed thread");
    forked
        .thread
        .shutdown_and_wait()
        .await
        .expect("shutdown forked thread");
}

#[tokio::test]
async fn new_uses_active_provider_for_model_refresh() {
    let server = MockServer::start().await;
    let models_mock = mount_models_once(&server, ModelsResponse { models: vec![] }).await;

    let temp_dir = tempdir().expect("tempdir");
    let mut config = test_config().await;
    config.codex_home = temp_dir.path().join("codex-home").abs();
    config.cwd = config.codex_home.abs();
    std::fs::create_dir_all(&config.codex_home).expect("create codex home");
    config.model_catalog = None;
    config.model_provider.base_url = Some(server.uri());

    let auth_manager =
        AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    let manager = ThreadManager::new(
        &config,
        auth_manager,
        SessionSource::Exec,
        Arc::new(codex_exec_server::EnvironmentManager::default_for_tests()),
        empty_extension_registry(),
        Arc::new(crate::test_support::EmptyUserInstructionsProvider),
        /*analytics_events_client*/ None,
        thread_store_from_config(&config, /*state_db*/ None),
        /*state_db*/ None,
        TEST_INSTALLATION_ID.to_string(),
        /*attestation_provider*/ None,
    );

    let _ = manager.list_models(RefreshStrategy::Online).await;
    assert_eq!(models_mock.requests().len(), 1);
}

#[test]
fn interrupted_fork_snapshot_appends_interrupt_boundary() {
    let committed_history =
        InitialHistory::Forked(vec![RolloutItem::ResponseItem(user_msg("hello"))]);

    assert_eq!(
        serde_json::to_value(
            append_interrupted_boundary(
                committed_history,
                /*turn_id*/ None,
                InterruptedTurnHistoryMarker::ContextualUser,
            )
            .get_rollout_items()
        )
        .expect("serialize interrupted fork history"),
        serde_json::to_value(vec![
            RolloutItem::ResponseItem(user_msg("hello")),
            RolloutItem::ResponseItem(contextual_user_interrupted_marker()),
            RolloutItem::EventMsg(EventMsg::TurnAborted(TurnAbortedEvent {
                turn_id: None,
                reason: TurnAbortReason::Interrupted,
                completed_at: None,
                duration_ms: None,
            })),
        ])
        .expect("serialize expected interrupted fork history"),
    );
    assert_eq!(
        serde_json::to_value(
            append_interrupted_boundary(
                InitialHistory::New,
                /*turn_id*/ None,
                InterruptedTurnHistoryMarker::ContextualUser,
            )
            .get_rollout_items()
        )
        .expect("serialize interrupted empty fork history"),
        serde_json::to_value(vec![
            RolloutItem::ResponseItem(contextual_user_interrupted_marker()),
            RolloutItem::EventMsg(EventMsg::TurnAborted(TurnAbortedEvent {
                turn_id: None,
                reason: TurnAbortReason::Interrupted,
                completed_at: None,
                duration_ms: None,
            })),
        ])
        .expect("serialize expected interrupted empty history"),
    );
}

#[test]
fn disabled_interrupted_fork_snapshot_appends_only_interrupt_event() {
    let committed_history =
        InitialHistory::Forked(vec![RolloutItem::ResponseItem(user_msg("hello"))]);

    assert_eq!(
        serde_json::to_value(
            append_interrupted_boundary(
                committed_history,
                /*turn_id*/ None,
                InterruptedTurnHistoryMarker::Disabled,
            )
            .get_rollout_items()
        )
        .expect("serialize disabled interrupted fork history"),
        serde_json::to_value(vec![
            RolloutItem::ResponseItem(user_msg("hello")),
            RolloutItem::EventMsg(EventMsg::TurnAborted(TurnAbortedEvent {
                turn_id: None,
                reason: TurnAbortReason::Interrupted,
                completed_at: None,
                duration_ms: None,
            })),
        ])
        .expect("serialize expected disabled interrupted fork history"),
    );
    assert_eq!(
        serde_json::to_value(
            append_interrupted_boundary(
                InitialHistory::New,
                /*turn_id*/ None,
                InterruptedTurnHistoryMarker::Disabled,
            )
            .get_rollout_items()
        )
        .expect("serialize disabled interrupted empty fork history"),
        serde_json::to_value(vec![RolloutItem::EventMsg(EventMsg::TurnAborted(
            TurnAbortedEvent {
                turn_id: None,
                reason: TurnAbortReason::Interrupted,
                completed_at: None,
                duration_ms: None,
            },
        ))])
        .expect("serialize expected disabled interrupted empty fork history"),
    );
}

#[test]
fn interrupted_snapshot_is_not_mid_turn() {
    let interrupted_history = InitialHistory::Forked(vec![
        RolloutItem::ResponseItem(user_msg("hello")),
        RolloutItem::ResponseItem(assistant_msg("partial")),
        RolloutItem::ResponseItem(contextual_user_interrupted_marker()),
        RolloutItem::EventMsg(EventMsg::TurnAborted(TurnAbortedEvent {
            turn_id: Some("turn-1".to_string()),
            reason: TurnAbortReason::Interrupted,
            completed_at: None,
            duration_ms: None,
        })),
    ]);

    assert_eq!(
        snapshot_turn_state(&interrupted_history),
        SnapshotTurnState {
            ends_mid_turn: false,
            active_turn_id: None,
            active_turn_start_index: None,
        },
    );
}

#[test]
fn multi_agent_v2_interrupted_marker_uses_developer_input_message() {
    let marker = developer_interrupted_marker();

    let ResponseItem::Message { role, content, .. } = marker else {
        panic!("expected interrupted marker to be a message");
    };
    assert_eq!(role, "developer");
    assert!(
        matches!(
            content.as_slice(),
            [ContentItem::InputText { text }]
                if text.contains(crate::context::TurnAborted::INTERRUPTED_DEVELOPER_GUIDANCE)
        ),
        "expected interrupted marker to use developer InputText content"
    );
}

#[test]
fn completed_legacy_event_history_is_not_mid_turn() {
    let completed_history = InitialHistory::Forked(vec![
        RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
            client_id: None,
            message: "hello".to_string(),
            images: None,
            text_elements: Vec::new(),
            local_images: Vec::new(),
            ..Default::default()
        })),
        RolloutItem::EventMsg(EventMsg::AgentMessage(AgentMessageEvent {
            message: "done".to_string(),
            phase: None,
            memory_citation: None,
        })),
    ]);

    assert_eq!(
        snapshot_turn_state(&completed_history),
        SnapshotTurnState {
            ends_mid_turn: false,
            active_turn_id: None,
            active_turn_start_index: None,
        },
    );
}

#[test]
fn mixed_response_and_legacy_user_event_history_is_mid_turn() {
    let mixed_history = InitialHistory::Forked(vec![
        RolloutItem::ResponseItem(user_msg("hello")),
        RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
            client_id: None,
            message: "hello".to_string(),
            images: None,
            text_elements: Vec::new(),
            local_images: Vec::new(),
            ..Default::default()
        })),
    ]);

    assert_eq!(
        snapshot_turn_state(&mixed_history),
        SnapshotTurnState {
            ends_mid_turn: true,
            active_turn_id: None,
            active_turn_start_index: None,
        },
    );
}

#[tokio::test]
async fn interrupted_fork_snapshot_does_not_synthesize_turn_id_for_legacy_history() {
    let temp_dir = tempdir().expect("tempdir");
    let mut config = test_config().await;
    config.codex_home = temp_dir.path().join("codex-home").abs();
    config.cwd = config.codex_home.abs();
    std::fs::create_dir_all(&config.codex_home).expect("create codex home");

    let auth_manager =
        AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    let state_db = init_state_db(&config).await;
    let manager = ThreadManager::new(
        &config,
        auth_manager.clone(),
        SessionSource::Exec,
        Arc::new(codex_exec_server::EnvironmentManager::default_for_tests()),
        empty_extension_registry(),
        Arc::new(crate::test_support::EmptyUserInstructionsProvider),
        /*analytics_events_client*/ None,
        thread_store_from_config(&config, state_db.clone()),
        state_db.clone(),
        TEST_INSTALLATION_ID.to_string(),
        /*attestation_provider*/ None,
    );

    let source = manager
        .resume_thread_with_history(
            config.clone(),
            InitialHistory::Forked(vec![
                RolloutItem::ResponseItem(user_msg("hello")),
                RolloutItem::ResponseItem(assistant_msg("partial")),
            ]),
            auth_manager,
            /*parent_trace*/ None,
        )
        .await
        .expect("create source thread from completed history");
    let source_path = source
        .thread
        .rollout_path()
        .expect("source rollout path should exist");
    let source_history = RolloutRecorder::get_rollout_history(&source_path)
        .await
        .expect("read source rollout history");
    let source_snapshot_state = snapshot_turn_state(&source_history);
    assert!(source_snapshot_state.ends_mid_turn);
    let expected_turn_id = source_snapshot_state.active_turn_id.clone();
    assert_eq!(expected_turn_id, None);

    let forked = manager
        .fork_thread(
            ForkSnapshot::Interrupted,
            config.clone(),
            source_path,
            /*thread_source*/ None,
            /*parent_trace*/ None,
        )
        .await
        .expect("fork interrupted snapshot");
    let forked_path = forked
        .thread
        .rollout_path()
        .expect("forked rollout path should exist");
    let history = RolloutRecorder::get_rollout_history(&forked_path)
        .await
        .expect("read forked rollout history");
    assert!(!snapshot_turn_state(&history).ends_mid_turn);
    let rollout_items: Vec<_> = history
        .get_rollout_items()
        .into_iter()
        .filter(|item| !matches!(item, RolloutItem::SessionMeta(_)))
        .collect();
    let interrupted_marker_json = serde_json::to_value(RolloutItem::ResponseItem(
        contextual_user_interrupted_marker(),
    ))
    .expect("serialize interrupted marker");
    let interrupted_abort_json = serde_json::to_value(RolloutItem::EventMsg(
        EventMsg::TurnAborted(TurnAbortedEvent {
            turn_id: expected_turn_id,
            reason: TurnAbortReason::Interrupted,
            completed_at: None,
            duration_ms: None,
        }),
    ))
    .expect("serialize interrupted abort event");
    assert_eq!(
        rollout_items
            .iter()
            .filter(|item| {
                serde_json::to_value(item).expect("serialize rollout item")
                    == interrupted_marker_json
            })
            .count(),
        1,
    );
    assert_eq!(
        rollout_items
            .iter()
            .filter(|item| {
                serde_json::to_value(item).expect("serialize rollout item")
                    == interrupted_abort_json
            })
            .count(),
        1,
    );
}

#[tokio::test]
async fn interrupted_fork_snapshot_preserves_explicit_turn_id() {
    let temp_dir = tempdir().expect("tempdir");
    let mut config = test_config().await;
    config.codex_home = temp_dir.path().join("codex-home").abs();
    config.cwd = config.codex_home.abs();
    std::fs::create_dir_all(&config.codex_home).expect("create codex home");

    let auth_manager =
        AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    let state_db = init_state_db(&config).await;
    let manager = ThreadManager::new(
        &config,
        auth_manager.clone(),
        SessionSource::Exec,
        Arc::new(codex_exec_server::EnvironmentManager::default_for_tests()),
        empty_extension_registry(),
        Arc::new(crate::test_support::EmptyUserInstructionsProvider),
        /*analytics_events_client*/ None,
        thread_store_from_config(&config, state_db.clone()),
        state_db.clone(),
        TEST_INSTALLATION_ID.to_string(),
        /*attestation_provider*/ None,
    );

    let source = manager
        .resume_thread_with_history(
            config.clone(),
            InitialHistory::Forked(vec![
                RolloutItem::EventMsg(EventMsg::TurnStarted(TurnStartedEvent {
                    turn_id: "turn-explicit".to_string(),
                    trace_id: None,
                    started_at: None,
                    model_context_window: None,
                    collaboration_mode_kind: Default::default(),
                })),
                RolloutItem::ResponseItem(user_msg("hello")),
                RolloutItem::ResponseItem(assistant_msg("partial")),
            ]),
            auth_manager,
            /*parent_trace*/ None,
        )
        .await
        .expect("create source thread from explicit partial history");
    let source_path = source
        .thread
        .rollout_path()
        .expect("source rollout path should exist");
    let source_history = RolloutRecorder::get_rollout_history(&source_path)
        .await
        .expect("read source rollout history");
    let source_snapshot_state = snapshot_turn_state(&source_history);
    assert_eq!(
        source_snapshot_state,
        SnapshotTurnState {
            ends_mid_turn: true,
            active_turn_id: Some("turn-explicit".to_string()),
            active_turn_start_index: Some(1),
        },
    );

    let forked = manager
        .fork_thread(
            ForkSnapshot::Interrupted,
            config.clone(),
            source_path,
            /*thread_source*/ None,
            /*parent_trace*/ None,
        )
        .await
        .expect("fork interrupted snapshot");
    let forked_path = forked
        .thread
        .rollout_path()
        .expect("forked rollout path should exist");
    let history = RolloutRecorder::get_rollout_history(&forked_path)
        .await
        .expect("read forked rollout history");
    let rollout_items: Vec<_> = history
        .get_rollout_items()
        .into_iter()
        .filter(|item| !matches!(item, RolloutItem::SessionMeta(_)))
        .collect();

    assert!(rollout_items.iter().any(|item| {
        matches!(
            item,
            RolloutItem::EventMsg(EventMsg::TurnAborted(TurnAbortedEvent {
                turn_id: Some(turn_id),
                reason: TurnAbortReason::Interrupted,
            completed_at: None,
            duration_ms: None,
            })) if turn_id == "turn-explicit"
        )
    }));
}

#[tokio::test]
async fn interrupted_fork_snapshot_uses_persisted_mid_turn_history_without_live_source() {
    let temp_dir = tempdir().expect("tempdir");
    let mut config = test_config().await;
    config.codex_home = temp_dir.path().join("codex-home").abs();
    config.cwd = config.codex_home.abs();
    std::fs::create_dir_all(&config.codex_home).expect("create codex home");

    let auth_manager =
        AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    let state_db = init_state_db(&config).await;
    let manager = ThreadManager::new(
        &config,
        auth_manager.clone(),
        SessionSource::Exec,
        Arc::new(codex_exec_server::EnvironmentManager::default_for_tests()),
        empty_extension_registry(),
        Arc::new(crate::test_support::EmptyUserInstructionsProvider),
        /*analytics_events_client*/ None,
        thread_store_from_config(&config, state_db.clone()),
        state_db.clone(),
        TEST_INSTALLATION_ID.to_string(),
        /*attestation_provider*/ None,
    );

    let source = manager
        .resume_thread_with_history(
            config.clone(),
            InitialHistory::Forked(vec![
                RolloutItem::ResponseItem(user_msg("hello")),
                RolloutItem::ResponseItem(assistant_msg("partial")),
            ]),
            auth_manager,
            /*parent_trace*/ None,
        )
        .await
        .expect("create source thread from partial history");
    let source_path = source
        .thread
        .rollout_path()
        .expect("source rollout path should exist");
    let source_history = RolloutRecorder::get_rollout_history(&source_path)
        .await
        .expect("read source rollout history");
    assert!(snapshot_turn_state(&source_history).ends_mid_turn);
    manager.remove_thread(&source.thread_id).await;

    let forked = manager
        .fork_thread(
            ForkSnapshot::Interrupted,
            config.clone(),
            source_path,
            /*thread_source*/ None,
            /*parent_trace*/ None,
        )
        .await
        .expect("fork interrupted snapshot");
    let forked_path = forked
        .thread
        .rollout_path()
        .expect("forked rollout path should exist");
    let history = RolloutRecorder::get_rollout_history(&forked_path)
        .await
        .expect("read forked rollout history");
    assert!(!snapshot_turn_state(&history).ends_mid_turn);

    let forked_rollout_items: Vec<_> = history
        .get_rollout_items()
        .into_iter()
        .filter(|item| !matches!(item, RolloutItem::SessionMeta(_)))
        .collect();
    let interrupted_marker_json = serde_json::to_value(RolloutItem::ResponseItem(
        contextual_user_interrupted_marker(),
    ))
    .expect("serialize interrupted marker");
    assert_eq!(
        forked_rollout_items
            .iter()
            .filter(|item| {
                serde_json::to_value(item).expect("serialize forked rollout item")
                    == interrupted_marker_json
            })
            .count(),
        1,
    );

    manager.remove_thread(&forked.thread_id).await;
    let reforked = manager
        .fork_thread(
            ForkSnapshot::Interrupted,
            config.clone(),
            forked_path,
            /*thread_source*/ None,
            /*parent_trace*/ None,
        )
        .await
        .expect("re-fork interrupted snapshot");
    let reforked_path = reforked
        .thread
        .rollout_path()
        .expect("re-forked rollout path should exist");
    let reforked_history = RolloutRecorder::get_rollout_history(&reforked_path)
        .await
        .expect("read re-forked rollout history");
    let reforked_rollout_items: Vec<_> = reforked_history
        .get_rollout_items()
        .into_iter()
        .filter(|item| !matches!(item, RolloutItem::SessionMeta(_)))
        .collect();

    assert_eq!(
        reforked_rollout_items
            .iter()
            .filter(|item| {
                serde_json::to_value(item).expect("serialize re-forked rollout item")
                    == interrupted_marker_json
            })
            .count(),
        1,
    );
    assert_eq!(
        reforked_rollout_items
            .iter()
            .filter(|item| {
                matches!(
                    item,
                    RolloutItem::EventMsg(EventMsg::TurnAborted(TurnAbortedEvent {
                        reason: TurnAbortReason::Interrupted,
                        ..
                    }))
                )
            })
            .count(),
        1,
    );
}
