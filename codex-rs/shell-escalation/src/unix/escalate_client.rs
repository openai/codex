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

async fn connect_escalation_stream(
    handshake_client: AsyncDatagramSocket,
) -> anyhow::Result<(AsyncSocket, OwnedFd)> {
    let (server, client) = AsyncSocket::pair()?;
    let server_stream_guard: OwnedFd = server.into_inner().into();
    let transferred_server_stream =
        duplicate_fd_for_transfer(&server_stream_guard, "handshake stream")?;
    const HANDSHAKE_MESSAGE: [u8; 1] = [0];
    // Keep one local reference to the transferred stream alive until the server
    // answers the first request. On macOS, dropping the sender's last local copy
    // immediately after the datagram handshake can make the peer observe EOF
    // before the received fd is fully servicing the stream.
    handshake_client
        .send_with_fds(&HANDSHAKE_MESSAGE, &[transferred_server_stream])
        .await
        .context("failed to send handshake datagram")?;
    Ok((client, server_stream_guard))
}

pub async fn run_shell_escalation_execve_wrapper(
    file: String,
    argv: Vec<String>,
) -> anyhow::Result<i32> {
    let handshake_client = get_escalate_client()?;
    let (client, server_stream_guard) = connect_escalation_stream(handshake_client).await?;
    let env = std::env::vars()
        .filter(|(k, _)| !matches!(k.as_str(), ESCALATE_SOCKET_ENV_VAR | EXEC_WRAPPER_ENV_VAR))
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
    // Once the first request has been written into the stream, the local guard
    // is no longer needed to bridge the datagram handoff. Dropping it here
    // lets client-side reads still observe EOF if the server exits before
    // replying.
    drop(server_stream_guard);
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
    use std::path::PathBuf;
    use std::time::Duration;

    use pretty_assertions::assert_eq;
    use tokio::time::sleep;
    use tokio::time::timeout;

    #[test]
    fn duplicate_fd_for_transfer_does_not_close_original() {
        let (left, _right) = UnixStream::pair().expect("socket pair");
        let original_fd = left.as_raw_fd();

        let duplicate = duplicate_fd_for_transfer(&left, "test fd").expect("duplicate fd");
        assert_ne!(duplicate.as_raw_fd(), original_fd);

        drop(duplicate);

        assert_ne!(unsafe { libc::fcntl(original_fd, libc::F_GETFD) }, -1);
    }

    #[tokio::test]
    async fn connect_escalation_stream_keeps_sender_alive_until_first_request_write()
    -> anyhow::Result<()> {
        let (server_datagram, client_datagram) = AsyncDatagramSocket::pair()?;
        let client_task = tokio::spawn(async move {
            let (client_stream, server_stream_guard) =
                connect_escalation_stream(client_datagram).await?;
            let guard_fd = server_stream_guard.as_raw_fd();
            assert_ne!(unsafe { libc::fcntl(guard_fd, libc::F_GETFD) }, -1);
            client_stream
                .send(EscalateRequest {
                    file: PathBuf::from("/bin/echo"),
                    argv: vec!["echo".to_string(), "hello".to_string()],
                    workdir: AbsolutePathBuf::current_dir()?,
                    env: Default::default(),
                })
                .await?;
            drop(server_stream_guard);
            assert_eq!(-1, unsafe { libc::fcntl(guard_fd, libc::F_GETFD) });
            let response = client_stream.receive::<EscalateResponse>().await?;
            Ok::<EscalateResponse, anyhow::Error>(response)
        });

        let (_, mut fds) = server_datagram.receive_with_fds().await?;
        assert_eq!(fds.len(), 1);
        sleep(Duration::from_millis(20)).await;
        let server_stream = AsyncSocket::from_fd(fds.remove(0))?;
        let request = server_stream.receive::<EscalateRequest>().await?;
        assert_eq!(request.file, PathBuf::from("/bin/echo"));
        assert_eq!(request.argv, vec!["echo".to_string(), "hello".to_string()]);

        let expected = EscalateResponse {
            action: EscalateAction::Deny {
                reason: Some("not now".to_string()),
            },
        };
        server_stream.send(expected.clone()).await?;
        let response = client_task.await??;
        assert_eq!(response, expected);
        Ok(())
    }

    #[tokio::test]
    async fn dropping_guard_after_request_write_preserves_server_eof() -> anyhow::Result<()> {
        let (server_datagram, client_datagram) = AsyncDatagramSocket::pair()?;
        let client_task = tokio::spawn(async move {
            let (client_stream, server_stream_guard) =
                connect_escalation_stream(client_datagram).await?;
            client_stream
                .send(EscalateRequest {
                    file: PathBuf::from("/bin/echo"),
                    argv: vec!["echo".to_string()],
                    workdir: AbsolutePathBuf::current_dir()?,
                    env: Default::default(),
                })
                .await?;
            drop(server_stream_guard);
            let err = timeout(
                Duration::from_millis(250),
                client_stream.receive::<EscalateResponse>(),
            )
            .await
            .expect("server close should not hang the client")
            .expect_err("expected EOF after server closes without replying");
            assert_eq!(err.kind(), std::io::ErrorKind::UnexpectedEof);
            Ok::<(), anyhow::Error>(())
        });

        let (_, mut fds) = server_datagram.receive_with_fds().await?;
        assert_eq!(fds.len(), 1);
        let server_stream = AsyncSocket::from_fd(fds.remove(0))?;
        let request = server_stream.receive::<EscalateRequest>().await?;
        assert_eq!(request.file, PathBuf::from("/bin/echo"));
        drop(server_stream);

        client_task.await??;
        Ok(())
    }
}
