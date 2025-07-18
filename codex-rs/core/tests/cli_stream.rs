#![expect(clippy::unwrap_used)]

use assert_cmd::Command as AssertCommand;
use codex_core::exec::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use std::time::Duration;
use std::time::Instant;
use tempfile::TempDir;
use uuid::Uuid;
use walkdir::WalkDir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

/// Smoke-test streaming chat completions through the CLI using a mock server.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn chat_mode_stream_cli() {
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    let server = MockServer::start().await;
    let sse = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{}}]}\n\n",
        "data: [DONE]\n\n"
    );
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_raw(sse, "text/event-stream"),
        )
        .expect(1)
        .mount(&server)
        .await;

    let home = TempDir::new().unwrap();
    let provider_override = format!(
        "model_providers.mock={{ name = \"mock\", base_url = \"{}/v1\", env_key = \"PATH\", wire_api = \"chat\" }}",
        server.uri()
    );
    let mut cmd = AssertCommand::new("cargo");
    cmd.arg("run")
        .arg("-p")
        .arg("codex-cli")
        .arg("--quiet")
        .arg("--")
        .arg("exec")
        .arg("--skip-git-repo-check")
        .arg("-c")
        .arg(&provider_override)
        .arg("-c")
        .arg("model_provider=\"mock\"")
        .arg("-C")
        .arg(env!("CARGO_MANIFEST_DIR"))
        .arg("hello?");
    cmd.env("CODEX_HOME", home.path())
        .env("OPENAI_API_KEY", "dummy")
        .env("OPENAI_BASE_URL", format!("{}/v1", server.uri()));

    let output = cmd.output().unwrap();
    println!("Status: {}", output.status);
    println!("Stdout:\n{}", String::from_utf8_lossy(&output.stdout));
    println!("Stderr:\n{}", String::from_utf8_lossy(&output.stderr));
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let hi_lines = stdout.lines().filter(|line| line.trim() == "hi").count();
    assert_eq!(hi_lines, 1, "Expected exactly one line with 'hi'");

    server.verify().await;
}

/// Smoke-test streaming Responses API through the CLI using a local SSE fixture.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_api_stream_cli() {
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cli_responses_fixture.sse");

    let home = TempDir::new().unwrap();
    let mut cmd = AssertCommand::new("cargo");
    cmd.arg("run")
        .arg("-p")
        .arg("codex-cli")
        .arg("--quiet")
        .arg("--")
        .arg("exec")
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(env!("CARGO_MANIFEST_DIR"))
        .arg("hello?");
    cmd.env("CODEX_HOME", home.path())
        .env("OPENAI_API_KEY", "dummy")
        .env("CODEX_RS_SSE_FIXTURE", fixture)
        .env("OPENAI_BASE_URL", "http://unused.local");

    let output = cmd.output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("fixture hello"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn integration_creates_and_checks_session_file() {
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    let home = TempDir::new().unwrap();
    let marker = format!("integration-test-{}", Uuid::new_v4());
    let prompt = format!("echo {marker}");
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cli_responses_fixture.sse");

    let mut cmd = AssertCommand::new("cargo");
    cmd.arg("run")
        .arg("-p")
        .arg("codex-cli")
        .arg("--quiet")
        .arg("--")
        .arg("exec")
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(env!("CARGO_MANIFEST_DIR"))
        .arg(&prompt);
    cmd.env("CODEX_HOME", home.path())
        .env("OPENAI_API_KEY", "dummy")
        .env("CODEX_RS_SSE_FIXTURE", &fixture)
        .env("OPENAI_BASE_URL", "http://unused.local");

    let output = cmd.output().unwrap();
    assert!(
        output.status.success(),
        "codex-cli exec failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let sessions_dir = home.path().join("sessions");
    let dir_deadline = Instant::now() + Duration::from_secs(5);
    while !sessions_dir.exists() && Instant::now() < dir_deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(sessions_dir.exists(), "sessions directory never appeared");

    let deadline = Instant::now() + Duration::from_secs(10);
    let mut matching_path: Option<std::path::PathBuf> = None;

    while Instant::now() < deadline && matching_path.is_none() {
        for entry in WalkDir::new(&sessions_dir) {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            if !entry.file_type().is_file() {
                continue;
            }
            if !entry.file_name().to_string_lossy().ends_with(".jsonl") {
                continue;
            }
            let path = entry.path();
            let Ok(content) = std::fs::read_to_string(path) else {
                continue;
            };
            let mut lines = content.lines();
            if lines.next().is_none() {
                continue;
            }
            for line in lines {
                if line.trim().is_empty() {
                    continue;
                }
                let item: serde_json::Value = match serde_json::from_str(line) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                if item.get("type").and_then(|t| t.as_str()) == Some("message") {
                    if let Some(c) = item.get("content") {
                        if c.to_string().contains(&marker) {
                            if matching_path.as_ref().map(|p| p != path).unwrap_or(false) {
                                panic!(
                                    "Found marker in multiple session files: {:?} and {:?}",
                                    matching_path.unwrap(),
                                    path
                                );
                            }
                            matching_path = Some(path.to_path_buf());
                            break;
                        }
                    }
                }
            }
        }
        if matching_path.is_none() {
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    let path = match matching_path {
        Some(p) => p,
        None => panic!("No session file containing the marker was found"),
    };

    let rel = match path.strip_prefix(&sessions_dir) {
        Ok(r) => r,
        Err(_) => panic!("session file should live under sessions/"),
    };
    let comps: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        comps.len(),
        4,
        "Expected sessions/YYYY/MM/DD/<file>, got {rel:?}"
    );
    let year = &comps[0];
    let month = &comps[1];
    let day = &comps[2];
    assert!(
        year.len() == 4 && year.chars().all(|c| c.is_ascii_digit()),
        "Year dir not 4-digit numeric: {year}"
    );
    assert!(
        month.len() == 2 && month.chars().all(|c| c.is_ascii_digit()),
        "Month dir not zero-padded 2-digit numeric: {month}"
    );
    assert!(
        day.len() == 2 && day.chars().all(|c| c.is_ascii_digit()),
        "Day dir not zero-padded 2-digit numeric: {day}"
    );
    if let Ok(m) = month.parse::<u8>() {
        assert!((1..=12).contains(&m), "Month out of range: {m}");
    }
    if let Ok(d) = day.parse::<u8>() {
        assert!((1..=31).contains(&d), "Day out of range: {d}");
    }

    let content =
        std::fs::read_to_string(path).unwrap_or_else(|_| panic!("Failed to read session file"));
    let mut lines = content.lines();
    let meta_line = lines
        .next()
        .ok_or("missing session meta line")
        .unwrap_or_else(|_| panic!("missing session meta line"));
    let meta: serde_json::Value = serde_json::from_str(meta_line)
        .unwrap_or_else(|_| panic!("Failed to parse session meta line as JSON"));
    assert!(meta.get("id").is_some(), "SessionMeta missing id");
    assert!(
        meta.get("timestamp").is_some(),
        "SessionMeta missing timestamp"
    );

    let mut found_message = false;
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(item) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if item.get("type").and_then(|t| t.as_str()) == Some("message") {
            if let Some(c) = item.get("content") {
                if c.to_string().contains(&marker) {
                    found_message = true;
                    break;
                }
            }
        }
    }
    assert!(
        found_message,
        "No message found in session file containing the marker"
    );
}
