use std::collections::HashMap;
use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::Utc;
use codex_app_server_protocol::AuthMode;
use codex_core::auth::AuthCredentialsStoreMode;
use codex_core::auth::AuthDotJson;
use codex_core::auth::save_auth;
use codex_core::token_data::TokenData;
use serde_json::Value as JsonValue;
use tempfile::tempdir;
use tokio::select;
use tokio::time::sleep;
use tokio::time::timeout;

fn write_chatgpt_auth(codex_home: &std::path::Path) -> Result<()> {
    let header = serde_json::json!({
        "alg": "none",
        "typ": "JWT",
    });
    let payload = serde_json::json!({
        "email": "user@example.com",
        "https://api.openai.com/auth": {
            "chatgpt_account_id": "workspace-1",
            "chatgpt_plan_type": "pro",
        },
    });
    let encode = |value: &serde_json::Value| -> Result<String> {
        Ok(URL_SAFE_NO_PAD.encode(serde_json::to_vec(value)?))
    };
    let id_token_raw = format!(
        "{}.{}.{}",
        encode(&header)?,
        encode(&payload)?,
        URL_SAFE_NO_PAD.encode(b"signature")
    );
    let auth = AuthDotJson {
        auth_mode: Some(AuthMode::Chatgpt),
        openai_api_key: None,
        tokens: Some(TokenData {
            id_token: codex_core::token_data::parse_chatgpt_jwt_claims(&id_token_raw)?,
            access_token: "chatgpt-token".to_string(),
            refresh_token: "refresh-token".to_string(),
            account_id: Some("workspace-1".to_string()),
        }),
        last_refresh: Some(Utc::now()),
    };
    save_auth(codex_home, &auth, AuthCredentialsStoreMode::File)?;
    Ok(())
}

fn spawn_rate_limits_server() -> Result<(String, std::thread::JoinHandle<()>)> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let addr = listener.local_addr()?;
    let server_url = format!("http://{addr}");
    let handle = std::thread::spawn(move || {
        let Ok((mut stream, _addr)) = listener.accept() else {
            return;
        };
        let mut buffer = [0_u8; 4096];
        let Ok(size) = stream.read(&mut buffer) else {
            return;
        };
        let request = String::from_utf8_lossy(&buffer[..size]);
        let response = if request.starts_with("GET /api/codex/usage ") {
            let body = serde_json::json!({
                "plan_type": "pro",
                "rate_limit": {
                    "allowed": true,
                    "limit_reached": false,
                    "primary_window": {
                        "used_percent": 42,
                        "limit_window_seconds": 3600,
                        "reset_after_seconds": 120,
                        "reset_at": 1735689720,
                    },
                },
            })
            .to_string();
            format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            )
        } else {
            let body = "not found";
            format!(
                "HTTP/1.1 404 Not Found\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            )
        };
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
    });
    Ok((server_url, handle))
}

#[tokio::test]
async fn resume_startup_does_not_consume_model_availability_nux_count() -> Result<()> {
    // run_codex_cli() does not work on Windows due to PTY limitations.
    if cfg!(windows) {
        return Ok(());
    }

    let repo_root = codex_utils_cargo_bin::repo_root()?;
    let codex_home = tempdir()?;
    write_chatgpt_auth(codex_home.path())?;
    let (chatgpt_base_url, rate_limits_server) = spawn_rate_limits_server()?;

    let source_catalog_path = codex_utils_cargo_bin::find_resource!("../core/models.json")?;
    let source_catalog = std::fs::read_to_string(&source_catalog_path)?;
    let mut source_catalog: JsonValue = serde_json::from_str(&source_catalog)?;
    let models = source_catalog
        .get_mut("models")
        .and_then(JsonValue::as_array_mut)
        .context("models array missing")?;
    for model in models.iter_mut() {
        if let Some(object) = model.as_object_mut() {
            object.remove("availability_nux");
        }
    }
    let first_model = models.first_mut().context("models array is empty")?;
    let first_model_object = first_model
        .as_object_mut()
        .context("first model was not a JSON object")?;
    let model_slug = first_model_object
        .get("slug")
        .and_then(JsonValue::as_str)
        .context("first model missing slug")?
        .to_string();
    first_model_object.insert(
        "availability_nux".to_string(),
        serde_json::json!({
            "message": "Model now available",
        }),
    );

    let custom_catalog_path = codex_home.path().join("catalog.json");
    std::fs::write(
        &custom_catalog_path,
        serde_json::to_string(&source_catalog)?,
    )?;

    let repo_root_display = repo_root.display();
    let catalog_display = custom_catalog_path.display();
    let config_contents = format!(
        r#"model = "{model_slug}"
model_provider = "openai"
model_catalog_json = "{catalog_display}"
chatgpt_base_url = "{chatgpt_base_url}"

[projects."{repo_root_display}"]
trust_level = "trusted"

[tui.model_availability_nux]
"{model_slug}" = 1
"#
    );
    std::fs::write(codex_home.path().join("config.toml"), config_contents)?;

    let fixture_path =
        codex_utils_cargo_bin::find_resource!("../core/tests/cli_responses_fixture.sse")?;
    let codex = if let Ok(path) = codex_utils_cargo_bin::cargo_bin("codex") {
        path
    } else {
        let fallback = repo_root.join("codex-rs/target/debug/codex");
        if fallback.is_file() {
            fallback
        } else {
            eprintln!("skipping integration test because codex binary is unavailable");
            return Ok(());
        }
    };

    let exec_output = std::process::Command::new(&codex)
        .arg("exec")
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(&repo_root)
        .arg("seed session for resume")
        .env("CODEX_HOME", codex_home.path())
        .env("CODEX_RS_SSE_FIXTURE", fixture_path)
        .env("OPENAI_BASE_URL", "http://unused.local")
        .output()
        .context("failed to execute codex exec")?;
    anyhow::ensure!(
        exec_output.status.success(),
        "codex exec failed: {}",
        String::from_utf8_lossy(&exec_output.stderr)
    );

    let mut env = HashMap::new();
    env.insert(
        "CODEX_HOME".to_string(),
        codex_home.path().display().to_string(),
    );

    let args = vec![
        "resume".to_string(),
        "--last".to_string(),
        "--no-alt-screen".to_string(),
        "--enable".to_string(),
        "tui_app_server".to_string(),
        "-C".to_string(),
        repo_root.display().to_string(),
        "-c".to_string(),
        "analytics.enabled=false".to_string(),
    ];

    let spawned = codex_utils_pty::spawn_pty_process(
        codex.to_string_lossy().as_ref(),
        &args,
        &repo_root,
        &env,
        &None,
        codex_utils_pty::TerminalSize::default(),
    )
    .await?;

    let mut output = Vec::new();
    let codex_utils_pty::SpawnedProcess {
        session,
        stdout_rx,
        stderr_rx,
        exit_rx,
    } = spawned;
    let mut output_rx = codex_utils_pty::combine_output_receivers(stdout_rx, stderr_rx);
    let mut exit_rx = exit_rx;
    let writer_tx = session.writer_sender();
    let interrupt_writer = writer_tx.clone();
    let mut startup_ready = false;
    let mut answered_cursor_query = false;

    let exit_code_result = timeout(Duration::from_secs(15), async {
        loop {
            select! {
                result = output_rx.recv() => match result {
                    Ok(chunk) => {
                        let has_cursor_query = chunk.windows(4).any(|window| window == b"\x1b[6n");
                        if has_cursor_query {
                            let _ = writer_tx.send(b"\x1b[1;1R".to_vec()).await;
                            answered_cursor_query = true;
                        }
                        output.extend_from_slice(&chunk);
                        if !startup_ready && answered_cursor_query && !has_cursor_query {
                            startup_ready = true;
                            for _ in 0..4 {
                                let _ = interrupt_writer.send(vec![3]).await;
                                sleep(Duration::from_millis(500)).await;
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break exit_rx.await,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                },
                result = &mut exit_rx => break result,
            }
        }
    })
    .await;

    let exit_code = match exit_code_result {
        Ok(Ok(code)) => code,
        Ok(Err(err)) => return Err(err.into()),
        Err(_) => {
            session.terminate();
            anyhow::bail!("timed out waiting for codex resume to exit");
        }
    };
    let output_text = String::from_utf8_lossy(&output);
    let interrupted_startup = exit_code == 1 && output_text.trim() == "^C";
    anyhow::ensure!(
        exit_code == 0 || exit_code == 130 || interrupted_startup,
        "unexpected exit code from codex resume: {exit_code}; output: {output_text}",
    );

    let config_contents = std::fs::read_to_string(codex_home.path().join("config.toml"))?;
    let config: toml::Value = toml::from_str(&config_contents)?;
    let shown_count = config
        .get("tui")
        .and_then(|tui| tui.get("model_availability_nux"))
        .and_then(|nux| nux.get(&model_slug))
        .and_then(toml::Value::as_integer)
        .context("missing tui.model_availability_nux count")?;

    assert_eq!(shown_count, 1);
    rate_limits_server
        .join()
        .map_err(|_| anyhow::anyhow!("rate limit server thread panicked"))?;

    Ok(())
}
