use codex_protocol::models::PermissionProfile;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

pub struct ElevatedSandboxProfileCaptureRequest<'a> {
    pub permission_profile: &'a PermissionProfile,
    pub workspace_roots: &'a [AbsolutePathBuf],
    pub codex_home: &'a Path,
    pub command: Vec<String>,
    pub cwd: &'a Path,
    pub env_map: HashMap<String, String>,
    pub timeout_ms: Option<u64>,
    pub cancellation: Option<crate::WindowsSandboxCancellationToken>,
    pub use_private_desktop: bool,
    pub proxy_enforced: bool,
    pub read_roots_override: Option<&'a [PathBuf]>,
    pub read_roots_include_platform_defaults: bool,
    pub write_roots_override: Option<&'a [PathBuf]>,
    pub deny_read_paths_override: &'a [AbsolutePathBuf],
    pub deny_write_paths_override: &'a [AbsolutePathBuf],
}

mod windows_impl {
    use super::ElevatedSandboxProfileCaptureRequest;
    use crate::identity::refresh_logon_sandbox_creds;
    use crate::ipc_framed::EmptyPayload;
    use crate::ipc_framed::FramedMessage;
    use crate::ipc_framed::Message;
    use crate::ipc_framed::OutputStream;
    use crate::ipc_framed::SpawnRequest;
    use crate::ipc_framed::decode_bytes;
    use crate::ipc_framed::read_frame;
    use crate::ipc_framed::write_frame;
    use crate::logging::log_failure;
    use crate::logging::log_success;
    use crate::process::WindowsProcessLaunch;
    use crate::resolved_permissions::ResolvedWindowsSandboxPermissions;
    use crate::runner_client::retry_runner_spawn_once;
    use crate::runner_client::spawn_runner_transport;
    use crate::spawn_prep::prepare_elevated_spawn_context_for_permissions;
    use anyhow::Result;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use std::fs::File;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    pub use crate::windows_impl::CaptureResult;

    /// Polls for cancellation and sends the runner's terminate IPC frame when requested.
    ///
    /// The 50 ms park bounds cancellation latency without busy-waiting.
    fn spawn_cancel_writer(
        pipe_write: &File,
        cancellation: Option<crate::WindowsSandboxCancellationToken>,
    ) -> Result<Option<(std::thread::JoinHandle<()>, Arc<AtomicBool>)>> {
        let Some(cancellation) = cancellation else {
            return Ok(None);
        };
        let mut pipe_write = pipe_write.try_clone()?;
        let done = Arc::new(AtomicBool::new(false));
        let done_for_thread = Arc::clone(&done);
        let handle = std::thread::spawn(move || {
            while !done_for_thread.load(Ordering::SeqCst) {
                if cancellation.is_cancelled() {
                    let _ = write_frame(
                        &mut pipe_write,
                        &FramedMessage {
                            version: 1,
                            message: Message::Terminate {
                                payload: EmptyPayload::default(),
                            },
                        },
                    );
                    break;
                }
                std::thread::park_timeout(Duration::from_millis(50));
            }
        });
        Ok(Some((handle, done)))
    }

    /// Launches the command runner under the sandbox user and captures its output.
    #[allow(clippy::too_many_arguments)]
    pub fn run_windows_sandbox_capture_for_permission_profile(
        request: ElevatedSandboxProfileCaptureRequest<'_>,
    ) -> Result<CaptureResult> {
        let ElevatedSandboxProfileCaptureRequest {
            permission_profile,
            workspace_roots,
            codex_home,
            command,
            cwd,
            mut env_map,
            timeout_ms,
            cancellation,
            use_private_desktop,
            proxy_enforced,
            read_roots_override,
            read_roots_include_platform_defaults,
            write_roots_override,
            deny_read_paths_override,
            deny_write_paths_override,
        } = request;
        let permissions =
            ResolvedWindowsSandboxPermissions::try_from_permission_profile_for_workspace_roots(
                permission_profile,
                workspace_roots,
            )?;
        let deny_read_paths_override = deny_read_paths_override
            .iter()
            .map(AbsolutePathBuf::to_path_buf)
            .collect::<Vec<_>>();
        let deny_write_paths_override = deny_write_paths_override
            .iter()
            .map(AbsolutePathBuf::to_path_buf)
            .collect::<Vec<_>>();
        let elevated = prepare_elevated_spawn_context_for_permissions(
            permissions.clone(),
            codex_home,
            cwd,
            &mut env_map,
            WindowsProcessLaunch {
                application_path: None,
                command: command.clone(),
            },
            read_roots_override,
            read_roots_include_platform_defaults,
            write_roots_override,
            &deny_read_paths_override,
            &deny_write_paths_override,
            proxy_enforced,
            crate::WindowsSandboxProxySettingsMode::Reconcile,
        )?;
        let sandbox_creds = elevated.sandbox_creds;
        let sandbox_base = elevated.sandbox_base;
        let logs_base_dir = elevated.logs_base_dir;
        let cap_sids = elevated.cap_sids;
        let launch = elevated.launch;
        let resolved_read_roots = elevated.read_roots_override;
        let logs_base_dir = logs_base_dir.as_deref();

        (|| -> Result<CaptureResult> {
            let spawn_request = SpawnRequest {
                launch,
                cwd: cwd.to_path_buf(),
                env: env_map.clone(),
                permission_profile: permission_profile.clone(),
                workspace_roots: workspace_roots.to_vec(),
                codex_home: sandbox_base.clone(),
                real_codex_home: codex_home.to_path_buf(),
                cap_sids,
                timeout_ms,
                tty: false,
                stdin_open: false,
                use_private_desktop,
            };
            let transport = retry_runner_spawn_once(
                sandbox_creds,
                &spawn_request.launch,
                |sandbox_creds| {
                    spawn_runner_transport(
                        codex_home,
                        cwd,
                        &sandbox_creds,
                        logs_base_dir,
                        spawn_request.clone(),
                    )
                },
                || {
                    refresh_logon_sandbox_creds(
                        &permissions,
                        cwd,
                        &env_map,
                        codex_home,
                        Some(&resolved_read_roots),
                        read_roots_include_platform_defaults,
                        write_roots_override,
                        &deny_read_paths_override,
                        &deny_write_paths_override,
                        proxy_enforced,
                        crate::WindowsSandboxProxySettingsMode::Reconcile,
                    )
                },
            )?;
            let (pipe_write, mut pipe_read) = transport.into_files();
            let cancel_writer = spawn_cancel_writer(&pipe_write, cancellation)?;

            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            let result = loop {
                let msg = match read_frame(&mut pipe_read) {
                    Ok(Some(msg)) => msg,
                    Ok(None) => break Err(anyhow::anyhow!("runner pipe closed before exit")),
                    Err(err) => break Err(err),
                };
                match msg.message {
                    Message::SpawnReady { .. } => {}
                    Message::Output { payload } => match decode_bytes(&payload.data_b64) {
                        Ok(bytes) => match payload.stream {
                            OutputStream::Stdout => stdout.extend_from_slice(&bytes),
                            OutputStream::Stderr => stderr.extend_from_slice(&bytes),
                        },
                        Err(err) => {
                            break Err(err);
                        }
                    },
                    Message::Exit { payload } => break Ok((payload.exit_code, payload.timed_out)),
                    Message::Error { payload } => {
                        break Err(anyhow::anyhow!("runner error: {}", payload.message));
                    }
                    other => {
                        break Err(anyhow::anyhow!(
                            "unexpected runner message during capture: {other:?}"
                        ));
                    }
                }
            };
            if let Some((cancel_handle, done)) = cancel_writer {
                done.store(true, Ordering::SeqCst);
                cancel_handle.thread().unpark();
                let _ = cancel_handle.join();
            }
            drop(pipe_write);
            let (exit_code, timed_out) = result?;

            if exit_code == 0 {
                log_success(&command, logs_base_dir);
            } else {
                log_failure(&command, &format!("exit code {exit_code}"), logs_base_dir);
            }

            Ok(CaptureResult {
                exit_code,
                stdout,
                stderr,
                timed_out,
            })
        })()
    }
}

#[cfg(target_os = "windows")]
pub use windows_impl::run_windows_sandbox_capture_for_permission_profile;

#[cfg(not(target_os = "windows"))]
mod stub {
    use super::ElevatedSandboxProfileCaptureRequest;
    use anyhow::Result;
    use anyhow::bail;

    #[derive(Debug, Default)]
    pub struct CaptureResult {
        pub exit_code: i32,
        pub stdout: Vec<u8>,
        pub stderr: Vec<u8>,
        pub timed_out: bool,
    }

    /// Stub implementation for non-Windows targets; sandboxing only works on Windows.
    #[allow(clippy::too_many_arguments)]
    pub fn run_windows_sandbox_capture_for_permission_profile(
        _request: ElevatedSandboxProfileCaptureRequest<'_>,
    ) -> Result<CaptureResult> {
        bail!("Windows sandbox is only available on Windows")
    }
}

#[cfg(not(target_os = "windows"))]
pub use stub::run_windows_sandbox_capture_for_permission_profile;
