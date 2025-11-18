use std::collections::HashMap;
use std::os::fd::AsRawFd;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use anyhow::Context as _;
use path_absolutize::Absolutize as _;

use codex_core::exec::SandboxType;
use codex_core::exec::process_exec_tool_call;
use codex_core::get_platform_sandbox;
use codex_core::protocol::SandboxPolicy;
use tokio::process::Command;

use crate::posix::escalate_protocol::BASH_EXEC_WRAPPER_ENV_VAR;
use crate::posix::escalate_protocol::ESCALATE_SOCKET_ENV_VAR;
use crate::posix::escalate_protocol::EscalateAction;
use crate::posix::escalate_protocol::EscalateRequest;
use crate::posix::escalate_protocol::EscalateResponse;
use crate::posix::escalate_protocol::SuperExecMessage;
use crate::posix::escalate_protocol::SuperExecResult;
use crate::posix::socket::AsyncDatagramSocket;
use crate::posix::socket::AsyncSocket;

/// This is the policy which decides how to handle an exec() call.
///
/// `file` is the absolute, canonical path to the executable to run, i.e. the first arg to exec.
/// `argv` is the argv, including the program name (`argv[0]`).
/// `workdir` is the absolute, canonical path to the working directory in which to execute the
/// command.
pub(crate) type ExecPolicy = fn(file: &Path, argv: &[String], workdir: &Path) -> EscalateAction;

pub(crate) struct EscalateServer {
    bash_path: PathBuf,
    policy: ExecPolicy,
}

impl EscalateServer {
    pub fn new(bash_path: PathBuf, policy: ExecPolicy) -> Self {
        Self { bash_path, policy }
    }

    pub async fn exec(
        &self,
        command: String,
        env: HashMap<String, String>,
        workdir: PathBuf,
        timeout_ms: Option<u64>,
    ) -> anyhow::Result<ExecResult> {
        let (escalate_server, escalate_client) = AsyncDatagramSocket::pair()?;
        let client_socket = escalate_client.into_inner();
        client_socket.set_cloexec(false)?;

        let escalate_task = tokio::spawn(escalate_task(escalate_server, self.policy));
        let mut env = env.clone();
        env.insert(
            ESCALATE_SOCKET_ENV_VAR.to_string(),
            client_socket.as_raw_fd().to_string(),
        );
        env.insert(
            BASH_EXEC_WRAPPER_ENV_VAR.to_string(),
            format!("{} escalate", std::env::current_exe()?.to_string_lossy()),
        );
        let result = process_exec_tool_call(
            codex_core::exec::ExecParams {
                command: vec![
                    self.bash_path.to_string_lossy().to_string(),
                    "-c".to_string(),
                    command,
                ],
                cwd: PathBuf::from(&workdir),
                timeout_ms,
                env,
                with_escalated_permissions: None,
                justification: None,
                arg0: None,
            },
            get_platform_sandbox().unwrap_or(SandboxType::None),
            // TODO: use the sandbox policy and cwd from the calling client
            &SandboxPolicy::ReadOnly,
            &PathBuf::from("/__NONEXISTENT__"), // This is ignored for ReadOnly
            &None,
            None,
        )
        .await?;
        escalate_task.abort();
        let result = ExecResult {
            exit_code: result.exit_code,
            output: result.aggregated_output.text,
            duration: result.duration,
            timed_out: result.timed_out,
        };
        Ok(result)
    }
}

async fn escalate_task(socket: AsyncDatagramSocket, policy: ExecPolicy) -> anyhow::Result<()> {
    loop {
        let (_, mut fds) = socket.receive_with_fds().await?;
        if fds.len() != 1 {
            tracing::error!("expected 1 fd in datagram handshake, got {}", fds.len());
            continue;
        }
        let stream_socket = AsyncSocket::from_fd(fds.remove(0))?;
        tokio::spawn(async move {
            if let Err(err) = handle_escalate_session_with_policy(stream_socket, policy).await {
                tracing::error!("escalate session failed: {err:?}");
            }
        });
    }
}

#[derive(Debug)]
pub(crate) struct ExecResult {
    pub(crate) exit_code: i32,
    pub(crate) output: String,
    pub(crate) duration: Duration,
    pub(crate) timed_out: bool,
}

async fn handle_escalate_session_with_policy(
    socket: AsyncSocket,
    policy: ExecPolicy,
) -> anyhow::Result<()> {
    let EscalateRequest {
        file,
        argv,
        workdir,
        env,
    } = socket.receive::<EscalateRequest>().await?;
    let file = PathBuf::from(&file).absolutize()?.into_owned();
    let workdir = PathBuf::from(&workdir).absolutize()?.into_owned();
    let action = policy(file.as_path(), &argv, &workdir);
    tracing::debug!("decided {action:?} for {file:?} {argv:?} {workdir:?}");
    match action {
        EscalateAction::Run => {
            socket
                .send(EscalateResponse {
                    action: EscalateAction::Run,
                })
                .await?;
        }
        EscalateAction::Escalate => {
            socket
                .send(EscalateResponse {
                    action: EscalateAction::Escalate,
                })
                .await?;
            let (msg, fds) = socket
                .receive_with_fds::<SuperExecMessage>()
                .await
                .context("failed to receive SuperExecMessage")?;
            if fds.len() != msg.fds.len() {
                return Err(anyhow::anyhow!(
                    "mismatched number of fds in SuperExecMessage: {} in the message, {} from the control message",
                    msg.fds.len(),
                    fds.len()
                ));
            }

            if msg
                .fds
                .iter()
                .any(|src_fd| fds.iter().any(|dst_fd| dst_fd.as_raw_fd() == *src_fd))
            {
                return Err(anyhow::anyhow!(
                    "overlapping fds not yet supported in SuperExecMessage"
                ));
            }

            let mut command = Command::new(file);
            command
                .args(&argv[1..])
                .arg0(argv[0].clone())
                .envs(&env)
                .current_dir(&workdir)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            unsafe {
                command.pre_exec(move || {
                    for (dst_fd, src_fd) in msg.fds.iter().zip(&fds) {
                        libc::dup2(src_fd.as_raw_fd(), *dst_fd);
                    }
                    Ok(())
                });
            }
            let mut child = command.spawn()?;
            let exit_status = child.wait().await?;
            socket
                .send(SuperExecResult {
                    exit_code: exit_status.code().unwrap_or(127),
                })
                .await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::collections::HashMap;
    use std::path::PathBuf;

    #[tokio::test]
    async fn handle_escalate_session_respects_run_in_sandbox_decision() -> anyhow::Result<()> {
        let (server, client) = AsyncSocket::pair()?;
        let server_task = tokio::spawn(handle_escalate_session_with_policy(
            server,
            |_file, _argv, _workdir| EscalateAction::Run,
        ));

        client
            .send(EscalateRequest {
                file: PathBuf::from("/bin/echo"),
                argv: vec!["echo".to_string()],
                workdir: PathBuf::from("/tmp"),
                env: HashMap::new(),
            })
            .await?;

        let response = client.receive::<EscalateResponse>().await?;
        assert_eq!(
            EscalateResponse {
                action: EscalateAction::Run,
            },
            response
        );
        server_task.await?
    }

    #[tokio::test]
    async fn handle_escalate_session_executes_escalated_command() -> anyhow::Result<()> {
        let (server, client) = AsyncSocket::pair()?;
        let server_task = tokio::spawn(handle_escalate_session_with_policy(
            server,
            |_file, _argv, _workdir| EscalateAction::Escalate,
        ));

        client
            .send(EscalateRequest {
                file: PathBuf::from("/bin/sh"),
                argv: vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    r#"if [ "$KEY" = VALUE ]; then exit 42; else exit 1; fi"#.to_string(),
                ],
                workdir: std::env::current_dir()?,
                env: HashMap::from([("KEY".to_string(), "VALUE".to_string())]),
            })
            .await?;

        let response = client.receive::<EscalateResponse>().await?;
        assert_eq!(
            EscalateResponse {
                action: EscalateAction::Escalate,
            },
            response
        );

        client
            .send_with_fds(SuperExecMessage { fds: Vec::new() }, &[])
            .await?;

        let result = client.receive::<SuperExecResult>().await?;
        assert_eq!(42, result.exit_code);

        server_task.await?
    }
}
