//! Verify embedded TUI reasoning configuration and its outbound model request.

use super::*;
use crate::legacy_core::config::ConfigBuilder;
use codex_app_server_protocol::ServerNotification;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::json;

#[tokio::test]
async fn embedded_reasoning_defaults_reach_responses() -> Result<()> {
    for (settings, summary, stream_options) in [
        ("", json!(null), json!(null)),
        (
            "[features]\nconcurrent_reasoning_summaries = false",
            json!(null),
            json!(null),
        ),
        (
            "[features]\nconcurrent_reasoning_summaries = true",
            json!(null),
            json!(null),
        ),
        (
            "model_reasoning_summary = 'detailed'",
            json!("detailed"),
            json!(null),
        ),
        (
            "model_reasoning_summary = 'detailed'\n[features]\nconcurrent_reasoning_summaries = true",
            json!("detailed"),
            json!({"reasoning_summary_delivery": "sequential_cutoff"}),
        ),
        (
            "model_reasoning_summary = 'none'\n[features]\nconcurrent_reasoning_summaries = true",
            json!(null),
            json!(null),
        ),
    ] {
        let server = responses::start_mock_server().await;
        let response = responses::mount_sse_once(
            &server,
            responses::sse(vec![
                responses::ev_response_created("response"),
                responses::ev_completed("response"),
            ]),
        )
        .await;
        let home = tempfile::tempdir()?;
        let base_url = server.uri();
        std::fs::write(
            home.path().join("config.toml"),
            format!(
                r#"
model = "gpt-5.2"
model_provider = "reasoning-test"
{settings}
[model_providers.reasoning-test]
name = "OpenAI"
base_url = "{base_url}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#
            ),
        )?;
        let config = ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .build()
            .await?;
        let mut app_server = crate::start_embedded_app_server_for_picker(&config).await?;
        let started = app_server.start_thread(&config).await?;
        let request_id = app_server.next_request_id();
        let _: TurnStartResponse = app_server
            .client
            .request_typed(ClientRequest::TurnStart {
                request_id,
                params: TurnStartParams {
                    thread_id: started.session.thread_id.to_string(),
                    input: vec![UserInput::Text {
                        text: "hello".to_string(),
                        text_elements: Vec::new(),
                    }],
                    ..Default::default()
                },
            })
            .await?;
        tokio::time::timeout(std::time::Duration::from_secs(/*secs*/ 30), async {
            while let Some(event) = app_server.next_event().await {
                if let AppServerEvent::ServerNotification(notification) = event
                    && let ServerNotification::TurnCompleted(completed) = *notification
                {
                    assert_eq!(
                        completed.turn.status,
                        codex_app_server_protocol::TurnStatus::Completed
                    );
                    return;
                }
            }
            panic!("app-server disconnected before completing the turn");
        })
        .await?;
        let body = response.single_request().body_json();
        assert_eq!(
            json!({"summary": body["reasoning"]["summary"], "stream_options": body["stream_options"]}),
            json!({"summary": summary, "stream_options": stream_options}),
            "settings: {settings}"
        );
        app_server.shutdown().await?;
    }
    Ok(())
}

#[tokio::test]
async fn new_tui_threads_disable_summaries_unless_explicitly_enabled() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    for (config_text, expected_summary, expected_concurrent) in [
        ("", "none", false),
        (
            "[features]\nconcurrent_reasoning_summaries = true",
            "none",
            false,
        ),
        ("model_reasoning_summary = 'auto'", "auto", false),
        ("model_reasoning_summary = 'detailed'", "detailed", false),
        (
            "model_reasoning_summary = 'detailed'\n[features]\nconcurrent_reasoning_summaries = true",
            "detailed",
            true,
        ),
        ("model_reasoning_summary = 'none'", "none", false),
        (
            "model_reasoning_summary = 'none'\n[features]\nconcurrent_reasoning_summaries = true",
            "none",
            false,
        ),
        (
            "model_reasoning_summary = 'concise'\n[features]\nconcurrent_reasoning_summaries = false",
            "concise",
            false,
        ),
    ] {
        std::fs::write(temp_dir.path().join("config.toml"), config_text).expect("config");
        let config = ConfigBuilder::default()
            .codex_home(temp_dir.path().to_path_buf())
            .build()
            .await
            .expect("config should build");
        let start = thread_start_params_from_config(
            &config,
            ThreadParamsMode::Embedded,
            /*remote_cwd_override*/ None,
            /*session_start_source*/ None,
        );
        let overrides = start.config.expect("thread config");
        assert_eq!(
            overrides.get("model_reasoning_summary"),
            Some(&serde_json::json!(expected_summary)),
        );
        assert_eq!(
            overrides["features"]["concurrent_reasoning_summaries"],
            expected_concurrent,
        );
    }
    std::fs::write(
        temp_dir.path().join("config.toml"),
        "model_reasoning_summary = 'detailed'",
    )
    .expect("config");
    let config = ConfigBuilder::default()
        .codex_home(temp_dir.path().to_path_buf())
        .cli_overrides(vec![(
            "model_reasoning_summary".to_string(),
            toml::Value::String("none".to_string()),
        )])
        .build()
        .await
        .expect("override config");
    let start = thread_start_params_from_config(
        &config,
        ThreadParamsMode::Embedded,
        /*remote_cwd_override*/ None,
        /*session_start_source*/ None,
    );
    let overrides = start.config.expect("thread config");
    assert_eq!(overrides["model_reasoning_summary"], "none");
    assert_eq!(
        overrides["features"]["concurrent_reasoning_summaries"],
        false
    );
}
