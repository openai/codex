use std::collections::HashMap;
use std::ffi::OsString;
#[cfg(windows)]
use std::os::windows::io::AsRawHandle;
#[cfg(windows)]
use std::os::windows::io::FromRawHandle;
#[cfg(windows)]
use std::os::windows::io::OwnedHandle;
use std::sync::Arc;
use std::time::Duration;

use codex_exec_server::Environment;
use codex_rmcp_client::ElicitationAction;
use codex_rmcp_client::ElicitationResponse;
use codex_rmcp_client::ExecutorStdioServerLauncher;
use codex_rmcp_client::LocalStdioServerLauncher;
use codex_rmcp_client::McpProtocolMode;
use codex_rmcp_client::RmcpClient;
use codex_rmcp_client::StdioServerLauncher;
use futures::FutureExt;
use rmcp::model::ClientCapabilities;
use rmcp::model::Implementation;
use rmcp::model::InitializeRequestParams;
use rmcp::model::ProtocolVersion;

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn OpenProcess(
        desired_access: u32,
        inherit_handle: i32,
        process_id: u32,
    ) -> *mut std::ffi::c_void;
    fn TerminateProcess(handle: *mut std::ffi::c_void, exit_code: u32) -> i32;
    fn WaitForSingleObject(handle: *mut std::ffi::c_void, milliseconds: u32) -> u32;
}

#[cfg(windows)]
fn open_process_for_wait(process_id: u32) -> std::io::Result<OwnedHandle> {
    let handle = unsafe {
        OpenProcess(
            /*desired_access*/ 0x0010_0001,
            /*inherit_handle*/ 0,
            process_id,
        )
    };
    if handle.is_null() {
        return Err(std::io::Error::last_os_error());
    }
    Ok(unsafe { OwnedHandle::from_raw_handle(handle.cast()) })
}

#[cfg(windows)]
fn wait_for_process_exit(process: &OwnedHandle) -> std::io::Result<()> {
    match unsafe {
        WaitForSingleObject(process.as_raw_handle().cast(), /*milliseconds*/ 5_000)
    } {
        0 => Ok(()),
        u32::MAX => Err(std::io::Error::last_os_error()),
        _ => Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "process did not exit",
        )),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stdio_message_limits_preserve_legacy_local_compatibility() -> anyhow::Result<()> {
    let server = codex_utils_cargo_bin::cargo_bin("test_stdio_server")?;

    for (executor, protocol_mode, accepts_oversized) in [
        (false, McpProtocolMode::Legacy, true),
        (false, McpProtocolMode::V20260728, false),
        (true, McpProtocolMode::Legacy, false),
    ] {
        let launcher: Arc<dyn StdioServerLauncher> = if executor {
            Arc::new(ExecutorStdioServerLauncher::new(
                Environment::default_for_tests().get_exec_backend(),
            ))
        } else {
            Arc::new(LocalStdioServerLauncher::new(std::env::current_dir()?))
        };
        let mut env = HashMap::from([(
            OsString::from("MCP_TEST_OVERSIZED_TOOL_DESCRIPTION"),
            OsString::from("1"),
        )]);
        if protocol_mode == McpProtocolMode::V20260728 {
            env.insert(
                OsString::from("CODEX_MCP_PROTOCOL_VERSION"),
                OsString::from("2026-07-28"),
            );
        }
        let client = RmcpClient::new_stdio_client_with_protocol_mode(
            server.clone().into(),
            Vec::new(),
            Some(env),
            &[],
            Some(std::env::current_dir()?.to_string_lossy().into_owned()),
            launcher,
            protocol_mode,
        )
        .await?;
        client
            .initialize(
                InitializeRequestParams::new(
                    ClientCapabilities::default(),
                    Implementation::new("stdio-limit-test", "1.0.0"),
                )
                .with_protocol_version(ProtocolVersion::V_2025_06_18),
                Some(Duration::from_secs(10)),
                Box::new(|_, _| {
                    async {
                        Ok(ElicitationResponse {
                            action: ElicitationAction::Decline,
                            content: None,
                            meta: None,
                        })
                    }
                    .boxed()
                }),
            )
            .await?;

        let result = client
            .list_tools(/*params*/ None, Some(Duration::from_secs(10)))
            .await;
        assert_eq!(
            result.is_ok(),
            accepts_oversized,
            "unexpected stdio size handling (executor={executor}, mode={protocol_mode:?})"
        );
        client.shutdown().await;
    }
    Ok(())
}

#[cfg(windows)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_stdio_shutdown_terminates_descendants_after_server_exit() -> anyhow::Result<()> {
    let server = codex_utils_cargo_bin::cargo_bin("test_stdio_server")?;

    for protocol_mode in [McpProtocolMode::Legacy, McpProtocolMode::V20260728] {
        let temp_dir = tempfile::tempdir()?;
        let server_pid_file = temp_dir.path().join("server.pid");
        let descendant_pid_file = temp_dir.path().join("descendant.pid");
        let breakaway_denied_file = temp_dir.path().join("breakaway.denied");
        let mut env = HashMap::from([
            (
                OsString::from("MCP_TEST_PID_FILE"),
                server_pid_file.clone().into(),
            ),
            (
                OsString::from("MCP_TEST_DESCENDANT_PID_FILE"),
                descendant_pid_file.clone().into(),
            ),
            (
                OsString::from("MCP_TEST_BREAKAWAY_DENIED_FILE"),
                breakaway_denied_file.clone().into(),
            ),
        ]);
        if protocol_mode == McpProtocolMode::V20260728 {
            env.insert(
                OsString::from("CODEX_MCP_PROTOCOL_VERSION"),
                OsString::from("2026-07-28"),
            );
        }

        let client = RmcpClient::new_stdio_client_with_protocol_mode(
            server.clone().into(),
            Vec::new(),
            Some(env),
            &[],
            Some(std::env::current_dir()?.to_string_lossy().into_owned()),
            Arc::new(LocalStdioServerLauncher::new(std::env::current_dir()?)),
            protocol_mode,
        )
        .await?;
        client
            .initialize(
                InitializeRequestParams::new(
                    ClientCapabilities::default(),
                    Implementation::new("stdio-cleanup-test", "1.0.0"),
                )
                .with_protocol_version(ProtocolVersion::V_2025_06_18),
                Some(Duration::from_secs(10)),
                Box::new(|_, _| {
                    async {
                        Ok(ElicitationResponse {
                            action: ElicitationAction::Decline,
                            content: None,
                            meta: None,
                        })
                    }
                    .boxed()
                }),
            )
            .await?;

        assert_eq!(
            std::fs::read_to_string(breakaway_denied_file)?,
            "denied",
            "MCP descendant escaped its Windows job (mode={protocol_mode:?})"
        );
        let server_pid = std::fs::read_to_string(server_pid_file)?
            .trim()
            .parse::<u32>()?;
        let descendant_pid = std::fs::read_to_string(descendant_pid_file)?
            .trim()
            .parse::<u32>()?;
        let server_process = open_process_for_wait(server_pid)?;
        let descendant_process = open_process_for_wait(descendant_pid)?;
        let terminated = unsafe {
            TerminateProcess(server_process.as_raw_handle().cast(), /*exit_code*/ 0)
        };
        assert_ne!(terminated, 0, "failed to terminate test MCP server");
        wait_for_process_exit(&server_process)?;

        client.shutdown().await;

        assert!(
            wait_for_process_exit(&descendant_process).is_ok(),
            "MCP descendant survived its exited server (mode={protocol_mode:?})"
        );
    }
    Ok(())
}
