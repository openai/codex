use std::process::Command;
use std::sync::Arc;

use codex_core::CodexAuth;
use codex_core::ContentItem;
use codex_core::ModelClient;
use codex_core::ModelProviderInfo;
use codex_core::Prompt;
use codex_core::ResponseEvent;
use codex_core::ResponseItem;
use codex_core::WireApi;
use codex_otel::OtelManager;
use codex_otel::TelemetryAuthMode;
use codex_protocol::ThreadId;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use core_test_support::load_default_config_for_test;
use core_test_support::responses;
use core_test_support::test_codex::test_codex;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use wiremock::matchers::header;

#[tokio::test]
async fn responses_stream_includes_subagent_header_on_review() {
    core_test_support::skip_if_no_network!();

    let server = responses::start_mock_server().await;
    let response_body = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_completed("resp-1"),
    ]);

    let request_recorder = responses::mount_sse_once_match(
        &server,
        header("x-openai-subagent", "review"),
        response_body,
    )
    .await;

    let provider = ModelProviderInfo {
        name: "mock".into(),
        base_url: Some(format!("{}/v1", server.uri())),
        env_key: None,
        env_key_instructions: None,
        experimental_bearer_token: None,
        wire_api: WireApi::Responses,
        query_params: None,
        http_headers: None,
        env_http_headers: None,
        request_max_retries: Some(0),
        stream_max_retries: Some(0),
        stream_idle_timeout_ms: Some(5_000),
        requires_openai_auth: false,
        supports_websockets: false,
    };

    let codex_home = TempDir::new().expect("failed to create TempDir");
    let mut config = load_default_config_for_test(&codex_home).await;
    config.model_provider_id = provider.name.clone();
    config.model_provider = provider.clone();
    let effort = config.model_reasoning_effort;
    let summary = config.model_reasoning_summary;
    let model = codex_core::test_support::get_model_offline(config.model.as_deref());
    config.model = Some(model.clone());
    let config = Arc::new(config);

    let conversation_id = ThreadId::new();
    let auth_mode = TelemetryAuthMode::Chatgpt;
    let session_source = SessionSource::SubAgent(SubAgentSource::Review);
    let model_info =
        codex_core::test_support::construct_model_info_offline(model.as_str(), &config);
    let otel_manager = OtelManager::new(
        conversation_id,
        model.as_str(),
        model_info.slug.as_str(),
        None,
        Some("test@test.com".to_string()),
        Some(auth_mode),
        "test_originator".to_string(),
        false,
        "test".to_string(),
        session_source.clone(),
    );

    let client = ModelClient::new(
        None,
        conversation_id,
        provider.clone(),
        session_source,
        config.model_verbosity,
        false,
        false,
        false,
        false,
        None,
    );
    let mut client_session = client.new_session();

    let mut prompt = Prompt::default();
    prompt.input = vec![ResponseItem::Message {
        id: None,
        role: "user".into(),
        content: vec![ContentItem::InputText {
            text: "hello".into(),
        }],
        end_turn: None,
        phase: None,
    }];

    let mut stream = client_session
        .stream(
            &prompt,
            &model_info,
            &otel_manager,
            effort,
            summary,
            None,
            None,
        )
        .await
        .expect("stream failed");
    while let Some(event) = stream.next().await {
        if matches!(event, Ok(ResponseEvent::Completed { .. })) {
            break;
        }
    }

    let request = request_recorder.single_request();
    assert_eq!(
        request.header("x-openai-subagent").as_deref(),
        Some("review")
    );
    assert_eq!(request.header("x-codex-sandbox"), None);
}

#[tokio::test]
async fn responses_stream_includes_subagent_header_on_other() {
    core_test_support::skip_if_no_network!();

    let server = responses::start_mock_server().await;
    let response_body = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_completed("resp-1"),
    ]);

    let request_recorder = responses::mount_sse_once_match(
        &server,
        header("x-openai-subagent", "my-task"),
        response_body,
    )
    .await;

    let provider = ModelProviderInfo {
        name: "mock".into(),
        base_url: Some(format!("{}/v1", server.uri())),
        env_key: None,
        env_key_instructions: None,
        experimental_bearer_token: None,
        wire_api: WireApi::Responses,
        query_params: None,
        http_headers: None,
        env_http_headers: None,
        request_max_retries: Some(0),
        stream_max_retries: Some(0),
        stream_idle_timeout_ms: Some(5_000),
        requires_openai_auth: false,
        supports_websockets: false,
    };

    let codex_home = TempDir::new().expect("failed to create TempDir");
    let mut config = load_default_config_for_test(&codex_home).await;
    config.model_provider_id = provider.name.clone();
    config.model_provider = provider.clone();
    let effort = config.model_reasoning_effort;
    let summary = config.model_reasoning_summary;
    let model = codex_core::test_support::get_model_offline(config.model.as_deref());
    config.model = Some(model.clone());
    let config = Arc::new(config);

    let conversation_id = ThreadId::new();
    let auth_mode = TelemetryAuthMode::Chatgpt;
    let session_source = SessionSource::SubAgent(SubAgentSource::Other("my-task".to_string()));
    let model_info =
        codex_core::test_support::construct_model_info_offline(model.as_str(), &config);

    let otel_manager = OtelManager::new(
        conversation_id,
        model.as_str(),
        model_info.slug.as_str(),
        None,
        Some("test@test.com".to_string()),
        Some(auth_mode),
        "test_originator".to_string(),
        false,
        "test".to_string(),
        session_source.clone(),
    );

    let client = ModelClient::new(
        None,
        conversation_id,
        provider.clone(),
        session_source,
        config.model_verbosity,
        false,
        false,
        false,
        false,
        None,
    );
    let mut client_session = client.new_session();

    let mut prompt = Prompt::default();
    prompt.input = vec![ResponseItem::Message {
        id: None,
        role: "user".into(),
        content: vec![ContentItem::InputText {
            text: "hello".into(),
        }],
        end_turn: None,
        phase: None,
    }];

    let mut stream = client_session
        .stream(
            &prompt,
            &model_info,
            &otel_manager,
            effort,
            summary,
            None,
            None,
        )
        .await
        .expect("stream failed");
    while let Some(event) = stream.next().await {
        if matches!(event, Ok(ResponseEvent::Completed { .. })) {
            break;
        }
    }

    let request = request_recorder.single_request();
    assert_eq!(
        request.header("x-openai-subagent").as_deref(),
        Some("my-task")
    );
}

#[tokio::test]
async fn responses_respects_model_info_overrides_from_config() {
    core_test_support::skip_if_no_network!();

    let server = responses::start_mock_server().await;
    let response_body = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_completed("resp-1"),
    ]);

    let request_recorder = responses::mount_sse_once(&server, response_body).await;

    let provider = ModelProviderInfo {
        name: "mock".into(),
        base_url: Some(format!("{}/v1", server.uri())),
        env_key: None,
        env_key_instructions: None,
        experimental_bearer_token: None,
        wire_api: WireApi::Responses,
        query_params: None,
        http_headers: None,
        env_http_headers: None,
        request_max_retries: Some(0),
        stream_max_retries: Some(0),
        stream_idle_timeout_ms: Some(5_000),
        requires_openai_auth: false,
        supports_websockets: false,
    };

    let codex_home = TempDir::new().expect("failed to create TempDir");
    let mut config = load_default_config_for_test(&codex_home).await;
    config.model = Some("gpt-3.5-turbo".to_string());
    config.model_provider_id = provider.name.clone();
    config.model_provider = provider.clone();
    config.model_supports_reasoning_summaries = Some(true);
    config.model_reasoning_summary = ReasoningSummary::Detailed;
    let effort = config.model_reasoning_effort;
    let summary = config.model_reasoning_summary;
    let model = config.model.clone().expect("model configured");
    let config = Arc::new(config);

    let conversation_id = ThreadId::new();
    let auth_mode =
        codex_core::test_support::auth_manager_from_auth(CodexAuth::from_api_key("Test API Key"))
            .auth_mode()
            .map(TelemetryAuthMode::from);
    let session_source =
        SessionSource::SubAgent(SubAgentSource::Other("override-check".to_string()));
    let model_info =
        codex_core::test_support::construct_model_info_offline(model.as_str(), &config);
    let otel_manager = OtelManager::new(
        conversation_id,
        model.as_str(),
        model_info.slug.as_str(),
        None,
        Some("test@test.com".to_string()),
        auth_mode,
        "test_originator".to_string(),
        false,
        "test".to_string(),
        session_source.clone(),
    );

    let client = ModelClient::new(
        None,
        conversation_id,
        provider.clone(),
        session_source,
        config.model_verbosity,
        false,
        false,
        false,
        false,
        None,
    );
    let mut client_session = client.new_session();

    let mut prompt = Prompt::default();
    prompt.input = vec![ResponseItem::Message {
        id: None,
        role: "user".into(),
        content: vec![ContentItem::InputText {
            text: "hello".into(),
        }],
        end_turn: None,
        phase: None,
    }];

    let mut stream = client_session
        .stream(
            &prompt,
            &model_info,
            &otel_manager,
            effort,
            summary,
            None,
            None,
        )
        .await
        .expect("stream failed");
    while let Some(event) = stream.next().await {
        if matches!(event, Ok(ResponseEvent::Completed { .. })) {
            break;
        }
    }

    let request = request_recorder.single_request();
    let body = request.body_json();
    let reasoning = body
        .get("reasoning")
        .and_then(|value| value.as_object())
        .cloned();

    assert!(
        reasoning.is_some(),
        "reasoning should be present when config enables summaries"
    );

    assert_eq!(
        reasoning
            .as_ref()
            .and_then(|value| value.get("summary"))
            .and_then(|value| value.as_str()),
        Some("detailed")
    );
}

#[tokio::test]
async fn responses_stream_includes_turn_metadata_header_for_git_workspace_e2e() {
    core_test_support::skip_if_no_network!();

    let server = responses::start_mock_server().await;
    let first_response = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_shell_command_call("call-1", "sleep 1"),
        responses::ev_completed("resp-1"),
    ]);
    let second_response = responses::sse(vec![
        responses::ev_response_created("resp-2"),
        responses::ev_assistant_message("msg-1", "done"),
        responses::ev_completed("resp-2"),
    ]);

    let test = test_codex().build(&server).await.expect("build test codex");
    let cwd = test.cwd_path();

    let git_config_global = cwd.join("empty-git-config");
    std::fs::write(&git_config_global, "").expect("write empty git config");
    let run_git = |args: &[&str]| {
        let output = Command::new("git")
            .env("GIT_CONFIG_GLOBAL", &git_config_global)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .args(args)
            .current_dir(cwd)
            .output()
            .expect("git command should run");
        assert!(
            output.status.success(),
            "git {:?} failed: stdout={} stderr={}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        output
    };

    run_git(&["init"]);
    run_git(&["config", "user.name", "Test User"]);
    run_git(&["config", "user.email", "test@example.com"]);
    std::fs::write(cwd.join("README.md"), "hello").expect("write README");
    run_git(&["add", "."]);
    run_git(&["commit", "-m", "initial commit"]);
    run_git(&[
        "remote",
        "add",
        "origin",
        "https://github.com/openai/codex.git",
    ]);

    let expected_head = String::from_utf8(run_git(&["rev-parse", "HEAD"]).stdout)
        .expect("git rev-parse output should be valid UTF-8")
        .trim()
        .to_string();
    let expected_origin = String::from_utf8(run_git(&["remote", "get-url", "origin"]).stdout)
        .expect("git remote get-url output should be valid UTF-8")
        .trim()
        .to_string();
    let expected_repo_root = std::fs::canonicalize(cwd)
        .unwrap_or_else(|_| cwd.to_path_buf())
        .to_string_lossy()
        .into_owned();

    let clean_turn_recorder = responses::mount_response_sequence(
        &server,
        vec![
            responses::sse_response(first_response.clone()),
            responses::sse_response(second_response.clone()),
        ],
    )
    .await;
    test.submit_turn("run a shell command")
        .await
        .expect("submit clean turn prompt");

    let clean_requests = clean_turn_recorder.requests();
    assert_eq!(clean_requests.len(), 2);
    let clean_turn_id_initial = clean_requests[0]
        .header("x-codex-turn-id")
        .expect("x-codex-turn-id should be present on initial request");
    let clean_turn_id_follow_up = clean_requests[1]
        .header("x-codex-turn-id")
        .expect("x-codex-turn-id should be present on follow-up request");
    assert_eq!(clean_turn_id_initial, clean_turn_id_follow_up);

    let clean_metadata_header = clean_requests[1]
        .header("x-codex-turn-metadata")
        .expect("follow-up request should include x-codex-turn-metadata");
    let clean_parsed: serde_json::Value = serde_json::from_str(&clean_metadata_header)
        .expect("x-codex-turn-metadata should be valid JSON");
    let clean_workspace = clean_parsed
        .get("workspaces")
        .and_then(serde_json::Value::as_object)
        .and_then(|workspaces| {
            workspaces
                .get(&expected_repo_root)
                .or_else(|| workspaces.values().next())
        })
        .expect("metadata should include expected repository root");
    assert_eq!(
        clean_workspace
            .get("latest_git_commit_hash")
            .and_then(serde_json::Value::as_str),
        Some(expected_head.as_str())
    );
    assert_eq!(
        clean_workspace
            .get("associated_remote_urls")
            .and_then(serde_json::Value::as_object)
            .and_then(|remotes| remotes.get("origin"))
            .and_then(serde_json::Value::as_str),
        Some(expected_origin.as_str())
    );
    assert_eq!(
        clean_workspace
            .get("has_changes")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );

    std::fs::write(cwd.join("untracked.txt"), "new file").expect("write untracked file");

    let dirty_turn_recorder = responses::mount_response_sequence(
        &server,
        vec![
            responses::sse_response(first_response),
            responses::sse_response(second_response),
        ],
    )
    .await;
    test.submit_turn("run a shell command")
        .await
        .expect("submit dirty turn prompt");

    let dirty_requests = dirty_turn_recorder.requests();
    assert_eq!(dirty_requests.len(), 2);
    let dirty_turn_id_initial = dirty_requests[0]
        .header("x-codex-turn-id")
        .expect("x-codex-turn-id should be present on initial dirty request");
    let dirty_turn_id_follow_up = dirty_requests[1]
        .header("x-codex-turn-id")
        .expect("x-codex-turn-id should be present on follow-up dirty request");
    assert_eq!(dirty_turn_id_initial, dirty_turn_id_follow_up);
    assert_ne!(clean_turn_id_initial, dirty_turn_id_initial);

    let dirty_metadata_header = dirty_requests[1]
        .header("x-codex-turn-metadata")
        .expect("dirty follow-up request should include x-codex-turn-metadata");
    let dirty_parsed: serde_json::Value = serde_json::from_str(&dirty_metadata_header)
        .expect("x-codex-turn-metadata should be valid JSON");
    let dirty_workspace = dirty_parsed
        .get("workspaces")
        .and_then(serde_json::Value::as_object)
        .and_then(|workspaces| {
            workspaces
                .get(&expected_repo_root)
                .or_else(|| workspaces.values().next())
        })
        .expect("metadata should include expected repository root");
    assert_eq!(
        dirty_workspace
            .get("has_changes")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
}
