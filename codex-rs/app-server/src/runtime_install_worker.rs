use std::collections::HashMap;

use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::RuntimeInstallParams;
use codex_app_server_protocol::RuntimeInstallProgressNotification;
use codex_app_server_protocol::RuntimeInstallResponse;
use codex_exec_server::Environment;
use codex_exec_server::ExecEnvPolicy;
use codex_exec_server::ExecOutputStream;
use codex_exec_server::ExecParams;
use codex_exec_server::ExecProcessEvent;
use codex_exec_server::ProcessId;
use codex_exec_server::WriteStatus;
use codex_protocol::config_types::ShellEnvironmentPolicyInherit;
use codex_runtime_install::CODEX_RUNTIME_INSTALL_HELPER_ARG1;
use codex_runtime_install::RuntimeInstallHelperMessage;
use codex_runtime_install::RuntimeInstallHelperRequest;
use codex_utils_path_uri::PathUri;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::error_code::internal_error;

pub(crate) async fn install_runtime_with_progress(
    environment: &Environment,
    params: RuntimeInstallParams,
    progress: mpsc::UnboundedSender<RuntimeInstallProgressNotification>,
    cancellation: CancellationToken,
) -> Result<RuntimeInstallResponse, JSONRPCErrorError> {
    let executable = environment.codex_self_exe().await?;
    let cwd = environment.codex_home().await?;
    let started = environment
        .get_exec_backend()
        .start(ExecParams {
            process_id: ProcessId::from(format!("runtime-install-{}", Uuid::now_v7())),
            argv: vec![
                executable.as_path().to_string_lossy().into_owned(),
                CODEX_RUNTIME_INSTALL_HELPER_ARG1.to_string(),
            ],
            cwd: PathUri::from_abs_path(&cwd),
            env_policy: Some(ExecEnvPolicy {
                inherit: ShellEnvironmentPolicyInherit::All,
                ignore_default_excludes: true,
                exclude: Vec::new(),
                r#set: HashMap::new(),
                include_only: Vec::new(),
            }),
            env: HashMap::new(),
            tty: false,
            pipe_stdin: true,
            arg0: None,
        })
        .await
        .map_err(|err| internal_error(format!("failed to start runtime install helper: {err}")))?;
    write_helper_request(
        started.process.as_ref(),
        &RuntimeInstallHelperRequest::Install { params },
    )
    .await?;

    let mut events = started.process.subscribe_events();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut exit_code = None;
    let mut cancellation_sent = false;
    loop {
        tokio::select! {
            _ = cancellation.cancelled(), if !cancellation_sent => {
                write_helper_request(started.process.as_ref(), &RuntimeInstallHelperRequest::Cancel).await?;
                cancellation_sent = true;
            }
            event = events.recv() => {
                let event = event.map_err(|err| {
                    internal_error(format!("runtime install helper output stream failed: {err}"))
                })?;
                match event {
                    ExecProcessEvent::Output(chunk) => match chunk.stream {
                        ExecOutputStream::Stdout => {
                            stdout.extend_from_slice(&chunk.chunk.0);
                            while let Some(line_end) = stdout.iter().position(|byte| *byte == b'\n') {
                                let line = stdout.drain(..=line_end).collect::<Vec<_>>();
                                let message: RuntimeInstallHelperMessage = serde_json::from_slice(
                                    line.strip_suffix(b"\n").unwrap_or(line.as_slice()),
                                )
                                .map_err(|err| {
                                    internal_error(format!("runtime install helper returned invalid output: {err}"))
                                })?;
                                match message {
                                    RuntimeInstallHelperMessage::Progress { progress: update } => {
                                        let _ = progress.send(update);
                                    }
                                    RuntimeInstallHelperMessage::Complete { response } => {
                                        return Ok(response);
                                    }
                                    RuntimeInstallHelperMessage::Error { error } => return Err(error),
                                }
                            }
                        }
                        ExecOutputStream::Stderr => stderr.extend_from_slice(&chunk.chunk.0),
                        ExecOutputStream::Pty => {
                            return Err(internal_error("runtime install helper unexpectedly used a pty"));
                        }
                    },
                    ExecProcessEvent::Exited { exit_code: code, .. } => exit_code = Some(code),
                    ExecProcessEvent::Closed { .. } => {
                        let stderr = String::from_utf8_lossy(&stderr);
                        return Err(internal_error(format!(
                            "runtime install helper exited without a result (exit code {}; stderr: {stderr})",
                            exit_code.unwrap_or(-1)
                        )));
                    }
                    ExecProcessEvent::Failed(message) => {
                        return Err(internal_error(format!("runtime install helper process failed: {message}")));
                    }
                }
            }
        }
    }
}

async fn write_helper_request(
    process: &dyn codex_exec_server::ExecProcess,
    request: &RuntimeInstallHelperRequest,
) -> Result<(), JSONRPCErrorError> {
    let mut encoded = serde_json::to_vec(request).map_err(|err| {
        internal_error(format!(
            "failed to serialize runtime install request: {err}"
        ))
    })?;
    encoded.push(b'\n');
    let response = process.write(encoded).await.map_err(|err| {
        internal_error(format!(
            "failed to write runtime install helper input: {err}"
        ))
    })?;
    if response.status != WriteStatus::Accepted {
        return Err(internal_error(format!(
            "runtime install helper rejected stdin: {:?}",
            response.status
        )));
    }
    Ok(())
}
