use codex_core::CodexAuth;
use codex_core::ConversationManager;
use codex_core::ModelProviderInfo;
use codex_core::built_in_model_providers;
use codex_core::features::Feature;
use codex_core::model_family::find_family_for_model;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol_config_types::ReasoningEffort;
use codex_protocol::user_input::UserInput;
use core_test_support::load_default_config_for_test;
use core_test_support::load_sse_fixture_with_id;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

const CONFIG_TOML: &str = "config.toml";

fn sse_completed(id: &str) -> String {
    load_sse_fixture_with_id("tests/fixtures/completed_template.json", id)
}

fn tool_identifiers(body: &Value) -> Vec<String> {
    let tools = match body["tools"].as_array() {
        Some(array) => array,
        None => panic!("tool list missing in Responses payload: {body:?}"),
    };

    tools
        .iter()
        .map(|tool| {
            if let Some(name) = tool.get("name").and_then(Value::as_str) {
                name.to_string()
            } else if let Some(kind) = tool.get("type").and_then(Value::as_str) {
                kind.to_string()
            } else {
                panic!("tool entry missing identifiers: {tool:?}");
            }
        })
        .collect()
}

fn allowed_tool_specs(body: &Value) -> Option<Vec<Value>> {
    match body.get("tool_choice") {
        None => panic!("tool_choice missing in Responses payload: {body:?}"),
        Some(Value::String(name)) => {
            assert_eq!(name, "auto", "unexpected tool_choice string: {name}");
            None
        }
        Some(Value::Object(obj)) => {
            let ty = obj
                .get("type")
                .and_then(Value::as_str)
                .expect("tool_choice.type field");
            assert_eq!(ty, "allowed_tools", "unexpected tool_choice type: {ty}");
            let mode = obj
                .get("mode")
                .and_then(Value::as_str)
                .expect("tool_choice.mode field");
            assert_eq!(mode, "auto", "unexpected tool_choice mode: {mode}");
            let tools = obj
                .get("tools")
                .and_then(Value::as_array)
                .expect("tool_choice.tools field");
            Some(tools.to_vec())
        }
        Some(other) => panic!("unexpected tool_choice payload: {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn override_turn_context_does_not_persist_when_config_exists() {
    let codex_home = TempDir::new().unwrap();
    let config_path = codex_home.path().join(CONFIG_TOML);
    let initial_contents = "model = \"gpt-4o\"\n";
    tokio::fs::write(&config_path, initial_contents)
        .await
        .expect("seed config.toml");

    let mut config = load_default_config_for_test(&codex_home);
    config.model = "gpt-4o".to_string();

    let conversation_manager =
        ConversationManager::with_auth(CodexAuth::from_api_key("Test API Key"));
    let codex = conversation_manager
        .new_conversation(config)
        .await
        .expect("create conversation")
        .conversation;

    codex
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: None,
            sandbox_policy: None,
            model: Some("o3".to_string()),
            effort: Some(Some(ReasoningEffort::High)),
            summary: None,
            include_web_search_request: None,
        })
        .await
        .expect("submit override");

    codex.submit(Op::Shutdown).await.expect("request shutdown");
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ShutdownComplete)).await;

    let contents = tokio::fs::read_to_string(&config_path)
        .await
        .expect("read config.toml after override");
    assert_eq!(contents, initial_contents);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn override_turn_context_does_not_create_config_file() {
    let codex_home = TempDir::new().unwrap();
    let config_path = codex_home.path().join(CONFIG_TOML);
    assert!(
        !config_path.exists(),
        "test setup should start without config"
    );

    let config = load_default_config_for_test(&codex_home);

    let conversation_manager =
        ConversationManager::with_auth(CodexAuth::from_api_key("Test API Key"));
    let codex = conversation_manager
        .new_conversation(config)
        .await
        .expect("create conversation")
        .conversation;

    codex
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: None,
            sandbox_policy: None,
            model: Some("o3".to_string()),
            effort: Some(Some(ReasoningEffort::Medium)),
            summary: None,
            include_web_search_request: None,
        })
        .await
        .expect("submit override");

    codex.submit(Op::Shutdown).await.expect("request shutdown");
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ShutdownComplete)).await;

    assert!(
        !config_path.exists(),
        "override should not create config.toml"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn override_turn_context_toggles_web_search_tool() {
    let server = MockServer::start().await;

    let sse = sse_completed("toggle-web-search");
    let template = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse, "text/event-stream");

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(template.clone())
        .expect(2)
        .mount(&server)
        .await;

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

    let cwd = TempDir::new().unwrap();
    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home);
    config.cwd = cwd.path().to_path_buf();
    config.model = "gpt-5-codex".to_string();
    config.model_family = find_family_for_model("gpt-5-codex").expect("model family");
    config.model_provider = model_provider;
    config.model_provider_id = "openai".to_string();
    config.features.disable(Feature::WebSearchRequest);
    config.features.disable(Feature::ViewImageTool);
    config.features.disable(Feature::ApplyPatchFreeform);
    config.features.disable(Feature::StreamableShell);
    config.features.disable(Feature::UnifiedExec);
    config.features.disable(Feature::SandboxCommandAssessment);
    config.tools_web_search_request = config.features.enabled(Feature::WebSearchRequest);

    let conversation_manager =
        ConversationManager::with_auth(CodexAuth::from_api_key("Test API Key"));
    let codex = conversation_manager
        .new_conversation(config)
        .await
        .expect("create conversation")
        .conversation;

    codex
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: None,
            sandbox_policy: None,
            model: None,
            effort: None,
            summary: None,
            include_web_search_request: Some(true),
        })
        .await
        .expect("enable web search");

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "first turn".to_string(),
            }],
        })
        .await
        .expect("submit first input");
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    codex
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: None,
            sandbox_policy: None,
            model: None,
            effort: None,
            summary: None,
            include_web_search_request: Some(false),
        })
        .await
        .expect("disable web search");

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "second turn".to_string(),
            }],
        })
        .await
        .expect("submit second input");
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let requests = server
        .received_requests()
        .await
        .unwrap_or_else(|| Vec::new());
    assert_eq!(requests.len(), 2, "expected two requests for two turns");

    let first = match requests[0].body_json::<Value>() {
        Ok(json) => json,
        Err(err) => panic!("first request body should be JSON: {err}"),
    };
    let first_tools = tool_identifiers(&first);
    assert!(
        first_tools.iter().any(|tool| tool == "web_search"),
        "expected web_search tool after enabling; got {first_tools:?}"
    );
    assert!(
        allowed_tool_specs(&first).is_none(),
        "expected auto tool choice when web_search enabled"
    );

    let second = match requests[1].body_json::<Value>() {
        Ok(json) => json,
        Err(err) => panic!("second request body should be JSON: {err}"),
    };
    let second_tools = tool_identifiers(&second);
    assert!(
        second_tools.iter().any(|tool| tool == "web_search"),
        "expected web_search tool spec to remain present; got {second_tools:?}"
    );
    let second_allowed = allowed_tool_specs(&second).expect("expected allowed tools payload");
    assert!(
        second_allowed.iter().all(|tool| tool
            .get("type")
            .and_then(Value::as_str)
            .map(|ty| ty != "web_search")
            .unwrap_or(true)),
        "expected web_search not to be allowed after disabling; got {second_allowed:?}"
    );
    assert!(
        second_allowed.iter().any(|tool| {
            tool.get("type") == Some(&Value::String("function".to_string()))
                && tool.get("name") == Some(&Value::String("shell".to_string()))
        }),
        "expected shell function to remain allowed; got {second_allowed:?}"
    );
}
