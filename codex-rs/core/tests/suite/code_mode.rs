#![allow(clippy::unwrap_used)]

use anyhow::Result;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_config::types::McpServerConfig;
use codex_config::types::McpServerTransportConfig;
use codex_core::StartThreadOptions;
use codex_core::TurnInputRequest;
use codex_core::config::Config;
use codex_core::config::Constrained;
use codex_core::config::CurrentTimeReminderConfig;
use codex_core::context::NodeReplReviewEvidence;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ToolCallOutcome;
use codex_extension_api::ToolContributor;
use codex_extension_api::ToolFinishInput;
use codex_extension_api::ToolLifecycleContributor;
use codex_extension_api::ToolLifecycleFuture;
use codex_extension_api::ToolStartInput;
use codex_features::CurrentTimeSource;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_models_manager::bundled_models_response;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::dynamic_tools::DynamicToolCallOutputContentItem;
use codex_protocol::dynamic_tools::DynamicToolFunctionSpec;
use codex_protocol::dynamic_tools::DynamicToolNamespaceSpec;
use codex_protocol::dynamic_tools::DynamicToolNamespaceTool;
use codex_protocol::dynamic_tools::DynamicToolResponse;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ToolMode;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use codex_tools::FreeformTool;
use codex_tools::FreeformToolFormat;
use codex_tools::FunctionCallError;
use codex_tools::JsonToolOutput;
use codex_tools::ResponsesApiNamespace;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::ToolCall;
use codex_tools::ToolExecutor;
use codex_tools::ToolExecutorFuture;
use codex_tools::ToolName;
use codex_tools::ToolOutput;
use codex_tools::ToolPayload;
use codex_tools::ToolSpec;
use codex_web_search_extension::install as install_web_search_extension;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::AppsTestToolLoading;
use core_test_support::apps_test_server::DIRECT_CALENDAR_APP_ONLY_TOOL;
use core_test_support::apps_test_server::recorded_apps_tool_calls;
use core_test_support::apps_test_server::search_capable_apps_builder;
use core_test_support::assert_regex_match;
use core_test_support::responses;
use core_test_support::responses::ResponseMock;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_custom_tool_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::namespace_child_tool;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_sandbox;
use core_test_support::skip_if_wine_exec;
use core_test_support::stdio_server_bin;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::TestCodexBuilder;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use core_test_support::wait_for_mcp_server;
use image::DynamicImage;
use image::GenericImageView;
use image::ImageBuffer;
use image::ImageDecoder;
use image::ImageEncoder;
use image::ImageFormat;
use image::ImageReader;
use image::Rgba;
use image::codecs::png::PngEncoder;
use image::metadata::Orientation;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;
use test_case::test_case;
use tokio::sync::oneshot;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::rmcp_client::remote_aware_environment_id;
use super::rmcp_client::remote_aware_stdio_server_bin;

fn custom_tool_output_items(req: &ResponsesRequest, call_id: &str) -> Vec<Value> {
    match req.custom_tool_call_output(call_id).get("output") {
        Some(Value::Array(items)) => items.clone(),
        Some(Value::String(text)) => {
            vec![serde_json::json!({ "type": "input_text", "text": text })]
        }
        _ => panic!("custom tool output should be serialized as text or content items"),
    }
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

fn function_tool_output_items(req: &ResponsesRequest, call_id: &str) -> Vec<Value> {
    match req.function_call_output(call_id).get("output") {
        Some(Value::Array(items)) => items.clone(),
        Some(Value::String(text)) => {
            vec![serde_json::json!({ "type": "input_text", "text": text })]
        }
        _ => panic!("function tool output should be serialized as text or content items"),
    }
}

fn text_item(items: &[Value], index: usize) -> &str {
    items[index]
        .get("text")
        .and_then(Value::as_str)
        .expect("content item should be input_text")
}

fn extract_running_cell_id(text: &str) -> String {
    text.strip_prefix("Script running with cell ID ")
        .and_then(|rest| rest.split('\n').next())
        .expect("running header should contain a cell ID")
        .to_string()
}

fn wait_for_file_source(path: &Path) -> Result<String> {
    let quoted_path = shlex::try_join([path.to_string_lossy().as_ref()])?;
    let command = format!("if [ -f {quoted_path} ]; then printf ready; fi");
    Ok(format!(
        r#"while ((await tools.exec_command({{ cmd: {command:?} }})).output !== "ready") {{
}}"#
    ))
}

fn custom_tool_output_body_and_success(
    req: &ResponsesRequest,
    call_id: &str,
) -> (String, Option<bool>) {
    let (content, success) = req
        .custom_tool_call_output_content_and_success(call_id)
        .expect("custom tool output should be present");
    let items = custom_tool_output_items(req, call_id);
    let text_items = items
        .iter()
        .filter_map(|item| item.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>();
    let output = match text_items.as_slice() {
        [] => content.unwrap_or_default(),
        [only] => (*only).to_string(),
        [_, rest @ ..] => rest.concat(),
    };
    (output, success)
}

fn custom_tool_output_last_non_empty_text(req: &ResponsesRequest, call_id: &str) -> Option<String> {
    match req.custom_tool_call_output(call_id).get("output") {
        Some(Value::String(text)) if !text.trim().is_empty() => Some(text.clone()),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| item.get("text").and_then(Value::as_str))
            .rfind(|text| !text.trim().is_empty())
            .map(str::to_string),
        Some(Value::String(_))
        | Some(Value::Object(_))
        | Some(Value::Number(_))
        | Some(Value::Bool(_))
        | Some(Value::Null)
        | None => None,
    }
}

async fn run_code_mode_turn(
    server: &MockServer,
    prompt: &str,
    code: &str,
) -> Result<(TestCodex, ResponseMock)> {
    run_code_mode_turn_with_config(server, prompt, code, |_| {}).await
}

async fn run_code_mode_turn_with_config(
    server: &MockServer,
    prompt: &str,
    code: &str,
    configure: impl FnOnce(&mut Config) + Send + 'static,
) -> Result<(TestCodex, ResponseMock)> {
    run_code_mode_turn_with_model_and_config(server, prompt, code, "test-gpt-5.1-codex", configure)
        .await
}

async fn run_code_mode_turn_with_model_and_config(
    server: &MockServer,
    prompt: &str,
    code: &str,
    model: &'static str,
    configure: impl FnOnce(&mut Config) + Send + 'static,
) -> Result<(TestCodex, ResponseMock)> {
    let builder = test_codex().with_model(model).with_config(move |config| {
        let _ = config.features.enable(Feature::CodeMode);
        let _ = config.features.enable(Feature::ExecutedToolCallMetadata);
        configure(config);
    });
    run_code_mode_turn_with_builder(server, prompt, code, builder).await
}

async fn run_code_mode_turn_with_builder(
    server: &MockServer,
    prompt: &str,
    code: &str,
    mut builder: TestCodexBuilder,
) -> Result<(TestCodex, ResponseMock)> {
    let test = builder.build(server).await?;

    responses::mount_sse_once(
        server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call("call-1", "exec", code),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let second_mock = responses::mount_sse_once(
        server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn(prompt).await?;
    Ok((test, second_mock))
}

async fn run_unavailable_code_mode_turn(
    server: &MockServer,
    test: &TestCodex,
) -> Result<(Value, Vec<String>)> {
    let response_mock = responses::mount_sse_once(
        server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "list available tools".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;

    let mut warnings = Vec::new();
    loop {
        match wait_for_event(&test.codex, |_| true).await {
            EventMsg::Warning(warning) => warnings.push(warning.message),
            EventMsg::TurnComplete(_) => break,
            _ => {}
        }
    }

    Ok((response_mock.single_request().body_json(), warnings))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn missing_process_host_falls_back_to_direct_tools_and_warns_once() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mut builder = test_codex()
        .with_model("test-gpt-5.1-codex")
        .with_code_mode_host_program("codex-code-mode-host-does-not-exist".into())
        .with_config(|config| {
            config
                .features
                .enable(Feature::CodeMode)
                .expect("code mode should be enabled");
        });
    let test = builder.build_with_auto_env(&server).await?;
    let (first_body, first_warnings) = run_unavailable_code_mode_turn(&server, &test).await?;
    let first_tools = tool_names(&first_body);
    assert!(
        first_tools
            .iter()
            .all(|name| name != "exec" && name != "wait"),
        "unavailable code mode must not expose code-mode tools: {first_tools:?}"
    );
    assert!(
        first_warnings.iter().any(|warning| {
            warning.contains("Code Mode is unavailable")
                && warning.contains("codex-code-mode-host-does-not-exist")
        }),
        "missing host should produce an actionable warning: {first_warnings:?}"
    );

    let (_, second_warnings) = run_unavailable_code_mode_turn(&server, &test).await?;
    assert!(
        second_warnings
            .iter()
            .all(|warning| !warning.contains("Code Mode is unavailable")),
        "host availability warning should be emitted once per thread: {second_warnings:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn missing_process_host_keeps_code_mode_only_and_fails_closed() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let builder = test_codex()
        .with_model("test-gpt-5.1-codex")
        .with_code_mode_host_program("codex-code-mode-host-does-not-exist".into())
        .with_config(|config| {
            config
                .features
                .enable(Feature::CodeModeOnly)
                .expect("code mode should be enabled");
        });
    let (_test, follow_up_mock) = run_code_mode_turn_with_builder(
        &server,
        "Run required code mode",
        "text('unreachable')",
        builder,
    )
    .await?;
    let request = follow_up_mock.single_request();
    let tools = tool_names(&request.body_json());
    assert!(
        tools.iter().any(|name| name == "exec") && tools.iter().any(|name| name == "wait"),
        "code-mode-only must retain code-mode tools: {tools:?}"
    );
    assert!(
        tools
            .iter()
            .all(|name| !matches!(name.as_str(), "shell" | "exec_command")),
        "code-mode-only must never expose direct shell tools: {tools:?}"
    );
    let (output, _) = custom_tool_output_body_and_success(&request, "call-1");
    assert!(
        output.contains("codex-code-mode-host-does-not-exist"),
        "code-mode-only must report the host failure: {output}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn missing_process_host_fails_closed_when_direct_fallback_is_disabled() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let builder = test_codex()
        .with_model("test-gpt-5.1-codex")
        .with_code_mode_host_program("codex-code-mode-host-does-not-exist".into())
        .with_config(|config| {
            config
                .features
                .enable(Feature::CodeMode)
                .expect("code mode should be enabled");
            config.code_mode.disable_in_process_fallback = true;
        });
    let (_test, follow_up_mock) = run_code_mode_turn_with_builder(
        &server,
        "Run required code mode",
        "text('unreachable')",
        builder,
    )
    .await?;
    let request = follow_up_mock.single_request();
    let tools = tool_names(&request.body_json());
    assert!(
        tools.iter().any(|name| name == "exec") && tools.iter().any(|name| name == "wait"),
        "disabled fallback must retain code-mode tools: {tools:?}"
    );
    let (output, _) = custom_tool_output_body_and_success(&request, "call-1");
    assert!(
        output.contains("codex-code-mode-host-does-not-exist"),
        "disabled fallback must report the host failure: {output}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disabled_process_host_with_fallback_disabled_attempts_the_host() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let builder = test_codex()
        .with_model("test-gpt-5.1-codex")
        .with_config(|config| {
            config
                .features
                .enable(Feature::CodeMode)
                .expect("code mode should be enabled");
            config
                .features
                .disable(Feature::CodeModeHost)
                .expect("code-mode host should be disabled");
            config.code_mode.disable_in_process_fallback = true;
        });
    let (_test, follow_up_mock) = run_code_mode_turn_with_builder(
        &server,
        "Run required code mode",
        "text('unreachable')",
        builder,
    )
    .await?;
    let request = follow_up_mock.single_request();
    let (output, _) = custom_tool_output_body_and_success(&request, "call-1");
    assert!(
        output.contains("failed to spawn code-mode host"),
        "disabled fallback must still attempt the standalone host: {output}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_can_call_standalone_web_search() -> Result<()> {
    assert_code_mode_standalone_web_search(WebSearchMode::Live, serde_json::json!(true)).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_can_call_indexed_standalone_web_search() -> Result<()> {
    assert_code_mode_standalone_web_search(WebSearchMode::Indexed, serde_json::json!("indexed"))
        .await
}

async fn assert_code_mode_standalone_web_search(
    web_search_mode: WebSearchMode,
    expected_external_web_access: Value,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    Mock::given(method("POST"))
        .and(path("/v1/alpha/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "output": "Search result",
        })))
        .expect(1)
        .mount(&server)
        .await;

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call(
                "call-1",
                "exec",
                r#"
const result = await tools.web__run({
  search_query: [{ q: "standalone web search" }],
});
text(result);
"#,
            ),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let follow_up_mock = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    let auth = CodexAuth::from_api_key("dummy");
    let auth_manager = codex_core::test_support::auth_manager_from_auth(auth.clone());
    let mut extension_builder = ExtensionRegistryBuilder::<Config>::new();
    install_web_search_extension(&mut extension_builder, auth_manager);
    let mut builder = test_codex()
        .with_auth(auth)
        .with_extensions(Arc::new(extension_builder.build()))
        .with_model("test-gpt-5.1-codex")
        .with_config(move |config| {
            config
                .features
                .enable(Feature::CodeMode)
                .expect("code mode should be enabled");
            config
                .features
                .enable(Feature::StandaloneWebSearch)
                .expect("standalone web search should be enabled");
            config
                .web_search_mode
                .set(web_search_mode)
                .expect("web search mode should be accepted");
        });
    let test = builder.build(&server).await?;

    test.submit_turn("Search the web from code mode").await?;

    let search_request = server
        .received_requests()
        .await
        .expect("received requests should be available")
        .into_iter()
        .find(|request| request.url.path() == "/v1/alpha/search")
        .expect("standalone search request should be sent");
    let search_body = search_request
        .body_json::<Value>()
        .expect("search request body should be JSON");
    assert_eq!(
        search_body["model"],
        serde_json::json!("test-gpt-5.1-codex")
    );
    assert_eq!(
        search_body["commands"],
        serde_json::json!({
            "search_query": [{"q": "standalone web search"}],
        })
    );
    assert_eq!(
        search_body["settings"],
        serde_json::json!({
            "allowed_callers": ["direct"],
            "external_web_access": expected_external_web_access,
        })
    );
    assert_eq!(
        custom_tool_output_last_non_empty_text(&follow_up_mock.single_request(), "call-1"),
        Some("Search result".to_string())
    );

    Ok(())
}

async fn run_code_mode_turn_with_rmcp(
    server: &MockServer,
    prompt: &str,
    code: &str,
) -> Result<(TestCodex, ResponseMock)> {
    run_code_mode_turn_with_rmcp_model(server, prompt, code, "test-gpt-5.1-codex").await
}

async fn run_code_mode_turn_with_rmcp_model(
    server: &MockServer,
    prompt: &str,
    code: &str,
    model: &'static str,
) -> Result<(TestCodex, ResponseMock)> {
    run_code_mode_turn_with_rmcp_config(
        server, prompt, code, model, /*code_mode_only*/ false,
        /*non_prefixed_mcp_tool_names*/ false,
    )
    .await
}

async fn run_code_mode_turn_with_rmcp_mode(
    server: &MockServer,
    prompt: &str,
    code: &str,
    code_mode_only: bool,
) -> Result<(TestCodex, ResponseMock)> {
    run_code_mode_turn_with_rmcp_config(
        server,
        prompt,
        code,
        "test-gpt-5.1-codex",
        code_mode_only,
        /*non_prefixed_mcp_tool_names*/ false,
    )
    .await
}

async fn run_code_mode_turn_with_rmcp_config(
    server: &MockServer,
    prompt: &str,
    code: &str,
    model: &'static str,
    code_mode_only: bool,
    non_prefixed_mcp_tool_names: bool,
) -> Result<(TestCodex, ResponseMock)> {
    let rmcp_test_server_bin = stdio_server_bin()?;
    let mut builder = test_codex().with_model(model).with_config(move |config| {
        let _ = if code_mode_only {
            config.features.enable(Feature::CodeModeOnly)
        } else {
            config.features.enable(Feature::CodeMode)
        };
        if non_prefixed_mcp_tool_names {
            let _ = config.features.enable(Feature::NonPrefixedMcpToolNames);
        }

        let mut servers = config.mcp_servers.get().clone();
        servers.insert(
            "rmcp".to_string(),
            McpServerConfig {
                auth: Default::default(),
                transport: McpServerTransportConfig::Stdio {
                    command: rmcp_test_server_bin,
                    args: Vec::new(),
                    env: Some(HashMap::from([(
                        "MCP_TEST_VALUE".to_string(),
                        "propagated-env".to_string(),
                    )])),
                    env_vars: Vec::new(),
                    cwd: None,
                },
                environment_id: "local".to_string(),
                enabled: true,
                required: false,
                supports_parallel_tool_calls: false,
                omit_tools_from: None,
                disabled_reason: None,
                startup_timeout_sec: Some(Duration::from_secs(10)),
                tool_timeout_sec: None,
                default_tools_approval_mode: None,
                enabled_tools: None,
                disabled_tools: None,
                scopes: None,
                oauth: None,
                oauth_resource: None,
                tools: HashMap::new(),
            },
        );
        config
            .mcp_servers
            .set(servers)
            .expect("test mcp servers should accept any configuration");
    });
    let test = builder.build(server).await?;
    wait_for_mcp_server(&test.codex, "rmcp").await?;

    responses::mount_sse_once(
        server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call("call-1", "exec", code),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let second_mock = responses::mount_sse_once(
        server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn(prompt).await?;
    Ok((test, second_mock))
}

#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_can_return_exec_command_output() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (_test, second_mock) = run_code_mode_turn(
        &server,
        "use exec to run exec_command",
        r#"
text(JSON.stringify(await tools.exec_command({ cmd: "printf code_mode_exec_marker" })));
"#,
    )
    .await?;

    let items = custom_tool_output_items(&second_mock.single_request(), "call-1");
    assert_eq!(items.len(), 2);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script completed\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&items, /*index*/ 0),
    );
    let parsed: Value = serde_json::from_str(text_item(&items, /*index*/ 1))?;
    assert!(
        parsed
            .get("chunk_id")
            .and_then(Value::as_str)
            .is_some_and(|chunk_id| !chunk_id.is_empty())
    );
    assert_eq!(
        parsed.get("output").and_then(Value::as_str),
        Some("code_mode_exec_marker"),
    );
    assert_eq!(parsed.get("exit_code").and_then(Value::as_i64), Some(0));
    assert!(parsed.get("wall_time_seconds").is_some());
    assert!(parsed.get("session_id").is_none());

    Ok(())
}

#[tokio::test]
async fn code_mode_exec_holds_captured_result_during_elicitation() -> Result<()> {
    let server = responses::start_mock_server().await;
    let mut builder = test_codex()
        .with_model("test-gpt-5.1-codex")
        .with_config(|config| {
            let _ = config.features.enable(Feature::CodeMode);
        });
    let test = builder.build(&server).await?;

    let first_mock = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call("call-1", "exec", "text('captured');"),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let second_mock = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    assert_eq!(
        test.codex.increment_out_of_band_elicitation_count().await?,
        1
    );
    assert_eq!(
        test.codex.increment_out_of_band_elicitation_count().await?,
        2
    );
    let release_elicitation = async {
        tokio::time::timeout(Duration::from_secs(5), async {
            while first_mock.requests().is_empty() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("initial response request should arrive");
        tokio::time::sleep(Duration::from_millis(250)).await;
        assert!(
            second_mock.requests().is_empty(),
            "captured exec result should not return during an elicitation"
        );
        assert_eq!(
            test.codex.decrement_out_of_band_elicitation_count().await?,
            1
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(
            second_mock.requests().is_empty(),
            "captured exec result should wait for every elicitation"
        );
        assert_eq!(
            test.codex.decrement_out_of_band_elicitation_count().await?,
            0
        );
        Ok::<(), anyhow::Error>(())
    };

    tokio::try_join!(test.submit_turn("run code mode"), release_elicitation)?;
    second_mock.single_request();
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_only_restricts_prompt_tools() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let resp_mock = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let mut builder = test_codex().with_config(|config| {
        let _ = config.features.enable(Feature::CodeModeOnly);
    });
    let test = builder.build(&server).await?;
    test.submit_turn("list tools in code mode only").await?;

    let first_body = resp_mock.single_request().body_json();
    assert_eq!(
        tool_names(&first_body),
        vec![
            "exec".to_string(),
            "wait".to_string(),
            "request_user_input".to_string(),
            "web_search".to_string()
        ]
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_excludes_mcp_servers_using_their_configured_identity() -> Result<()> {
    skip_if_wine_exec!(
        Ok(()),
        "requires a Windows test_stdio_server in the Wine-exec environment"
    );
    skip_if_no_network!(Ok(()));

    for omit_tools_from in [
        &["code_mode"][..],
        &["deferred"][..],
        &["direct"][..],
        &["code_mode", "deferred"][..],
        &["code_mode", "direct"][..],
        &["deferred", "direct"][..],
        &["code_mode", "deferred", "direct"][..],
        &[][..],
    ] {
        for (code_mode_only, supports_search_tool, non_prefixed_mcp_tool_names) in [
            (false, false, false),
            (false, false, true),
            (false, true, false),
            (false, true, true),
            (true, false, false),
            (true, false, true),
            (true, true, false),
            (true, true, true),
        ] {
            let server = responses::start_mock_server().await;
            let response = responses::mount_sse_once(
                &server,
                sse(vec![
                    ev_response_created("resp-1"),
                    ev_assistant_message("msg-1", "done"),
                    ev_completed("resp-1"),
                ]),
            )
            .await;
            let rmcp_test_server_bin = remote_aware_stdio_server_bin()?;
            let environment_id = remote_aware_environment_id();
            let mut builder = test_codex()
                .with_model_info_override("gpt-5.4", move |model| {
                    model.supports_search_tool = supports_search_tool;
                })
                .with_config(move |config| {
                    config
                        .features
                        .enable(if code_mode_only {
                            Feature::CodeModeOnly
                        } else {
                            Feature::CodeMode
                        })
                        .expect("test config should allow Code Mode");
                    if non_prefixed_mcp_tool_names {
                        config
                            .features
                            .enable(Feature::NonPrefixedMcpToolNames)
                            .expect("test config should allow unprefixed MCP tools");
                    }

                    let mut servers = config.mcp_servers.get().clone();
                    servers.insert(
                        "rmcp".to_string(),
                        serde_json::from_value(serde_json::json!({
                            "command": rmcp_test_server_bin,
                            "environment_id": environment_id,
                            "omit_tools_from": omit_tools_from,
                        }))
                        .expect("test MCP server config should be valid"),
                    );
                    config
                        .mcp_servers
                        .set(servers)
                        .expect("test config should allow MCP servers");
                });
            let base_test = builder.build_with_auto_env(&server).await?;
            let namespace = if non_prefixed_mcp_tool_names {
                "rmcp"
            } else {
                "mcp__rmcp"
            };
            let new_thread = base_test
                .thread_manager
                .start_thread(StartThreadOptions {
                    dynamic_tools: vec![DynamicToolSpec::Namespace(DynamicToolNamespaceSpec {
                        name: namespace.to_string(),
                        description: "An unrelated dynamic tool sharing the MCP namespace."
                            .to_string(),
                        tools: vec![DynamicToolNamespaceTool::Function(
                            DynamicToolFunctionSpec {
                                name: "unrelated".to_string(),
                                description: "A dynamic tool that must remain in Code Mode."
                                    .to_string(),
                                input_schema: serde_json::json!({
                                    "type": "object",
                                    "properties": {},
                                    "additionalProperties": false,
                                }),
                                defer_loading: false,
                            },
                        )],
                    })],
                    ..StartThreadOptions::new(base_test.config.clone())
                })
                .await?;
            let mut test = base_test;
            test.codex = new_thread.thread;
            test.session_configured = new_thread.session_configured;
            wait_for_mcp_server(&test.codex, "rmcp").await?;
            test.submit_turn("inspect the directly callable MCP tool")
                .await?;

            let body = response.single_request().body_json();
            let omit_code_mode = omit_tools_from.contains(&"code_mode");
            let omit_deferred = omit_tools_from.contains(&"deferred");
            let omit_direct = omit_tools_from.contains(&"direct");
            let available_directly = !omit_direct
                && (!supports_search_tool || omit_deferred || code_mode_only && omit_code_mode);
            let visible_directly = available_directly && (!code_mode_only || omit_code_mode);
            let echo_tool = namespace_child_tool(&body, namespace, "echo");
            assert_eq!(
                echo_tool.is_some(),
                visible_directly,
                "MCP omissions must determine initial prompt exposure; \
             omit_tools_from={omit_tools_from:?}, code_mode_only={code_mode_only}, \
             supports_search_tool={supports_search_tool}, namespace={namespace}, tools={:?}",
                tool_names(&body)
            );
            if let Some(echo_tool) = echo_tool {
                assert_eq!(
                    echo_tool["description"]
                        .as_str()
                        .is_some_and(|description| description.contains("exec tool declaration:")),
                    !omit_code_mode,
                    "the MCP tool description must agree with Code Mode availability; \
                 omit_tools_from={omit_tools_from:?}, code_mode_only={code_mode_only}, \
                 namespace={namespace}"
                );
            }

            let exec_description = body["tools"]
                .as_array()
                .expect("request should contain tools")
                .iter()
                .find_map(|tool| {
                    (tool["name"].as_str() == Some("exec"))
                        .then(|| tool["description"].as_str())
                        .flatten()
                })
                .expect("Code Mode exec should remain available");
            let nested_tool_name = format!("{namespace}__echo");
            if code_mode_only {
                assert_eq!(
                    exec_description.contains(&nested_tool_name),
                    !omit_code_mode && (!supports_search_tool || omit_deferred),
                    "the Code Mode declaration must agree with MCP tool exposure; \
                 omit_tools_from={omit_tools_from:?}, \
                 supports_search_tool={supports_search_tool}, \
                 nested_tool_name={nested_tool_name}"
                );
            }
            let unrelated_nested_tool_name = format!("{namespace}__unrelated");
            if code_mode_only {
                assert!(
                    exec_description.contains(&unrelated_nested_tool_name),
                    "MCP omissions must not exclude unrelated tools sharing its namespace; \
                 omit_tools_from={omit_tools_from:?}, \
                 supports_search_tool={supports_search_tool}, \
                 nested_tool_name={unrelated_nested_tool_name}"
                );
            } else {
                let unrelated_tool = namespace_child_tool(&body, namespace, "unrelated")
                    .unwrap_or_else(|| {
                        panic!(
                            "MCP omissions must not hide an unrelated same-namespace tool; \
                         omit_tools_from={omit_tools_from:?}, \
                         supports_search_tool={supports_search_tool}, namespace={namespace}, \
                         tools={:?}",
                            tool_names(&body)
                        )
                    });
                assert!(
                    unrelated_tool["description"]
                        .as_str()
                        .is_some_and(|description| {
                            description.contains(&unrelated_nested_tool_name)
                                && description.contains("exec tool declaration:")
                        }),
                    "an unrelated same-namespace tool must remain available inside Code Mode; \
                 omit_tools_from={omit_tools_from:?}, \
                 supports_search_tool={supports_search_tool}, \
                 nested_tool_name={unrelated_nested_tool_name}"
                );
            }
        }
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_code_mode_exclusion_does_not_change_direct_mode_tool_exposure() -> Result<()> {
    skip_if_wine_exec!(
        Ok(()),
        "requires a Windows test_stdio_server in the Wine-exec environment"
    );
    skip_if_no_network!(Ok(()));

    for omit_tools_from in [
        &["code_mode"][..],
        &["deferred"][..],
        &["direct"][..],
        &["code_mode", "deferred"][..],
        &["code_mode", "direct"][..],
        &["deferred", "direct"][..],
        &["code_mode", "deferred", "direct"][..],
        &[][..],
    ] {
        for (supports_search_tool, non_prefixed_mcp_tool_names, direct_only_namespace) in [
            (false, false, false),
            (false, false, true),
            (false, true, false),
            (false, true, true),
            (true, false, false),
            (true, false, true),
            (true, true, false),
            (true, true, true),
        ] {
            let namespace = if non_prefixed_mcp_tool_names {
                "rmcp"
            } else {
                "mcp__rmcp"
            };
            let visible_directly = !omit_tools_from.contains(&"direct")
                && (!supports_search_tool
                    || omit_tools_from.contains(&"deferred")
                    || direct_only_namespace);
            let server = responses::start_mock_server().await;
            let response = responses::mount_sse_once(
                &server,
                sse(vec![
                    ev_response_created("resp-1"),
                    if visible_directly {
                        responses::ev_function_call_with_namespace(
                            "call-1",
                            namespace,
                            "echo",
                            r#"{"message":"ping"}"#,
                        )
                    } else {
                        ev_assistant_message("msg-1", "done")
                    },
                    ev_completed("resp-1"),
                ]),
            )
            .await;
            let follow_up_response = if visible_directly {
                Some(
                    responses::mount_sse_once(
                        &server,
                        sse(vec![
                            ev_assistant_message("msg-1", "done"),
                            ev_completed("resp-2"),
                        ]),
                    )
                    .await,
                )
            } else {
                None
            };
            let rmcp_test_server_bin = remote_aware_stdio_server_bin()?;
            let environment_id = remote_aware_environment_id();
            let mut builder = test_codex()
                .with_model_info_override("gpt-5.4", move |model| {
                    model.supports_search_tool = supports_search_tool;
                })
                .with_config(move |config| {
                    if non_prefixed_mcp_tool_names {
                        config
                            .features
                            .enable(Feature::NonPrefixedMcpToolNames)
                            .expect("test config should allow unprefixed MCP tools");
                    }
                    if direct_only_namespace {
                        config.code_mode.direct_only_tool_namespaces = vec![
                            if non_prefixed_mcp_tool_names {
                                "rmcp"
                            } else {
                                "mcp__rmcp"
                            }
                            .to_string(),
                        ];
                    }

                    let mut servers = config.mcp_servers.get().clone();
                    servers.insert(
                        "rmcp".to_string(),
                        serde_json::from_value(serde_json::json!({
                            "command": rmcp_test_server_bin,
                            "environment_id": environment_id,
                            "omit_tools_from": omit_tools_from,
                        }))
                        .expect("test MCP server config should be valid"),
                    );
                    config
                        .mcp_servers
                        .set(servers)
                        .expect("test config should allow MCP servers");
                });
            let test = builder.build_with_auto_env(&server).await?;
            wait_for_mcp_server(&test.codex, "rmcp").await?;
            test.submit_turn("inspect ordinary direct-mode MCP tool exposure")
                .await?;

            let body = response.single_request().body_json();
            assert_eq!(
                namespace_child_tool(&body, namespace, "echo").is_some(),
                visible_directly,
                "MCP omissions must determine direct-mode tool exposure; \
             omit_tools_from={omit_tools_from:?}, \
             supports_search_tool={supports_search_tool}, \
             direct_only_namespace={direct_only_namespace}, namespace={namespace}, tools={:?}",
                tool_names(&body)
            );
            assert_eq!(
                tool_names(&body).iter().any(|name| name == "tool_search"),
                supports_search_tool,
                "MCP omissions must not disable tool search for other tools; \
             omit_tools_from={omit_tools_from:?}, \
             supports_search_tool={supports_search_tool}, \
             direct_only_namespace={direct_only_namespace}, namespace={namespace}, tools={:?}",
                tool_names(&body)
            );
            if let Some(follow_up_response) = follow_up_response {
                let output = follow_up_response
                    .single_request()
                    .function_call_output("call-1");
                assert!(
                    output["output"]
                        .as_str()
                        .is_some_and(|output| output.contains("ECHOING: ping")),
                    "the advertised MCP tool must remain callable; \
                     omit_tools_from={omit_tools_from:?}, \
                     supports_search_tool={supports_search_tool}, \
                     direct_only_namespace={direct_only_namespace}, \
                     namespace={namespace}, output={output:?}"
                );
            }
        }
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_only_guides_all_tools_search_and_calls_deferred_app_tools() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let apps_server = AppsTestServer::mount_searchable(&server).await?;
    let resp_mock = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call(
                "call-1",
                "exec",
                r#"
const tool = ALL_TOOLS.find(
  ({ name }) => name === "mcp__codex_apps__calendar_timezone_option_99"
);
if (!tool) {
  text(JSON.stringify({ found: false }));
} else {
  const result = await tools[tool.name]({ timezone: "UTC" });
  text(JSON.stringify({
    found: true,
    isError: Boolean(result.isError),
    text: result.content?.[0]?.text ?? "",
  }));
}
"#,
            ),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let follow_up_mock = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    let apps_base_url = apps_server.chatgpt_base_url.clone();
    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(move |config| {
            config
                .features
                .enable(Feature::Apps)
                .expect("test config should allow feature update");
            config
                .features
                .enable(Feature::CodeMode)
                .expect("test config should allow feature update");
            config
                .features
                .enable(Feature::CodeModeOnly)
                .expect("test config should allow feature update");
            let mut model_catalog =
                bundled_models_response().expect("bundled models.json should parse");
            let model = model_catalog
                .models
                .iter_mut()
                .find(|model| model.slug == "gpt-5.4")
                .expect("gpt-5.4 exists in bundled models.json");
            config.chatgpt_base_url = apps_base_url;
            config.model = Some("gpt-5.4".to_string());
            model.supports_search_tool = true;
            config.model_catalog = Some(model_catalog);
        });
    let test = builder.build(&server).await?;
    test.submit_turn("inspect tools in code mode only").await?;

    let first_body = resp_mock.single_request().body_json();
    assert_eq!(
        tool_names(&first_body),
        vec![
            "exec".to_string(),
            "wait".to_string(),
            "request_user_input".to_string(),
            "web_search".to_string()
        ]
    );

    let exec_description = first_body
        .get("tools")
        .and_then(Value::as_array)
        .and_then(|tools| {
            tools.iter().find_map(|tool| {
                if tool
                    .get("name")
                    .or_else(|| tool.get("type"))
                    .and_then(Value::as_str)
                    == Some("exec")
                {
                    tool.get("description").and_then(Value::as_str)
                } else {
                    None
                }
            })
        })
        .expect("exec description should be present");
    assert!(exec_description.contains("filter `ALL_TOOLS` by `name` and `description`"));
    assert!(exec_description.contains("Shared MCP Types:"));
    assert!(!exec_description.contains("calendar_timezone_option_99"));

    let request = follow_up_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&request, "call-1");
    assert_ne!(
        success,
        Some(false),
        "code_mode_only deferred app tool call failed unexpectedly: {output}"
    );
    let parsed: Value = serde_json::from_str(&output)?;
    assert_eq!(
        parsed,
        serde_json::json!({
            "found": true,
            "isError": false,
            "text": "called calendar_timezone_option_99 for  at  with ",
        })
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn app_only_tools_are_not_visible_or_runnable_by_code_mode_model() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let apps_server =
        AppsTestServer::mount_with_app_only_tool(&server, AppsTestToolLoading::Searchable).await?;
    let code = format!(
        r#"
const visibleTool = ALL_TOOLS.find(({{ name }}) => name === {visible_tool_name:?});
const tool = ALL_TOOLS.find(({{ name }}) => name === {tool_name:?});
let error = null;
try {{
  await tools[{tool_name:?}]({{}});
}} catch (caught) {{
  error = String(caught);
}}
text(JSON.stringify({{
  visibleListed: visibleTool !== undefined,
  listed: tool !== undefined,
  callable: typeof tools[{tool_name:?}] === "function",
  error,
}}));
"#,
        visible_tool_name = "mcp__codex_apps__calendar_timezone_option_99",
        tool_name = DIRECT_CALENDAR_APP_ONLY_TOOL,
    );

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call("call-1", "exec", &code),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let second_mock = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    let mut builder = search_capable_apps_builder(apps_server.chatgpt_base_url.clone())
        .with_config(|config| {
            config
                .features
                .enable(Feature::CodeMode)
                .expect("test config should allow feature update");
            config
                .features
                .enable(Feature::CodeModeOnly)
                .expect("test config should allow feature update");
        });
    let test = builder.build(&server).await?;
    test.submit_turn("try to call the app-only calendar tool through exec")
        .await?;

    let req = second_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "code mode visibility check should complete successfully: {output}"
    );
    let parsed: Value = serde_json::from_str(&output)?;
    assert_eq!(parsed["visibleListed"], true);
    assert_eq!(parsed["listed"], false);
    assert_eq!(parsed["callable"], false);
    assert!(
        parsed["error"]
            .as_str()
            .is_some_and(|error| error.contains("is not a function")),
        "app-only code mode call should fail before MCP dispatch: {parsed:?}"
    );
    assert!(
        recorded_apps_tool_calls(&server).await.is_empty(),
        "app-only code mode call should not reach the MCP server"
    );

    Ok(())
}

#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_only_can_call_nested_tools() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call(
                "call-1",
                "exec",
                r#"
const output = await tools.exec_command({ cmd: "printf code_mode_only_nested_tool_marker" });
text(output.output);
"#,
            ),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let follow_up_mock = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    let mut builder = test_codex().with_config(|config| {
        let _ = config.features.enable(Feature::CodeModeOnly);
    });
    let test = builder.build(&server).await?;
    test.submit_turn("use exec to run nested tool in code mode only")
        .await?;

    let request = follow_up_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&request, "call-1");
    assert_ne!(
        success,
        Some(false),
        "code_mode_only nested tool call failed unexpectedly: {output}"
    );
    assert_eq!(output, "code_mode_only_nested_tool_marker");
    assert!(
        request
            .custom_tool_call_output("call-1")
            .pointer("/internal_chat_message_metadata_passthrough/executed_tool_calls")
            .is_none(),
        "nested tool calls must not be recorded while executed-tool-call metadata is disabled",
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_update_plan_nested_tool_result_is_empty_object() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (_test, second_mock) = run_code_mode_turn(
        &server,
        "use exec to run update_plan",
        r#"
const result = await tools.update_plan({
  plan: [{ step: "Run update_plan from code mode", status: "in_progress" }],
});
text(JSON.stringify(result));
"#,
    )
    .await?;

    let req = second_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "exec update_plan call failed unexpectedly: {output}"
    );

    let parsed: Value = serde_json::from_str(&output)?;
    assert_eq!(parsed, serde_json::json!({}));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_get_context_remaining_returns_structured_result() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (_test, second_mock) = run_code_mode_turn_with_config(
        &server,
        "use exec to get remaining context",
        r#"
const result = await tools.get_context_remaining({});
text(JSON.stringify(result));
"#,
        |config| {
            config.model_context_window = Some(10_000);
            config
                .features
                .enable(Feature::TokenBudget)
                .expect("test config should allow token budget");
        },
    )
    .await?;

    let req = second_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "exec get_context_remaining call failed unexpectedly: {output}"
    );

    let parsed: Value = serde_json::from_str(&output)?;
    assert_eq!(
        parsed,
        serde_json::json!({
            "tokens_left": 9000,
        })
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_current_time_returns_structured_result() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (_test, second_mock) = run_code_mode_turn_with_config(
        &server,
        "use exec to get the current time",
        r#"
const result = await tools.clock__curr_time({});
text(JSON.stringify(result));
"#,
        |config| {
            config
                .features
                .enable(Feature::CurrentTimeReminder)
                .expect("test config should allow current-time reminders");
            config.current_time_reminder = Some(CurrentTimeReminderConfig {
                reminder_interval_seconds: 3_000,
                clock_source: CurrentTimeSource::System,
                ..CurrentTimeReminderConfig::default()
            });
        },
    )
    .await?;

    let req = second_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "exec clock.curr_time call failed unexpectedly: {output}"
    );

    let parsed: Value = serde_json::from_str(&output)?;
    let current_time = parsed
        .get("current_time")
        .and_then(Value::as_str)
        .expect("clock.curr_time should return current_time");
    assert_regex_match(r"^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2} UTC$", current_time);

    Ok(())
}

#[cfg_attr(windows, ignore = "flaky on windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_nested_tool_calls_can_run_in_parallel() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mut builder = test_codex()
        .with_model("test-gpt-5.1-codex")
        .with_config(move |config| {
            let _ = config.features.enable(Feature::CodeMode);
        });
    let test = builder.build(&server).await?;

    let warmup_code = r#"
const args = {
  sleep_after_ms: 10,
  barrier: {
    id: "code-mode-parallel-tools-warmup",
    participants: 2,
    timeout_ms: 1_000,
  },
};

await Promise.all([
  tools.test_sync_tool(args),
  tools.test_sync_tool(args),
]);
"#;
    let code = r#"
const args = {
  sleep_after_ms: 300,
  barrier: {
    id: "code-mode-parallel-tools",
    participants: 2,
    timeout_ms: 1_000,
  },
};

const results = await Promise.all([
  tools.test_sync_tool(args),
  tools.test_sync_tool(args),
]);

text(JSON.stringify(results));
"#;

    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-warm-1"),
                ev_custom_tool_call("call-warm-1", "exec", warmup_code),
                ev_completed("resp-warm-1"),
            ]),
            sse(vec![
                ev_assistant_message("msg-warm-1", "warmup done"),
                ev_completed("resp-warm-2"),
            ]),
            sse(vec![
                ev_response_created("resp-1"),
                ev_custom_tool_call("call-1", "exec", code),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    test.submit_turn("warm up nested tools in parallel").await?;

    let start = Instant::now();
    test.submit_turn("run nested tools in parallel").await?;
    let duration = start.elapsed();

    assert!(
        duration < Duration::from_millis(1_600),
        "expected nested tools to finish in parallel, got {duration:?}",
    );

    let req = response_mock
        .last_request()
        .expect("parallel code mode run should send a completion request");
    let items = custom_tool_output_items(&req, "call-1");
    assert_eq!(items.len(), 2);
    assert_eq!(text_item(&items, /*index*/ 1), "[\"ok\",\"ok\"]");

    Ok(())
}

#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_write_stdin_calls_run_in_parallel_across_sessions() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (_test, second_mock) = run_code_mode_turn(
        &server,
        "write to independent terminal sessions in parallel",
        r#"
const sessions = await Promise.all([
  tools.exec_command({ cmd: "bash --noprofile --norc", tty: true, yield_time_ms: 250 }),
  tools.exec_command({ cmd: "bash --noprofile --norc", tty: true, yield_time_ms: 250 }),
]);

const results = await Promise.all([
  tools.write_stdin({
    session_id: sessions[0].session_id,
    chars: ": > .code-mode-a; while [ ! -e .code-mode-b ]; do sleep 0.01; done; printf 'code-alpha-%s\\n' ready; exit\n",
    yield_time_ms: 5000,
  }),
  tools.write_stdin({
    session_id: sessions[1].session_id,
    chars: ": > .code-mode-b; while [ ! -e .code-mode-a ]; do sleep 0.01; done; printf 'code-beta-%s\\n' ready; exit\n",
    yield_time_ms: 5000,
  }),
]);

text(JSON.stringify([results[0].output.includes("code-alpha-ready"), results[1].output.includes("code-beta-ready")]));
"#,
    )
    .await?;

    let items = custom_tool_output_items(&second_mock.single_request(), "call-1");
    let result: Value = serde_json::from_str(text_item(&items, /*index*/ 1))?;
    assert_eq!(result, serde_json::json!([true, true]));

    Ok(())
}

// This model uses token-based tool-output truncation, giving the downstream
// history assertions a stable `…N tokens truncated…` marker.
const TOKEN_POLICY_TEST_MODEL: &str = "gpt-5.4";

// A nested `exec_command` limit applies to `result.output` inside JavaScript.
// The outer code-mode and history budgets apply after the script calls `text`.
#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_exec_nested_limit_formats_truncated_result_with_warning() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (_test, second_mock) = run_code_mode_turn(
        &server,
        "use exec_command from code mode",
        r#"
const result = await tools.exec_command({
  cmd: "printf '0123456789012345678901234567890123456789'",
  max_output_tokens: 5
});
text(result.output);
"#,
    )
    .await?;

    assert_eq!(
        text_item(
            &custom_tool_output_items(&second_mock.single_request(), "call-1"),
            /*index*/ 1
        ),
        "Warning: truncated output (original token count: 10)\nTotal output lines: 1\n\n0123456789…5 tokens truncated…0123456789"
    );

    Ok(())
}

#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_exec_nested_limit_preserves_result_variable_before_default_history_truncation()
-> Result<()> {
    // TODO(anp): Remove after Wine exec returns complete nested-tool output to code mode.
    skip_if_wine_exec!(
        Ok(()),
        "only part of nested exec_command stdout reaches the code-mode result"
    );
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (_test, second_mock) = run_code_mode_turn_with_model_and_config(
        &server,
        "use exec_command from code mode",
        r#"// @exec: {"max_output_tokens": 20000}
const result = await tools.exec_command({
  cmd: "python3 -c \"import sys; sys.stdout.write('x' * 50000)\"",
  max_output_tokens: 20000
});
const resultVariableWasTruncated = result.output.length !== 50000;
text(`Variable truncated: ${resultVariableWasTruncated ? "True" : "False"}. Variable: ${result.output}`);
"#,
        TOKEN_POLICY_TEST_MODEL,
        |_| {},
    )
    .await?;

    let items = custom_tool_output_items(&second_mock.single_request(), "call-1");
    let output = text_item(&items, /*index*/ 1);
    assert_regex_match(
        r"^Variable truncated: False\. Variable: x+…\d+ tokens truncated…x+$",
        output,
    );

    Ok(())
}

#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_exec_nested_limit_truncates_result_variable_when_exceeded() -> Result<()> {
    // TODO(anp): Remove after Wine exec returns complete nested-tool output to code mode.
    skip_if_wine_exec!(
        Ok(()),
        "only part of nested exec_command stdout reaches the code-mode result"
    );
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (_test, second_mock) = run_code_mode_turn_with_model_and_config(
        &server,
        "use exec_command from code mode",
        r#"// @exec: {"max_output_tokens": 25000}
const result = await tools.exec_command({
  cmd: "python3 -c \"import sys; sys.stdout.write('A' * 90000)\"",
  max_output_tokens: 20000
});
const resultVariableWasTruncated = result.output.includes("…2500 tokens truncated…");
text(`Variable truncated: ${resultVariableWasTruncated ? "True" : "False"}. Variable: ${result.output}`);
"#,
        TOKEN_POLICY_TEST_MODEL,
        |_| {},
    )
    .await?;

    let items = custom_tool_output_items(&second_mock.single_request(), "call-1");
    let output = text_item(&items, /*index*/ 1);
    // The nested 20,000-token budget leaves about 80,000 characters. This
    // ceiling independently proves that history applied its smaller cap.
    assert!(
        output.len() < 60_000,
        "expected history to truncate the emitted value, got {} bytes",
        output.len()
    );
    // The boolean describes the nested result; the marker below comes from
    // history truncating the value emitted with `text` afterward.
    assert_regex_match(
        r"(?s)^Variable truncated: True\. Variable: .*…\d+ tokens truncated…A+$",
        output,
    );

    Ok(())
}

#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_exec_nested_limit_preserves_result_variable_before_configured_history_truncation()
-> Result<()> {
    // TODO(anp): Remove after Wine exec returns complete nested-tool output to code mode.
    skip_if_wine_exec!(
        Ok(()),
        "only part of nested exec_command stdout reaches the code-mode result"
    );
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (_test, second_mock) = run_code_mode_turn_with_model_and_config(
        &server,
        "use exec_command from code mode",
        r#"// @exec: {"max_output_tokens": 20000}
const result = await tools.exec_command({
  cmd: "python3 -c \"import sys; sys.stdout.write('x' * 50000)\"",
  max_output_tokens: 20000
});
const resultVariableWasTruncated = result.output.length !== 50000;
text(`Variable truncated: ${resultVariableWasTruncated ? "True" : "False"}. Variable: ${result.output}`);
"#,
        TOKEN_POLICY_TEST_MODEL,
        |config| {
            config.tool_output_token_limit = Some(50);
        },
    )
    .await?;

    let items = custom_tool_output_items(&second_mock.single_request(), "call-1");
    let output = text_item(&items, /*index*/ 1);
    // The 50-token override must shrink this 50,000-character value far below
    // what the default 10,000-token history cap would retain.
    assert!(
        output.len() < 1_000,
        "expected configured history cap to truncate the emitted value, got {} bytes",
        output.len()
    );
    assert_regex_match(
        r"^Variable truncated: False\. Variable: x+…\d+ tokens truncated…x+$",
        output,
    );

    Ok(())
}

#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_exec_without_nested_limit_preserves_result_variable_before_default_history_truncation()
-> Result<()> {
    // TODO(anp): Remove after Wine exec returns complete nested-tool output to code mode.
    skip_if_wine_exec!(
        Ok(()),
        "only part of nested exec_command stdout reaches the code-mode result"
    );
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (_test, second_mock) = run_code_mode_turn_with_model_and_config(
        &server,
        "use exec_command from code mode",
        r#"// @exec: {"max_output_tokens": 20000}
const result = await tools.exec_command({
  cmd: "python3 -c \"import sys; sys.stdout.write('x' * 50000)\""
});
const resultVariableWasTruncated = result.output.length !== 50000;
text(`Variable truncated: ${resultVariableWasTruncated ? "True" : "False"}. Variable: ${result.output}`);
"#,
        TOKEN_POLICY_TEST_MODEL,
        |_| {},
    )
    .await?;

    let items = custom_tool_output_items(&second_mock.single_request(), "call-1");
    let output = text_item(&items, /*index*/ 1);
    assert_regex_match(
        r"^Variable truncated: False\. Variable: x+…\d+ tokens truncated…x+$",
        output,
    );

    Ok(())
}

#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_exec_without_nested_limit_preserves_result_variable_before_configured_history_truncation()
-> Result<()> {
    // TODO(anp): Remove after Wine exec returns complete nested-tool output to code mode.
    skip_if_wine_exec!(
        Ok(()),
        "only part of nested exec_command stdout reaches the code-mode result"
    );
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (_test, second_mock) = run_code_mode_turn_with_model_and_config(
        &server,
        "use exec_command from code mode",
        r#"// @exec: {"max_output_tokens": 20000}
const result = await tools.exec_command({
  cmd: "python3 -c \"import sys; sys.stdout.write('x' * 50000)\""
});
const resultVariableWasTruncated = result.output.length !== 50000;
text(`Variable truncated: ${resultVariableWasTruncated ? "True" : "False"}. Variable: ${result.output}`);
"#,
        TOKEN_POLICY_TEST_MODEL,
        |config| {
            config.tool_output_token_limit = Some(50);
        },
    )
    .await?;

    let items = custom_tool_output_items(&second_mock.single_request(), "call-1");
    let output = text_item(&items, /*index*/ 1);
    // The 50-token override must shrink this 50,000-character value far below
    // what the default 10,000-token history cap would retain.
    assert!(
        output.len() < 1_000,
        "expected configured history cap to truncate the emitted value, got {} bytes",
        output.len()
    );
    assert_regex_match(
        r"^Variable truncated: False\. Variable: x+…\d+ tokens truncated…x+$",
        output,
    );

    Ok(())
}

// The outer directive limits output after JavaScript emits it; it does not
// limit `result.output` returned by the nested command.
#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_exec_outer_limit_truncates_emitted_output() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (_test, second_mock) = run_code_mode_turn(
        &server,
        "use exec_command from code mode",
        r#"// @exec: {"max_output_tokens": 5}
const result = await tools.exec_command({
  cmd: "printf '0123456789012345678901234567890123456789'"
});
text(result.output);
"#,
    )
    .await?;

    assert_eq!(
        text_item(
            &custom_tool_output_items(&second_mock.single_request(), "call-1"),
            /*index*/ 1
        ),
        "Warning: truncated output (original token count: 10)\nTotal output lines: 1\n\n0123456789…5 tokens truncated…0123456789"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_returns_accumulated_output_when_script_fails() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (_test, second_mock) = run_code_mode_turn(
        &server,
        "use code_mode to surface script failures",
        r#"
text("before crash");
text("still before crash");
throw new Error("boom");
"#,
    )
    .await?;

    let req = second_mock.single_request();
    let items = custom_tool_output_items(&req, "call-1");
    assert_eq!(items.len(), 4);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script failed\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&items, /*index*/ 0),
    );
    assert_eq!(text_item(&items, /*index*/ 1), "before crash");
    assert_eq!(text_item(&items, /*index*/ 2), "still before crash");
    assert_regex_match(
        r#"(?sx)
\A
Script\ error:\n
Error:\ boom\n
(?:\s+at\ .+\n?)+
\z
"#,
        text_item(&items, /*index*/ 3),
    );

    Ok(())
}

#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_exec_surfaces_handler_errors_as_exceptions() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (_test, second_mock) = run_code_mode_turn(
        &server,
        "surface nested tool handler failures as script exceptions",
        r#"
try {
  await tools.exec_command({});
  text("no-exception");
} catch (error) {
  text(`caught:${error?.message ?? String(error)}`);
}
"#,
    )
    .await?;

    let request = second_mock.single_request();
    assert_eq!(
        request.custom_tool_call_output("call-1")["internal_chat_message_metadata_passthrough"]["executed_tool_calls"],
        serde_json::json!([
            {"name": "exec_command", "arguments": {}},
        ]),
        "failed nested tool attempts remain private request metadata",
    );
    let (output, success) = custom_tool_output_body_and_success(&request, "call-1");
    assert_ne!(
        success,
        Some(false),
        "script should catch the nested tool error: {output}"
    );
    assert!(
        output.contains("caught:"),
        "expected caught exception text in output: {output}"
    );
    assert!(
        !output.contains("no-exception"),
        "nested tool error should not allow success path: {output}"
    );

    Ok(())
}

/// A stalled host wait must return its timeout to the model and reconnect for the next exec.
#[tokio::test(flavor = "current_thread")]
async fn code_mode_wait_timeout_reconnects_on_next_exec() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mut builder = test_codex()
        .with_model("test-gpt-5.1-codex")
        .with_config(|config| {
            config
                .features
                .enable(Feature::CodeMode)
                .expect("code mode should be enabled");
        });
    let test = builder.build_with_auto_env(&server).await?;

    let first_turn = responses::mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_custom_tool_call(
                    "call-1",
                    "exec",
                    "yield_control(); await new Promise(() => {});",
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_assistant_message("msg-1", "waiting"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    test.submit_turn("start a stalled code-mode cell").await?;
    let first_request = first_turn
        .last_request()
        .expect("initial exec should be returned to the model");
    let first_items = custom_tool_output_items(&first_request, "call-1");
    let cell_id = extract_running_cell_id(text_item(&first_items, /*index*/ 0));

    let timeout_completion = responses::mount_function_call_agent_response(
        &server,
        "call-2",
        &serde_json::to_string(&serde_json::json!({
            "cell_id": cell_id,
            "yield_time_ms": 60_000,
        }))?,
        "wait",
    )
    .await
    .completion;

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "wait for the stalled cell".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event_match(&test.codex, |event| match event {
        EventMsg::RawResponseItem(raw) => match &raw.item {
            ResponseItem::FunctionCall { call_id, .. } if call_id == "call-2" => Some(()),
            _ => None,
        },
        _ => None,
    })
    .await;

    tokio::time::pause();
    for _ in 0..130 {
        tokio::time::advance(Duration::from_secs(1)).await;
        if timeout_completion
            .function_call_output_text("call-2")
            .is_some()
        {
            break;
        }
    }
    tokio::time::resume();
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let timeout_output = timeout_completion
        .function_call_output_text("call-2")
        .expect("timed-out wait should be returned to the model");
    assert!(
        timeout_output.contains("code-mode host timed out waiting for wait response"),
        "unexpected wait output: {timeout_output}"
    );

    let reconnect_turn = responses::mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-5"),
                ev_custom_tool_call(
                    "call-3",
                    "exec",
                    r#"text("reconnected"); yield_control(); await new Promise(() => {});"#,
                ),
                ev_completed("resp-5"),
            ]),
            sse(vec![
                ev_assistant_message("msg-3", "reconnected"),
                ev_completed("resp-6"),
            ]),
        ],
    )
    .await;

    test.submit_turn("run a cell after the timeout").await?;
    let reconnect_request = reconnect_turn
        .last_request()
        .expect("replacement exec should be returned to the model");
    let reconnect_items = custom_tool_output_items(&reconnect_request, "call-3");
    assert_eq!(
        extract_running_cell_id(text_item(&reconnect_items, /*index*/ 0)),
        "g2:1"
    );
    assert_eq!(text_item(&reconnect_items, /*index*/ 1), "reconnected");

    Ok(())
}

#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_can_yield_and_resume_with_wait() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mut builder = test_codex().with_config(move |config| {
        let _ = config.features.enable(Feature::CodeMode);
    });
    let test = builder.build(&server).await?;
    let phase_2_gate = test.workspace_path("code-mode-phase-2.ready");
    let phase_3_gate = test.workspace_path("code-mode-phase-3.ready");
    let phase_2_wait = wait_for_file_source(&phase_2_gate)?;
    let phase_3_wait = wait_for_file_source(&phase_3_gate)?;

    let code = format!(
        r#"
text("phase 1");
yield_control();
{phase_2_wait}
text("phase 2");
{phase_3_wait}
text("phase 3");
"#
    );

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call("call-1", "exec", &code),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let first_completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "waiting"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn("start the long exec").await?;

    let first_request = first_completion.single_request();
    let first_items = custom_tool_output_items(&first_request, "call-1");
    assert_eq!(first_items.len(), 2);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script running with cell ID \d+\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&first_items, /*index*/ 0),
    );
    assert_eq!(text_item(&first_items, /*index*/ 1), "phase 1");
    let cell_id = extract_running_cell_id(text_item(&first_items, /*index*/ 0));

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-3"),
            responses::ev_function_call(
                "call-2",
                "wait",
                &serde_json::to_string(&serde_json::json!({
                    "cell_id": cell_id.clone(),
                    "yield_time_ms": 1_000,
                }))?,
            ),
            ev_completed("resp-3"),
        ]),
    )
    .await;
    let second_completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-2", "still waiting"),
            ev_completed("resp-4"),
        ]),
    )
    .await;

    fs::write(&phase_2_gate, "ready")?;
    test.submit_turn("wait again").await?;

    let second_request = second_completion.single_request();
    let second_items = function_tool_output_items(&second_request, "call-2");
    assert_eq!(second_items.len(), 2);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script running with cell ID \d+\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&second_items, /*index*/ 0),
    );
    assert_eq!(
        extract_running_cell_id(text_item(&second_items, /*index*/ 0)),
        cell_id
    );
    assert_eq!(text_item(&second_items, /*index*/ 1), "phase 2");

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-5"),
            responses::ev_function_call(
                "call-3",
                "wait",
                &serde_json::to_string(&serde_json::json!({
                    "cell_id": cell_id.clone(),
                    "yield_time_ms": 1_000,
                }))?,
            ),
            ev_completed("resp-5"),
        ]),
    )
    .await;
    let third_completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-3", "done"),
            ev_completed("resp-6"),
        ]),
    )
    .await;

    fs::write(&phase_3_gate, "ready")?;
    test.submit_turn("wait for completion").await?;

    let third_request = third_completion.single_request();
    let third_items = function_tool_output_items(&third_request, "call-3");
    assert_eq!(third_items.len(), 2);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script completed\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&third_items, /*index*/ 0),
    );
    assert_eq!(text_item(&third_items, /*index*/ 1), "phase 3");

    Ok(())
}

#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_yield_and_termination_are_not_starved_by_runtime_output() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mut builder = test_codex().with_config(move |config| {
        let _ = config.features.enable(Feature::CodeMode);
    });
    let test = builder.build(&server).await?;
    let termination_gate = test.workspace_path("code-mode-output-termination.ready");
    let termination_wait = wait_for_file_source(&termination_gate)?;

    // Exact controller arbitration is covered by deterministic code-mode contract tests. Keep
    // this end-to-end load bounded while exercising a substantial runtime output backlog.
    let code = format!(
        r#"// @exec: {{"yield_time_ms": 0, "max_output_tokens": 16}}
for (let index = 0; index < 256; index++) {{
    text(`event ${{index}}`);
}}
{termination_wait}
"#
    );

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call("call-1", "exec", &code),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let first_completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "waiting"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn("start the bounded output backlog").await?;

    let first_request = first_completion.single_request();
    let first_items = custom_tool_output_items(&first_request, "call-1");
    assert_eq!(first_items.len(), 1);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script running with cell ID \d+\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&first_items, /*index*/ 0),
    );
    let cell_id = extract_running_cell_id(text_item(&first_items, /*index*/ 0));

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-3"),
            responses::ev_function_call(
                "call-2",
                "wait",
                &serde_json::to_string(&serde_json::json!({
                    "cell_id": cell_id.clone(),
                    "terminate": true,
                }))?,
            ),
            ev_completed("resp-3"),
        ]),
    )
    .await;
    let second_completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-2", "terminated"),
            ev_completed("resp-4"),
        ]),
    )
    .await;

    test.submit_turn("terminate it").await?;

    let second_request = second_completion.single_request();
    let second_items = function_tool_output_items(&second_request, "call-2");
    assert!(!second_items.is_empty());
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script terminated\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&second_items, /*index*/ 0),
    );

    Ok(())
}

#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_can_run_multiple_yielded_sessions() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mut builder = test_codex().with_config(move |config| {
        let _ = config.features.enable(Feature::CodeMode);
    });
    let test = builder.build(&server).await?;
    let session_a_gate = test.workspace_path("code-mode-session-a.ready");
    let session_b_gate = test.workspace_path("code-mode-session-b.ready");
    let session_a_wait = wait_for_file_source(&session_a_gate)?;
    let session_b_wait = wait_for_file_source(&session_b_gate)?;

    let session_a_code = format!(
        r#"
text("session a start");
yield_control();
{session_a_wait}
text("session a done");
"#
    );
    let session_b_code = format!(
        r#"
text("session b start");
yield_control();
{session_b_wait}
text("session b done");
"#
    );

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call("call-1", "exec", &session_a_code),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let first_completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "session a waiting"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn("start session a").await?;

    let first_request = first_completion.single_request();
    let first_items = custom_tool_output_items(&first_request, "call-1");
    assert_eq!(first_items.len(), 2);
    let session_a_id = extract_running_cell_id(text_item(&first_items, /*index*/ 0));
    assert_eq!(text_item(&first_items, /*index*/ 1), "session a start");

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-3"),
            ev_custom_tool_call("call-2", "exec", &session_b_code),
            ev_completed("resp-3"),
        ]),
    )
    .await;
    let second_completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-2", "session b waiting"),
            ev_completed("resp-4"),
        ]),
    )
    .await;

    test.submit_turn("start session b").await?;

    let second_request = second_completion.single_request();
    let second_items = custom_tool_output_items(&second_request, "call-2");
    assert_eq!(second_items.len(), 2);
    let session_b_id = extract_running_cell_id(text_item(&second_items, /*index*/ 0));
    assert_eq!(text_item(&second_items, /*index*/ 1), "session b start");
    assert_ne!(session_a_id, session_b_id);

    fs::write(&session_a_gate, "ready")?;
    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-5"),
            responses::ev_function_call(
                "call-3",
                "wait",
                &serde_json::to_string(&serde_json::json!({
                    "cell_id": session_a_id.clone(),
                    "yield_time_ms": 1_000,
                }))?,
            ),
            ev_completed("resp-5"),
        ]),
    )
    .await;
    let third_completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-3", "session a done"),
            ev_completed("resp-6"),
        ]),
    )
    .await;

    test.submit_turn("wait session a").await?;

    let third_request = third_completion.single_request();
    let third_items = function_tool_output_items(&third_request, "call-3");
    assert_eq!(third_items.len(), 2);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script completed\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&third_items, /*index*/ 0),
    );
    assert_eq!(text_item(&third_items, /*index*/ 1), "session a done");

    fs::write(&session_b_gate, "ready")?;
    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-7"),
            responses::ev_function_call(
                "call-4",
                "wait",
                &serde_json::to_string(&serde_json::json!({
                    "cell_id": session_b_id.clone(),
                    "yield_time_ms": 1_000,
                }))?,
            ),
            ev_completed("resp-7"),
        ]),
    )
    .await;
    let fourth_completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-4", "session b done"),
            ev_completed("resp-8"),
        ]),
    )
    .await;

    test.submit_turn("wait session b").await?;

    let fourth_request = fourth_completion.single_request();
    let fourth_items = function_tool_output_items(&fourth_request, "call-4");
    assert_eq!(fourth_items.len(), 2);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script completed\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&fourth_items, /*index*/ 0),
    );
    assert_eq!(text_item(&fourth_items, /*index*/ 1), "session b done");

    Ok(())
}

#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_concurrent_cells_merge_only_the_stored_values_they_write() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mut builder = test_codex().with_config(move |config| {
        let _ = config.features.enable(Feature::CodeMode);
    });
    let test = builder.build(&server).await?;
    let first_gate = test.workspace_path("code-mode-first-store.ready");
    let first_wait = wait_for_file_source(&first_gate)?;

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call(
                "call-init",
                "exec",
                r#"
store("a", 1);
store("b", 2);
"#,
            ),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "initialized"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn("initialize stored values").await?;

    let first_code = format!(
        r#"
store("a", 3);
yield_control();
{first_wait}
"#
    );
    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-3"),
            ev_custom_tool_call("call-first", "exec", &first_code),
            ev_completed("resp-3"),
        ]),
    )
    .await;
    let first_started = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-2", "first pending"),
            ev_completed("resp-4"),
        ]),
    )
    .await;

    test.submit_turn("start first store").await?;

    let first_request = first_started.single_request();
    let first_items = custom_tool_output_items(&first_request, "call-first");
    let first_cell_id = extract_running_cell_id(text_item(&first_items, /*index*/ 0));

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-5"),
            ev_custom_tool_call("call-second", "exec", r#"store("b", 4);"#),
            ev_completed("resp-5"),
        ]),
    )
    .await;
    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-3", "second complete"),
            ev_completed("resp-6"),
        ]),
    )
    .await;

    test.submit_turn("write the second key").await?;

    fs::write(&first_gate, "ready")?;
    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-7"),
            responses::ev_function_call(
                "call-wait",
                "wait",
                &serde_json::to_string(&serde_json::json!({
                    "cell_id": first_cell_id,
                    "yield_time_ms": 1_000,
                }))?,
            ),
            ev_completed("resp-7"),
        ]),
    )
    .await;
    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-4", "first completed"),
            ev_completed("resp-8"),
        ]),
    )
    .await;

    test.submit_turn("complete the first store").await?;

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-9"),
            ev_custom_tool_call(
                "call-check",
                "exec",
                r#"text(JSON.stringify({ a: load("a"), b: load("b") }));"#,
            ),
            ev_completed("resp-9"),
        ]),
    )
    .await;
    let check_response = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-5", "checked"),
            ev_completed("resp-10"),
        ]),
    )
    .await;

    test.submit_turn("check merged stored values").await?;

    let check_request = check_response.single_request();
    let stored_values: Value = serde_json::from_str(
        &custom_tool_output_last_non_empty_text(&check_request, "call-check")
            .expect("checking stored values should emit JSON"),
    )?;
    assert_eq!(stored_values, serde_json::json!({ "a": 3, "b": 4 }));

    Ok(())
}

#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_wait_can_terminate_and_continue() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mut builder = test_codex().with_config(move |config| {
        let _ = config.features.enable(Feature::CodeMode);
    });
    let test = builder.build(&server).await?;
    let termination_gate = test.workspace_path("code-mode-terminate.ready");
    let termination_wait = wait_for_file_source(&termination_gate)?;

    let code = format!(
        r#"
text("phase 1");
yield_control();
{termination_wait}
text("phase 2");
"#
    );

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call("call-1", "exec", &code),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let first_completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "waiting"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn("start the long exec").await?;

    let first_request = first_completion.single_request();
    let first_items = custom_tool_output_items(&first_request, "call-1");
    assert_eq!(first_items.len(), 2);
    let cell_id = extract_running_cell_id(text_item(&first_items, /*index*/ 0));
    assert_eq!(text_item(&first_items, /*index*/ 1), "phase 1");

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-3"),
            responses::ev_function_call(
                "call-2",
                "wait",
                &serde_json::to_string(&serde_json::json!({
                    "cell_id": cell_id.clone(),
                    "terminate": true,
                }))?,
            ),
            ev_completed("resp-3"),
        ]),
    )
    .await;
    let second_completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-2", "terminated"),
            ev_completed("resp-4"),
        ]),
    )
    .await;

    test.submit_turn("terminate it").await?;

    let second_request = second_completion.single_request();
    let second_items = function_tool_output_items(&second_request, "call-2");
    assert_eq!(second_items.len(), 1);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script terminated\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&second_items, /*index*/ 0),
    );

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-5"),
            ev_custom_tool_call(
                "call-3",
                "exec",
                r#"
text("after terminate");
"#,
            ),
            ev_completed("resp-5"),
        ]),
    )
    .await;
    let third_completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-3", "done"),
            ev_completed("resp-6"),
        ]),
    )
    .await;

    test.submit_turn("run another exec").await?;

    let third_request = third_completion.single_request();
    let third_items = custom_tool_output_items(&third_request, "call-3");
    assert_eq!(third_items.len(), 2);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script completed\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&third_items, /*index*/ 0),
    );
    assert_eq!(text_item(&third_items, /*index*/ 1), "after terminate");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_wait_returns_error_for_unknown_session() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mut builder = test_codex().with_config(move |config| {
        let _ = config.features.enable(Feature::CodeMode);
    });
    let test = builder.build(&server).await?;

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            responses::ev_function_call(
                "call-1",
                "wait",
                &serde_json::to_string(&serde_json::json!({
                    "cell_id": "999999",
                    "yield_time_ms": 1_000,
                }))?,
            ),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn("wait on an unknown exec cell").await?;

    let request = completion.single_request();
    let (_, success) = request
        .function_call_output_content_and_success("call-1")
        .expect("function tool output should be present");
    assert_ne!(success, Some(true));

    let items = function_tool_output_items(&request, "call-1");
    assert_eq!(items.len(), 2);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script failed\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&items, /*index*/ 0),
    );
    assert_eq!(
        text_item(&items, /*index*/ 1),
        "Script error:\nexec cell 999999 not found"
    );

    Ok(())
}

#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_wait_terminate_returns_completed_session_if_it_finished_after_yield_control()
-> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mut builder = test_codex().with_config(move |config| {
        let _ = config.features.enable(Feature::CodeMode);
    });
    let test = builder.build(&server).await?;
    let session_a_gate = test.workspace_path("code-mode-session-a-finished.ready");
    let session_b_gate = test.workspace_path("code-mode-session-b-blocked.ready");
    let session_a_done_marker = test.workspace_path("code-mode-session-a-done.txt");
    let session_a_wait = wait_for_file_source(&session_a_gate)?;
    let session_b_wait = wait_for_file_source(&session_b_gate)?;
    let session_a_done_marker_quoted =
        shlex::try_join([session_a_done_marker.to_string_lossy().as_ref()])?;
    let session_a_done_command = format!("printf done > {session_a_done_marker_quoted}");

    let session_a_code = format!(
        r#"
text("session a start");
yield_control();
{session_a_wait}
text("session a done");
await tools.exec_command({{ cmd: {session_a_done_command:?} }});
"#
    );
    let session_b_code = format!(
        r#"
text("session b start");
yield_control();
{session_b_wait}
text("session b done");
"#
    );

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call("call-1", "exec", &session_a_code),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let first_completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "session a waiting"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn("start session a").await?;

    let first_request = first_completion.single_request();
    let first_items = custom_tool_output_items(&first_request, "call-1");
    assert_eq!(first_items.len(), 2);
    let session_a_id = extract_running_cell_id(text_item(&first_items, /*index*/ 0));
    assert_eq!(text_item(&first_items, /*index*/ 1), "session a start");

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-3"),
            ev_custom_tool_call("call-2", "exec", &session_b_code),
            ev_completed("resp-3"),
        ]),
    )
    .await;
    let second_completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-2", "session b waiting"),
            ev_completed("resp-4"),
        ]),
    )
    .await;

    test.submit_turn("start session b").await?;

    let second_request = second_completion.single_request();
    let second_items = custom_tool_output_items(&second_request, "call-2");
    assert_eq!(second_items.len(), 2);
    let session_b_id = extract_running_cell_id(text_item(&second_items, /*index*/ 0));
    assert_eq!(text_item(&second_items, /*index*/ 1), "session b start");

    fs::write(&session_a_gate, "ready")?;
    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-5"),
            responses::ev_function_call(
                "call-3",
                "wait",
                &serde_json::to_string(&serde_json::json!({
                    "cell_id": session_b_id.clone(),
                    "yield_time_ms": 1_000,
                }))?,
            ),
            ev_completed("resp-5"),
        ]),
    )
    .await;
    let third_completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-3", "session b still waiting"),
            ev_completed("resp-6"),
        ]),
    )
    .await;

    test.submit_turn("wait session b").await?;

    let third_request = third_completion.single_request();
    let third_items = function_tool_output_items(&third_request, "call-3");
    assert_eq!(third_items.len(), 1);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script running with cell ID \d+\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&third_items, /*index*/ 0),
    );
    assert_eq!(
        extract_running_cell_id(text_item(&third_items, /*index*/ 0)),
        session_b_id
    );

    for _ in 0..100 {
        if session_a_done_marker.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(session_a_done_marker.exists());

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-7"),
            responses::ev_function_call(
                "call-4",
                "wait",
                &serde_json::to_string(&serde_json::json!({
                    "cell_id": session_a_id.clone(),
                    "terminate": true,
                }))?,
            ),
            ev_completed("resp-7"),
        ]),
    )
    .await;
    let fourth_completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-4", "session a already done"),
            ev_completed("resp-8"),
        ]),
    )
    .await;

    test.submit_turn("terminate session a").await?;

    let fourth_request = fourth_completion.single_request();
    let fourth_items = function_tool_output_items(&fourth_request, "call-4");
    match fourth_items.len() {
        1 => {
            assert_regex_match(
                concat!(
                    r"(?s)\A",
                    r"Script terminated\nWall time \d+\.\d seconds\nOutput:\n\z"
                ),
                text_item(&fourth_items, /*index*/ 0),
            );
        }
        2 => {
            assert_regex_match(
                concat!(
                    r"(?s)\A",
                    r"Script (?:completed|terminated)\nWall time \d+\.\d seconds\nOutput:\n\z"
                ),
                text_item(&fourth_items, /*index*/ 0),
            );
            assert_eq!(text_item(&fourth_items, /*index*/ 1), "session a done");
        }
        other => panic!("unexpected number of content items: {other}"),
    }

    Ok(())
}

#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_background_keeps_running_on_later_turn_without_wait() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mut builder = test_codex().with_config(move |config| {
        let _ = config.features.enable(Feature::CodeMode);
    });
    let test = builder.build(&server).await?;
    let resumed_file = test.workspace_path("code-mode-yield-resumed.txt");
    let resumed_file_quoted = shlex::try_join([resumed_file.to_string_lossy().as_ref()])?;
    let write_file_command = format!("printf resumed > {resumed_file_quoted}");
    let wait_for_file_command =
        format!("while [ ! -f {resumed_file_quoted} ]; do sleep 0.01; done; printf ready");
    let code = format!(
        r#"
text("before yield");
yield_control();
await tools.exec_command({{ cmd: {write_file_command:?} }});
text("after yield");
"#
    );

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call("call-1", "exec", &code),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let first_completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "exec yielded"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn("start yielded exec").await?;

    let first_request = first_completion.single_request();
    let first_items = custom_tool_output_items(&first_request, "call-1");
    assert_eq!(first_items.len(), 2);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script running with cell ID \d+\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&first_items, /*index*/ 0),
    );
    assert_eq!(text_item(&first_items, /*index*/ 1), "before yield");

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-3"),
            responses::ev_function_call(
                "call-2",
                "exec_command",
                &serde_json::to_string(&serde_json::json!({
                    "cmd": wait_for_file_command,
                }))?,
            ),
            ev_completed("resp-3"),
        ]),
    )
    .await;
    let second_completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-2", "file appeared"),
            ev_completed("resp-4"),
        ]),
    )
    .await;

    test.submit_turn("wait for resumed file").await?;

    let second_request = second_completion.single_request();
    assert!(
        second_request
            .function_call_output_text("call-2")
            .is_some_and(|output| output.ends_with("ready"))
    );
    assert_eq!(fs::read_to_string(&resumed_file)?, "resumed");

    Ok(())
}

struct InterruptedNestedToolObserver {
    started: Mutex<Option<oneshot::Sender<String>>>,
    finished: Mutex<Option<oneshot::Sender<ToolCallOutcome>>>,
}

impl ToolLifecycleContributor for InterruptedNestedToolObserver {
    fn on_tool_start<'a>(&'a self, input: ToolStartInput<'a>) -> ToolLifecycleFuture<'a> {
        Box::pin(async move {
            let codex_extension_api::ToolCallSource::CodeMode { cell_id, .. } = input.source else {
                return;
            };
            if input.tool_name.name != "test_sync_tool" {
                return;
            }
            if let Some(started) = self
                .started
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
            {
                let _ = started.send(cell_id);
            }
        })
    }

    fn on_tool_finish<'a>(&'a self, input: ToolFinishInput<'a>) -> ToolLifecycleFuture<'a> {
        Box::pin(async move {
            if input.tool_name.name != "test_sync_tool"
                || !matches!(
                    input.source,
                    codex_extension_api::ToolCallSource::CodeMode { .. }
                )
            {
                return;
            }
            if let Some(finished) = self
                .finished
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
            {
                let _ = finished.send(input.outcome);
            }
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_interrupt_terminates_active_cells_and_nested_tools() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let (started_tx, started_rx) = oneshot::channel();
    let (finished_tx, finished_rx) = oneshot::channel();
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions.tool_lifecycle_contributor(Arc::new(InterruptedNestedToolObserver {
        started: Mutex::new(Some(started_tx)),
        finished: Mutex::new(Some(finished_tx)),
    }));

    let server = responses::start_mock_server().await;
    let mut builder = test_codex()
        .with_model("test-gpt-5.1-codex")
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| {
            let _ = config.features.enable(Feature::CodeMode);
            let _ = config.features.enable(Feature::CodeModeInterrupt);
        });
    let test = builder.build_with_auto_env(&server).await?;

    let setup = responses::mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-store"),
                ev_custom_tool_call("call-store", "exec", r#"store("persisted", "preserved");"#),
                ev_completed("resp-store"),
            ]),
            sse(vec![
                ev_assistant_message("msg-store", "stored"),
                ev_completed("resp-store-complete"),
            ]),
            sse(vec![
                ev_response_created("resp-background"),
                ev_custom_tool_call(
                    "call-background",
                    "exec",
                    "yield_control(); await new Promise(() => {});",
                ),
                ev_completed("resp-background"),
            ]),
            sse(vec![
                ev_assistant_message("msg-background", "running"),
                ev_completed("resp-background-complete"),
            ]),
        ],
    )
    .await;

    test.submit_turn("store a value in the reusable code-mode session")
        .await?;
    test.submit_turn("start a background code-mode cell")
        .await?;
    let background_response = setup
        .last_request()
        .expect("background cell should be returned to the model");
    let background_items = custom_tool_output_items(&background_response, "call-background");
    assert!(
        text_item(&background_items, /*index*/ 0).starts_with("Script running with cell ID "),
        "background cell should remain active: {background_items:?}"
    );
    let background_cell_id =
        extract_running_cell_id(text_item(&background_items, /*index*/ 0));

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-interrupted"),
            ev_custom_tool_call(
                "call-interrupted",
                "exec",
                "await tools.test_sync_tool({ sleep_after_ms: 60_000 });",
            ),
            ev_completed("resp-interrupted"),
        ]),
    )
    .await;

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "start a long-running nested tool".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let active_cell_id = tokio::time::timeout(Duration::from_secs(10), started_rx).await??;

    test.codex.submit(Op::Interrupt).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnAborted(_))
    })
    .await;
    let nested_outcome = tokio::time::timeout(Duration::from_secs(10), finished_rx).await??;
    assert_eq!(nested_outcome, ToolCallOutcome::Aborted);

    let recovery = responses::mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-wait-background"),
                responses::ev_function_call(
                    "call-wait-background",
                    "wait",
                    &serde_json::to_string(&serde_json::json!({
                        "cell_id": background_cell_id,
                        "yield_time_ms": 1,
                    }))?,
                ),
                ev_completed("resp-wait-background"),
            ]),
            sse(vec![
                ev_response_created("resp-wait-active"),
                responses::ev_function_call(
                    "call-wait-active",
                    "wait",
                    &serde_json::to_string(&serde_json::json!({
                        "cell_id": active_cell_id,
                        "yield_time_ms": 1,
                    }))?,
                ),
                ev_completed("resp-wait-active"),
            ]),
            sse(vec![
                ev_response_created("resp-recovery"),
                ev_custom_tool_call("call-recovery", "exec", r#"text(load("persisted"));"#),
                ev_completed("resp-recovery"),
            ]),
            sse(vec![
                ev_assistant_message("msg-recovery", "recovered"),
                ev_completed("resp-recovery-complete"),
            ]),
        ],
    )
    .await;

    test.submit_turn("verify interrupted cells and reuse their session")
        .await?;
    let requests = recovery.requests();
    let background_output = function_tool_output_items(&requests[1], "call-wait-background");
    assert!(text_item(&background_output, /*index*/ 1).contains("not found"));
    let active_output = function_tool_output_items(&requests[2], "call-wait-active");
    assert!(text_item(&active_output, /*index*/ 1).contains("not found"));
    let recovery_items = custom_tool_output_items(&requests[3], "call-recovery");
    assert_eq!(text_item(&recovery_items, /*index*/ 1), "preserved");

    Ok(())
}

#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_wait_uses_its_own_max_tokens_budget() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mut builder = test_codex().with_config(move |config| {
        let _ = config.features.enable(Feature::CodeMode);
    });
    let test = builder.build(&server).await?;
    let completion_gate = test.workspace_path("code-mode-max-tokens.ready");
    let completion_wait = wait_for_file_source(&completion_gate)?;

    let code = format!(
        r#"// @exec: {{"max_output_tokens": 100}}
text("phase 1");
yield_control();
{completion_wait}
text("token one token two token three token four token five token six token seven");
"#
    );

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call("call-1", "exec", &code),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let first_completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "waiting"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn("start the long exec").await?;

    let first_request = first_completion.single_request();
    let first_items = custom_tool_output_items(&first_request, "call-1");
    assert_eq!(first_items.len(), 2);
    assert_eq!(text_item(&first_items, /*index*/ 1), "phase 1");
    let cell_id = extract_running_cell_id(text_item(&first_items, /*index*/ 0));

    fs::write(&completion_gate, "ready")?;
    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-3"),
            responses::ev_function_call(
                "call-2",
                "wait",
                &serde_json::to_string(&serde_json::json!({
                    "cell_id": cell_id.clone(),
                    "yield_time_ms": 1_000,
                    "max_tokens": 6,
                }))?,
            ),
            ev_completed("resp-3"),
        ]),
    )
    .await;
    let second_completion = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-2", "done"),
            ev_completed("resp-4"),
        ]),
    )
    .await;

    test.submit_turn("wait for completion").await?;

    let second_request = second_completion.single_request();
    let second_items = function_tool_output_items(&second_request, "call-2");
    assert_eq!(second_items.len(), 2);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script completed\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&second_items, /*index*/ 0),
    );
    let expected_pattern = r#"(?sx)
\A
Warning:\ truncated\ output\ \(original\ token\ count:\ \d+\)\n
Total\ output\ lines:\ 1\n
\n
.*…\d+\ tokens\ truncated….*
\z
"#;
    assert_regex_match(expected_pattern, text_item(&second_items, /*index*/ 1));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_can_resume_after_set_timeout() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (_test, second_mock) = run_code_mode_turn(
        &server,
        "use exec to wait for a timeout",
        r#"
await new Promise((resolve) => setTimeout(resolve, 10));
text("timer done");
"#,
    )
    .await?;

    let req = second_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "exec setTimeout call failed unexpectedly: {output}"
    );
    assert_eq!(output, "timer done");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_notify_injects_additional_exec_tool_output_into_active_context() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (_test, second_mock) = run_code_mode_turn(
        &server,
        "use exec notify helper",
        r#"
notify("code_mode_notify_marker");
text("done");
"#,
    )
    .await?;

    let req = second_mock.single_request();
    let has_notify_output = req
        .inputs_of_type("custom_tool_call_output")
        .iter()
        .any(|item| {
            item.get("call_id").and_then(serde_json::Value::as_str) == Some("call-1")
                && item
                    .get("output")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|text| text.contains("code_mode_notify_marker"))
                && item.get("name").and_then(serde_json::Value::as_str) == Some("exec")
        });
    assert!(
        has_notify_output,
        "expected notify marker in custom_tool_call_output item: {:?}",
        req.input()
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_exit_stops_script_immediately() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (_test, second_mock) = run_code_mode_turn(
        &server,
        "use exec to stop script early with exit helper",
        r#"
text("before");
exit();
text("after");
"#,
    )
    .await?;

    let req = second_mock.single_request();
    let items = custom_tool_output_items(&req, "call-1");
    let (output, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "exec exit helper call failed unexpectedly: {output}"
    );
    assert_eq!(items.len(), 2);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script completed\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&items, /*index*/ 0),
    );
    assert_eq!(text_item(&items, /*index*/ 1), "before");
    assert_eq!(output, "before");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_replaces_malformed_image() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (_test, second_mock) = run_code_mode_turn(
        &server,
        "use exec to return an image",
        r#"image("data:image/png;base64,AAA");"#,
    )
    .await?;

    let req = second_mock.single_request();
    let items = custom_tool_output_items(&req, "call-1");
    let (_, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(success, Some(false));
    assert_eq!(items.len(), 2);
    assert_eq!(
        items[1],
        serde_json::json!({
            "type": "input_text",
            "text": "image content omitted because it could not be processed"
        })
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_resizes_explicit_original_image() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let original_dimensions = (6401, 100);
    let image = ImageBuffer::from_pixel(
        original_dimensions.0,
        original_dimensions.1,
        Rgba([20, 40, 60, 255]),
    );
    let mut encoded = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image).write_to(&mut encoded, image::ImageFormat::Png)?;
    let image_data_url = format!(
        "data:image/png;base64,{}",
        BASE64_STANDARD.encode(encoded.into_inner())
    );
    let code = format!(
        "image({}, \"original\");",
        serde_json::to_string(&image_data_url)?
    );

    let server = responses::start_mock_server().await;
    let (_test, second_mock) = run_code_mode_turn_with_model_and_config(
        &server,
        "use exec to return a large original-detail image",
        &code,
        "gpt-5.4",
        |_| {},
    )
    .await?;

    let req = second_mock.single_request();
    let items = custom_tool_output_items(&req, "call-1");
    let (_, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(success, Some(false));
    let resized_url = items[1]["image_url"]
        .as_str()
        .expect("code mode image output should contain a data URL");
    assert_eq!(items[1]["detail"], "original");
    let (_, resized_base64) = resized_url
        .split_once(',')
        .expect("resized image should contain a data URL prefix");
    let resized_bytes = BASE64_STANDARD.decode(resized_base64)?;
    let resized = image::load_from_memory(&resized_bytes)?;
    let resized_dimensions = resized.dimensions();
    assert_eq!(resized_dimensions, (6000, 94));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_unified_image_budget_accepts_legacy_detail_hints() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let image = ImageBuffer::from_pixel(
        /*width*/ 2304,
        /*height*/ 864,
        Rgba([20, 40, 60, 255]),
    );
    let mut encoded = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image).write_to(&mut encoded, image::ImageFormat::Png)?;
    let image_data = BASE64_STANDARD.encode(encoded.into_inner());
    let code = format!(
        r#"
const data = {};
const imageUrl = `data:image/png;base64,${{data}}`;
image(imageUrl);
image(imageUrl, "auto");
image(imageUrl, "high");
image({{ image_url: imageUrl, detail: "low" }});
image({{
  type: "image",
  data,
  mimeType: "image/png",
  _meta: {{ "codex/imageDetail": "original" }}
}});
"#,
        serde_json::to_string(&image_data)?
    );

    let server = responses::start_mock_server().await;
    let (_test, second_mock) = run_code_mode_turn_with_model_and_config(
        &server,
        "emit images with legacy detail arguments and MCP metadata",
        &code,
        "gpt-5.4",
        |config| {
            let _ = config.features.enable(Feature::UnifiedImageBudget);
        },
    )
    .await?;

    let request = second_mock.single_request();
    let items = custom_tool_output_items(&request, "call-1");
    assert_eq!(items.len(), 6, "unexpected code-mode output: {items:?}");
    for item in &items[1..] {
        assert_eq!(item["type"], "input_image");
        assert_eq!(item["detail"], "original");
        let image_url = item["image_url"]
            .as_str()
            .expect("image helper should emit a data URL");
        let (_, payload) = image_url
            .split_once(',')
            .expect("image data URL should have a base64 payload");
        let image = image::load_from_memory(&BASE64_STANDARD.decode(payload)?)?;
        assert_eq!(image.dimensions(), (2304, 864));
    }

    let body = request.body_json();
    let exec_description = body["tools"]
        .as_array()
        .and_then(|tools| tools.iter().find(|tool| tool["name"] == "exec"))
        .and_then(|tool| tool["description"].as_str())
        .expect("the model request should contain the code-mode exec tool");
    assert!(
        exec_description
            .contains("`image(imageUrlOrItem: string | { image_url: string } | ImageContent)`")
    );
    assert!(!exec_description.contains("codex/imageDetail"));
    assert!(!exec_description.contains("detail?:"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_unified_image_budget_preserves_legacy_contract_for_unsupported_model()
-> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (_test, second_mock) = run_code_mode_turn_with_model_and_config(
        &server,
        "emit an image on a legacy model",
        r#"image("data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==");"#,
        "gpt-5.2",
        |config| {
            let _ = config.features.enable(Feature::UnifiedImageBudget);
        },
    )
    .await?;

    let request = second_mock.single_request();
    let items = custom_tool_output_items(&request, "call-1");
    assert_eq!(items.len(), 2);
    assert_eq!(items[1]["detail"], "high");

    let body = request.body_json();
    let exec_description = body["tools"]
        .as_array()
        .and_then(|tools| tools.iter().find(|tool| tool["name"] == "exec"))
        .and_then(|tool| tool["description"].as_str())
        .expect("the model request should contain the code-mode exec tool");
    assert!(exec_description.contains("codex/imageDetail"));
    assert!(exec_description.contains("detail?:"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_view_image_rejects_invalid_file_without_exposing_contents() -> Result<()> {
    skip_if_no_network!(Ok(()));

    const INVALID_IMAGE_CONTENTS: &str = "private-file-contents-must-not-be-exposed";

    let server = responses::start_mock_server().await;
    let builder = test_codex()
        .with_model("gpt-5.4")
        .with_config(|config| {
            let _ = config.features.enable(Feature::CodeMode);
        })
        .with_workspace_setup(|cwd, _fs| async move {
            fs::write(
                cwd.join("not-an-image.txt").as_path(),
                INVALID_IMAGE_CONTENTS,
            )?;
            Ok(())
        });
    let (_test, second_mock) = run_code_mode_turn_with_builder(
        &server,
        "use exec to call view_image on a non-image file",
        r#"await tools.view_image({ path: "not-an-image.txt" });"#,
        builder,
    )
    .await?;

    let request = second_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&request, "call-1");
    assert_ne!(
        success,
        Some(true),
        "code-mode view_image unexpectedly accepted a non-image file"
    );
    assert!(
        output.contains("unable to process image: invalid or unsupported image data"),
        "unexpected code-mode failure: {output}"
    );
    let model_visible_output = serde_json::to_string(&request.custom_tool_call_output("call-1"))?;
    assert!(
        !model_visible_output.contains(INVALID_IMAGE_CONTENTS),
        "invalid file contents leaked into model-visible output: {model_visible_output}"
    );

    Ok(())
}

#[test_case(false; "legacy detail")]
#[test_case(true; "unified image budget")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_can_use_view_image_result_with_image_helper(
    unified_image_budget: bool,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mut builder = test_codex()
        .with_model("gpt-5.4")
        .with_config(move |config| {
            let _ = config.features.enable(Feature::CodeMode);
            if unified_image_budget {
                let _ = config.features.enable(Feature::UnifiedImageBudget);
            }
        });
    let test = builder.build(&server).await?;

    let image = ImageBuffer::from_pixel(
        /*width*/ 2,
        /*height*/ 1,
        Rgba([255u8, 0, 0, 255]),
    );
    let rotate_90_exif = vec![
        0x49, 0x49, 0x2a, 0x00, 0x08, 0x00, 0x00, 0x00, 0x01, 0x00, 0x12, 0x01, 0x03, 0x00, 0x01,
        0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    let mut image_bytes = Vec::new();
    let mut encoder = PngEncoder::new(&mut image_bytes);
    encoder.set_exif_metadata(rotate_90_exif.clone())?;
    encoder.write_image(
        image.as_raw(),
        image.width(),
        image.height(),
        image::ColorType::Rgba8.into(),
    )?;
    let image_path = test.cwd_path().join("code_mode_view_image.png");
    fs::write(&image_path, &image_bytes)?;

    let image_path_json = serde_json::to_string(&image_path.to_string_lossy().to_string())?;
    let expected_output_keys = if unified_image_budget {
        r#"["image_url"]"#
    } else {
        r#"["detail","image_url"]"#
    };
    let code = format!(
        r#"
const out = await tools.view_image({{ path: {image_path_json}, detail: "original" }});
const keys = Object.keys(out).sort();
if (JSON.stringify(keys) !== JSON.stringify({expected_output_keys})) {{
  throw new Error(`unexpected view_image output keys: ${{JSON.stringify(keys)}}`);
}}
image(out);
"#
    );

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call("call-1", "exec", &code),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let second_mock = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn("use exec to call view_image and emit its image output")
        .await?;

    let req = second_mock.single_request();
    let items = custom_tool_output_items(&req, "call-1");
    let (_, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "code_mode view_image call failed unexpectedly"
    );
    assert_eq!(items.len(), 2);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script completed\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&items, /*index*/ 0),
    );

    assert_eq!(
        items[1].get("type").and_then(Value::as_str),
        Some("input_image")
    );

    let emitted_image_url = items[1]
        .get("image_url")
        .and_then(Value::as_str)
        .expect("image helper should emit an input_image item with image_url");
    assert!(emitted_image_url.starts_with("data:image/png;base64,"));
    let (_, emitted_image_base64) = emitted_image_url
        .split_once(',')
        .expect("emitted image should contain a data URL prefix");
    let emitted_image_bytes = BASE64_STANDARD.decode(emitted_image_base64)?;
    assert_eq!(emitted_image_bytes, image_bytes);
    let mut decoder = ImageReader::with_format(Cursor::new(&emitted_image_bytes), ImageFormat::Png)
        .into_decoder()?;
    assert_eq!(
        (
            decoder.dimensions(),
            decoder.orientation()?,
            decoder.exif_metadata()?
        ),
        ((2, 1), Orientation::Rotate90, Some(rotate_90_exif))
    );
    assert_eq!(
        items[1].get("detail").and_then(Value::as_str),
        Some("original")
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_can_use_mcp_image_result_with_image_helper() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let code = r#"
const out = await tools.mcp__rmcp__image_scenario({
  scenario: "image_only_original_detail",
});
const imageItem = out.content.find((item) => item.type === "image");
image(imageItem);
"#;

    let (_test, second_mock) = run_code_mode_turn_with_rmcp_model(
        &server,
        "use exec to call the rmcp image scenario tool and emit its image output",
        code,
        "gpt-5.4",
    )
    .await?;

    let req = second_mock.single_request();
    let items = custom_tool_output_items(&req, "call-1");
    let (_, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "code_mode mcp image scenario call failed unexpectedly"
    );
    assert_eq!(items.len(), 2);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script completed\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&items, /*index*/ 0),
    );

    assert_eq!(
        items[1].get("type").and_then(Value::as_str),
        Some("input_image")
    );

    let emitted_image_url = items[1]
        .get("image_url")
        .and_then(Value::as_str)
        .expect("image helper should emit an input_image item with image_url");
    assert!(emitted_image_url.starts_with("data:image/png;base64,"));
    assert_eq!(
        items[1].get("detail").and_then(Value::as_str),
        Some("original")
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_can_apply_patch_via_nested_tool() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let file_name = "code_mode_apply_patch.txt";
    let patch = format!(
        "*** Begin Patch\n*** Add File: {file_name}\n+hello from code_mode\n*** End Patch\n"
    );
    let code = format!("text(await tools.apply_patch({patch:?}));\n");

    let (test, second_mock) =
        run_code_mode_turn(&server, "use exec to run apply_patch", &code).await?;

    let req = second_mock.single_request();
    let items = custom_tool_output_items(&req, "call-1");
    let (_, success) = req
        .custom_tool_call_output_content_and_success("call-1")
        .expect("custom tool output should be present");
    assert_ne!(
        success,
        Some(false),
        "exec apply_patch call failed unexpectedly: {items:?}"
    );
    assert_eq!(items.len(), 2);
    assert_regex_match(
        concat!(
            r"(?s)\A",
            r"Script completed\nWall time \d+\.\d seconds\nOutput:\n\z"
        ),
        text_item(&items, /*index*/ 0),
    );
    assert_eq!(text_item(&items, /*index*/ 1), "{}");

    let file_path = test.cwd_path().join(file_name);
    assert_eq!(fs::read_to_string(&file_path)?, "hello from code_mode\n");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_can_print_structured_mcp_tool_result_fields() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let code = r#"
const { content, structuredContent, isError } = await tools.mcp__rmcp__echo({
  message: "ping",
});
text(
  `echo=${structuredContent?.echo ?? "missing"}\n` +
    `env=${structuredContent?.env ?? "missing"}\n` +
    `isError=${String(isError)}\n` +
    `contentLength=${content.length}`
);
"#;

    let (_test, second_mock) =
        run_code_mode_turn_with_rmcp(&server, "use exec to run the rmcp echo tool", code).await?;

    let req = second_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "exec rmcp echo call failed unexpectedly: {output}"
    );
    assert_eq!(
        output,
        "echo=ECHOING: ping
env=propagated-env
isError=false
contentLength=0"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_node_repl_screenshots_can_be_captured_without_guardian_transcript_flags()
-> Result<()> {
    skip_if_wine_exec!(
        Ok(()),
        "requires a Windows test_stdio_server in the Wine-exec environment"
    );
    skip_if_no_network!(Ok(()));

    const SCREENSHOT: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";
    let server = responses::start_mock_server().await;
    let mcp_server_bin = remote_aware_stdio_server_bin()?;
    let mut builder = test_codex().with_config(move |config| {
        config
            .features
            .enable(Feature::CodeMode)
            .expect("enable Code Mode");
        let mcp = serde_json::from_value(serde_json::json!({
            "command": mcp_server_bin,
            "environment_id": remote_aware_environment_id(),
            "env": { "MCP_TEST_ENABLE_NODE_REPL_JS": "1" },
            "omit_tools_from": ["deferred"],
        }))
        .expect("valid node_repl MCP server config");
        config
            .mcp_servers
            .set(HashMap::from([("node_repl".to_owned(), mcp)]))
            .expect("configure node_repl MCP server");
    });
    let test = builder.build_with_auto_env(&server).await?;
    core_test_support::wait_for_mcp_server(&test.codex, "node_repl").await?;
    let evidence = test
        .codex
        .thread_extension_data()
        .get_or_init(NodeReplReviewEvidence::default);
    evidence.enable_image_capture();

    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_custom_tool_call(
                    "node-repl-image",
                    "exec",
                    r#"for (let index = 0; index < 2; index++) await tools.mcp__node_repl__js({ code: 'await nodeRepl.emitImage(await tab.screenshot())' });"#,
                ),
                ev_completed("response-node-repl"),
            ]),
            sse(vec![ev_completed("response-done")]),
        ],
    )
    .await;
    test.submit_text_turn("capture a computer-use screenshot")
        .await?;

    assert_eq!(
        evidence.images(),
        vec![ContentItem::InputImage {
            image_url: SCREENSHOT.to_owned(),
            detail: Some(ImageDetail::Low),
        }]
    );
    let parent_request = response_mock
        .requests()
        .pop()
        .expect("parent turn should complete");
    assert!(!serde_json::to_string(&parent_request.input())?.contains(SCREENSHOT));

    Ok(())
}

#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_node_repl_image_flag_without_enhanced_stays_disabled() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));
    skip_if_wine_exec!(
        Ok(()),
        "Guardian approval actions require host-native paths"
    );

    let server = responses::start_mock_server().await;
    let mcp_server_bin = remote_aware_stdio_server_bin()?;
    let mut builder = test_codex()
        .with_model_info_override("gpt-5.5", |model| {
            model.node_repl_auto_review_required = false;
        })
        .with_config(move |config| {
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config
                .features
                .enable(Feature::CodeMode)
                .expect("enable Code Mode");
            config
                .features
                .set_enabled(
                    Feature::GuardianEnhancedNodeReplTranscripts,
                    /*enabled*/ false,
                )
                .expect("disable enhanced Guardian transcripts");
            config
                .features
                .enable(Feature::GuardianNodeReplTranscriptImages)
                .expect("enable Guardian transcript images");
            let mcp = serde_json::from_value(serde_json::json!({
                "command": mcp_server_bin,
                "environment_id": remote_aware_environment_id(),
                "env": { "MCP_TEST_ENABLE_NODE_REPL_JS": "1" },
                "omit_tools_from": ["deferred"],
            }))
            .expect("valid node_repl MCP server config");
            config
                .mcp_servers
                .set(HashMap::from([("node_repl".to_owned(), mcp)]))
                .expect("configure node_repl MCP server");
        });
    let test = builder.build_with_auto_env(&server).await?;
    wait_for_mcp_server(&test.codex, "node_repl").await?;

    let code = r#"
await tools.mcp__node_repl__js({ code: 'await nodeRepl.emitImage(await tab.screenshot())' });
await tools.exec_command({ cmd: "true", sandbox_permissions: "require_escalated", justification: "review" });
"#;
    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_custom_tool_call("code-mode-call", "exec", code),
                ev_completed("resp-parent"),
            ]),
            sse(vec![
                ev_assistant_message("guardian", r#"{"outcome":"allow"}"#),
                ev_completed("resp-guardian"),
            ]),
            sse(vec![ev_completed("resp-done")]),
        ],
    )
    .await;
    test.submit_text_turn("review a nested node_repl screenshot")
        .await?;

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 3);
    let guardian_request = requests
        .iter()
        .find(|request| {
            request.body_json()["client_metadata"]["x-openai-subagent"].as_str() == Some("guardian")
        })
        .expect("the shell command should trigger Guardian review");
    assert!(guardian_request.message_input_image_urls("user").is_empty());
    let guardian_text = guardian_request.message_input_texts("user").concat();
    let parent_input = serde_json::to_string(&requests.last().unwrap().input())?;
    for marker in [
        "guardian-visible-before-image",
        "guardian-visible-after-image",
        "data:image/png;base64,",
    ] {
        assert!(!guardian_text.contains(marker));
        assert!(!parent_input.contains(marker));
    }

    Ok(())
}

#[cfg_attr(windows, ignore = "no exec_command on Windows")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[test_case(false, false, false, None; "disabled")]
#[test_case(true, false, false, None; "manually_enabled_text_only")]
#[test_case(true, true, false, None; "manually_enabled_multimodal")]
#[test_case(false, false, true, None; "required_model_forces_multimodal")]
#[test_case(true, true, false, Some("unsupported"); "text_fallback_without_image_support")]
#[test_case(true, true, false, Some("unbounded"); "text_fallback_without_context_bound")]
#[test_case(true, true, false, Some("small"); "text_fallback_with_insufficient_context")]
#[test_case(true, true, false, Some("large_prompt"); "images_resume_after_prompt_pressure")]
async fn code_mode_node_repl_text_evidence_is_visible_only_to_guardian(
    enhanced_transcripts: bool,
    transcript_images: bool,
    auto_review_required: bool,
    reviewer_constraint: Option<&'static str>,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));
    skip_if_wine_exec!(
        Ok(()),
        "Guardian approval actions require host-native paths"
    );
    const NODE_REPL_DOM_MIDDLE: &str = "guardian-visible-dom-middle";
    const DIRECT_NODE_REPL_MIDDLE: &str = "direct-node-repl-visible-middle";
    const DIRECT_UNRELATED_MIDDLE: &str = "direct-unrelated-hidden-middle";
    const OTHER_NODE_REPL_RESULT: &str = "ECHOING: guardian-visible-other-tool-result";
    const UNRELATED_RESULT: &str = "ECHOING: guardian-hidden-unrelated-result";
    const PRIVATE_IMAGE: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";
    let server = responses::start_mock_server().await;
    let mcp_server_bin = remote_aware_stdio_server_bin()?;
    let check_detail = enhanced_transcripts && transcript_images && reviewer_constraint.is_none();
    let mut large_image = Cursor::new(Vec::new());
    if check_detail {
        DynamicImage::new_rgba8(/*w*/ 2049, /*h*/ 32)
            .write_to(&mut large_image, ImageFormat::Png)?;
    }
    let mut builder = test_codex()
        .with_model_info_override("gpt-5.5", move |model| {
            model.node_repl_auto_review_required = auto_review_required
        })
        .with_config(move |config| {
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            if reviewer_constraint.is_some_and(|value| value != "large_prompt") || check_detail {
                let reviewer = config
                    .model_catalog
                    .as_mut()
                    .expect("bundled model catalog")
                    .models
                    .iter_mut()
                    .find(|model| model.slug == "gpt-5.6-luna")
                    .expect("API-key Guardian reviewer");
                if check_detail {
                    reviewer.use_responses_lite = false;
                } else if reviewer_constraint == Some("unsupported") {
                    reviewer.input_modalities =
                        vec![codex_protocol::openai_models::InputModality::Text];
                } else {
                    reviewer.context_window =
                        (reviewer_constraint == Some("small")).then_some(10_000);
                    reviewer.max_context_window = reviewer.context_window;
                }
            }
            config
                .features
                .enable(Feature::CodeMode)
                .expect("enable Code Mode");
            config
                .features
                .set_enabled(
                    Feature::GuardianEnhancedNodeReplTranscripts,
                    enhanced_transcripts,
                )
                .expect("configure enhanced transcripts");
            config
                .features
                .set_enabled(Feature::GuardianNodeReplTranscriptImages, transcript_images)
                .expect("configure Guardian transcript images");
            let mcp: McpServerConfig = serde_json::from_value(serde_json::json!({
                "command": mcp_server_bin,
                "environment_id": remote_aware_environment_id(),
                "env": {
                    "MCP_TEST_ENABLE_NODE_REPL_JS": "1",
                    "MCP_TEST_IMAGE_DATA_URL": format!("data:image/png;base64,{}", BASE64_STANDARD.encode(large_image.into_inner())),
                    "MCP_TEST_OVERSIZED_INVALID_IMAGE": u8::from(!core_test_support::is_remote_test_environment()).to_string()
                },
                "omit_tools_from": ["deferred"],
            }))
            .expect("valid MCP server config");
            config
                .mcp_servers
                .set(
                    ["node_repl", "node_repl_"]
                        .into_iter()
                        .map(|name| (name.into(), mcp.clone()))
                        .collect(),
                )
                .expect("configure MCP servers");
        });
    let test = builder.build_with_auto_env(&server).await?;
    wait_for_mcp_server(&test.codex, "node_repl").await?;
    let images_enabled = auto_review_required || (enhanced_transcripts && transcript_images);
    let reviewer_images = images_enabled && reviewer_constraint.is_none();
    let snapshot_padding = if images_enabled && reviewer_constraint != Some("large_prompt") {
        2_500
    } else {
        10_000
    };
    let padding = "a".repeat(snapshot_padding);
    let snapshot_args = |marker| {
        let padding = if check_detail && marker == DIRECT_NODE_REPL_MIDDLE {
            ""
        } else {
            padding.as_str()
        };
        serde_json::json!({
            "message": format!("{padding}{marker}{padding}"),
        })
        .to_string()
    };

    let code = r#"
await tools.mcp__node_repl_echo({ message: "guardian-hidden-unrelated-result" });
await tools.mcp__node_repl__js({ code: 'nodeRepl.fail()' });
await tools.mcp__node_repl__js({ code: `nodeRepl.write(${JSON.stringify("a".repeat(SNAPSHOT_PADDING) + ["guardian-visible", "dom-middle"].join("-") + "b".repeat(SNAPSHOT_PADDING))})` });
await tools.mcp__node_repl__echo({ message: ["guardian-visible-other-", "tool-result"].join("") });
await tools.mcp__node_repl__encrypted_output({});
await tools.mcp__node_repl__js({ code: 'nodeRepl.empty()' });
await tools.mcp__node_repl__js({ code: 'await nodeRepl.emitImage(await tab.screenshot())' });
if (LARGE_IMAGE) await tools.mcp__node_repl__image_scenario({ scenario: "invalid_image_bytes_then_image" });
await tools.exec_command({ cmd: "true", sandbox_permissions: "require_escalated", justification: "review" });
await tools.mcp__node_repl__js({ code: 'await nodeRepl.emitImage(await tab.screenshot())' });
if (LARGE_IMAGE) await tools.mcp__node_repl__image({});
await tools.exec_command({ cmd: "printf second", sandbox_permissions: "require_escalated", justification: "review again" });
"#
    .replace("SNAPSHOT_PADDING", &snapshot_padding.to_string())
    .replace("LARGE_IMAGE", &check_detail.to_string());
    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                responses::ev_function_call_with_namespace(
                    "node-repl-call",
                    "mcp__node_repl",
                    "echo",
                    &snapshot_args(DIRECT_NODE_REPL_MIDDLE),
                ),
                ev_completed("resp-node-repl"),
            ]),
            sse(vec![
                responses::ev_function_call_with_namespace(
                    "unrelated-call",
                    "mcp__node_repl_",
                    "echo",
                    &snapshot_args(DIRECT_UNRELATED_MIDDLE),
                ),
                ev_completed("resp-unrelated"),
            ]),
            sse(vec![
                ev_custom_tool_call("code-mode-call", "exec", &code),
                ev_completed("resp-parent"),
            ]),
            sse(vec![
                ev_assistant_message("guardian", r#"{"outcome":"allow"}"#),
                ev_completed("resp-guardian"),
            ]),
            sse(vec![
                ev_assistant_message("guardian-again", r#"{"outcome":"allow"}"#),
                ev_completed("resp-guardian-again"),
            ]),
            sse(vec![ev_completed("resp-done")]),
        ],
    )
    .await;
    test.submit_text_turn("review a nested node_repl tool response")
        .await?;
    let requests = response_mock.requests();
    let guardian_requests = requests
        .iter()
        .filter(|request| {
            request.body_json()["client_metadata"]["x-openai-subagent"].as_str() == Some("guardian")
        })
        .collect::<Vec<_>>();
    assert_eq!(guardian_requests.len(), 2);
    let guardian_text = guardian_requests[0].message_input_texts("user").concat();
    let evidence_enabled = enhanced_transcripts || auto_review_required;
    for included in [
        DIRECT_NODE_REPL_MIDDLE,
        NODE_REPL_DOM_MIDDLE,
        OTHER_NODE_REPL_RESULT,
        "Lookup completed",
        "<completed without visible text>",
        "guardian-visible-before-image",
        "guardian-visible-after-image",
    ] {
        assert_eq!(
            guardian_text.contains(included),
            evidence_enabled,
            "unexpected Guardian evidence visibility for {included}"
        );
    }
    let guardian_request = guardian_requests[0];
    let reviewer_image_urls = guardian_request.message_input_image_urls("user");
    let expected_images = usize::from(reviewer_images) + usize::from(check_detail);
    assert_eq!(reviewer_image_urls.len(), expected_images);
    if reviewer_images {
        assert_eq!(reviewer_image_urls[0], PRIVATE_IMAGE);
        let reviewer_user_content = guardian_request
            .inputs_of_type("message")
            .into_iter()
            .filter(|item| item.get("role").and_then(Value::as_str) == Some("user"))
            .filter_map(|item| item.get("content").and_then(Value::as_array).cloned())
            .flatten()
            .collect::<Vec<_>>();
        let image_index = reviewer_user_content
            .iter()
            .position(|item| item.get("image_url").and_then(Value::as_str) == Some(PRIVATE_IMAGE))
            .expect("private reviewer image should be present");
        if check_detail {
            assert_eq!(reviewer_user_content[image_index]["detail"], "high");
            let payload = reviewer_image_urls[1].split_once(',').unwrap().1;
            let dimensions =
                image::load_from_memory(&BASE64_STANDARD.decode(payload)?)?.dimensions();
            assert_eq!(dimensions, (2048, 32));
        }
        for (index, marker) in [(image_index - 1, "before"), (image_index + 1, "after")] {
            let text = reviewer_user_content[index]["text"].as_str().unwrap();
            assert!(text.contains(marker));
        }
    }
    for excluded in [
        DIRECT_UNRELATED_MIDDLE,
        UNRELATED_RESULT,
        "guardian-hidden-failed-result",
        "gAAAA-test",
        "guardian-hidden-ui-preview",
        "guardian-hidden-structured-override",
    ] {
        assert!(
            !guardian_text.contains(excluded),
            "protected, failed, unrelated, or duplicate evidence leaked: {excluded}"
        );
    }
    assert_eq!(
        guardian_requests[1]
            .message_input_texts("user")
            .concat()
            .matches(NODE_REPL_DOM_MIDDLE)
            .count(),
        usize::from(evidence_enabled),
        "a reused Guardian session must not append the same evidence twice"
    );
    assert_eq!(
        guardian_requests[1].message_input_image_urls("user"),
        if reviewer_constraint == Some("large_prompt") {
            vec![PRIVATE_IMAGE.to_string()]
        } else {
            reviewer_image_urls
        }
    );
    let parent_request = requests.last().expect("parent turn should complete");
    let parent_input = serde_json::to_string(&parent_request.input())?;
    assert!(!parent_input.contains("data:image/png;base64,"));
    assert!(!parent_input.contains("guardian-visible-before-image"));
    assert!(!parent_input.contains("guardian-visible-after-image"));
    assert!(
        !parent_input.contains(NODE_REPL_DOM_MIDDLE)
            && !parent_input.contains(OTHER_NODE_REPL_RESULT)
            && !parent_input.contains(UNRELATED_RESULT),
        "nested MCP responses leaked into parent model history: {parent_input}"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_only_can_call_mcp_tool() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let code = r#"
const result = await tools.mcp__rmcp__echo({ message: "ping" });
text(`echo=${result.structuredContent?.echo ?? "missing"}`);
"#;

    let (_test, second_mock) = run_code_mode_turn_with_rmcp_mode(
        &server,
        "use exec to run the rmcp echo tool in code mode only",
        code,
        /*code_mode_only*/ true,
    )
    .await?;

    let req = second_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "code_mode_only rmcp tool call failed unexpectedly: {output}"
    );
    assert_eq!(output, "echo=ECHOING: ping");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_exposes_mcp_tools_on_global_tools_object() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let code = r#"
const { content, structuredContent, isError } = await tools.mcp__rmcp__echo({
  message: "ping",
});
text(
  `hasEcho=${String(Object.keys(tools).includes("mcp__rmcp__echo"))}\n` +
    `echoType=${typeof tools.mcp__rmcp__echo}\n` +
    `echo=${structuredContent?.echo ?? "missing"}\n` +
    `isError=${String(isError)}\n` +
    `contentLength=${content.length}`
);
"#;

    let (_test, second_mock) =
        run_code_mode_turn_with_rmcp(&server, "use exec to inspect the global tools object", code)
            .await?;

    let req = second_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "exec global rmcp access failed unexpectedly: {output}"
    );
    assert_eq!(
        output,
        "hasEcho=true
echoType=function
echo=ECHOING: ping
isError=false
contentLength=0"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_uses_non_prefixed_mcp_tool_names_when_feature_enabled() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let code = r#"
const result = await tools.rmcp__echo({ message: "ping" });
text(JSON.stringify({
  hasNonPrefixedEcho: typeof tools.rmcp__echo === "function",
  hasPrefixedEcho: typeof tools.mcp__rmcp__echo === "function",
  echo: result.structuredContent?.echo ?? "missing",
}));
"#;

    let (_test, second_mock) = run_code_mode_turn_with_rmcp_config(
        &server,
        "use exec to inspect non-prefixed MCP names",
        code,
        "test-gpt-5.1-codex",
        /*code_mode_only*/ false,
        /*non_prefixed_mcp_tool_names*/ true,
    )
    .await?;

    let req = second_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "exec non-prefixed rmcp access failed unexpectedly: {output}"
    );
    let parsed: Value = serde_json::from_str(&output)?;
    assert_eq!(
        parsed,
        serde_json::json!({
            "hasNonPrefixedEcho": true,
            "hasPrefixedEcho": false,
            "echo": "ECHOING: ping",
        })
    );

    Ok(())
}

struct NamespacedCustomTool {
    generation: usize,
    generations: Arc<AtomicUsize>,
}

impl ToolContributor for NamespacedCustomTool {
    fn tools(
        &self,
        _session_store: &ExtensionData,
        _thread_store: &ExtensionData,
    ) -> Vec<Arc<dyn ToolExecutor<ToolCall>>> {
        Vec::new()
    }

    fn tools_for_step(
        &self,
        _session_store: &ExtensionData,
        _thread_store: &ExtensionData,
        _step_store: &ExtensionData,
    ) -> Vec<Arc<dyn ToolExecutor<ToolCall>>> {
        vec![Arc::new(Self {
            generation: self.generations.fetch_add(1, Ordering::Relaxed) + 1,
            generations: Arc::clone(&self.generations),
        })]
    }
}

impl ToolExecutor<ToolCall> for NamespacedCustomTool {
    fn tool_name(&self) -> ToolName {
        ToolName::namespaced("editor", "apply_patch")
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec::Namespace(ResponsesApiNamespace {
            name: "editor".to_string(),
            description: "Editing tools.".to_string(),
            tools: vec![ResponsesApiNamespaceTool::Custom(FreeformTool {
                name: "apply_patch".to_string(),
                description: format!("Apply a raw editor patch (step {}).", self.generation),
                defer_loading: None,
                format: FreeformToolFormat {
                    r#type: "grammar".to_string(),
                    syntax: "lark".to_string(),
                    definition: "start: /.+/".to_string(),
                },
            })],
        })
    }

    fn handle(&self, call: ToolCall) -> ToolExecutorFuture<'_> {
        Box::pin(async move {
            let ToolPayload::Custom { input } = call.payload else {
                return Err(FunctionCallError::Fatal(
                    "expected custom tool payload".to_string(),
                ));
            };
            Ok(Box::new(JsonToolOutput::new(serde_json::json!({
                "namespace": call.tool_name.namespace,
                "name": call.tool_name.name,
                "input": input,
                "generation": self.generation,
            }))) as Box<dyn ToolOutput>)
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_exposes_and_dispatches_namespaced_custom_tools() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions.tool_contributor(Arc::new(NamespacedCustomTool {
        generation: 0,
        generations: Arc::new(AtomicUsize::new(0)),
    }));
    let mut builder = test_codex()
        .with_model("test-gpt-5.1-codex")
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| {
            let _ = config.features.enable(Feature::CodeMode);
        });
    let test = builder.build(&server).await?;
    let code = r#"
const tool = ALL_TOOLS.find(({ name }) => name === "editor__apply_patch");
const result = await tools.editor__apply_patch("nested patch");
text(JSON.stringify({
  name: tool?.name ?? null,
  description: tool?.description ?? null,
  result,
}));
"#;

    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                responses::ev_custom_tool_call_with_namespace(
                    "call-direct",
                    "editor",
                    "apply_patch",
                    "direct patch",
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_custom_tool_call("call-exec", "exec", code),
                ev_completed("resp-2"),
            ]),
            sse(vec![
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-3"),
            ]),
        ],
    )
    .await;

    test.submit_turn("call the namespaced custom editor tool directly and through exec")
        .await?;

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 3);

    let declaration =
        "declare const tools: { editor__apply_patch(input: string): Promise<unknown>; };";
    let description = |generation| {
        format!(
            "Apply a raw editor patch (step {generation}).\n\nexec tool declaration:\n```ts\n{declaration}\n```"
        )
    };
    let first_body = requests[0].body_json();
    let namespaced_custom_tool = namespace_child_tool(&first_body, "editor", "apply_patch")
        .expect("namespaced custom tool should be included in the model request");
    assert_eq!(
        namespaced_custom_tool,
        &serde_json::json!({
            "type": "custom",
            "name": "apply_patch",
            "description": description(1),
            "format": {
                "type": "grammar",
                "syntax": "lark",
                "definition": "start: /.+/",
            },
        })
    );
    let second_body = requests[1].body_json();
    let second_description = description(2);
    assert_eq!(
        namespace_child_tool(&second_body, "editor", "apply_patch")
            .and_then(|tool| tool.get("description"))
            .and_then(Value::as_str),
        Some(second_description.as_str())
    );

    let (direct_output, direct_success) =
        custom_tool_output_body_and_success(&requests[1], "call-direct");
    assert_ne!(direct_success, Some(false));
    let direct_output = serde_json::from_str::<Value>(&direct_output).unwrap_or_else(|error| {
        panic!("invalid direct custom tool output `{direct_output}`: {error}")
    });
    assert_eq!(
        direct_output,
        serde_json::json!({
            "namespace": "editor",
            "name": "apply_patch",
            "input": "direct patch",
            "generation": 1,
        })
    );

    let (exec_output, exec_success) =
        custom_tool_output_body_and_success(&requests[2], "call-exec");
    assert_ne!(exec_success, Some(false));
    let exec_output = serde_json::from_str::<Value>(&exec_output).unwrap_or_else(|error| {
        panic!("invalid code mode custom tool output `{exec_output}`: {error}")
    });
    assert_eq!(
        exec_output,
        serde_json::json!({
            "name": "editor__apply_patch",
            "description": format!("Editing tools.\n\n{second_description}"),
            "result": {
                "namespace": "editor",
                "name": "apply_patch",
                "input": "nested patch",
                "generation": 2,
            },
        })
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_exposes_namespaced_mcp_tools_on_global_tools_object() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let code = r#"
text(JSON.stringify({
  hasExecCommand: typeof tools.exec_command === "function",
  hasNamespacedEcho: typeof tools.mcp__rmcp__echo === "function",
}));
"#;

    let (_test, second_mock) =
        run_code_mode_turn_with_rmcp(&server, "use exec to inspect the global tools object", code)
            .await?;

    let req = second_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "exec global tools inspection failed unexpectedly: {output}"
    );

    let parsed: Value = serde_json::from_str(&output)?;
    assert_eq!(
        parsed,
        serde_json::json!({
            "hasExecCommand": true,
            "hasNamespacedEcho": true,
        })
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_exposes_normalized_illegal_mcp_tool_names() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let code = r#"
const result = await tools.mcp__rmcp__echo_tool({ message: "ping" });
text(`echo=${result.structuredContent.echo}`);
"#;

    let (_test, second_mock) = run_code_mode_turn_with_rmcp(
        &server,
        "use exec to call a normalized rmcp tool name",
        code,
    )
    .await?;

    let req = second_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "exec normalized rmcp tool call failed unexpectedly: {output}"
    );
    assert_eq!(output, "echo=ECHOING: ping");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_exports_all_tools_metadata_for_builtin_tools() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let code = r#"
const tool = ALL_TOOLS.find(({ name }) => name === "view_image");
text(JSON.stringify(tool));
"#;

    let (_test, second_mock) =
        run_code_mode_turn(&server, "use exec to inspect ALL_TOOLS", code).await?;

    let req = second_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "exec ALL_TOOLS lookup failed unexpectedly: {output}"
    );

    let parsed: Value = serde_json::from_str(
        &custom_tool_output_last_non_empty_text(&req, "call-1")
            .expect("exec ALL_TOOLS lookup should emit JSON"),
    )?;
    assert_eq!(
        parsed,
        serde_json::json!({
            "name": "view_image",
            "description": "View a local image file from the filesystem when visual inspection is needed. Use this for images already available on disk.\n\nexec tool declaration:\n```ts\ndeclare const tools: { view_image(args: {\n  // Local filesystem path to an image file.\n  path: string;\n}): Promise<{\n  // Image detail hint returned by view_image. Returns `high` for default resized behavior or `original` when original resolution is preserved.\n  detail: \"high\" | \"original\";\n  // Data URL for the loaded image.\n  image_url: string;\n}>; };\n```",
        })
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_exports_all_tools_metadata_for_namespaced_mcp_tools() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let code = r#"
const tool = ALL_TOOLS.find(
  ({ name }) => name === "mcp__rmcp__echo"
);
text(JSON.stringify(tool));
"#;

    let (_test, second_mock) =
        run_code_mode_turn_with_rmcp(&server, "use exec to inspect ALL_TOOLS", code).await?;

    let req = second_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "exec ALL_TOOLS MCP lookup failed unexpectedly: {output}"
    );

    let parsed: Value = serde_json::from_str(
        &custom_tool_output_last_non_empty_text(&req, "call-1")
            .expect("exec ALL_TOOLS MCP lookup should emit JSON"),
    )?;
    assert_eq!(
        parsed,
        serde_json::json!({
            "name": "mcp__rmcp__echo",
            "description": concat!(
                "Use these tools to exercise the rmcp test server.\n\n",
                "Echo back the provided message and include environment data.\n\n",
                "exec tool declaration:\n",
                "```ts\n",
                "declare const tools: { mcp__rmcp__echo(args: { env_var?: string; message: string; }): ",
                "Promise<CallToolResult<{ echo: string; env: string | null; }>>; };\n",
                "```",
            ),
        })
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_uses_the_first_dynamic_tool_for_a_normalized_name() -> Result<()> {
    skip_if_no_network!(Ok(()));

    for use_responses_lite in [false, true] {
        let server = responses::start_mock_server().await;
        let mut builder = test_codex()
            .with_model_info_override("gpt-5.5", move |model_info| {
                model_info.use_responses_lite = use_responses_lite;
                model_info.tool_mode = Some(ToolMode::CodeMode);
            })
            .with_config(|config| {
                config.tool_registry.turn_metadata_includes_tool_info = true;
                config
                    .features
                    .enable(Feature::CodeMode)
                    .expect("code mode should be enabled");
            });
        let base_test = builder.build_with_auto_env(&server).await?;
        let new_thread = base_test
            .thread_manager
            .start_thread(StartThreadOptions {
                dynamic_tools: [
                    ("foo-bar", "First normalized dynamic tool."),
                    ("foo_bar", "Shadowed normalized dynamic tool."),
                ]
                .into_iter()
                .map(|(name, description)| {
                    DynamicToolSpec::Function(DynamicToolFunctionSpec {
                        name: name.to_string(),
                        description: description.to_string(),
                        input_schema: serde_json::json!({
                            "type": "object",
                            "properties": {},
                            "additionalProperties": false,
                        }),
                        defer_loading: false,
                    })
                })
                .collect(),
                ..StartThreadOptions::new(base_test.config.clone())
            })
            .await?;
        let mut test = base_test;
        test.codex = new_thread.thread;
        test.session_configured = new_thread.session_configured;

        let first_response = if use_responses_lite {
            sse(vec![
                ev_response_created("resp-1"),
                ev_custom_tool_call(
                    "call-1",
                    "exec",
                    r#"
const matches = ALL_TOOLS.filter(({ name }) => name === "foo_bar");
const output = await tools.foo_bar({});
text(JSON.stringify({
  count: matches.length,
  name: matches[0]?.name ?? null,
  description: matches[0]?.description ?? null,
  output,
}));
"#,
                ),
                ev_completed("resp-1"),
            ])
        } else {
            sse(vec![
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-1"),
            ])
        };
        let first_mock = responses::mount_sse_once(&server, first_response).await;
        let second_mock = if use_responses_lite {
            Some(
                responses::mount_sse_once(
                    &server,
                    sse(vec![
                        ev_assistant_message("msg-1", "done"),
                        ev_completed("resp-2"),
                    ]),
                )
                .await,
            )
        } else {
            None
        };

        let cwd = test.config.cwd.clone();
        let (sandbox_policy, permission_profile) =
            turn_permission_fields(PermissionProfile::Disabled, cwd.as_path());
        test.codex
            .start_or_steer_turn(
                TurnInputRequest::user_input(vec![UserInput::Text {
                    text: "inspect and call normalized dynamic tools".to_string(),
                    text_elements: Vec::new(),
                }])
                .with_thread_settings(ThreadSettingsOverrides {
                    environments: Some(codex_protocol::protocol::TurnEnvironmentSelections::new(
                        cwd,
                        Vec::new(),
                    )),
                    approval_policy: Some(AskForApproval::Never),
                    sandbox_policy: Some(sandbox_policy),
                    permission_profile,
                    collaboration_mode: Some(CollaborationMode {
                        mode: ModeKind::Default,
                        settings: Settings {
                            model: test.session_configured.model.clone(),
                            reasoning_effort: None,
                            developer_instructions: None,
                        },
                    }),
                    ..Default::default()
                }),
            )
            .await?;

        let turn_id = wait_for_event_match(&test.codex, |event| match event {
            EventMsg::TurnStarted(event) => Some(event.turn_id.clone()),
            _ => None,
        })
        .await;
        if use_responses_lite {
            let request = wait_for_event_match(&test.codex, |event| match event {
                EventMsg::DynamicToolCallRequest(request) => Some(request.clone()),
                _ => None,
            })
            .await;
            assert_eq!(request.namespace, None);
            assert_eq!(request.tool, "foo-bar");
            assert_eq!(request.arguments, serde_json::json!({}));
            test.codex
                .submit(Op::DynamicToolResponse {
                    id: request.call_id,
                    response: DynamicToolResponse {
                        content_items: vec![DynamicToolCallOutputContentItem::InputText {
                            text: "first-winner".to_string(),
                        }],
                        success: true,
                    },
                })
                .await?;
        }
        wait_for_event(&test.codex, |event| match event {
            EventMsg::TurnComplete(event) => event.turn_id == turn_id,
            _ => false,
        })
        .await;

        let first_body = first_mock.single_request().body_json();
        let model_tools = if use_responses_lite {
            first_body["input"]
                .as_array()
                .and_then(|input| {
                    input.iter().find(|item| {
                        item.get("type").and_then(Value::as_str) == Some("additional_tools")
                    })
                })
                .and_then(|item| item["tools"].as_array())
                .and_then(|tools| {
                    tools.iter().find(|tool| {
                        tool.get("type").and_then(Value::as_str) == Some("namespace")
                            && tool.get("name").and_then(Value::as_str) == Some("functions")
                    })
                })
                .and_then(|namespace| namespace["tools"].as_array())
                .expect("the Responses Lite request should contain its default-namespace tools")
        } else {
            first_body["tools"]
                .as_array()
                .expect("the Responses request should contain its visible tools")
        };
        let visible_dynamic_tools = model_tools
            .iter()
            .filter(|tool| matches!(tool["name"].as_str(), Some("foo-bar" | "foo_bar")))
            .map(|tool| {
                (
                    tool["name"]
                        .as_str()
                        .expect("dynamic tools should have a name"),
                    tool["description"]
                        .as_str()
                        .expect("dynamic tools should have a description"),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            visible_dynamic_tools,
            [
                (
                    "foo-bar",
                    concat!(
                        "First normalized dynamic tool.\n\n",
                        "exec tool declaration:\n",
                        "```ts\n",
                        "declare const tools: { foo_bar(args: {}): Promise<unknown>; };\n",
                        "```",
                    ),
                ),
                ("foo_bar", "Shadowed normalized dynamic tool."),
            ]
        );

        if use_responses_lite {
            let metadata: Value = serde_json::from_str(
                first_body["client_metadata"]["x-codex-turn-metadata"]
                    .as_str()
                    .expect("Responses Lite should contain serialized turn metadata"),
            )?;
            let functions = &metadata["tool_namespaces_info"]["functions"]["functions"];
            assert_eq!(functions["foo-bar"]["code_mode_name"], "foo_bar");
            assert!(functions["foo_bar"]["code_mode_name"].is_null());
        }

        let exec_description = model_tools
            .iter()
            .find(|tool| tool["name"] == "exec")
            .and_then(|tool| tool["description"].as_str())
            .expect("the model request should contain the code-mode exec tool");
        assert!(!exec_description.contains("First normalized dynamic tool."));
        assert!(!exec_description.contains("Shadowed normalized dynamic tool."));

        if let Some(second_mock) = second_mock {
            let request = second_mock.single_request();
            let output = custom_tool_output_last_non_empty_text(&request, "call-1")
                .expect("code mode should return normalized tool metadata");
            let result: Value = serde_json::from_str(&output)?;
            assert_eq!(result["count"], serde_json::json!(1));
            assert_eq!(result["name"], serde_json::json!("foo_bar"));
            assert_eq!(result["output"], serde_json::json!("first-winner"));
            let description = result["description"]
                .as_str()
                .expect("the winning tool should have a description");
            assert!(description.contains("First normalized dynamic tool."));
            assert!(!description.contains("Shadowed normalized dynamic tool."));
        }
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_renders_local_refs_in_outbound_exec_description() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let response = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let mut builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::CodeModeOnly)
            .expect("code mode only should be enabled");
    });
    let base_test = builder.build_with_auto_env(&server).await?;
    let new_thread = base_test
        .thread_manager
        .start_thread(StartThreadOptions {
            dynamic_tools: vec![DynamicToolSpec::Function(DynamicToolFunctionSpec {
                name: "boolean_search".to_string(),
                description: "Search with nested Boolean clauses.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "clauses": {
                            "type": "array",
                            "items": { "$ref": "#/$defs/Clause" }
                        }
                    },
                    "$defs": {
                        "Clause": {
                            "type": "object",
                            "properties": {
                                "query": {
                                    "oneOf": [
                                        { "type": "string" },
                                        {
                                            "type": "object",
                                            "properties": {
                                                "clauses": {
                                                    "type": "array",
                                                    "items": { "$ref": "#/$defs/Clause" }
                                                }
                                            }
                                        }
                                    ]
                                }
                            }
                        }
                    }
                }),
                defer_loading: false,
            })],
            ..StartThreadOptions::new(base_test.config.clone())
        })
        .await?;
    let mut test = base_test;
    test.codex = new_thread.thread;
    test.session_configured = new_thread.session_configured;

    test.submit_turn("inspect the tool schema").await?;

    let body = response.single_request().body_json();
    let exec_description = body["tools"]
        .as_array()
        .expect("request should contain tools")
        .iter()
        .find_map(|tool| {
            (tool["name"].as_str() == Some("exec"))
                .then(|| tool["description"].as_str())
                .flatten()
        })
        .expect("Code Mode exec should remain available");
    assert!(exec_description.contains("query?: string | { clauses?: Array<"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_can_call_hidden_dynamic_tools() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mut builder = test_codex().with_config(move |config| {
        let _ = config.features.enable(Feature::CodeMode);
    });
    let base_test = builder.build(&server).await?;
    let new_thread = base_test
        .thread_manager
        .start_thread(StartThreadOptions {
            dynamic_tools: vec![DynamicToolSpec::Namespace(DynamicToolNamespaceSpec {
                name: "codex_app".to_string(),
                description: "Codex app tools.".to_string(),
                tools: vec![DynamicToolNamespaceTool::Function(
                    DynamicToolFunctionSpec {
                        name: "hidden_dynamic_tool".to_string(),
                        description: "A hidden dynamic tool.".to_string(),
                        input_schema: serde_json::json!({
                                "type": "object",
                                "properties": {
                                    "city": { "type": "string" }
                                },
                            "required": ["city"],
                            "additionalProperties": false,
                        }),
                        defer_loading: true,
                    },
                )],
            })],
            ..StartThreadOptions::new(base_test.config.clone())
        })
        .await?;
    let mut test = base_test;
    test.codex = new_thread.thread;
    test.session_configured = new_thread.session_configured;

    let code = r#"
const tool = ALL_TOOLS.find(({ name }) => name === "codex_app__hidden_dynamic_tool");
const out = await tools.codex_app__hidden_dynamic_tool({ city: "Paris" });
text(
  JSON.stringify({
    name: tool?.name ?? null,
    description: tool?.description ?? null,
    out,
  })
);
"#;

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call("call-1", "exec", code),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let second_mock = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    let cwd = test.config.cwd.clone();
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, cwd.as_path());

    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "use exec to inspect and call hidden tools".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(codex_protocol::protocol::TurnEnvironmentSelections::new(
                    cwd,
                    Vec::new(),
                )),
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(CollaborationMode {
                    mode: ModeKind::Default,
                    settings: Settings {
                        model: test.session_configured.model.clone(),
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            }),
        )
        .await?;

    let turn_id = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::TurnStarted(event) => Some(event.turn_id.clone()),
        _ => None,
    })
    .await;
    let request = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::DynamicToolCallRequest(request) => Some(request.clone()),
        _ => None,
    })
    .await;
    assert_eq!(request.namespace.as_deref(), Some("codex_app"));
    assert_eq!(request.tool, "hidden_dynamic_tool");
    assert_eq!(request.arguments, serde_json::json!({ "city": "Paris" }));
    test.codex
        .submit(Op::DynamicToolResponse {
            id: request.call_id,
            response: DynamicToolResponse {
                content_items: vec![DynamicToolCallOutputContentItem::InputText {
                    text: "hidden-ok".to_string(),
                }],
                success: true,
            },
        })
        .await?;
    wait_for_event(&test.codex, |event| match event {
        EventMsg::TurnComplete(event) => event.turn_id == turn_id,
        _ => false,
    })
    .await;

    let req = second_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "exec hidden dynamic tool call failed unexpectedly: {output}"
    );

    let parsed: Value = serde_json::from_str(
        &custom_tool_output_last_non_empty_text(&req, "call-1")
            .expect("exec hidden dynamic tool lookup should emit JSON"),
    )?;
    assert_eq!(
        parsed.get("name"),
        Some(&Value::String("codex_app__hidden_dynamic_tool".to_string()))
    );
    assert_eq!(
        parsed.get("out"),
        Some(&Value::String("hidden-ok".to_string()))
    );
    assert!(
        parsed
            .get("description")
            .and_then(Value::as_str)
            .is_some_and(|description| {
                description.contains("Codex app tools.")
                    && description.contains("A hidden dynamic tool.")
                    && description.contains("declare const tools:")
                    && description.contains("codex_app__hidden_dynamic_tool(args:")
            })
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_excludes_configured_nested_tool_namespaces() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        let _ = config.features.enable(Feature::CodeMode);
        config.code_mode.excluded_tool_namespaces = vec!["excluded".to_string()];
    });
    let base_test = builder.build(&server).await?;
    let new_thread = base_test
        .thread_manager
        .start_thread(StartThreadOptions {
            dynamic_tools: vec![DynamicToolSpec::Namespace(DynamicToolNamespaceSpec {
                name: "excluded".to_string(),
                description: "Excluded tools.".to_string(),
                tools: vec![DynamicToolNamespaceTool::Function(
                    DynamicToolFunctionSpec {
                        name: "lookup".to_string(),
                        description: "An excluded dynamic tool.".to_string(),
                        input_schema: serde_json::json!({
                            "type": "object",
                            "properties": {},
                            "additionalProperties": false,
                        }),
                        defer_loading: false,
                    },
                )],
            })],
            ..StartThreadOptions::new(base_test.config.clone())
        })
        .await?;
    let mut test = base_test;
    test.codex = new_thread.thread;
    test.session_configured = new_thread.session_configured;

    let first_mock = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call(
                "call-1",
                "exec",
                r#"
text(JSON.stringify({
  excludedType: typeof tools.excluded__lookup,
  excludedMetadata: ALL_TOOLS.some(({ name }) => name === "excluded__lookup"),
  allowedType: typeof tools.update_plan,
  allowedMetadata: ALL_TOOLS.some(({ name }) => name === "update_plan"),
}));
"#,
            ),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let second_mock = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn("use exec to inspect nested tool namespaces")
        .await?;

    assert!(
        tool_names(&first_mock.single_request().body_json()).contains(&"excluded".to_string()),
        "excluded namespace should remain directly exposed in mixed code mode"
    );
    let request = second_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&request, "call-1");
    assert_ne!(
        success,
        Some(false),
        "exec configured namespace exclusion failed unexpectedly: {output}"
    );
    assert_eq!(
        serde_json::from_str::<Value>(&output)?,
        serde_json::json!({
            "excludedType": "undefined",
            "excludedMetadata": false,
            "allowedType": "function",
            "allowedMetadata": true,
        })
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_omits_configured_mcp_server_tools() -> Result<()> {
    skip_if_wine_exec!(
        Ok(()),
        "requires a Windows test_stdio_server in the Wine-exec environment"
    );
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let rmcp_test_server_bin = remote_aware_stdio_server_bin()?;
    let environment_id = remote_aware_environment_id();
    let mut builder = test_codex()
        .with_model_info_override("gpt-5.4", |model| {
            model.supports_search_tool = false;
        })
        .with_config(move |config| {
            let _ = config.features.enable(Feature::CodeMode);
            let mut servers = config.mcp_servers.get().clone();
            servers.insert(
                "rmcp".to_string(),
                serde_json::from_value(serde_json::json!({
                    "command": rmcp_test_server_bin,
                    "environment_id": environment_id,
                    "omit_tools_from": ["code_mode"],
                }))
                .expect("test MCP server config should be valid"),
            );
            config
                .mcp_servers
                .set(servers)
                .expect("test config should allow MCP servers");
        });
    let test = builder.build_with_auto_env(&server).await?;
    wait_for_mcp_server(&test.codex, "rmcp").await?;

    let first_mock = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call(
                "call-1",
                "exec",
                r#"
text(JSON.stringify({
  excludedType: typeof tools.mcp__rmcp__echo,
  excludedMetadata: ALL_TOOLS.some(({ name }) => name === "mcp__rmcp__echo"),
  allowedType: typeof tools.update_plan,
  allowedMetadata: ALL_TOOLS.some(({ name }) => name === "update_plan"),
}));
"#,
            ),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let second_mock = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn("use exec to inspect nested tool namespaces")
        .await?;

    assert!(
        tool_names(&first_mock.single_request().body_json()).contains(&"mcp__rmcp".to_string()),
        "omitted MCP server should remain directly exposed in mixed code mode"
    );
    let request = second_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&request, "call-1");
    assert_ne!(
        success,
        Some(false),
        "exec configured MCP omission failed unexpectedly: {output}"
    );
    assert_eq!(
        serde_json::from_str::<Value>(&output)?,
        serde_json::json!({
            "excludedType": "undefined",
            "excludedMetadata": false,
            "allowedType": "function",
            "allowedMetadata": true,
        })
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_only_keeps_mcp_tools_direct_when_nested_exposure_is_omitted() -> Result<()> {
    skip_if_wine_exec!(
        Ok(()),
        "requires a Windows test_stdio_server in the Wine-exec environment"
    );
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let rmcp_test_server_bin = remote_aware_stdio_server_bin()?;
    let environment_id = remote_aware_environment_id();
    let mut builder = test_codex()
        .with_model_info_override("gpt-5.4", |model| {
            model.supports_search_tool = true;
        })
        .with_config(move |config| {
            let _ = config.features.enable(Feature::CodeModeOnly);
            let mut servers = config.mcp_servers.get().clone();
            servers.insert(
                "rmcp".to_string(),
                serde_json::from_value(serde_json::json!({
                    "command": rmcp_test_server_bin,
                    "environment_id": environment_id,
                    "omit_tools_from": ["code_mode"],
                }))
                .expect("test MCP server config should be valid"),
            );
            config
                .mcp_servers
                .set(servers)
                .expect("test config should allow MCP servers");
        });
    let test = builder.build_with_auto_env(&server).await?;
    wait_for_mcp_server(&test.codex, "rmcp").await?;

    let first_mock = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            responses::ev_function_call_with_namespace(
                "call-1",
                "mcp__rmcp",
                "echo",
                r#"{"message":"ping"}"#,
            ),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let second_mock = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn("call the directly exposed MCP echo tool")
        .await?;

    let first_request = first_mock.single_request().body_json();
    assert!(
        namespace_child_tool(&first_request, "mcp__rmcp", "echo").is_some(),
        "an MCP tool omitted only from Code Mode must remain directly callable"
    );
    let exec_description = first_request["tools"]
        .as_array()
        .expect("request should contain tools")
        .iter()
        .find_map(|tool| {
            (tool["name"].as_str() == Some("exec"))
                .then(|| tool["description"].as_str())
                .flatten()
        })
        .expect("Code Mode exec should remain available");
    assert!(
        !exec_description.contains("mcp__rmcp__echo"),
        "an MCP tool omitted from Code Mode must not appear in the exec declaration"
    );

    let output = second_mock.single_request().function_call_output("call-1");
    assert!(
        output["output"]
            .as_str()
            .is_some_and(|output| output.contains("ECHOING: ping")),
        "the directly exposed MCP tool must execute successfully: {output:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_only_can_call_mcp_tools_hidden_from_direct_and_deferred_exposure() -> Result<()>
{
    skip_if_wine_exec!(
        Ok(()),
        "requires a Windows test_stdio_server in the Wine-exec environment"
    );
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let rmcp_test_server_bin = remote_aware_stdio_server_bin()?;
    let environment_id = remote_aware_environment_id();
    let mut builder = test_codex()
        .with_model_info_override("gpt-5.4", |model| {
            model.supports_search_tool = true;
        })
        .with_config(move |config| {
            let _ = config.features.enable(Feature::CodeModeOnly);
            let mut servers = config.mcp_servers.get().clone();
            servers.insert(
                "rmcp".to_string(),
                serde_json::from_value(serde_json::json!({
                    "command": rmcp_test_server_bin,
                    "environment_id": environment_id,
                    "omit_tools_from": ["direct", "deferred"],
                    "supports_parallel_tool_calls": true,
                }))
                .expect("test MCP server config should be valid"),
            );
            config
                .mcp_servers
                .set(servers)
                .expect("test config should allow MCP servers");
        });
    let test = builder.build_with_auto_env(&server).await?;
    wait_for_mcp_server(&test.codex, "rmcp").await?;

    let first_mock = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call(
                "call-1",
                "exec",
                r#"
const results = await Promise.all([
  tools.mcp__rmcp__echo({ message: "ping" }),
  tools.mcp__rmcp__echo({ message: "pong" }),
]);
text(results.map(({ structuredContent }) => structuredContent?.echo ?? "missing").join(","));
"#,
            ),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let second_mock = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn("call the MCP echo tool from Code Mode")
        .await?;

    let first_request = first_mock.single_request().body_json();
    assert!(
        namespace_child_tool(&first_request, "mcp__rmcp", "echo").is_none(),
        "MCP tool omitted from direct and deferred exposure must remain hidden at the top level"
    );
    let exec_description = first_request["tools"]
        .as_array()
        .expect("request should contain tools")
        .iter()
        .find_map(|tool| {
            (tool["name"].as_str() == Some("exec"))
                .then(|| tool["description"].as_str())
                .flatten()
        })
        .expect("Code Mode exec should remain available");
    assert!(
        exec_description.contains("mcp__rmcp__echo"),
        "hidden MCP tool must remain available inside Code Mode"
    );

    let request = second_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&request, "call-1");
    assert_ne!(
        success,
        Some(false),
        "hidden Code Mode MCP tool call failed unexpectedly: {output}"
    );
    assert_eq!(output, "ECHOING: ping,ECHOING: pong");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_can_print_content_only_mcp_tool_result_fields() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let code = r#"
const { content, structuredContent, isError } = await tools.mcp__rmcp__image_scenario({
  scenario: "text_only",
  caption: "caption from mcp",
});
text(
  `firstType=${content[0]?.type ?? "missing"}\n` +
    `firstText=${content[0]?.text ?? "missing"}\n` +
    `structuredContent=${String(structuredContent ?? null)}\n` +
    `isError=${String(isError)}`
);
"#;

    let (_test, second_mock) = run_code_mode_turn_with_rmcp(
        &server,
        "use exec to run the rmcp image scenario tool",
        code,
    )
    .await?;

    let req = second_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "exec rmcp image scenario call failed unexpectedly: {output}"
    );
    assert_eq!(
        output,
        "firstType=text
firstText=caption from mcp
structuredContent=null
isError=false"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_can_print_error_mcp_tool_result_fields() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let code = r#"
const { content, structuredContent, isError } = await tools.mcp__rmcp__echo({});
const firstText = content[0]?.text ?? "";
const mentionsMissingMessage =
  firstText.includes("missing field") && firstText.includes("message");
text(
  `isError=${String(isError)}\n` +
    `contentLength=${content.length}\n` +
    `mentionsMissingMessage=${String(mentionsMissingMessage)}\n` +
    `structuredContent=${String(structuredContent ?? null)}`
);
"#;

    let (_test, second_mock) =
        run_code_mode_turn_with_rmcp(&server, "use exec to call rmcp echo badly", code).await?;

    let req = second_mock.single_request();
    let (output, success) = custom_tool_output_body_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "exec rmcp error call failed unexpectedly: {output}"
    );
    assert_eq!(
        output,
        "isError=true
contentLength=1
mentionsMissingMessage=true
structuredContent=null"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_can_store_and_load_values_across_turns() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mut builder = test_codex().with_config(move |config| {
        let _ = config.features.enable(Feature::CodeMode);
    });
    let test = builder.build(&server).await?;

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call(
                "call-1",
                "exec",
                r#"
store("nb", { title: "Notebook", items: [1, true, null] });
text("stored");
"#,
            ),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let first_follow_up = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "stored"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn("store value for later").await?;

    let first_request = first_follow_up.single_request();
    let (first_output, first_success) =
        custom_tool_output_body_and_success(&first_request, "call-1");
    assert_ne!(
        first_success,
        Some(false),
        "exec store call failed unexpectedly: {first_output}"
    );
    assert_eq!(first_output, "stored");

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-3"),
            ev_custom_tool_call(
                "call-2",
                "exec",
                r#"
text(JSON.stringify(load("nb")));
"#,
            ),
            ev_completed("resp-3"),
        ]),
    )
    .await;
    let second_follow_up = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-2", "loaded"),
            ev_completed("resp-4"),
        ]),
    )
    .await;

    test.submit_turn("load the stored value").await?;

    let second_request = second_follow_up.single_request();
    let (second_output, second_success) =
        custom_tool_output_body_and_success(&second_request, "call-2");
    assert_ne!(
        second_success,
        Some(false),
        "exec load call failed unexpectedly: {second_output}"
    );
    let loaded: Value = serde_json::from_str(
        &custom_tool_output_last_non_empty_text(&second_request, "call-2")
            .expect("exec load call should emit JSON"),
    )?;
    assert_eq!(
        loaded,
        serde_json::json!({ "title": "Notebook", "items": [1, true, null] })
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_can_compare_elapsed_time_around_set_timeout() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let (_test, second_mock) = run_code_mode_turn(
        &server,
        "measure elapsed time around setTimeout",
        r#"
const start_ms = Date.now();
await new Promise((resolve) => setTimeout(resolve, 100));
const end_ms = Date.now();
text(JSON.stringify({
  start_ms,
  end_ms,
  elapsed_ms: end_ms - start_ms,
  waited_long_enough: end_ms - start_ms >= 100,
}));
"#,
    )
    .await?;

    let second_request = second_mock.single_request();
    let (second_output, second_success) =
        custom_tool_output_body_and_success(&second_request, "call-1");
    assert_ne!(
        second_success,
        Some(false),
        "exec compare time call failed unexpectedly: {second_output}"
    );
    let compared: Value = serde_json::from_str(
        &custom_tool_output_last_non_empty_text(&second_request, "call-1")
            .expect("exec compare time call should emit JSON"),
    )?;
    let elapsed_ms = compared
        .get("elapsed_ms")
        .and_then(Value::as_i64)
        .expect("elapsed_ms should be an integer");
    assert!(
        elapsed_ms >= 100,
        "expected elapsed_ms >= 100, got {elapsed_ms}"
    );
    assert_eq!(compared.get("waited_long_enough"), Some(&Value::Bool(true)));

    Ok(())
}
