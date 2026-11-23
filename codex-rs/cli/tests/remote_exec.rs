//! Exercise remote exec overrides through the CLI, a real app server, and mock inference.
#![cfg(unix)]

#[path = "common/remote_exec.rs"]
mod remote_exec_support;

use remote_exec_support::RemoteExecFixture;

use anyhow::Context;
use anyhow::Result;
use codex_app_server_client::AppServerEvent;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadReadResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

#[tokio::test]
async fn remote_exec_forwards_model_and_effort_and_preserves_omitted_settings() -> Result<()> {
    // Keep the control socket below macOS's Unix socket path limit.
    let server_home = tempfile::tempdir_in("/tmp")?;
    let codex = codex_utils_cargo_bin::cargo_bin("codex")?;
    let mut catalog = codex_models_manager::bundled_models_response()?;
    let template = catalog
        .models
        .first()
        .context("empty bundled catalog")?
        .clone();
    catalog.models = ["gpt-5.1", "gpt-5.2"]
        .into_iter()
        .map(|slug| {
            let mut model = template.clone();
            model.slug = slug.to_string();
            model.model_messages = Some(codex_protocol::openai_models::ModelMessages {
                instructions_template: Some(format!("Base instructions for {slug}")),
                ..Default::default()
            });
            model
        })
        .collect();
    let catalog_path = server_home.path().join("models.json");
    std::fs::write(&catalog_path, serde_json::to_vec(&catalog)?)?;
    let catalog_config = format!(
        "model_catalog_json = {}",
        serde_json::to_string(&catalog_path)?
    );
    let (fixture, mut app, thread_id) = RemoteExecFixture::start(
        &codex,
        server_home.path(),
        "remote override completed",
        /*expected_requests*/ 7,
        |config| {
            config
                .with_model("gpt-5.1")
                .with_root_config(&catalog_config)
                .with_root_config("model_reasoning_effort = 'low'")
        },
        &[],
    )
    .await?;
    // Client config must not replace the server's credentials or thread settings.
    std::fs::write(
        fixture.caller_home.path().join("config.toml"),
        "model = 'client-model'\nmodel_reasoning_effort = 'minimal'\n",
    )?;
    // A new thread's rollout is persisted lazily on its first turn. Seed a
    // completed interactive turn before another client resumes the shared thread.
    let _: TurnStartResponse = app
        .request_typed(ClientRequest::TurnStart {
            request_id: RequestId::Integer(2),
            params: TurnStartParams {
                thread_id: thread_id.clone(),
                input: vec![UserInput::Text {
                    text: "seed the shared conversation".to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            },
        })
        .await?;
    timeout(Duration::from_secs(30), async {
        while let Some(event) = app.next_event().await {
            if let AppServerEvent::ServerNotification(notification) = event
                && let ServerNotification::TurnCompleted(_) = *notification
            {
                return anyhow::Ok(());
            }
        }
        anyhow::bail!("app server disconnected before the initial turn completed")
    })
    .await??;

    let cases: &[(&[&str], &str, &str)] = &[
        (&[], "gpt-5.1", "low"),
        (
            &["--model", "gpt-5.2", "-c", "model_reasoning_effort=high"],
            "gpt-5.2",
            "high",
        ),
        (&[], "gpt-5.2", "high"),
        (
            &["-c", "model_reasoning_effort=medium"],
            "gpt-5.2",
            "medium",
        ),
        (&["-c", "model='gpt-5.1'"], "gpt-5.1", "medium"),
        (
            &[
                "-c",
                "model='gpt-5.1'",
                "--model",
                "gpt-5.2",
                "-c",
                "model_reasoning_effort=low",
                "-c",
                "model_reasoning_effort='high'",
            ],
            "gpt-5.2",
            "high",
        ),
    ];
    for (index, &(overrides, expected_model, expected_effort)) in cases.iter().enumerate() {
        let mut command = fixture.caller_command();
        command.arg("exec");
        // Also cover creating a fresh thread, not just resuming a loaded one.
        let resume = index + 1 < cases.len();
        if resume {
            command.args(["resume", &thread_id]);
        }
        let output = timeout(
            Duration::from_secs(30),
            command
                .args(["--remote", &fixture.endpoint, "--json"])
                .args(overrides)
                .arg(format!("remote override case {index}"))
                .output(),
        )
        .await??;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let events = String::from_utf8(output.stdout)?
            .lines()
            .map(serde_json::from_str::<Value>)
            .collect::<serde_json::Result<Vec<_>>>()?;
        if resume {
            assert_eq!(events[0]["thread_id"], thread_id);
        } else {
            assert_ne!(events[0]["thread_id"], thread_id);
            let fresh: ThreadReadResponse = app
                .request_typed(ClientRequest::ThreadRead {
                    request_id: RequestId::Integer(4),
                    params: ThreadReadParams {
                        thread_id: events[0]["thread_id"]
                            .as_str()
                            .context("missing thread id")?
                            .to_string(),
                        include_turns: false,
                    },
                })
                .await?;
            let rollout =
                std::fs::read_to_string(fresh.thread.path.context("missing rollout path")?)?;
            let metadata: Value =
                serde_json::from_str(rollout.lines().next().context("empty rollout")?)?;
            assert_eq!(
                metadata["payload"]["base_instructions"]["text"],
                format!("Base instructions for {expected_model}")
            );
        }
        assert!(events.iter().any(|event| event["type"] == "turn.completed"));
        assert!(
            events
                .iter()
                .any(|event| event["item"]["text"] == "remote override completed")
        );
        let captured = fixture.requests.received_requests().await;
        assert_eq!(captured.len(), index + 2);
        let body: Value = captured
            .last()
            .context("missing inference request")?
            .body_json()?;
        assert_eq!(
            json!({"model": body["model"], "effort": body["reasoning"]["effort"]}),
            json!({"model": expected_model, "effort": expected_effort})
        );
    }
    let shared: ThreadReadResponse = app
        .request_typed(ClientRequest::ThreadRead {
            request_id: RequestId::Integer(3),
            params: ThreadReadParams {
                thread_id,
                include_turns: true,
            },
        })
        .await?;
    assert_eq!(shared.thread.turns.len(), 6);
    Ok(())
}

#[test]
fn remote_exec_rejects_invalid_or_unrelated_overrides_before_connecting() -> Result<()> {
    let home = TempDir::new()?;
    for (config, expected) in [
        ("model=42", "invalid model override"),
        (
            "model_reasoning_effort=42",
            "invalid model_reasoning_effort override",
        ),
        (
            "approval_policy='never'",
            "does not support config override `approval_policy`",
        ),
    ] {
        let output = std::process::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
            .env("CODEX_HOME", home.path())
            .args([
                "exec",
                "--remote",
                "ws://127.0.0.1:1",
                "-c",
                config,
                "hello",
            ])
            .output()?;
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}
