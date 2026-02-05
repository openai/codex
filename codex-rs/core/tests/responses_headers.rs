use std::process::Command;
use std::sync::Arc;

use codex_app_server_protocol::AuthMode;
use codex_core::AuthManager;
use codex_core::CodexAuth;
use codex_core::ContentItem;
use codex_core::ModelClient;
use codex_core::ModelProviderInfo;
use codex_core::Prompt;
use codex_core::ResponseEvent;
use codex_core::ResponseItem;
use codex_core::WEB_SEARCH_ELIGIBLE_HEADER;
use codex_core::WireApi;
use codex_core::models_manager::manager::ModelsManager;
use codex_otel::OtelManager;
use codex_protocol::ThreadId;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::config_types::WebSearchMode;
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
    let model = ModelsManager::get_model_offline(config.model.as_deref());
    config.model = Some(model.clone());
    let config = Arc::new(config);

    let conversation_id = ThreadId::new();
    let auth_mode = AuthMode::Chatgpt;
    let session_source = SessionSource::SubAgent(SubAgentSource::Review);
    let model_info = ModelsManager::construct_model_info_offline(model.as_str(), &config);
    let otel_manager = OtelManager::new(
        conversation_id,
        model.as_str(),
        model_info.slug.as_str(),
        None,
        Some("test@test.com".to_string()),
        Some(auth_mode),
        false,
        "test".to_string(),
        session_source.clone(),
    );

    let web_search_eligible = !matches!(config.web_search_mode, Some(WebSearchMode::Disabled));
    let client = ModelClient::new(
        None,
        conversation_id,
        provider.clone(),
        session_source,
        config.model_verbosity,
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
            web_search_eligible,
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
    let model = ModelsManager::get_model_offline(config.model.as_deref());
    config.model = Some(model.clone());
    let config = Arc::new(config);

    let conversation_id = ThreadId::new();
    let auth_mode = AuthMode::Chatgpt;
    let session_source = SessionSource::SubAgent(SubAgentSource::Other("my-task".to_string()));
    let model_info = ModelsManager::construct_model_info_offline(model.as_str(), &config);

    let otel_manager = OtelManager::new(
        conversation_id,
        model.as_str(),
        model_info.slug.as_str(),
        None,
        Some("test@test.com".to_string()),
        Some(auth_mode),
        false,
        "test".to_string(),
        session_source.clone(),
    );

    let web_search_eligible = !matches!(config.web_search_mode, Some(WebSearchMode::Disabled));
    let client = ModelClient::new(
        None,
        conversation_id,
        provider.clone(),
        session_source,
        config.model_verbosity,
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
            web_search_eligible,
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
async fn responses_stream_includes_web_search_eligible_header_true_by_default() {
    core_test_support::skip_if_no_network!();

    let server = responses::start_mock_server().await;
    let response_body = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_completed("resp-1"),
    ]);

    let request_recorder = responses::mount_sse_once_match(
        &server,
        header(WEB_SEARCH_ELIGIBLE_HEADER, "true"),
        response_body,
    )
    .await;

    let test = test_codex().build(&server).await.expect("build test codex");
    test.submit_turn("hello").await.expect("submit test prompt");

    let request = request_recorder.single_request();
    assert_eq!(
        request.header(WEB_SEARCH_ELIGIBLE_HEADER).as_deref(),
        Some("true")
    );
}

#[tokio::test]
async fn responses_stream_includes_web_search_eligible_header_false_when_disabled() {
    core_test_support::skip_if_no_network!();

    let server = responses::start_mock_server().await;
    let response_body = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_completed("resp-1"),
    ]);

    let request_recorder = responses::mount_sse_once_match(
        &server,
        header(WEB_SEARCH_ELIGIBLE_HEADER, "false"),
        response_body,
    )
    .await;

    let test = test_codex()
        .with_config(|config| {
            config.web_search_mode = Some(WebSearchMode::Disabled);
        })
        .build(&server)
        .await
        .expect("build test codex");
    test.submit_turn("hello").await.expect("submit test prompt");

    let request = request_recorder.single_request();
    assert_eq!(
        request.header(WEB_SEARCH_ELIGIBLE_HEADER).as_deref(),
        Some("false")
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
        AuthManager::from_auth_for_testing(CodexAuth::from_api_key("Test API Key")).get_auth_mode();
    let session_source =
        SessionSource::SubAgent(SubAgentSource::Other("override-check".to_string()));
    let model_info = ModelsManager::construct_model_info_offline(model.as_str(), &config);
    let otel_manager = OtelManager::new(
        conversation_id,
        model.as_str(),
        model_info.slug.as_str(),
        None,
        Some("test@test.com".to_string()),
        auth_mode,
        false,
        "test".to_string(),
        session_source.clone(),
    );

    let web_search_eligible = !matches!(config.web_search_mode, Some(WebSearchMode::Disabled));
    let client = ModelClient::new(
        None,
        conversation_id,
        provider.clone(),
        session_source,
        config.model_verbosity,
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
            web_search_eligible,
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
    let response_body = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_completed("resp-1"),
    ]);

    let test = test_codex().build(&server).await.expect("build test codex");
    let cwd = test.cwd_path();

    let first_request = responses::mount_sse_once(&server, response_body.clone()).await;
    test.submit_turn("hello")
        .await
        .expect("submit first turn prompt");
    assert_eq!(
        first_request
            .single_request()
            .header("x-codex-turn-metadata"),
        None
    );

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

    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let request_recorder = responses::mount_sse_once(&server, response_body.clone()).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        test.submit_turn("hello")
            .await
            .expect("submit post-git turn prompt");

        let maybe_header = request_recorder
            .single_request()
            .header("x-codex-turn-metadata");
        if let Some(header_value) = maybe_header {
            let parsed: serde_json::Value = serde_json::from_str(&header_value)
                .expect("x-codex-turn-metadata should be valid JSON");
            let workspaces = parsed
                .get("workspaces")
                .and_then(serde_json::Value::as_object)
                .expect("metadata should include workspaces");
            let workspace = workspaces
                .values()
                .next()
                .expect("metadata should include at least one workspace entry");

            assert_eq!(
                workspace
                    .get("latest_git_commit_hash")
                    .and_then(serde_json::Value::as_str),
                Some(expected_head.as_str())
            );
            assert_eq!(
                workspace
                    .get("associated_remote_urls")
                    .and_then(serde_json::Value::as_object)
                    .and_then(|remotes| remotes.get("origin"))
                    .and_then(serde_json::Value::as_str),
                Some(expected_origin.as_str())
            );
            return;
        }

        if tokio::time::Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }

    panic!("x-codex-turn-metadata was never observed within 5s after git setup");
}
