use assert_cmd::Command as AssertCommand;
use assert_cmd::cargo::cargo_bin;
use codex_core::RolloutRecorder;
use codex_core::protocol::GitInfo;
use core_test_support::fs_wait;
use core_test_support::skip_if_no_network;
use std::time::Duration;
use tempfile::TempDir;
use uuid::Uuid;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

/// Tests streaming chat completions through the CLI using a mock server.
/// This test:
/// 1. Sets up a mock server that simulates OpenAI's chat completions API
/// 2. Configures codex to use this mock server via a custom provider
/// 3. Sends a simple "hello?" prompt and verifies the streamed response
/// 4. Ensures the response is received exactly once and contains "hi"
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn chat_mode_stream_cli() {
    skip_if_no_network!();

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
    let bin = cargo_bin("codex");
    let mut cmd = AssertCommand::new(bin);
    cmd.arg("exec")
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

    // Verify a new session rollout was created and is discoverable via list_conversations
    let provider_filter = vec!["mock".to_string()];
    let page = RolloutRecorder::list_conversations(
        home.path(),
        10,
        None,
        &[],
        Some(provider_filter.as_slice()),
        "mock",
    )
    .await
    .expect("list conversations");
    assert!(
        !page.items.is_empty(),
        "expected at least one session to be listed"
    );
    // First line of head must be the SessionMeta payload (id/timestamp)
    let head0 = page.items[0].head.first().expect("missing head record");
    assert!(head0.get("id").is_some(), "head[0] missing id");
    assert!(
        head0.get("timestamp").is_some(),
        "head[0] missing timestamp"
    );
}

/// Verify that passing `-c experimental_instructions_file=...` to the CLI
/// overrides the built-in base instructions by inspecting the request body
/// received by a mock OpenAI Responses endpoint.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_cli_applies_experimental_instructions_file() {
    skip_if_no_network!();

    // Start mock server which will capture the request and return a minimal
    // SSE stream for a single turn.
    let server = MockServer::start().await;
    let sse = concat!(
        "data: {\"type\":\"response.created\",\"response\":{}}\n\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"r1\"}}\n\n"
    );
    let resp_mock = core_test_support::responses::mount_sse_once(&server, sse.to_string()).await;

    // Create a temporary instructions file with a unique marker we can assert
    // appears in the outbound request payload.
    let custom = TempDir::new().unwrap();
    let marker = "cli-experimental-instructions-marker";
    let custom_path = custom.path().join("instr.md");
    std::fs::write(&custom_path, marker).unwrap();
    let custom_path_str = custom_path.to_string_lossy().replace('\\', "/");

    // Build a provider override that points at the mock server and instructs
    // Codex to use the Responses API with the dummy env var.
    let provider_override = format!(
        "model_providers.mock={{ name = \"mock\", base_url = \"{}/v1\", env_key = \"PATH\", wire_api = \"responses\" }}",
        server.uri()
    );

    let home = TempDir::new().unwrap();
    let bin = cargo_bin("codex");
    let mut cmd = AssertCommand::new(bin);
    cmd.arg("exec")
        .arg("--skip-git-repo-check")
        .arg("-c")
        .arg(&provider_override)
        .arg("-c")
        .arg("model_provider=\"mock\"")
        .arg("-c")
        .arg(format!(
            "experimental_instructions_file=\"{custom_path_str}\""
        ))
        .arg("-C")
        .arg(env!("CARGO_MANIFEST_DIR"))
        .arg("hello?\n");
    cmd.env("CODEX_HOME", home.path())
        .env("OPENAI_API_KEY", "dummy")
        .env("OPENAI_BASE_URL", format!("{}/v1", server.uri()));

    let output = cmd.output().unwrap();
    println!("Status: {}", output.status);
    println!("Stdout:\n{}", String::from_utf8_lossy(&output.stdout));
    println!("Stderr:\n{}", String::from_utf8_lossy(&output.stderr));
    assert!(output.status.success());

    // Inspect the captured request and verify our custom base instructions were
    // included in the `instructions` field.
    let request = resp_mock.single_request();
    let body = request.body_json();
    let instructions = body
        .get("instructions")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    assert!(
        instructions.contains(marker),
        "instructions did not contain custom marker; got: {instructions}"
    );
}

/// Tests streaming responses through the CLI using a local SSE fixture file.
/// This test:
/// 1. Uses a pre-recorded SSE response fixture instead of a live server
/// 2. Configures codex to read from this fixture via CODEX_RS_SSE_FIXTURE env var
/// 3. Sends a "hello?" prompt and verifies the response
/// 4. Ensures the fixture content is correctly streamed through the CLI
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_api_stream_cli() {
    skip_if_no_network!();

    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cli_responses_fixture.sse");

    let home = TempDir::new().unwrap();
    let bin = cargo_bin("codex");
    let mut cmd = AssertCommand::new(bin);
    cmd.arg("exec")
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

/// End-to-end: create a session (writes rollout), verify the file, then resume and confirm append.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn integration_creates_and_checks_session_file() -> anyhow::Result<()> {
    // Honor sandbox network restrictions for CI parity with the other tests.
    skip_if_no_network!(Ok(()));

    // 1. Temp home so we read/write isolated session files.
    let home = TempDir::new()?;

    // 2. Unique marker we'll look for in the session log.
    let marker = format!("integration-test-{}", Uuid::new_v4());
    let prompt = format!("echo {marker}");

    // 3. Use the same offline SSE fixture as responses_api_stream_cli so the test is hermetic.
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cli_responses_fixture.sse");

    // 4. Run the codex CLI and invoke `exec`, which is what records a session.
    let bin = cargo_bin("codex");
    let mut cmd = AssertCommand::new(bin);
    cmd.arg("exec")
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(env!("CARGO_MANIFEST_DIR"))
        .arg(&prompt);
    cmd.env("CODEX_HOME", home.path())
        .env("OPENAI_API_KEY", "dummy")
        .env("CODEX_RS_SSE_FIXTURE", &fixture)
        // Required for CLI arg parsing even though fixture short-circuits network usage.
        .env("OPENAI_BASE_URL", "http://unused.local");

    let output = cmd.output().unwrap();
    assert!(
        output.status.success(),
        "codex-cli exec failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Wait for sessions dir to appear.
    let sessions_dir = home.path().join("sessions");
    fs_wait::wait_for_path_exists(&sessions_dir, Duration::from_secs(5)).await?;

    // Find the session file that contains `marker`.
    let marker_clone = marker.clone();
    let path = fs_wait::wait_for_matching_file(&sessions_dir, Duration::from_secs(10), move |p| {
        if p.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
            return false;
        }
        let Ok(content) = std::fs::read_to_string(p) else {
            return false;
        };
        content.contains(&marker_clone)
    })
    .await?;

    // Basic sanity checks on location and metadata.
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
        std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("Failed to read session file"));
    let mut lines = content.lines();
    let meta_line = lines
        .next()
        .ok_or("missing session meta line")
        .unwrap_or_else(|_| panic!("missing session meta line"));
    let meta: serde_json::Value = serde_json::from_str(meta_line)
        .unwrap_or_else(|_| panic!("Failed to parse session meta line as JSON"));
    assert_eq!(
        meta.get("type").and_then(|v| v.as_str()),
        Some("session_meta")
    );
    let payload = meta
        .get("payload")
        .unwrap_or_else(|| panic!("Missing payload in meta line"));
    assert!(payload.get("id").is_some(), "SessionMeta missing id");
    assert!(
        payload.get("timestamp").is_some(),
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
        if item.get("type").and_then(|t| t.as_str()) == Some("response_item")
            && let Some(payload) = item.get("payload")
            && payload.get("type").and_then(|t| t.as_str()) == Some("message")
            && let Some(c) = payload.get("content")
            && c.to_string().contains(&marker)
        {
            found_message = true;
            break;
        }
    }
    assert!(
        found_message,
        "No message found in session file containing the marker"
    );

    // Second run: resume should update the existing file.
    let marker2 = format!("integration-resume-{}", Uuid::new_v4());
    let prompt2 = format!("echo {marker2}");
    let bin2 = cargo_bin("codex");
    let mut cmd2 = AssertCommand::new(bin2);
    cmd2.arg("exec")
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(env!("CARGO_MANIFEST_DIR"))
        .arg(&prompt2)
        .arg("resume")
        .arg("--last");
    cmd2.env("CODEX_HOME", home.path())
        .env("OPENAI_API_KEY", "dummy")
        .env("CODEX_RS_SSE_FIXTURE", &fixture)
        .env("OPENAI_BASE_URL", "http://unused.local");

    let output2 = cmd2.output().unwrap();
    assert!(output2.status.success(), "resume codex-cli run failed");

    // Find the new session file containing the resumed marker.
    let marker2_clone = marker2.clone();
    let resumed_path =
        fs_wait::wait_for_matching_file(&sessions_dir, Duration::from_secs(10), move |p| {
            if p.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
                return false;
            }
            std::fs::read_to_string(p)
                .map(|content| content.contains(&marker2_clone))
                .unwrap_or(false)
        })
        .await?;

    // Resume should write to the existing log file.
    assert_eq!(
        resumed_path, path,
        "resume should create a new session file"
    );

    let resumed_content = std::fs::read_to_string(&resumed_path)?;
    assert!(
        resumed_content.contains(&marker),
        "resumed file missing original marker"
    );
    assert!(
        resumed_content.contains(&marker2),
        "resumed file missing resumed marker"
    );
    Ok(())
}

/// Integration test to verify git info is collected and recorded in session files.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn integration_git_info_unit_test() {
    // This test verifies git info collection works independently
    // without depending on the full CLI integration

    // 1. Create temp directory for git repo
    let temp_dir = TempDir::new().unwrap();
    let git_repo = temp_dir.path().to_path_buf();
    let envs = vec![
        ("GIT_CONFIG_GLOBAL", "/dev/null"),
        ("GIT_CONFIG_NOSYSTEM", "1"),
    ];

    // 2. Initialize a git repository with some content
    let init_output = std::process::Command::new("git")
        .envs(envs.clone())
        .args(["init"])
        .current_dir(&git_repo)
        .output()
        .unwrap();
    assert!(init_output.status.success(), "git init failed");

    // Configure git user (required for commits)
    std::process::Command::new("git")
        .envs(envs.clone())
        .args(["config", "user.name", "Integration Test"])
        .current_dir(&git_repo)
        .output()
        .unwrap();

    std::process::Command::new("git")
        .envs(envs.clone())
        .args(["config", "user.email", "test@example.com"])
        .current_dir(&git_repo)
        .output()
        .unwrap();

    // Create a test file and commit it
    let test_file = git_repo.join("test.txt");
    std::fs::write(&test_file, "integration test content").unwrap();

    std::process::Command::new("git")
        .envs(envs.clone())
        .args(["add", "."])
        .current_dir(&git_repo)
        .output()
        .unwrap();

    let commit_output = std::process::Command::new("git")
        .envs(envs.clone())
        .args(["commit", "-m", "Integration test commit"])
        .current_dir(&git_repo)
        .output()
        .unwrap();
    assert!(commit_output.status.success(), "git commit failed");

    // Create a branch to test branch detection
    std::process::Command::new("git")
        .envs(envs.clone())
        .args(["checkout", "-b", "integration-test-branch"])
        .current_dir(&git_repo)
        .output()
        .unwrap();

    // Add a remote to test repository URL detection
    std::process::Command::new("git")
        .envs(envs.clone())
        .args([
            "remote",
            "add",
            "origin",
            "https://github.com/example/integration-test.git",
        ])
        .current_dir(&git_repo)
        .output()
        .unwrap();

    // 3. Test git info collection directly
    let git_info = codex_core::git_info::collect_git_info(&git_repo).await;

    // 4. Verify git info is present and contains expected data
    assert!(git_info.is_some(), "Git info should be collected");

    let git_info = git_info.unwrap();

    // Check that we have a commit hash
    assert!(
        git_info.commit_hash.is_some(),
        "Git info should contain commit_hash"
    );
    let commit_hash = git_info.commit_hash.as_ref().unwrap();
    assert_eq!(commit_hash.len(), 40, "Commit hash should be 40 characters");
    assert!(
        commit_hash.chars().all(|c| c.is_ascii_hexdigit()),
        "Commit hash should be hexadecimal"
    );

    // Check that we have the correct branch
    assert!(git_info.branch.is_some(), "Git info should contain branch");
    let branch = git_info.branch.as_ref().unwrap();
    assert_eq!(
        branch, "integration-test-branch",
        "Branch should match what we created"
    );

    // Check that we have the repository URL
    assert!(
        git_info.repository_url.is_some(),
        "Git info should contain repository_url"
    );
    let repo_url = git_info.repository_url.as_ref().unwrap();
    // Some hosts rewrite remotes (e.g., github.com → git@github.com), so assert against
    // the actual remote reported by git instead of a static URL.
    let expected_remote_url = std::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(&git_repo)
        .output()
        .unwrap();
    let expected_remote_url = String::from_utf8(expected_remote_url.stdout)
        .unwrap()
        .trim()
        .to_string();
    assert_eq!(
        repo_url, &expected_remote_url,
        "Repository URL should match git remote get-url output"
    );

    println!("✅ Git info collection test passed!");
    println!("   Commit: {commit_hash}");
    println!("   Branch: {branch}");
    println!("   Repo: {repo_url}");

    // 5. Test serialization to ensure it works in SessionMeta
    let serialized = serde_json::to_string(&git_info).unwrap();
    let deserialized: GitInfo = serde_json::from_str(&serialized).unwrap();

    assert_eq!(git_info.commit_hash, deserialized.commit_hash);
    assert_eq!(git_info.branch, deserialized.branch);
    assert_eq!(git_info.repository_url, deserialized.repository_url);

    println!("✅ Git info serialization test passed!");
}
