use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_exec_server::CODEX_EXEC_SERVER_NOISE_AUTH_TOKEN_ENV_VAR;
use codex_exec_server::CODEX_EXEC_SERVER_NOISE_ENVIRONMENT_ID_ENV_VAR;
use codex_exec_server::CODEX_EXEC_SERVER_NOISE_REGISTRY_URL_ENV_VAR;
use codex_exec_server::CODEX_EXEC_SERVER_URL_ENV_VAR;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::io::Lines;
use tokio::net::TcpListener;
use tokio::process::ChildStdin;
use tokio::process::ChildStdout;
use tokio::process::Command;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

use crate::relay_support::ENVIRONMENT_ID;
use crate::relay_support::EXECUTOR_REGISTRATION_ID;
use crate::relay_support::HARNESS_KEY_AUTHORIZATION;
use crate::relay_support::REGISTRY_TOKEN;
use crate::relay_support::accept_websocket;
use crate::relay_support::assert_relay_data_is_encrypted;
use crate::relay_support::proxy_relay_frames;
use crate::relay_support::registered_executor_public_key;

const RELEASED_CODEX_ENV_VAR: &str = "CODEX_TEST_RELEASED_CODEX";
const CURRENT_CODEX_ENV_VAR: &str = "CODEX_TEST_CURRENT_CODEX";
const EXECUTOR_MARKER_ENV_VAR: &str = "CODEX_EXECUTOR_VERSION_SKEW_MARKER";
const VERSION_SKEW_TIMEOUT: Duration = Duration::from_secs(30);
const EXPECTED_OUTPUT: &str = "executor-version-skew-ok";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn current_app_server_runs_commands_on_released_exec_server_over_noise() -> Result<()> {
    let Some((current, released)) = version_skew_binaries()? else {
        return Ok(());
    };
    assert_noise_version_skew(&current, &released).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn released_app_server_runs_commands_on_current_exec_server_over_noise() -> Result<()> {
    let Some((current, released)) = version_skew_binaries()? else {
        return Ok(());
    };
    assert_noise_version_skew(&released, &current).await
}

fn version_skew_binaries() -> Result<Option<(PathBuf, PathBuf)>> {
    let Some(released) = std::env::var_os(RELEASED_CODEX_ENV_VAR) else {
        return Ok(None);
    };
    let current = std::env::var_os(CURRENT_CODEX_ENV_VAR)
        .with_context(|| format!("{CURRENT_CODEX_ENV_VAR} must name the current Codex binary"))?;
    let current = PathBuf::from(current);
    let released = PathBuf::from(released);
    anyhow::ensure!(
        current.is_file(),
        "current Codex does not exist: {}",
        current.display()
    );
    anyhow::ensure!(
        released.is_file(),
        "released Codex does not exist: {}",
        released.display()
    );
    Ok(Some((current, released)))
}

async fn assert_noise_version_skew(app_binary: &Path, executor_binary: &Path) -> Result<()> {
    let codex_home = TempDir::new()?;
    let model = mock_model(codex_home.path()).await?;
    let model_url = model.uri();
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "danger-full-access"
model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider"
base_url = "{model_url}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#
        ),
    )?;

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let rendezvous_url = format!("ws://{}", listener.local_addr()?);
    let registry = MockServer::start().await;
    let registry_url = registry.uri();
    Mock::given(method("POST"))
        .and(path(format!(
            "/cloud/environment/{ENVIRONMENT_ID}/register"
        )))
        .and(header("authorization", format!("Bearer {REGISTRY_TOKEN}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "environment_id": ENVIRONMENT_ID,
            "url": format!("{rendezvous_url}/relay?role=environment"),
            "security_profile": "noise_hybrid_ik_v1",
            "executor_registration_id": EXECUTOR_REGISTRATION_ID,
        })))
        .expect(1)
        .mount(&registry)
        .await;
    Mock::given(method("POST"))
        .and(path(format!(
            "/cloud/environment/{ENVIRONMENT_ID}/validate"
        )))
        .and(header("authorization", format!("Bearer {REGISTRY_TOKEN}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"valid": true})))
        .expect(1)
        .mount(&registry)
        .await;

    let mut executor = Command::new(executor_binary)
        .args([
            "exec-server",
            "--remote",
            registry_url.as_str(),
            "--environment-id",
            ENVIRONMENT_ID,
        ])
        .current_dir(codex_home.path())
        .env("CODEX_HOME", codex_home.path())
        .env("CODEX_API_KEY", REGISTRY_TOKEN)
        .env(EXECUTOR_MARKER_ENV_VAR, EXPECTED_OUTPUT)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("start remote executor from {}", executor_binary.display()))?;

    let environment_websocket = accept_websocket(&listener, "environment").await?;
    let executor_public_key = registered_executor_public_key(&registry).await?;
    Mock::given(method("POST"))
        .and(path(format!("/cloud/environment/{ENVIRONMENT_ID}/connect")))
        .and(header("authorization", format!("Bearer {REGISTRY_TOKEN}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "environment_id": ENVIRONMENT_ID,
            "url": format!("{rendezvous_url}/relay?role=harness"),
            "security_profile": "noise_hybrid_ik_v1",
            "executor_registration_id": EXECUTOR_REGISTRATION_ID,
            "executor_public_key": executor_public_key,
            "harness_key_authorization": HARNESS_KEY_AUTHORIZATION,
        })))
        .expect(1)
        .mount(&registry)
        .await;

    let captured_frames = Arc::new(Mutex::new(Vec::new()));
    let captured_relay_frames = Arc::clone(&captured_frames);
    let relay = tokio::spawn(async move {
        let harness_websocket = accept_websocket(&listener, "harness").await?;
        proxy_relay_frames(
            environment_websocket,
            harness_websocket,
            captured_relay_frames,
        )
        .await
    });

    let mut app_server = Command::new(app_binary)
        .arg("app-server")
        .current_dir(codex_home.path())
        .env("CODEX_HOME", codex_home.path())
        .env("CODEX_API_KEY", REGISTRY_TOKEN)
        .env(
            "CODEX_APP_SERVER_MANAGED_CONFIG_PATH",
            codex_home.path().join("managed_config.toml"),
        )
        .env(CODEX_EXEC_SERVER_NOISE_REGISTRY_URL_ENV_VAR, &registry_url)
        .env(
            CODEX_EXEC_SERVER_NOISE_ENVIRONMENT_ID_ENV_VAR,
            ENVIRONMENT_ID,
        )
        .env(CODEX_EXEC_SERVER_NOISE_AUTH_TOKEN_ENV_VAR, REGISTRY_TOKEN)
        .env_remove(CODEX_EXEC_SERVER_URL_ENV_VAR)
        .env_remove(EXECUTOR_MARKER_ENV_VAR)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("start app-server from {}", app_binary.display()))?;
    let mut stdin = app_server.stdin.take().context("app-server stdin")?;
    let stdout = app_server.stdout.take().context("app-server stdout")?;
    let mut stdout = BufReader::new(stdout).lines();
    let mut notifications = Vec::new();

    app_server_request(
        &mut stdin,
        &mut stdout,
        &mut notifications,
        /*id*/ 1,
        "initialize",
        json!({
            "clientInfo": {"name": "noise-version-skew", "version": "0.1.0"},
            "capabilities": {"experimentalApi": true},
        }),
    )
    .await?;
    stdin.write_all(b"{\"method\":\"initialized\"}\n").await?;

    let thread = app_server_request(
        &mut stdin,
        &mut stdout,
        &mut notifications,
        /*id*/ 2,
        "thread/start",
        json!({"cwd": codex_home.path()}),
    )
    .await?;
    let thread_id = thread["thread"]["id"]
        .as_str()
        .context("thread/start should return a thread id")?;
    app_server_request(
        &mut stdin,
        &mut stdout,
        &mut notifications,
        /*id*/ 3,
        "turn/start",
        json!({
            "threadId": thread_id,
            "input": [{
                "type": "text",
                "text": "run the Noise compatibility command",
                "textElements": [],
            }],
        }),
    )
    .await?;

    while !notifications
        .iter()
        .any(|notification: &Value| notification["method"] == "turn/completed")
    {
        let line = timeout(VERSION_SKEW_TIMEOUT, stdout.next_line())
            .await
            .context("waiting for turn/completed")??
            .context("app-server exited before turn/completed")?;
        notifications.push(serde_json::from_str(&line)?);
    }

    assert_eq!(
        std::fs::read_to_string(codex_home.path().join("version-skew-output.txt"))?,
        EXPECTED_OUTPUT
    );
    assert_relay_data_is_encrypted(&captured_frames)?;
    registry.verify().await;
    model.verify().await;

    let _ = app_server.start_kill();
    let _ = executor.start_kill();
    relay.abort();
    Ok(())
}

async fn app_server_request(
    stdin: &mut ChildStdin,
    stdout: &mut Lines<BufReader<ChildStdout>>,
    notifications: &mut Vec<Value>,
    id: u64,
    method: &str,
    params: Value,
) -> Result<Value> {
    let request = json!({"id": id, "method": method, "params": params});
    stdin.write_all(request.to_string().as_bytes()).await?;
    stdin.write_all(b"\n").await?;

    loop {
        let line = timeout(VERSION_SKEW_TIMEOUT, stdout.next_line())
            .await
            .with_context(|| format!("waiting for {method} response"))??
            .with_context(|| format!("app-server exited before {method} response"))?;
        let response: Value = serde_json::from_str(&line)?;
        if response["id"] == id {
            anyhow::ensure!(
                response.get("error").is_none(),
                "{method} failed: {}",
                response["error"]
            );
            return response.get("result").cloned().context("missing result");
        }
        notifications.push(response);
    }
}

async fn mock_model(codex_home: &Path) -> Result<MockServer> {
    let server = MockServer::start().await;
    let arguments = serde_json::to_string(&json!({
        "cmd": format!("printf '%s' \"${EXECUTOR_MARKER_ENV_VAR}\" > version-skew-output.txt"),
        "workdir": codex_home,
        "yield_time_ms": 5_000,
    }))?;
    let completed = |id| {
        json!({
            "type": "response.completed",
            "response": {
                "id": id,
                "usage": {
                    "input_tokens": 0,
                    "input_tokens_details": null,
                    "output_tokens": 0,
                    "output_tokens_details": null,
                    "total_tokens": 0,
                },
            },
        })
    };
    let responses = vec![
        event_stream(vec![
            json!({"type": "response.created", "response": {"id": "response-1"}}),
            json!({
                "type": "response.output_item.done",
                "item": {
                    "type": "function_call",
                    "call_id": "noise-version-skew-command",
                    "name": "exec_command",
                    "arguments": arguments,
                },
            }),
            completed("response-1"),
        ])?,
        event_stream(vec![
            json!({"type": "response.created", "response": {"id": "response-2"}}),
            json!({
                "type": "response.output_item.done",
                "item": {
                    "type": "message",
                    "role": "assistant",
                    "id": "message-1",
                    "content": [{"type": "output_text", "text": "done"}],
                },
            }),
            completed("response-2"),
        ])?,
    ];
    for response in responses {
        Mock::given(method("POST"))
            .and(path("/v1/responses"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(response, "text/event-stream"))
            .up_to_n_times(1)
            .expect(1)
            .mount(&server)
            .await;
    }
    Ok(server)
}

fn event_stream(events: Vec<Value>) -> Result<String> {
    events
        .into_iter()
        .map(|event| {
            let event_type = event["type"].as_str().context("SSE event type")?;
            Ok(format!("event: {event_type}\ndata: {event}\n\n"))
        })
        .collect()
}
