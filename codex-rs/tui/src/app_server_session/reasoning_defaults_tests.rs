//! Verify TUI reasoning defaults and server-owned settings on outbound model requests.

use super::*;
use crate::legacy_core::config::ConfigBuilder;
use codex_app_server_protocol::ServerNotification;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::json;

#[tokio::test]
async fn reasoning_defaults_reach_responses() -> Result<()> {
    for (settings, summary, stream_options, mode) in [
        ("", json!(null), json!(null), ThreadParamsMode::Embedded),
        (
            "[features]\nconcurrent_reasoning_summaries = false",
            json!(null),
            json!(null),
            ThreadParamsMode::Embedded,
        ),
        (
            "[features]\nconcurrent_reasoning_summaries = true",
            json!(null),
            json!(null),
            ThreadParamsMode::Embedded,
        ),
        (
            "model_reasoning_summary = 'detailed'",
            json!("detailed"),
            json!(null),
            ThreadParamsMode::Embedded,
        ),
        (
            "model_reasoning_summary = 'detailed'\n[features]\nconcurrent_reasoning_summaries = true",
            json!("detailed"),
            json!({"reasoning_summary_delivery": "sequential_cutoff"}),
            ThreadParamsMode::Embedded,
        ),
        (
            "model_reasoning_summary = 'none'\n[features]\nconcurrent_reasoning_summaries = true",
            json!(null),
            json!(null),
            ThreadParamsMode::Embedded,
        ),
        (
            "model_reasoning_summary = 'detailed'\nmodel_verbosity = 'high'\nweb_search = 'live'",
            json!("detailed"),
            json!(null),
            ThreadParamsMode::Remote,
        ),
    ] {
        let server = responses::start_mock_server().await;
        let home = tempfile::tempdir()?;
        let base_url = server.uri();
        std::fs::write(
            home.path().join("config.toml"),
            format!(
                r#"
model = "gpt-5.5"
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
        crate::legacy_core::config::set_project_trust_level(
            home.path(),
            &std::env::current_dir()?,
            codex_protocol::config_types::TrustLevel::Trusted,
        )
        .map_err(|error| color_eyre::eyre::eyre!(error.to_string()))?;
        let server_config = ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .build()
            .await?;
        let client_home = tempfile::tempdir()?;
        let config = if mode == ThreadParamsMode::Remote {
            std::fs::write(
                client_home.path().join("config.toml"),
                "model_reasoning_summary = 'concise'\nmodel_verbosity = 'low'\nweb_search = 'disabled'",
            )?;
            ConfigBuilder::default()
                .codex_home(client_home.path().to_path_buf())
                .build()
                .await?
        } else {
            server_config.clone()
        };
        let mut app_server = crate::start_embedded_app_server_for_picker(&server_config).await?;
        app_server.thread_params_mode = mode;
        let started = app_server.start_thread(&config).await?;
        for resume in [false, true] {
            if resume {
                if mode != ThreadParamsMode::Remote {
                    break;
                }
                app_server.shutdown().await?;
                app_server = crate::start_embedded_app_server_for_picker(&server_config).await?;
                app_server.thread_params_mode = mode;
                app_server
                    .resume_thread(
                        &crate::local_settings::LocalSettings::from(&config),
                        config.clone(),
                        started.session.thread_id,
                        ResumeModelSettings::RestoreFromThread,
                    )
                    .await?;
            }
            let response = responses::mount_sse_once(
                &server,
                responses::sse(vec![
                    responses::ev_response_created("response"),
                    responses::ev_completed("response"),
                ]),
            )
            .await;
            let turn = app_server
                .turn_start(
                    started.session.thread_id,
                    "tui-user-message".to_string(),
                    vec![UserInput::Text {
                        text: "hello".to_string(),
                        text_elements: Vec::new(),
                    }],
                    config.cwd.to_path_buf(),
                    /*approval_policy*/ None,
                    /*approvals_reviewer*/ None,
                    TurnPermissionsOverride::Preserve,
                    &config.workspace_roots,
                    started.session.model.clone(),
                    /*effort*/ None,
                    /*summary*/ None,
                    /*service_tier*/ None,
                    /*collaboration_mode*/ None,
                    /*output_schema*/ None,
                    /*cyber_access_program*/ None,
                )
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
            if mode == ThreadParamsMode::Remote {
                assert_eq!(body["text"]["verbosity"], json!("high"));
                let web_search = body["tools"]
                    .as_array()
                    .expect("tools")
                    .iter()
                    .find(|tool| tool["type"] == "web_search")
                    .expect("server's web search tool");
                assert_eq!(web_search["external_web_access"], json!(true));
            }
            let metadata: serde_json::Value = serde_json::from_str(
                body["client_metadata"]["x-codex-turn-metadata"]
                    .as_str()
                    .expect("canonical turn metadata"),
            )?;
            assert_eq!(
                (
                    &metadata["thread_id"],
                    &metadata["turn_id"],
                    &metadata["turn_trigger"],
                ),
                (
                    &json!(started.session.thread_id),
                    &json!(turn.turn.id),
                    &json!("user"),
                )
            );
        }
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
