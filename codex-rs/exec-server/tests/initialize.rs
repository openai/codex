mod common;

use anyhow::Context;
use codex_build_info::BuildInfo;
use codex_build_info::build_id;
use codex_exec_server::EnvironmentInfo;
use codex_exec_server::InitializeParams;
use codex_exec_server::InitializeResponse;
use codex_exec_server_protocol::JSONRPCError;
use codex_exec_server_protocol::JSONRPCErrorError;
use codex_exec_server_protocol::JSONRPCMessage;
use codex_exec_server_protocol::JSONRPCResponse;
use common::TEST_BUILD_COMMIT;
use common::exec_server::ExecServerHarness;
use common::exec_server::exec_server_with_env;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::process::Command;
use uuid::Uuid;

#[test_case::test_case(Some("1.2.3-alpha.4"); "packaged")]
#[test_case::test_case(None; "without_manifest")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_server_accepts_initialize(version: Option<&str>) -> anyhow::Result<()> {
    let package = TempDir::new()?;
    let bin_dir = package.path().join("bin");
    std::fs::create_dir(&bin_dir)?;
    let executable = bin_dir.join(format!("codex{}", std::env::consts::EXE_SUFFIX));
    std::fs::copy(std::env::current_exe()?, &executable)?;
    let manifest = package.path().join("codex-package.json");
    if let Some(version) = version {
        std::fs::write(
            &manifest,
            serde_json::to_vec(&serde_json::json!({ "version": version }))?,
        )?;
    }

    let mut command = Command::new(&executable);
    command.args(["exec-server", "--listen", "ws://127.0.0.1:0"]);
    // Runtime environment variables cannot replace the executable's build stamp.
    command.envs([
        (
            "STABLE_GIT_COMMIT",
            "ffffffffffffffffffffffffffffffffffffffff",
        ),
        ("GITHUB_SHA", "ffffffffffffffffffffffffffffffffffffffff"),
        ("CODEX_BUILD_TARGET", "runtime-override"),
    ]);
    let mut server = ExecServerHarness::start(command).await?;

    // Updates after startup cannot change the advertised release version.
    std::fs::write(&manifest, r#"{"version":"9.9.9"}"#)?;
    let initialize_id = server
        .send_request(
            "initialize",
            serde_json::to_value(InitializeParams {
                client_name: "exec-server-test".to_string(),
                resume_session_id: None,
            })?,
        )
        .await?;

    let response = server.next_event().await?;
    let JSONRPCMessage::Response(JSONRPCResponse { id, result }) = response else {
        panic!("expected initialize response");
    };
    assert_eq!(id, initialize_id);
    let initialize_response: InitializeResponse = serde_json::from_value(result)?;
    Uuid::parse_str(&initialize_response.session_id)?;
    let mut expected_environment = EnvironmentInfo::local();
    expected_environment.executor_version = version.unwrap_or("0.0.0").to_string();
    let build_info = BuildInfo::get();
    let target = build_info
        .target()
        .context("the test binary has a compiled target")?;
    expected_environment.provider_id = build_id(TEST_BUILD_COMMIT, target);
    assert!(expected_environment.provider_id.is_some());
    assert_eq!(
        initialize_response.environment_info,
        Some(expected_environment.clone())
    );

    server
        .send_notification("initialized", serde_json::json!({}))
        .await?;
    std::fs::remove_file(&manifest)?;
    let environment_id = server
        .send_request("environment/info", serde_json::json!({}))
        .await?;
    let JSONRPCMessage::Response(JSONRPCResponse { id, result }) = server.next_event().await?
    else {
        panic!("expected environment info response");
    };
    assert_eq!(id, environment_id);
    assert_eq!(
        serde_json::from_value::<EnvironmentInfo>(result)?,
        expected_environment
    );

    server.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn discovery_support_does_not_require_client_opt_in() -> anyhow::Result<()> {
    let codex_home = TempDir::new()?;
    #[cfg(not(windows))]
    let discovery_home = {
        // Fail the scan before startup, without racing the background task.
        std::fs::write(codex_home.path().join("plugins"), "not a directory")?;
        codex_home.path().to_path_buf()
    };
    // Windows treats the file-parent fixture as NotFound, which discovery skips.
    // A missing explicit home instead makes home resolution fail deterministically.
    #[cfg(windows)]
    let discovery_home = codex_home.path().join("missing");
    let user_home = codex_home.path().join("home");
    std::fs::create_dir_all(user_home.join(".agents/skills"))?;
    let stderr_path = codex_home.path().join("executor.log");
    let stderr = std::fs::File::create(&stderr_path)?;
    let helper_paths = common::exec_server::test_codex_helper_paths()?;
    let mut command = Command::new(helper_paths.codex_exe);
    command.args(["exec-server", "--listen", "ws://127.0.0.1:0"]);
    command.env("CODEX_HOME", discovery_home);
    command.env("HOME", &user_home);
    command.env("USERPROFILE", &user_home);
    command.env("RUST_LOG", "codex_exec_server=warn");
    let mut server = ExecServerHarness::start_with_stderr(command, stderr.into()).await?;
    let observed_failure = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let logs = tokio::fs::read_to_string(&stderr_path).await?;
            if logs.contains("capability location prewarming unavailable") {
                return Ok::<_, std::io::Error>(());
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await;
    anyhow::ensure!(
        observed_failure.is_ok(),
        "prewarm failure was not observed; executor stderr:\n{}",
        std::fs::read_to_string(&stderr_path)?
    );
    observed_failure??;
    let initialize_id = server
        .send_request(
            "initialize",
            serde_json::to_value(InitializeParams {
                client_name: "exec-server-test".to_string(),
                resume_session_id: None,
            })?,
        )
        .await?;

    let JSONRPCMessage::Response(JSONRPCResponse { id, result }) = server.next_event().await?
    else {
        anyhow::bail!("expected initialize response without V2 opt-in");
    };
    assert_eq!(id, initialize_id);
    let response: InitializeResponse = serde_json::from_value(result)?;
    assert!(
        response
            .environment_info
            .context("initialize metadata missing")?
            .capabilities
            .capability_discovery_v2
    );
    server
        .send_notification("initialized", serde_json::json!({}))
        .await?;

    let environment_info_id = server
        .send_request("environment/info", serde_json::json!({}))
        .await?;
    let JSONRPCMessage::Response(JSONRPCResponse { id, .. }) = server.next_event().await? else {
        panic!("expected environment info response after degraded discovery startup");
    };
    assert_eq!(id, environment_info_id);

    server.shutdown().await?;
    Ok(())
}

/// Requests retain their wire-order initialization errors even when later handshake messages are pipelined.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_server_rejects_pipelined_requests_before_initialized() -> anyhow::Result<()> {
    let mut server = exec_server_with_env(
        std::iter::empty::<(&str, &str)>(),
        &["--concurrent-requests", "32"],
    )
    .await?;
    let before_initialize_id = server
        .send_request("environment/info", serde_json::json!({}))
        .await?;
    let initialize_id = server
        .send_request(
            "initialize",
            serde_json::to_value(InitializeParams {
                client_name: "exec-server-test".to_string(),
                resume_session_id: None,
            })?,
        )
        .await?;

    assert_eq!(
        server.next_event().await?,
        JSONRPCMessage::Error(JSONRPCError {
            id: before_initialize_id,
            error: JSONRPCErrorError {
                code: -32600,
                data: None,
                message: "client must call initialize before using environment info methods"
                    .to_string(),
            },
        })
    );
    let JSONRPCMessage::Response(JSONRPCResponse { id, .. }) = server.next_event().await? else {
        panic!("expected initialize response");
    };
    assert_eq!(id, initialize_id);

    let before_initialized_id = server
        .send_request("environment/info", serde_json::json!({}))
        .await?;
    server
        .send_notification("initialized", serde_json::json!({}))
        .await?;
    assert_eq!(
        server.next_event().await?,
        JSONRPCMessage::Error(JSONRPCError {
            id: before_initialized_id,
            error: JSONRPCErrorError {
                code: -32600,
                data: None,
                message: "client must send initialized before using environment info methods"
                    .to_string(),
            },
        })
    );

    server.shutdown().await?;
    Ok(())
}
