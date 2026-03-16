use std::io;
use std::os::fd::AsFd;
use std::os::fd::AsRawFd;
use std::os::fd::OwnedFd;

use anyhow::Context as _;
use codex_utils_absolute_path::AbsolutePathBuf;

use crate::unix::escalate_protocol::ESCALATE_SOCKET_ENV_VAR;
use crate::unix::escalate_protocol::EXEC_WRAPPER_ENV_VAR;
use crate::unix::escalate_protocol::EscalateAction;
use crate::unix::escalate_protocol::EscalateRequest;
use crate::unix::escalate_protocol::EscalateResponse;
use crate::unix::escalate_protocol::LEGACY_BASH_EXEC_WRAPPER_ENV_VAR;
use crate::unix::escalate_protocol::SuperExecMessage;
use crate::unix::escalate_protocol::SuperExecResult;
use crate::unix::socket::AsyncDatagramSocket;
use crate::unix::socket::AsyncSocket;

fn get_escalate_client() -> anyhow::Result<AsyncDatagramSocket> {
    // TODO: we should defensively require only calling this once, since AsyncSocket will take ownership of the fd.
    let client_fd = std::env::var(ESCALATE_SOCKET_ENV_VAR)?.parse::<i32>()?;
    if client_fd < 0 {
        return Err(anyhow::anyhow!(
            "{ESCALATE_SOCKET_ENV_VAR} is not a valid file descriptor: {client_fd}"
        ));
    }
    Ok(unsafe { AsyncDatagramSocket::from_raw_fd(client_fd) }?)
}

fn duplicate_fd_for_transfer(fd: impl AsFd, name: &str) -> anyhow::Result<OwnedFd> {
    fd.as_fd()
        .try_clone_to_owned()
        .with_context(|| format!("failed to duplicate {name} for escalation transfer"))
}

pub async fn run_shell_escalation_execve_wrapper(
    file: String,
    argv: Vec<String>,
) -> anyhow::Result<i32> {
    let handshake_client = get_escalate_client()?;
    let (server, client) = AsyncSocket::pair()?;
    const HANDSHAKE_MESSAGE: [u8; 1] = [0];
    handshake_client
        .send_with_fds(&HANDSHAKE_MESSAGE, &[server.into_inner().into()])
        .await
        .context("failed to send handshake datagram")?;
    let env = std::env::vars()
        .filter(|(k, _)| {
            !matches!(
                k.as_str(),
                ESCALATE_SOCKET_ENV_VAR | EXEC_WRAPPER_ENV_VAR | LEGACY_BASH_EXEC_WRAPPER_ENV_VAR
            )
        })
        .collect();
    client
        .send(EscalateRequest {
            file: file.clone().into(),
            argv: argv.clone(),
            workdir: AbsolutePathBuf::current_dir()?,
            env,
        })
        .await
        .context("failed to send EscalateRequest")?;
    let message = client
        .receive::<EscalateResponse>()
        .await
        .context("failed to receive EscalateResponse")?;
    match message.action {
        EscalateAction::Escalate => {
            // Duplicate stdio before transferring ownership to the server. The
            // wrapper must keep using its own stdin/stdout/stderr until the
            // escalated child takes over.
            let destination_fds = [
                io::stdin().as_raw_fd(),
                io::stdout().as_raw_fd(),
                io::stderr().as_raw_fd(),
            ];
            let fds_to_send = [
                duplicate_fd_for_transfer(io::stdin(), "stdin")?,
                duplicate_fd_for_transfer(io::stdout(), "stdout")?,
                duplicate_fd_for_transfer(io::stderr(), "stderr")?,
            ];

            // TODO: also forward signals over the super-exec socket

            client
                .send_with_fds(
                    SuperExecMessage {
                        fds: destination_fds.into_iter().collect(),
                    },
                    &fds_to_send,
                )
                .await
                .context("failed to send SuperExecMessage")?;
            let SuperExecResult { exit_code } = client.receive::<SuperExecResult>().await?;
            Ok(exit_code)
        }
        EscalateAction::Run => {
            // We avoid std::process::Command here because we want to be as transparent as
            // possible. std::os::unix::process::CommandExt has .exec() but it does some funky
            // stuff with signal masks and dup2() on its standard FDs, which we don't want.
            use std::ffi::CString;
            let file = CString::new(file).context("NUL in file")?;

            let argv_cstrs: Vec<CString> = argv
                .iter()
                .map(|s| CString::new(s.as_str()).context("NUL in argv"))
                .collect::<Result<Vec<_>, _>>()?;

            let mut argv: Vec<*const libc::c_char> =
                argv_cstrs.iter().map(|s| s.as_ptr()).collect();
            argv.push(std::ptr::null());

            let err = unsafe {
                libc::execv(file.as_ptr(), argv.as_ptr());
                std::io::Error::last_os_error()
            };

            Err(err.into())
        }
        EscalateAction::Deny { reason } => {
            match reason {
                Some(reason) => eprintln!("Execution denied: {reason}"),
                None => eprintln!("Execution denied"),
            }
            Ok(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::fd::AsRawFd;
    use std::os::unix::net::UnixStream;

    #[test]
    fn duplicate_fd_for_transfer_does_not_close_original() {
        let (left, _right) = UnixStream::pair().expect("socket pair");
        let original_fd = left.as_raw_fd();

        let duplicate = duplicate_fd_for_transfer(&left, "test fd").expect("duplicate fd");
        assert_ne!(duplicate.as_raw_fd(), original_fd);

        drop(duplicate);

        assert_ne!(unsafe { libc::fcntl(original_fd, libc::F_GETFD) }, -1);
    }
}
