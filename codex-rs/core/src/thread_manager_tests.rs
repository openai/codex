use super::*;
use crate::config::test_config;
use crate::rollout::RolloutRecorder;
use crate::session::tests::make_session_and_context;
use crate::tasks::interrupted_turn_history_marker;
use codex_models_manager::collaboration_mode_presets::CollaborationModesConfig;
use codex_models_manager::manager::RefreshStrategy;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ReasoningItemReasoningSummary;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::protocol::AgentMessageEvent;
use codex_protocol::protocol::TurnStartedEvent;
use codex_protocol::protocol::UserMessageEvent;
use core_test_support::PathBufExt;
use core_test_support::PathExt;
use core_test_support::responses::mount_models_once;
use pretty_assertions::assert_eq;
use std::time::Duration;
use tempfile::tempdir;
use wiremock::MockServer;

fn user_msg(text: &str) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::OutputText {
            text: text.to_string(),
        }],
        end_turn: None,
        phase: None,
        exclude_from_compaction: false,
    }
}
fn assistant_msg(text: &str) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: "assistant".to_string(),
        content: vec![ContentItem::OutputText {
            text: text.to_string(),
        }],
        end_turn: None,
        phase: None,
        exclude_from_compaction: false,
    }
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
        },
        ResponseItem::FunctionCall {
            id: None,
            call_id: "c1".to_string(),
            name: "tool".to_string(),
            namespace: None,
            arguments: "{}".to_string(),
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
            /*persist_extended_history*/ false,
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
        .start_thread(config)
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
async fn new_uses_configured_openai_provider_for_model_refresh() {
    let server = MockServer::start().await;
    let models_mock = mount_models_once(&server, ModelsResponse { models: vec![] }).await;

    let temp_dir = tempdir().expect("tempdir");
    let mut config = test_config().await;
    config.codex_home = temp_dir.path().join("codex-home").abs();
    config.cwd = config.codex_home.abs();
    std::fs::create_dir_all(&config.codex_home).expect("create codex home");
    config.model_catalog = None;
    config
        .model_providers
        .get_mut("openai")
        .expect("openai provider should exist")
        .base_url = Some(server.uri());

    let auth_manager =
        AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    let manager = ThreadManager::new(
        &config,
        auth_manager,
        SessionSource::Exec,
        CollaborationModesConfig::default(),
        Arc::new(codex_exec_server::EnvironmentManager::default_for_tests()),
        /*analytics_events_client*/ None,
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
            append_interrupted_boundary(committed_history, /*turn_id*/ None).get_rollout_items()
        )
        .expect("serialize interrupted fork history"),
        serde_json::to_value(vec![
            RolloutItem::ResponseItem(user_msg("hello")),
            RolloutItem::ResponseItem(interrupted_turn_history_marker()),
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
            append_interrupted_boundary(InitialHistory::New, /*turn_id*/ None).get_rollout_items()
        )
        .expect("serialize interrupted empty fork history"),
        serde_json::to_value(vec![
            RolloutItem::ResponseItem(interrupted_turn_history_marker()),
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
fn interrupted_snapshot_is_not_mid_turn() {
    let interrupted_history = InitialHistory::Forked(vec![
        RolloutItem::ResponseItem(user_msg("hello")),
        RolloutItem::ResponseItem(assistant_msg("partial")),
        RolloutItem::ResponseItem(interrupted_turn_history_marker()),
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
fn completed_legacy_event_history_is_not_mid_turn() {
    let completed_history = InitialHistory::Forked(vec![
        RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
            message: "hello".to_string(),
            images: None,
            text_elements: Vec::new(),
            local_images: Vec::new(),
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
            message: "hello".to_string(),
            images: None,
            text_elements: Vec::new(),
            local_images: Vec::new(),
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
    let manager = ThreadManager::new(
        &config,
        auth_manager.clone(),
        SessionSource::Exec,
        CollaborationModesConfig::default(),
        Arc::new(codex_exec_server::EnvironmentManager::default_for_tests()),
        /*analytics_events_client*/ None,
    );

    let source = manager
        .resume_thread_with_history(
            config.clone(),
            InitialHistory::Forked(vec![
                RolloutItem::ResponseItem(user_msg("hello")),
                RolloutItem::ResponseItem(assistant_msg("partial")),
            ]),
            auth_manager,
            /*persist_extended_history*/ false,
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
            config,
            source_path,
            /*persist_extended_history*/ false,
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
    let interrupted_marker_json =
        serde_json::to_value(RolloutItem::ResponseItem(interrupted_turn_history_marker()))
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
    let manager = ThreadManager::new(
        &config,
        auth_manager.clone(),
        SessionSource::Exec,
        CollaborationModesConfig::default(),
        Arc::new(codex_exec_server::EnvironmentManager::default_for_tests()),
        /*analytics_events_client*/ None,
    );

    let source = manager
        .resume_thread_with_history(
            config.clone(),
            InitialHistory::Forked(vec![
                RolloutItem::EventMsg(EventMsg::TurnStarted(TurnStartedEvent {
                    turn_id: "turn-explicit".to_string(),
                    started_at: None,
                    model_context_window: None,
                    collaboration_mode_kind: Default::default(),
                })),
                RolloutItem::ResponseItem(user_msg("hello")),
                RolloutItem::ResponseItem(assistant_msg("partial")),
            ]),
            auth_manager,
            /*persist_extended_history*/ false,
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
            config,
            source_path,
            /*persist_extended_history*/ false,
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
    let manager = ThreadManager::new(
        &config,
        auth_manager.clone(),
        SessionSource::Exec,
        CollaborationModesConfig::default(),
        Arc::new(codex_exec_server::EnvironmentManager::default_for_tests()),
        /*analytics_events_client*/ None,
    );

    let source = manager
        .resume_thread_with_history(
            config.clone(),
            InitialHistory::Forked(vec![
                RolloutItem::ResponseItem(user_msg("hello")),
                RolloutItem::ResponseItem(assistant_msg("partial")),
            ]),
            auth_manager,
            /*persist_extended_history*/ false,
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
            /*persist_extended_history*/ false,
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
    let interrupted_marker_json =
        serde_json::to_value(RolloutItem::ResponseItem(interrupted_turn_history_marker()))
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
            config,
            forked_path,
            /*persist_extended_history*/ false,
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
