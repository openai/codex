use super::*;
use crate::CodexAuth;
use crate::CodexThread;
use crate::ThreadManager;
use crate::agent::agent_status_from_event;
use crate::config::AgentRoleConfig;
use crate::config::Config;
use crate::config::ConfigBuilder;
use crate::config_loader::ConfigLayerEntry;
use crate::config_loader::ConfigLayerStack;
use crate::config_loader::ConfigLayerStackOrdering;
use crate::config_loader::ConfigRequirements;
use crate::config_loader::LoaderOverrides;
use crate::config_loader::RequirementSource;
use crate::config_loader::Sourced;
use crate::contextual_user_message::SUBAGENT_NOTIFICATION_OPEN_TAG;
use crate::features::Feature;
use assert_matches::assert_matches;
use codex_app_server_protocol::ConfigLayerSource;
use codex_config::RequirementsExecPolicy;
use codex_execpolicy::Decision;
use codex_execpolicy::Policy;
use codex_protocol::config_types::ModeKind;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::ErrorEvent;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::TurnAbortReason;
use codex_protocol::protocol::TurnAbortedEvent;
use codex_protocol::protocol::TurnCompleteEvent;
use codex_protocol::protocol::TurnStartedEvent;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::Duration;
use tokio::time::sleep;
use tokio::time::timeout;
use toml::Value as TomlValue;

async fn test_config_with_cli_overrides(
    cli_overrides: Vec<(String, TomlValue)>,
) -> (TempDir, Config) {
    let home = TempDir::new().expect("create temp dir");
    let config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .cli_overrides(cli_overrides)
        .loader_overrides(LoaderOverrides {
            #[cfg(target_os = "macos")]
            managed_preferences_base64: Some(String::new()),
            macos_managed_config_requirements_base64: Some(String::new()),
            ..LoaderOverrides::default()
        })
        .build()
        .await
        .expect("load default test config");
    (home, config)
}

async fn test_config() -> (TempDir, Config) {
    test_config_with_cli_overrides(Vec::new()).await
}

fn text_input(text: &str) -> Vec<UserInput> {
    vec![UserInput::Text {
        text: text.to_string(),
        text_elements: Vec::new(),
    }]
}

struct AgentControlHarness {
    _home: TempDir,
    config: Config,
    manager: ThreadManager,
    control: AgentControl,
}

impl AgentControlHarness {
    async fn new() -> Self {
        let (home, config) = test_config().await;
        let manager = ThreadManager::with_models_provider_and_home_for_tests(
            CodexAuth::from_api_key("dummy"),
            config.model_provider.clone(),
            config.codex_home.clone(),
        );
        let control = manager.agent_control();
        Self {
            _home: home,
            config,
            manager,
            control,
        }
    }

    async fn start_thread(&self) -> (ThreadId, Arc<CodexThread>) {
        let new_thread = self
            .manager
            .start_thread(self.config.clone())
            .await
            .expect("start thread");
        (new_thread.thread_id, new_thread.thread)
    }
}

fn has_subagent_notification(history_items: &[ResponseItem]) -> bool {
    history_items.iter().any(|item| {
        let ResponseItem::Message { role, content, .. } = item else {
            return false;
        };
        if role != "user" {
            return false;
        }
        content.iter().any(|content_item| match content_item {
            ContentItem::InputText { text } | ContentItem::OutputText { text } => {
                text.contains(SUBAGENT_NOTIFICATION_OPEN_TAG)
            }
            ContentItem::InputImage { .. } => false,
        })
    })
}

/// Returns true when any message item contains `needle` in a text span.
fn history_contains_text(history_items: &[ResponseItem], needle: &str) -> bool {
    history_items.iter().any(|item| {
        let ResponseItem::Message { content, .. } = item else {
            return false;
        };
        content.iter().any(|content_item| match content_item {
            ContentItem::InputText { text } | ContentItem::OutputText { text } => {
                text.contains(needle)
            }
            ContentItem::InputImage { .. } => false,
        })
    })
}

#[tokio::test]
async fn child_uses_parent_exec_policy_when_layer_stack_matches() {
    let (_home, parent_config) = test_config().await;
    let child_config = parent_config.clone();

    assert!(child_uses_parent_exec_policy(&parent_config, &child_config));
}

#[tokio::test]
async fn child_uses_parent_exec_policy_when_non_exec_policy_layers_differ() {
    let (_home, parent_config) = test_config().await;
    let mut child_config = parent_config.clone();
    let mut layers: Vec<_> = child_config
        .config_layer_stack
        .get_layers(ConfigLayerStackOrdering::LowestPrecedenceFirst, true)
        .into_iter()
        .cloned()
        .collect();
    layers.push(ConfigLayerEntry::new(
        ConfigLayerSource::SessionFlags,
        TomlValue::Table(Default::default()),
    ));
    child_config.config_layer_stack = ConfigLayerStack::new(
        layers,
        child_config.config_layer_stack.requirements().clone(),
        child_config.config_layer_stack.requirements_toml().clone(),
    )
    .expect("config layer stack");

    assert!(child_uses_parent_exec_policy(&parent_config, &child_config));
}

#[tokio::test]
async fn child_does_not_use_parent_exec_policy_when_requirements_exec_policy_differs() {
    let (_home, parent_config) = test_config().await;
    let mut child_config = parent_config.clone();
    let mut requirements = ConfigRequirements {
        exec_policy: child_config
            .config_layer_stack
            .requirements()
            .exec_policy
            .clone(),
        ..ConfigRequirements::default()
    };
    let mut policy = Policy::empty();
    policy
        .add_prefix_rule(&["rm".to_string()], Decision::Forbidden)
        .expect("add prefix rule");
    requirements.exec_policy = Some(Sourced::new(
        RequirementsExecPolicy::new(policy),
        RequirementSource::Unknown,
    ));
    child_config.config_layer_stack = ConfigLayerStack::new(
        child_config
            .config_layer_stack
            .get_layers(ConfigLayerStackOrdering::LowestPrecedenceFirst, true)
            .into_iter()
            .cloned()
            .collect(),
        requirements,
        child_config.config_layer_stack.requirements_toml().clone(),
    )
    .expect("config layer stack");

    assert!(!child_uses_parent_exec_policy(
        &parent_config,
        &child_config
    ));
}

async fn wait_for_subagent_notification(parent_thread: &Arc<CodexThread>) -> bool {
    let wait = async {
        loop {
            let history_items = parent_thread
                .codex
                .session
                .clone_history()
                .await
                .raw_items()
                .to_vec();
            if has_subagent_notification(&history_items) {
                return true;
            }
            sleep(Duration::from_millis(25)).await;
        }
    };
    timeout(Duration::from_secs(2), wait).await.is_ok()
}

#[tokio::test]
async fn send_input_errors_when_manager_dropped() {
    let control = AgentControl::default();
    let err = control
        .send_input(
            ThreadId::new(),
            vec![UserInput::Text {
                text: "hello".to_string(),
                text_elements: Vec::new(),
            }],
        )
        .await
        .expect_err("send_input should fail without a manager");
    assert_eq!(
        err.to_string(),
        "unsupported operation: thread manager dropped"
    );
}

#[tokio::test]
async fn get_status_returns_not_found_without_manager() {
    let control = AgentControl::default();
    let got = control.get_status(ThreadId::new()).await;
    assert_eq!(got, AgentStatus::NotFound);
}

#[tokio::test]
async fn on_event_updates_status_from_task_started() {
    let status = agent_status_from_event(&EventMsg::TurnStarted(TurnStartedEvent {
        turn_id: "turn-1".to_string(),
        model_context_window: None,
        collaboration_mode_kind: ModeKind::Default,
    }));
    assert_eq!(status, Some(AgentStatus::Running));
}

#[tokio::test]
async fn on_event_updates_status_from_task_complete() {
    let status = agent_status_from_event(&EventMsg::TurnComplete(TurnCompleteEvent {
        turn_id: "turn-1".to_string(),
        last_agent_message: Some("done".to_string()),
    }));
    let expected = AgentStatus::Completed(Some("done".to_string()));
    assert_eq!(status, Some(expected));
}

#[tokio::test]
async fn on_event_updates_status_from_error() {
    let status = agent_status_from_event(&EventMsg::Error(ErrorEvent {
        message: "boom".to_string(),
        codex_error_info: None,
    }));

    let expected = AgentStatus::Errored("boom".to_string());
    assert_eq!(status, Some(expected));
}

#[tokio::test]
async fn on_event_updates_status_from_turn_aborted() {
    let status = agent_status_from_event(&EventMsg::TurnAborted(TurnAbortedEvent {
        turn_id: Some("turn-1".to_string()),
        reason: TurnAbortReason::Interrupted,
    }));

    let expected = AgentStatus::Interrupted;
    assert_eq!(status, Some(expected));
}

#[tokio::test]
async fn on_event_updates_status_from_shutdown_complete() {
    let status = agent_status_from_event(&EventMsg::ShutdownComplete);
    assert_eq!(status, Some(AgentStatus::Shutdown));
}

#[tokio::test]
async fn spawn_agent_errors_when_manager_dropped() {
    let control = AgentControl::default();
    let (_home, config) = test_config().await;
    let err = control
        .spawn_agent(config, text_input("hello"), None)
        .await
        .expect_err("spawn_agent should fail without a manager");
    assert_eq!(
        err.to_string(),
        "unsupported operation: thread manager dropped"
    );
}

#[tokio::test]
async fn resume_agent_errors_when_manager_dropped() {
    let control = AgentControl::default();
    let (_home, config) = test_config().await;
    let err = control
        .resume_agent_from_rollout(config, ThreadId::new(), SessionSource::Exec)
        .await
        .expect_err("resume_agent should fail without a manager");
    assert_eq!(
        err.to_string(),
        "unsupported operation: thread manager dropped"
    );
}

#[tokio::test]
async fn send_input_errors_when_thread_missing() {
    let harness = AgentControlHarness::new().await;
    let thread_id = ThreadId::new();
    let err = harness
        .control
        .send_input(
            thread_id,
            vec![UserInput::Text {
                text: "hello".to_string(),
                text_elements: Vec::new(),
            }],
        )
        .await
        .expect_err("send_input should fail for missing thread");
    assert_matches!(err, CodexErr::ThreadNotFound(id) if id == thread_id);
}

#[tokio::test]
async fn get_status_returns_not_found_for_missing_thread() {
    let harness = AgentControlHarness::new().await;
    let status = harness.control.get_status(ThreadId::new()).await;
    assert_eq!(status, AgentStatus::NotFound);
}

#[tokio::test]
async fn get_status_returns_pending_init_for_new_thread() {
    let harness = AgentControlHarness::new().await;
    let (thread_id, _) = harness.start_thread().await;
    let status = harness.control.get_status(thread_id).await;
    assert_eq!(status, AgentStatus::PendingInit);
}

#[tokio::test]
async fn subscribe_status_errors_for_missing_thread() {
    let harness = AgentControlHarness::new().await;
    let thread_id = ThreadId::new();
    let err = harness
        .control
        .subscribe_status(thread_id)
        .await
        .expect_err("subscribe_status should fail for missing thread");
    assert_matches!(err, CodexErr::ThreadNotFound(id) if id == thread_id);
}

#[tokio::test]
async fn subscribe_status_updates_on_shutdown() {
    let harness = AgentControlHarness::new().await;
    let (thread_id, thread) = harness.start_thread().await;
    let mut status_rx = harness
        .control
        .subscribe_status(thread_id)
        .await
        .expect("subscribe_status should succeed");
    assert_eq!(status_rx.borrow().clone(), AgentStatus::PendingInit);

    let _ = thread
        .submit(Op::Shutdown {})
        .await
        .expect("shutdown should submit");

    let _ = status_rx.changed().await;
    assert_eq!(status_rx.borrow().clone(), AgentStatus::Shutdown);
}

#[tokio::test]
async fn send_input_submits_user_message() {
    let harness = AgentControlHarness::new().await;
    let (thread_id, _thread) = harness.start_thread().await;

    let submission_id = harness
        .control
        .send_input(
            thread_id,
            vec![UserInput::Text {
                text: "hello from tests".to_string(),
                text_elements: Vec::new(),
            }],
        )
        .await
        .expect("send_input should succeed");
    assert!(!submission_id.is_empty());
    let expected = (
        thread_id,
        Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello from tests".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        },
    );
    let captured = harness
        .manager
        .captured_ops()
        .into_iter()
        .find(|entry| *entry == expected);
    assert_eq!(captured, Some(expected));
}

#[tokio::test]
async fn spawn_agent_creates_thread_and_sends_prompt() {
    let harness = AgentControlHarness::new().await;
    let thread_id = harness
        .control
        .spawn_agent(harness.config.clone(), text_input("spawned"), None)
        .await
        .expect("spawn_agent should succeed");
    let _thread = harness
        .manager
        .get_thread(thread_id)
        .await
        .expect("thread should be registered");
    let expected = (
        thread_id,
        Op::UserInput {
            items: vec![UserInput::Text {
                text: "spawned".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        },
    );
    let captured = harness
        .manager
        .captured_ops()
        .into_iter()
        .find(|entry| *entry == expected);
    assert_eq!(captured, Some(expected));
}

#[tokio::test]
async fn spawn_agent_can_fork_parent_thread_history() {
    let harness = AgentControlHarness::new().await;
    let (parent_thread_id, parent_thread) = harness.start_thread().await;
    parent_thread
        .inject_user_message_without_turn("parent seed context".to_string())
        .await;
    let turn_context = parent_thread.codex.session.new_default_turn().await;
    let parent_spawn_call_id = "spawn-call-history".to_string();
    let parent_spawn_call = ResponseItem::FunctionCall {
        id: None,
        name: "spawn_agent".to_string(),
        namespace: None,
        arguments: "{}".to_string(),
        call_id: parent_spawn_call_id.clone(),
    };
    parent_thread
        .codex
        .session
        .record_conversation_items(turn_context.as_ref(), &[parent_spawn_call])
        .await;
    parent_thread
        .codex
        .session
        .ensure_rollout_materialized()
        .await;
    parent_thread.codex.session.flush_rollout().await;

    let child_thread_id = harness
        .control
        .spawn_agent_with_options(
            harness.config.clone(),
            text_input("child task"),
            Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id,
                depth: 1,
                agent_nickname: None,
                agent_role: None,
            })),
            SpawnAgentOptions {
                fork_parent_spawn_call_id: Some(parent_spawn_call_id),
            },
        )
        .await
        .expect("forked spawn should succeed");

    let child_thread = harness
        .manager
        .get_thread(child_thread_id)
        .await
        .expect("child thread should be registered");
    assert_ne!(child_thread_id, parent_thread_id);
    let history = child_thread.codex.session.clone_history().await;
    assert!(history_contains_text(
        history.raw_items(),
        "parent seed context"
    ));

    let expected = (
        child_thread_id,
        Op::UserInput {
            items: vec![UserInput::Text {
                text: "child task".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        },
    );
    let captured = harness
        .manager
        .captured_ops()
        .into_iter()
        .find(|entry| *entry == expected);
    assert_eq!(captured, Some(expected));

    let _ = harness
        .control
        .shutdown_agent(child_thread_id)
        .await
        .expect("child shutdown should submit");
    let _ = parent_thread
        .submit(Op::Shutdown {})
        .await
        .expect("parent shutdown should submit");
}

#[tokio::test]
async fn spawn_agent_fork_injects_output_for_parent_spawn_call() {
    let harness = AgentControlHarness::new().await;
    let (parent_thread_id, parent_thread) = harness.start_thread().await;
    let turn_context = parent_thread.codex.session.new_default_turn().await;
    let parent_spawn_call_id = "spawn-call-1".to_string();
    let parent_spawn_call = ResponseItem::FunctionCall {
        id: None,
        name: "spawn_agent".to_string(),
        namespace: None,
        arguments: "{}".to_string(),
        call_id: parent_spawn_call_id.clone(),
    };
    parent_thread
        .codex
        .session
        .record_conversation_items(turn_context.as_ref(), &[parent_spawn_call])
        .await;
    parent_thread
        .codex
        .session
        .ensure_rollout_materialized()
        .await;
    parent_thread.codex.session.flush_rollout().await;

    let child_thread_id = harness
        .control
        .spawn_agent_with_options(
            harness.config.clone(),
            text_input("child task"),
            Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id,
                depth: 1,
                agent_nickname: None,
                agent_role: None,
            })),
            SpawnAgentOptions {
                fork_parent_spawn_call_id: Some(parent_spawn_call_id.clone()),
            },
        )
        .await
        .expect("forked spawn should succeed");

    let child_thread = harness
        .manager
        .get_thread(child_thread_id)
        .await
        .expect("child thread should be registered");
    let history = child_thread.codex.session.clone_history().await;
    let injected_output = history.raw_items().iter().find_map(|item| match item {
        ResponseItem::FunctionCallOutput { call_id, output }
            if call_id == &parent_spawn_call_id =>
        {
            Some(output)
        }
        _ => None,
    });
    let injected_output =
        injected_output.expect("forked child should contain synthetic tool output");
    assert_eq!(
        injected_output.text_content(),
        Some(FORKED_SPAWN_AGENT_OUTPUT_MESSAGE)
    );
    assert_eq!(injected_output.success, Some(true));

    let _ = harness
        .control
        .shutdown_agent(child_thread_id)
        .await
        .expect("child shutdown should submit");
    let _ = parent_thread
        .submit(Op::Shutdown {})
        .await
        .expect("parent shutdown should submit");
}

#[tokio::test]
async fn spawn_agent_fork_flushes_parent_rollout_before_loading_history() {
    let harness = AgentControlHarness::new().await;
    let (parent_thread_id, parent_thread) = harness.start_thread().await;
    let turn_context = parent_thread.codex.session.new_default_turn().await;
    let parent_spawn_call_id = "spawn-call-unflushed".to_string();
    let parent_spawn_call = ResponseItem::FunctionCall {
        id: None,
        name: "spawn_agent".to_string(),
        namespace: None,
        arguments: "{}".to_string(),
        call_id: parent_spawn_call_id.clone(),
    };
    parent_thread
        .codex
        .session
        .record_conversation_items(turn_context.as_ref(), &[parent_spawn_call])
        .await;

    let child_thread_id = harness
        .control
        .spawn_agent_with_options(
            harness.config.clone(),
            text_input("child task"),
            Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id,
                depth: 1,
                agent_nickname: None,
                agent_role: None,
            })),
            SpawnAgentOptions {
                fork_parent_spawn_call_id: Some(parent_spawn_call_id.clone()),
            },
        )
        .await
        .expect("forked spawn should flush parent rollout before loading history");

    let child_thread = harness
        .manager
        .get_thread(child_thread_id)
        .await
        .expect("child thread should be registered");
    let history = child_thread.codex.session.clone_history().await;

    let mut parent_call_index = None;
    let mut injected_output_index = None;
    for (idx, item) in history.raw_items().iter().enumerate() {
        match item {
            ResponseItem::FunctionCall { call_id, .. } if call_id == &parent_spawn_call_id => {
                parent_call_index = Some(idx);
            }
            ResponseItem::FunctionCallOutput { call_id, .. }
                if call_id == &parent_spawn_call_id =>
            {
                injected_output_index = Some(idx);
            }
            _ => {}
        }
    }

    let parent_call_index =
        parent_call_index.expect("forked child should include the parent spawn_agent call");
    let injected_output_index = injected_output_index
        .expect("forked child should include synthetic output for the parent spawn_agent call");
    assert!(parent_call_index < injected_output_index);

    let _ = harness
        .control
        .shutdown_agent(child_thread_id)
        .await
        .expect("child shutdown should submit");
    let _ = parent_thread
        .submit(Op::Shutdown {})
        .await
        .expect("parent shutdown should submit");
}

#[tokio::test]
async fn spawn_agent_respects_max_threads_limit() {
    let max_threads = 1usize;
    let (_home, config) = test_config_with_cli_overrides(vec![(
        "agents.max_threads".to_string(),
        TomlValue::Integer(max_threads as i64),
    )])
    .await;
    let manager = ThreadManager::with_models_provider_and_home_for_tests(
        CodexAuth::from_api_key("dummy"),
        config.model_provider.clone(),
        config.codex_home.clone(),
    );
    let control = manager.agent_control();

    let _ = manager
        .start_thread(config.clone())
        .await
        .expect("start thread");

    let first_agent_id = control
        .spawn_agent(config.clone(), text_input("hello"), None)
        .await
        .expect("spawn_agent should succeed");

    let err = control
        .spawn_agent(config, text_input("hello again"), None)
        .await
        .expect_err("spawn_agent should respect max threads");
    let CodexErr::AgentLimitReached {
        max_threads: seen_max_threads,
    } = err
    else {
        panic!("expected CodexErr::AgentLimitReached");
    };
    assert_eq!(seen_max_threads, max_threads);

    let _ = control
        .shutdown_agent(first_agent_id)
        .await
        .expect("shutdown agent");
}

#[tokio::test]
async fn spawn_agent_releases_slot_after_shutdown() {
    let max_threads = 1usize;
    let (_home, config) = test_config_with_cli_overrides(vec![(
        "agents.max_threads".to_string(),
        TomlValue::Integer(max_threads as i64),
    )])
    .await;
    let manager = ThreadManager::with_models_provider_and_home_for_tests(
        CodexAuth::from_api_key("dummy"),
        config.model_provider.clone(),
        config.codex_home.clone(),
    );
    let control = manager.agent_control();

    let first_agent_id = control
        .spawn_agent(config.clone(), text_input("hello"), None)
        .await
        .expect("spawn_agent should succeed");
    let _ = control
        .shutdown_agent(first_agent_id)
        .await
        .expect("shutdown agent");

    let second_agent_id = control
        .spawn_agent(config.clone(), text_input("hello again"), None)
        .await
        .expect("spawn_agent should succeed after shutdown");
    let _ = control
        .shutdown_agent(second_agent_id)
        .await
        .expect("shutdown agent");
}

#[tokio::test]
async fn spawn_agent_limit_shared_across_clones() {
    let max_threads = 1usize;
    let (_home, config) = test_config_with_cli_overrides(vec![(
        "agents.max_threads".to_string(),
        TomlValue::Integer(max_threads as i64),
    )])
    .await;
    let manager = ThreadManager::with_models_provider_and_home_for_tests(
        CodexAuth::from_api_key("dummy"),
        config.model_provider.clone(),
        config.codex_home.clone(),
    );
    let control = manager.agent_control();
    let cloned = control.clone();

    let first_agent_id = cloned
        .spawn_agent(config.clone(), text_input("hello"), None)
        .await
        .expect("spawn_agent should succeed");

    let err = control
        .spawn_agent(config, text_input("hello again"), None)
        .await
        .expect_err("spawn_agent should respect shared guard");
    let CodexErr::AgentLimitReached { max_threads } = err else {
        panic!("expected CodexErr::AgentLimitReached");
    };
    assert_eq!(max_threads, 1);

    let _ = control
        .shutdown_agent(first_agent_id)
        .await
        .expect("shutdown agent");
}

#[tokio::test]
async fn resume_agent_respects_max_threads_limit() {
    let max_threads = 1usize;
    let (_home, config) = test_config_with_cli_overrides(vec![(
        "agents.max_threads".to_string(),
        TomlValue::Integer(max_threads as i64),
    )])
    .await;
    let manager = ThreadManager::with_models_provider_and_home_for_tests(
        CodexAuth::from_api_key("dummy"),
        config.model_provider.clone(),
        config.codex_home.clone(),
    );
    let control = manager.agent_control();

    let resumable_id = control
        .spawn_agent(config.clone(), text_input("hello"), None)
        .await
        .expect("spawn_agent should succeed");
    let _ = control
        .shutdown_agent(resumable_id)
        .await
        .expect("shutdown resumable thread");

    let active_id = control
        .spawn_agent(config.clone(), text_input("occupy"), None)
        .await
        .expect("spawn_agent should succeed for active slot");

    let err = control
        .resume_agent_from_rollout(config, resumable_id, SessionSource::Exec)
        .await
        .expect_err("resume should respect max threads");
    let CodexErr::AgentLimitReached {
        max_threads: seen_max_threads,
    } = err
    else {
        panic!("expected CodexErr::AgentLimitReached");
    };
    assert_eq!(seen_max_threads, max_threads);

    let _ = control
        .shutdown_agent(active_id)
        .await
        .expect("shutdown active thread");
}

#[tokio::test]
async fn resume_agent_releases_slot_after_resume_failure() {
    let max_threads = 1usize;
    let (_home, config) = test_config_with_cli_overrides(vec![(
        "agents.max_threads".to_string(),
        TomlValue::Integer(max_threads as i64),
    )])
    .await;
    let manager = ThreadManager::with_models_provider_and_home_for_tests(
        CodexAuth::from_api_key("dummy"),
        config.model_provider.clone(),
        config.codex_home.clone(),
    );
    let control = manager.agent_control();

    let _ = control
        .resume_agent_from_rollout(config.clone(), ThreadId::new(), SessionSource::Exec)
        .await
        .expect_err("resume should fail for missing rollout path");

    let resumed_id = control
        .spawn_agent(config, text_input("hello"), None)
        .await
        .expect("spawn should succeed after failed resume");
    let _ = control
        .shutdown_agent(resumed_id)
        .await
        .expect("shutdown resumed thread");
}

#[tokio::test]
async fn spawn_child_completion_notifies_parent_history() {
    let harness = AgentControlHarness::new().await;
    let (parent_thread_id, parent_thread) = harness.start_thread().await;

    let child_thread_id = harness
        .control
        .spawn_agent(
            harness.config.clone(),
            text_input("hello child"),
            Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id,
                depth: 1,
                agent_nickname: None,
                agent_role: Some("explorer".to_string()),
            })),
        )
        .await
        .expect("child spawn should succeed");

    let child_thread = harness
        .manager
        .get_thread(child_thread_id)
        .await
        .expect("child thread should exist");
    let _ = child_thread
        .submit(Op::Shutdown {})
        .await
        .expect("child shutdown should submit");

    assert_eq!(wait_for_subagent_notification(&parent_thread).await, true);
}

#[tokio::test]
async fn completion_watcher_notifies_parent_when_child_is_missing() {
    let harness = AgentControlHarness::new().await;
    let (parent_thread_id, parent_thread) = harness.start_thread().await;
    let child_thread_id = ThreadId::new();

    harness.control.maybe_start_completion_watcher(
        child_thread_id,
        Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id,
            depth: 1,
            agent_nickname: None,
            agent_role: Some("explorer".to_string()),
        })),
    );

    assert_eq!(wait_for_subagent_notification(&parent_thread).await, true);

    let history_items = parent_thread
        .codex
        .session
        .clone_history()
        .await
        .raw_items()
        .to_vec();
    assert_eq!(
        history_contains_text(
            &history_items,
            &format!("\"agent_id\":\"{child_thread_id}\"")
        ),
        true
    );
    assert_eq!(
        history_contains_text(&history_items, "\"status\":\"not_found\""),
        true
    );
}

#[tokio::test]
async fn spawn_thread_subagent_gets_random_nickname_in_session_source() {
    let harness = AgentControlHarness::new().await;
    let (parent_thread_id, _parent_thread) = harness.start_thread().await;

    let child_thread_id = harness
        .control
        .spawn_agent(
            harness.config.clone(),
            text_input("hello child"),
            Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id,
                depth: 1,
                agent_nickname: None,
                agent_role: Some("explorer".to_string()),
            })),
        )
        .await
        .expect("child spawn should succeed");

    let child_thread = harness
        .manager
        .get_thread(child_thread_id)
        .await
        .expect("child thread should be registered");
    let snapshot = child_thread.config_snapshot().await;

    let SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
        parent_thread_id: seen_parent_thread_id,
        depth,
        agent_nickname,
        agent_role,
    }) = snapshot.session_source
    else {
        panic!("expected thread-spawn sub-agent source");
    };
    assert_eq!(seen_parent_thread_id, parent_thread_id);
    assert_eq!(depth, 1);
    assert!(agent_nickname.is_some());
    assert_eq!(agent_role, Some("explorer".to_string()));
}

#[tokio::test]
async fn spawn_thread_subagent_uses_role_specific_nickname_candidates() {
    let mut harness = AgentControlHarness::new().await;
    harness.config.agent_roles.insert(
        "researcher".to_string(),
        AgentRoleConfig {
            description: Some("Research role".to_string()),
            config_file: None,
            nickname_candidates: Some(vec!["Atlas".to_string()]),
        },
    );
    let (parent_thread_id, _parent_thread) = harness.start_thread().await;

    let child_thread_id = harness
        .control
        .spawn_agent(
            harness.config.clone(),
            text_input("hello child"),
            Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id,
                depth: 1,
                agent_nickname: None,
                agent_role: Some("researcher".to_string()),
            })),
        )
        .await
        .expect("child spawn should succeed");

    let child_thread = harness
        .manager
        .get_thread(child_thread_id)
        .await
        .expect("child thread should be registered");
    let snapshot = child_thread.config_snapshot().await;

    let SessionSource::SubAgent(SubAgentSource::ThreadSpawn { agent_nickname, .. }) =
        snapshot.session_source
    else {
        panic!("expected thread-spawn sub-agent source");
    };
    assert_eq!(agent_nickname, Some("Atlas".to_string()));
}

#[tokio::test]
async fn resume_thread_subagent_restores_stored_nickname_and_role() {
    let (home, mut config) = test_config().await;
    config
        .features
        .enable(Feature::Sqlite)
        .expect("test config should allow sqlite");
    let manager = ThreadManager::with_models_provider_and_home_for_tests(
        CodexAuth::from_api_key("dummy"),
        config.model_provider.clone(),
        config.codex_home.clone(),
    );
    let control = manager.agent_control();
    let harness = AgentControlHarness {
        _home: home,
        config,
        manager,
        control,
    };
    let (parent_thread_id, _parent_thread) = harness.start_thread().await;

    let child_thread_id = harness
        .control
        .spawn_agent(
            harness.config.clone(),
            text_input("hello child"),
            Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id,
                depth: 1,
                agent_nickname: None,
                agent_role: Some("explorer".to_string()),
            })),
        )
        .await
        .expect("child spawn should succeed");

    let child_thread = harness
        .manager
        .get_thread(child_thread_id)
        .await
        .expect("child thread should exist");
    let mut status_rx = harness
        .control
        .subscribe_status(child_thread_id)
        .await
        .expect("status subscription should succeed");
    if matches!(status_rx.borrow().clone(), AgentStatus::PendingInit) {
        timeout(Duration::from_secs(5), async {
            loop {
                status_rx
                    .changed()
                    .await
                    .expect("child status should advance past pending init");
                if !matches!(status_rx.borrow().clone(), AgentStatus::PendingInit) {
                    break;
                }
            }
        })
        .await
        .expect("child should initialize before shutdown");
    }
    let original_snapshot = child_thread.config_snapshot().await;
    let original_nickname = original_snapshot
        .session_source
        .get_nickname()
        .expect("spawned sub-agent should have a nickname");
    let state_db = child_thread
        .state_db()
        .expect("sqlite state db should be available for nickname resume test");
    timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(Some(metadata)) = state_db.get_thread(child_thread_id).await
                && metadata.agent_nickname.is_some()
                && metadata.agent_role.as_deref() == Some("explorer")
            {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("child thread metadata should be persisted to sqlite before shutdown");

    let _ = harness
        .control
        .shutdown_agent(child_thread_id)
        .await
        .expect("child shutdown should submit");

    let resumed_thread_id = harness
        .control
        .resume_agent_from_rollout(
            harness.config.clone(),
            child_thread_id,
            SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id,
                depth: 1,
                agent_nickname: None,
                agent_role: None,
            }),
        )
        .await
        .expect("resume should succeed");
    assert_eq!(resumed_thread_id, child_thread_id);

    let resumed_snapshot = harness
        .manager
        .get_thread(resumed_thread_id)
        .await
        .expect("resumed child thread should exist")
        .config_snapshot()
        .await;
    let SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
        parent_thread_id: resumed_parent_thread_id,
        depth: resumed_depth,
        agent_nickname: resumed_nickname,
        agent_role: resumed_role,
    }) = resumed_snapshot.session_source
    else {
        panic!("expected thread-spawn sub-agent source");
    };
    assert_eq!(resumed_parent_thread_id, parent_thread_id);
    assert_eq!(resumed_depth, 1);
    assert_eq!(resumed_nickname, Some(original_nickname));
    assert_eq!(resumed_role, Some("explorer".to_string()));

    let _ = harness
        .control
        .shutdown_agent(resumed_thread_id)
        .await
        .expect("resumed child shutdown should submit");
}
