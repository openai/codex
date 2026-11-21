use codex_core::config::set_project_trust_level;
use codex_protocol::config_types::TrustLevel;
use core_test_support::responses;
use core_test_support::test_codex_exec::test_codex_exec;
use predicates::str::contains;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::fs;
use std::net::TcpListener;
use wiremock::Mock;
use wiremock::Request;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_partial_json;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn optional_http_mcp_failure_is_reported_once_without_endpoint_credentials()
-> anyhow::Result<()> {
    let test = test_codex_exec();
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    drop(listener);
    let mcp = responses::start_mock_server().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(json!({"method": "initialize"})))
        .respond_with(|request: &Request| {
            let body: Value = request.body_json().unwrap();
            ResponseTemplate::new(200).set_body_json(json!({
                "jsonrpc": "2.0",
                "id": body["id"],
                "result": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "serverInfo": {"name": "test", "version": "1"}
                }
            }))
        })
        .mount(&mcp)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(body_partial_json(
            json!({"method": "notifications/initialized"}),
        ))
        .respond_with(ResponseTemplate::new(307).insert_header(
            "location",
            format!("http://{address}/mcp?apiKey=TEST_SECRET"),
        ))
        .expect(1..)
        .mount(&mcp)
        .await;
    Mock::given(method("POST"))
        .and(path("/status"))
        .respond_with(
            ResponseTemplate::new(503).set_body_string("Auth required; upstream HTTP 404"),
        )
        .mount(&mcp)
        .await;
    fs::write(
        test.home_path().join("config.toml"),
        format!(
            "mcp_optional_startup_grace_ms = 0\n\n[mcp_servers.deadhttp]\nurl = 'http://{address}/mcp?apiKey=TEST_SECRET'\n\n[mcp_servers.notification]\nurl = '{0}/mcp?apiKey=TEST_SECRET'\n\n[mcp_servers.status]\nurl = '{0}/status'\n",
            mcp.uri(),
        ),
    )?;
    let server = responses::start_mock_server().await;
    responses::mount_sse_once(&server, responses::sse_completed("resp_1")).await;
    let result = test
        .cmd_with_server(&server)
        .env_remove("RUST_LOG")
        .args(["--ephemeral", "--skip-git-repo-check", "hello"])
        .assert()
        .success();
    let stderr = std::str::from_utf8(&result.get_output().stderr)?;
    let mut warnings = stderr
        .lines()
        .filter_map(|line| {
            line.split_once("codex_mcp_startup: ")
                .map(|(_, warning)| warning)
        })
        .collect::<Vec<_>>();
    warnings.sort_unstable();
    insta::assert_snapshot!(warnings.join("\n"), @r#"
    MCP server unavailable server_name="deadhttp" reason="HTTP request failed"
    MCP server unavailable server_name="notification" reason="initialization failed"
    MCP server unavailable server_name="status" reason="HTTP response error"
    "#);
    assert!(!stderr.contains("rmcp::transport::worker"));
    assert!(!stderr.contains("TEST_SECRET"));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn optional_http_mcp_pending_at_grace_is_reported() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    drop(listener);
    fs::write(
        test.home_path().join("config.toml"),
        format!(
            "mcp_optional_startup_grace_ms = 20\n[mcp_servers.deadhttp]\nurl = 'http://{address}/mcp'\n"
        ),
    )?;
    let server = responses::start_mock_server().await;
    responses::mount_sse_once(&server, responses::sse_completed("resp_1")).await;
    let result = test
        .cmd_with_server(&server)
        .env_remove("RUST_LOG")
        .args(["--ephemeral", "--skip-git-repo-check", "hello"])
        .assert()
        .success();
    let stderr = std::str::from_utf8(&result.get_output().stderr)?;
    let warnings = stderr
        .lines()
        .filter_map(|line| {
            line.split_once("codex_mcp_startup: ")
                .map(|(_, warning)| warning)
        })
        .collect::<Vec<_>>();
    insta::assert_snapshot!(warnings.join("\n"), @"MCP server unavailable server_name=\"deadhttp\" reason=\"startup still pending; omitted from this turn\"");
    assert!(!stderr.contains("rmcp::transport::worker"));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_rust_log_preserves_http_mcp_worker_diagnostics() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    drop(listener);
    fs::write(
        test.home_path().join("config.toml"),
        format!("[mcp_servers.deadhttp]\nurl = 'http://{address}/mcp'\n"),
    )?;
    let server = responses::start_mock_server().await;
    responses::mount_sse_once(&server, responses::sse_completed("resp_1")).await;
    test.cmd_with_server(&server)
        .env("RUST_LOG", "rmcp::transport::worker=trace")
        .args(["--ephemeral", "--skip-git-repo-check", "hello"])
        .assert()
        .success()
        .stderr(contains("worker quit with fatal:"));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exits_non_zero_when_required_mcp_server_fails_to_initialize() -> anyhow::Result<()> {
    let test = test_codex_exec();

    let config_toml = r#"
        [mcp_servers.required_broken]
        command = "codex-definitely-not-a-real-binary"
        required = true
    "#;
    let user_config = test.home_path().join("config.toml");
    fs::write(&user_config, config_toml)?;
    let project = test.cwd_path().join("repo");
    let cwd = project.join("subdir");
    fs::create_dir_all(&cwd)?;
    fs::create_dir(project.join(".codex"))?;
    fs::create_dir(project.join(".git"))?;
    fs::write(project.join(".git/HEAD"), "ref: refs/heads/main\n")?;

    let server = responses::start_mock_server().await;
    responses::mount_sse_once(&server, responses::sse_completed("resp_1")).await;
    let run = |args: &[&str]| {
        test.cmd_with_server(&server)
            .arg("--cd")
            .arg(&cwd)
            .args(["--ephemeral", "--experimental-json"])
            .args(args)
            .arg("hello")
            .assert()
    };
    let expect_mcp_failure = |args: &[&str]| {
        run(args).code(1).stderr(contains(
            "required MCP servers failed to initialize: required_broken",
        ));
    };

    expect_mcp_failure(&[]);
    // The same server in project config requires explicit consent, even with write access.
    fs::rename(&user_config, project.join(".codex/config.toml"))?;
    let first = run(&["--sandbox", "workspace-write"]).success();
    let stderr = std::str::from_utf8(&first.get_output().stderr)?;
    let warning = stderr
        .lines()
        .find(|line| line.contains("codex exec --trust"))
        .expect("project trust warning");
    insta::assert_snapshot!(warning, @"warning: Untrusted project-local configuration is disabled. Run `codex exec --trust` to allow project configuration and MCP servers for this and future chats in the project, including MCP commands outside the sandbox. Explicitly untrusted projects must have their trust setting updated first.");
    expect_mcp_failure(&["--sandbox", "read-only", "--trust"]);
    expect_mcp_failure(&[]);

    set_project_trust_level(test.home_path(), &project, TrustLevel::Untrusted)?;
    let refusal = fs::read_to_string(&user_config)?;
    run(&["--trust"])
        .failure()
        .stderr(contains("is marked as untrusted"));
    run(&["--ignore-user-config", "resume", "--last", "--trust"])
        .failure()
        .stderr(contains(
            "--trust cannot be combined with --ignore-user-config",
        ));
    assert_eq!(fs::read_to_string(user_config)?, refusal);
    Ok(())
}
