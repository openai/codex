use super::materialize;
use codex_shell_command::shell_detect::ShellType;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use test_case::test_case;

#[test_case("/bin/bash", ShellType::Bash; "bash")]
#[cfg_attr(target_os = "macos", test_case("/bin/zsh", ShellType::Zsh; "zsh"))]
#[tokio::test]
async fn unnamed_snapshot_preserves_stdin_and_closes_read_only_carrier(
    shell: &str,
    shell_type: ShellType,
) -> anyhow::Result<()> {
    let state = "helper() { printf 'restored:%s' \"$1\"; }\nexec() { exit 42; }\n";
    let mut reader = materialize(shell_type, state)?;
    let fd = reader.as_raw_fd();
    #[cfg(target_os = "linux")]
    assert!(
        std::fs::read_link(format!("/proc/self/fd/{fd}"))?
            .starts_with(codex_uds::shared_daemon_socket_directory()?),
        "the transport must be protected even before it is unlinked"
    );
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    assert_eq!(
        (
            reader.metadata()?.nlink(),
            flags & libc::FD_CLOEXEC,
            reader.write(b"x").unwrap_err().raw_os_error()
        ),
        (0, libc::FD_CLOEXEC, Some(libc::EBADF)),
    );
    let closed_check = format!("if [ -e /dev/fd/{fd} ]; then exit 43; fi\n");
    let args = vec![
        "-c".to_string(),
        format!(". /dev/fd/{fd}\n{closed_check}IFS= read -r line; helper \"$line\"; exit 7"),
    ];
    let env = HashMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())]);
    let spawned =
        codex_utils_pty::spawn_pipe_process(shell, &args, Path::new("/"), &env, &None, &[fd])
            .await?;
    assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFD) }, flags);
    drop(reader);
    let codex_utils_pty::SpawnedProcess {
        session,
        mut stdout_rx,
        mut stderr_rx,
        exit_rx,
    } = spawned;
    session.writer_sender().send(b"input\n".to_vec()).await?;
    let result = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        let mut output = Vec::new();
        while let Some(chunk) = stdout_rx.recv().await {
            output.extend(chunk);
        }
        let mut errors = Vec::new();
        while let Some(chunk) = stderr_rx.recv().await {
            errors.extend(chunk);
        }
        Ok::<_, anyhow::Error>((String::from_utf8(output)?, errors, exit_rx.await?))
    })
    .await;
    session.terminate();
    let (output, errors, status) = result??;
    assert!(
        output.ends_with("restored:input"),
        "stdout={output:?}, stderr={errors:?}, status={status}"
    );
    assert_eq!((errors, status), (Vec::new(), 7));
    Ok(())
}
