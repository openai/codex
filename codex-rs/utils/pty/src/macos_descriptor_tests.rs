//! Native descriptor isolation, socket stdio, and launch errors.

use super::*;
use pretty_assertions::assert_eq;
use std::fs;
use std::io::Seek;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

#[tokio::test]
async fn explicit_stdio_excludes_inheritable_descriptors_without_changing_parent()
-> anyhow::Result<()> {
    let file = fs::File::open("/dev/null")?;
    // SAFETY: Duplicate a live descriptor without CLOEXEC so the control inherits it.
    let raw = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_DUPFD, 200) };
    assert!(raw >= 200);
    // SAFETY: fcntl returned a new descriptor owned by this test.
    let sentinel = unsafe { OwnedFd::from_raw_fd(raw) };
    // SAFETY: The test owns this descriptor throughout both launches.
    let flags = unsafe { libc::fcntl(raw, libc::F_GETFD) };
    let script = format!("if [ -e /dev/fd/{raw} ]; then printf open; else printf closed; fi");
    let mut control = Command::new("/bin/sh");
    control.args(["-c", &script]);
    let control = control.spawn()?.wait_with_output().await?;
    assert_eq!(
        (control.status.success(), control.stdout),
        (true, b"open".to_vec())
    );

    let mut command = Command::new("/bin/sh");
    command
        .args(["-c", &script])
        .descriptor_policy(DescriptorPolicy::Explicit);
    let output = command.spawn()?.wait_with_output().await?;
    assert_eq!(
        (output.status.success(), output.stdout, output.stderr),
        (true, b"closed".to_vec(), vec![])
    );
    // SAFETY: Spawning must not mutate the parent's descriptor flags.
    assert_eq!(
        unsafe { libc::fcntl(sentinel.as_raw_fd(), libc::F_GETFD) },
        flags
    );
    Ok(())
}

#[tokio::test]
async fn socket_stdin_preserves_bidirectional_io_and_custom_argv0() -> anyhow::Result<()> {
    let (parent, socket) = UnixStream::pair()?;
    parent.set_nonblocking(true)?;
    let mut parent = tokio::net::UnixStream::from_std(parent)?;
    let mut command = Command::new("/bin/sh");
    command
        .args(["--norc", "-c", "IFS= read -r line; printf 'reply:%s\\n' \"$line\" >&0; printf '%s:%s:%s' \"$0\" \"$FS_TEST\" \"${HOME-unset}\"; printf diagnostic >&2; exit 19"])
        .arg0("helper")
        .env("FS_TEST", "literal ; $value")
        .stdin(ChildStdin::File(socket.into()))
        .descriptor_policy(DescriptorPolicy::Explicit)
        .fallback(SpawnFallback::ReturnError);
    let child = command.spawn()?;
    assert!(child.stdin.is_none());
    parent.write_all(b"socket\n").await?;
    parent.shutdown().await?;
    let mut reply = Vec::new();
    parent.read_to_end(&mut reply).await?;
    let output = child.wait_with_output().await?;
    assert_eq!(reply, b"reply:socket\n");
    assert_eq!(
        (output.status.code(), output.stdout, output.stderr),
        (
            Some(19),
            b"helper:literal ; $value:unset".to_vec(),
            b"diagnostic".to_vec()
        )
    );
    Ok(())
}

#[tokio::test]
async fn native_launch_can_reject_executable_text_without_shell_fallback() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let script = directory.path().join("no-shebang");
    fs::write(&script, "printf unexpected-fallback\n")?;
    fs::set_permissions(&script, fs::Permissions::from_mode(/*mode*/ 0o755))?;
    let mut command = Command::new(script);
    command.fallback(SpawnFallback::ReturnError);
    let error = command.spawn().err().expect("executable text must fail");
    assert_eq!(error.raw_os_error(), Some(libc::ENOEXEC));
    Ok(())
}

#[tokio::test]
async fn inherited_descriptors_survive_native_and_executable_text_spawns() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let data = directory.path().join("data");
    fs::write(&data, "captured descriptor")?;
    let mut file = fs::File::open(&data)?;
    for native in [true, false] {
        file.rewind()?;
        // SAFETY: Duplicate this live file into a new inheritable descriptor.
        let raw = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_DUPFD, 100) };
        assert!(raw >= 100);
        // SAFETY: fcntl returned a newly owned descriptor.
        let _original = unsafe { OwnedFd::from_raw_fd(raw) };
        let descriptor = format!("/dev/fd/{raw}");
        let mut command = if native {
            let mut command = Command::new("/bin/cat");
            command.arg(&descriptor);
            command
        } else {
            let script = directory.path().join("executable-text");
            fs::write(&script, format!("/bin/cat {descriptor}\n"))?;
            fs::set_permissions(&script, fs::Permissions::from_mode(/*mode*/ 0o755))?;
            Command::new(script)
        };
        command
            .descriptor_policy(DescriptorPolicy::Explicit)
            .preserve_fds(&[raw]);
        let child = command.spawn()?;
        assert_eq!(
            matches!(child.inner, crate::child::ChildKind::Native(_)),
            native
        );
        let output = child.wait_with_output().await?;
        assert_eq!(
            (output.status.code(), output.stdout, output.stderr),
            (Some(0), b"captured descriptor".to_vec(), vec![])
        );
    }
    Ok(())
}
