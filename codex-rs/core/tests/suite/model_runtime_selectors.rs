use anyhow::Result;
use anyhow::bail;
use codex_core::config::Config;
use codex_core::config::Constrained;
use codex_core::sandboxing::SandboxPermissions;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_models_manager::manager::RefreshStrategy;
use codex_models_manager::manager::SharedModelsManager;
use codex_models_manager::model_info::model_info_from_slug;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::ToolMode;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_models_once;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_once_match;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::submit_thread_settings;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::io::Cursor;
use tokio::time::Duration;
use tokio::time::Instant;
use tokio::time::sleep;
use wiremock::Request;

const CHILD_PROMPT: &str = "inspect the child runtime";
const CHILD_MODEL: &str = "test-multi-agent-child";
const GUARDIAN_REASON: &str = "Allow a narrowly scoped network request";
const GUARDIAN_ROOT_PROMPT: &str = "request an escalated command";
const ROOT_MODEL: &str = "test-multi-agent-root";
const ROOT_PROMPT: &str = "spawn a child";
const SPAWN_CALL_ID: &str = "spawn-call-1";

fn remote_model(slug: &str) -> ModelInfo {
    ModelInfo {
        visibility: ModelVisibility::List,
        used_fallback_model_metadata: false,
        ..model_info_from_slug(slug)
    }
}

fn body_contains(req: &Request, text: &str) -> bool {
    let is_zstd = req
        .headers
        .get("content-encoding")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(',')
                .any(|entry| entry.trim().eq_ignore_ascii_case("zstd"))
        });
    let bytes = if is_zstd {
        zstd::stream::decode_all(Cursor::new(&req.body)).ok()
    } else {
        Some(req.body.clone())
    };
    bytes
        .and_then(|body| String::from_utf8(body).ok())
        .is_some_and(|body| body.contains(text))
}

fn tool_names(body: &Value) -> Vec<String> {
    body.get("tools")
        .and_then(Value::as_array)
        .map(|tools| {
            tools
                .iter()
                .filter_map(|tool| {
                    tool.get("name")
                        .or_else(|| tool.get("type"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default()
}

async fn wait_for_model_available(manager: &SharedModelsManager, slug: &str) -> ModelPreset {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(model) = manager
            .list_models(RefreshStrategy::Online)
            .await
            .iter()
            .find(|model| model.model == slug)
            .cloned()
        {
            return model;
        }
        if Instant::now() >= deadline {
            panic!("timed out waiting for the remote model {slug} to appear");
        }
        sleep(Duration::from_millis(25)).await;
    }
}

async fn response_body_for_remote_model(
    remote_model: ModelInfo,
    configure: impl FnOnce(&mut Config) + Send + 'static,
) -> Result<Value> {
    let server = responses::start_mock_server().await;
    let model_slug = remote_model.slug.clone();
    let models_mock = mount_models_once(
        &server,
        ModelsResponse {
            models: vec![remote_model],
        },
    )
    .await;
    let response_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(configure);
    let test = builder.build(&server).await?;
    let models_manager = test.thread_manager.get_models_manager();
    let available_model = wait_for_model_available(&models_manager, &model_slug).await;
    assert_eq!(available_model.model, model_slug);
    assert_eq!(models_mock.requests().len(), 1);

    submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            model: Some(model_slug),
            ..Default::default()
        },
    )
    .await?;
    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "list tools".into(),
                text_elements: Vec::new(),
            }],
            environments: None,
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    Ok(response_mock.single_request().body_json())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_tool_mode_selector_overrides_feature_flags() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let mut direct_model = remote_model("test-tool-mode-direct");
    direct_model.tool_mode = Some(ToolMode::Direct);
    let direct_body = response_body_for_remote_model(direct_model, |config| {
        config
            .features
            .enable(Feature::CodeModeOnly)
            .expect("test config should allow feature update");
    })
    .await?;
    let direct_tools = tool_names(&direct_body);
    assert!(
        direct_tools
            .iter()
            .all(|name| name != codex_code_mode::PUBLIC_TOOL_NAME
                && name != codex_code_mode::WAIT_TOOL_NAME),
        "direct mode should override enabled code mode flags: {direct_tools:?}"
    );

    let mut code_mode_only_model = remote_model("test-tool-mode-code-mode-only");
    code_mode_only_model.tool_mode = Some(ToolMode::CodeModeOnly);
    let code_mode_only_body = response_body_for_remote_model(code_mode_only_model, |_| {}).await?;
    assert_eq!(
        tool_names(&code_mode_only_body),
        vec![
            codex_code_mode::PUBLIC_TOOL_NAME.to_string(),
            codex_code_mode::WAIT_TOOL_NAME.to_string(),
        ]
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_multi_agent_selector_overrides_features_and_child_model_info() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = wiremock::MockServer::start().await;
    let mut root_model = remote_model(ROOT_MODEL);
    root_model.multi_agent_version = Some(MultiAgentVersion::V2);
    let mut child_model = remote_model(CHILD_MODEL);
    child_model.multi_agent_version = Some(MultiAgentVersion::V1);
    let models_mock = mount_models_once(
        &server,
        ModelsResponse {
            models: vec![root_model, child_model],
        },
    )
    .await;
    let spawn_args = serde_json::to_string(&json!({
        "message": CHILD_PROMPT,
        "task_name": "worker",
        "model": CHILD_MODEL,
        "fork_turns": "none",
    }))?;
    mount_sse_once_match(
        &server,
        |req: &Request| body_contains(req, ROOT_PROMPT),
        sse(vec![
            ev_response_created("resp-root-1"),
            ev_function_call(SPAWN_CALL_ID, "spawn_agent", &spawn_args),
            ev_completed("resp-root-1"),
        ]),
    )
    .await;
    mount_sse_once_match(
        &server,
        |req: &Request| body_contains(req, CHILD_PROMPT) && !body_contains(req, SPAWN_CALL_ID),
        sse(vec![
            ev_response_created("resp-child-1"),
            ev_assistant_message("msg-child-1", "child done"),
            ev_completed("resp-child-1"),
        ]),
    )
    .await;
    let root_followup_mock = mount_sse_once_match(
        &server,
        |req: &Request| body_contains(req, SPAWN_CALL_ID),
        sse(vec![
            ev_response_created("resp-root-2"),
            ev_assistant_message("msg-root-2", "root done"),
            ev_completed("resp-root-2"),
        ]),
    )
    .await;

    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(|config| {
            config
                .features
                .enable(Feature::Collab)
                .expect("test config should allow feature update");
            config.model = Some(ROOT_MODEL.to_string());
        });
    let test = builder.build(&server).await?;
    assert_eq!(
        (
            models_mock.requests().len(),
            test.codex.multi_agent_version(),
        ),
        (1, Some(MultiAgentVersion::V2))
    );
    test.submit_turn(ROOT_PROMPT).await?;
    let deadline = Instant::now() + Duration::from_secs(2);
    let child_id = loop {
        if let Some(child_id) = test
            .thread_manager
            .list_thread_ids()
            .await
            .into_iter()
            .find(|thread_id| *thread_id != test.session_configured.thread_id)
        {
            break child_id;
        }
        if Instant::now() >= deadline {
            bail!(
                "timed out waiting for spawn_agent to create a child thread: root lock {:?}, spawn output {:?}",
                test.codex.multi_agent_version(),
                root_followup_mock.function_call_output_text(SPAWN_CALL_ID),
            );
        }
        sleep(Duration::from_millis(10)).await;
    };
    let child = test.thread_manager.get_thread(child_id).await?;

    assert_eq!(
        (
            models_mock.requests().len(),
            test.codex.multi_agent_version(),
            child.config_snapshot().await.model,
            child.multi_agent_version(),
        ),
        (
            1,
            Some(MultiAgentVersion::V2),
            CHILD_MODEL.to_string(),
            Some(MultiAgentVersion::V2),
        )
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_stays_disabled_when_model_selects_multi_agent_v2() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = wiremock::MockServer::start().await;
    let mut model = remote_model(ROOT_MODEL);
    model.multi_agent_version = Some(MultiAgentVersion::V2);
    let models_mock = mount_models_once(
        &server,
        ModelsResponse {
            models: vec![model],
        },
    )
    .await;
    let exec_args = serde_json::to_string(&json!({
        "cmd": "true",
        "sandbox_permissions": SandboxPermissions::RequireEscalated,
        "justification": GUARDIAN_REASON,
    }))?;
    let request_log = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-root"),
                ev_function_call("exec-call", "exec_command", &exec_args),
                ev_completed("resp-root"),
            ]),
            sse(vec![
                ev_response_created("resp-guardian"),
                ev_assistant_message(
                    "msg-guardian",
                    &json!({
                        "risk_level": "low",
                        "user_authorization": "high",
                        "outcome": "deny",
                        "rationale": "Keep the test command from executing.",
                    })
                    .to_string(),
                ),
                ev_completed("resp-guardian"),
            ]),
            sse(vec![
                ev_response_created("resp-root-followup"),
                ev_completed("resp-root-followup"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(|config| {
            config.model = Some(ROOT_MODEL.to_string());
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config
                .features
                .disable(Feature::Apps)
                .expect("test config should allow feature update");
        });
    let test = builder.build(&server).await?;
    test.submit_turn(GUARDIAN_ROOT_PROMPT).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = request_log.requests();
    let [root_request, guardian_request, _root_followup_request] = requests.as_slice() else {
        panic!("expected root, guardian, and root follow-up requests");
    };
    assert_eq!(
        (
            models_mock.requests().len(),
            test.codex.multi_agent_version(),
            tool_names(&root_request.body_json()).contains(&"spawn_agent".to_string()),
            guardian_request.body_contains_text(GUARDIAN_REASON),
            guardian_request.body_json()["prompt_cache_key"]
                .as_str()
                .is_some_and(|key| key.starts_with("guardian:")),
            tool_names(&guardian_request.body_json()).contains(&"spawn_agent".to_string()),
        ),
        (1, Some(MultiAgentVersion::V2), true, true, true, false)
    );

    Ok(())
}
