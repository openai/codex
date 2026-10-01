use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use std::time::Duration;

use codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID;
use codex_config::types::McpServerConfig;
use codex_config::types::McpServerTransportConfig;
use codex_protocol::protocol::Op;
use core_test_support::process::wait_for_pid_file;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::stdio_server_bin;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_mcp_server;

struct McpServerProcess {
    pid: String,
    // Retain the process object instead of reopening its PID after lifecycle actions.
    #[cfg(windows)]
    handle: std::os::windows::io::OwnedHandle,
}

impl McpServerProcess {
    fn observe_running(pid: String) -> anyhow::Result<Self> {
        let process = cfg_select! {
            unix => { Self { pid } }
            windows => {{
                use std::io;
                use std::num::NonZeroU32;
                use std::os::windows::io::FromRawHandle;
                use std::os::windows::io::OwnedHandle;

                use anyhow::Context;
                use windows_sys::Win32::System::Threading::OpenProcess;
                use windows_sys::Win32::System::Threading::PROCESS_SYNCHRONIZE;

                let process_id = pid
                    .parse::<NonZeroU32>()
                    .with_context(|| format!("invalid MCP server PID {pid}"))?;
                // SAFETY: The PID is nonzero, and the returned handle is checked before use.
                let handle = unsafe {
                    OpenProcess(
                        PROCESS_SYNCHRONIZE,
                        /*binherithandle*/ 0,
                        process_id.get(),
                    )
                };
                if handle.is_null() {
                    return Err(io::Error::last_os_error())
                        .with_context(|| format!("failed to open MCP server process {pid}"));
                }
                // SAFETY: OpenProcess returned a non-null owned process handle.
                let handle = unsafe { OwnedHandle::from_raw_handle(handle) };
                Self { pid, handle }
            }}
        };
        let alive = process.is_alive()?;
        anyhow::ensure!(alive, "MCP server process {} is not running", process.pid);
        Ok(process)
    }

    fn is_alive(&self) -> anyhow::Result<bool> {
        cfg_select! {
            unix => {{
                use core_test_support::process::process_is_alive;

                let Self { pid } = self;
                process_is_alive(pid)
            }}
            windows => {{
                use std::io;
                use std::os::windows::io::AsRawHandle;

                use anyhow::Context;
                use windows_sys::Win32::Foundation::WAIT_FAILED;
                use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
                use windows_sys::Win32::Foundation::WAIT_TIMEOUT;
                use windows_sys::Win32::System::Threading::WaitForSingleObject;

                let Self { pid, handle } = self;
                // SAFETY: The owned handle stays open for this nonblocking wait.
                let wait_result = unsafe {
                    WaitForSingleObject(handle.as_raw_handle(), /*dwmilliseconds*/ 0)
                };
                match wait_result {
                    WAIT_TIMEOUT => Ok(true),
                    WAIT_OBJECT_0 => Ok(false),
                    WAIT_FAILED => Err(io::Error::last_os_error())
                        .with_context(|| format!("failed to wait for MCP server process {pid}")),
                    result => anyhow::bail!("unexpected wait result {result} for MCP server process {pid}"),
                }
            }}
        }
    }

    async fn wait_for_exit(&self) -> anyhow::Result<()> {
        cfg_select! {
            unix => {{
                use core_test_support::process::wait_for_process_exit;

                let Self { pid } = self;
                wait_for_process_exit(pid).await
            }}
            windows => {{
                use anyhow::Context;

                tokio::time::timeout(Duration::from_secs(2), async {
                    loop {
                        let alive = self.is_alive()?;
                        if !alive {
                            return Ok(());
                        }
                        tokio::time::sleep(Duration::from_millis(25)).await;
                    }
                })
                .await
                .context("timed out waiting for process to exit")?
            }}
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn refresh_keeps_superseded_mcp_server_alive_for_in_flight_calls() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let temp_dir = tempfile::tempdir()?;
    let pid_file = temp_dir.path().join("mcp.pid");
    let pid_file_for_config = pid_file.clone();
    let command = stdio_server_bin()?;
    let fixture = test_codex()
        .with_config(move |config| {
            let mut servers = config.mcp_servers.get().clone();
            servers.insert(
                "refresh_cleanup".to_string(),
                McpServerConfig {
                    auth: Default::default(),
                    transport: McpServerTransportConfig::Stdio {
                        command,
                        args: Vec::new(),
                        env: Some(HashMap::from([(
                            "MCP_TEST_PID_FILE".to_string(),
                            pid_file_for_config.to_string_lossy().into_owned(),
                        )])),
                        env_vars: Vec::new(),
                        cwd: None,
                    },
                    environment_id: DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
                    enabled: true,
                    required: false,
                    startup_readiness: Default::default(),
                    supports_parallel_tool_calls: false,
                    tool_input_schema_max_bytes: None,
                    omit_tools_from: None,
                    disabled_reason: None,
                    startup_timeout_sec: Some(Duration::from_secs(10)),
                    tool_timeout_sec: None,
                    default_tools_approval_mode: None,
                    enabled_tools: None,
                    disabled_tools: None,
                    scopes: None,
                    oauth: None,
                    oauth_resource: None,
                    tools: HashMap::new(),
                },
            );
            config
                .mcp_servers
                .set(servers)
                .expect("test MCP servers should accept any configuration");
        })
        .build(&server)
        .await?;
    wait_for_mcp_server(&fixture.codex, "refresh_cleanup").await?;

    let superseded_pid = wait_for_pid_file(&pid_file).await?;
    let superseded = McpServerProcess::observe_running(superseded_pid)?;

    let barrier = serde_json::json!({
        "id": "mcp-refresh-cleanup",
        "participants": 2,
        "timeout_ms": 1_000
    });
    let long_call = tokio::spawn({
        let codex = Arc::clone(&fixture.codex);
        let barrier = barrier.clone();
        async move {
            codex
                .call_mcp_tool(
                    "refresh_cleanup",
                    "sync",
                    Some(serde_json::json!({
                        "barrier": barrier,
                        "sleep_after_ms": 300_000
                    })),
                    /*meta*/ None,
                )
                .await
        }
    });
    fixture
        .codex
        .call_mcp_tool(
            "refresh_cleanup",
            "sync",
            Some(serde_json::json!({ "barrier": barrier })),
            /*meta*/ None,
        )
        .await?;
    fs::remove_file(&pid_file)?;

    responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_assistant_message("msg-1", "done"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    fixture.codex.submit(Op::RefreshMcpServers).await?;
    fixture.submit_turn("refresh MCP servers").await?;

    let replacement_pid = wait_for_pid_file(&pid_file).await?;
    let replacement = McpServerProcess::observe_running(replacement_pid)?;
    assert_ne!(replacement.pid, superseded.pid);
    let superseded_alive = superseded.is_alive()?;
    assert!(superseded_alive);
    long_call.abort();
    assert!(
        long_call
            .await
            .expect_err("call should be aborted")
            .is_cancelled()
    );
    superseded.wait_for_exit().await?;
    let replacement_alive = replacement.is_alive()?;
    assert!(replacement_alive);

    fixture.codex.shutdown_and_wait().await?;
    replacement.wait_for_exit().await
}
