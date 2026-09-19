//! Regression coverage for native spawn compatibility, descriptor inheritance, and reaping.

use super::*;
use crate::child::ChildKind;
use crate::child::macos::*;
use pretty_assertions::assert_eq;
use std::fs;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::fs::symlink;
use std::os::unix::process::ExitStatusExt;
use std::ptr;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

async fn native_output(command: Command) -> anyhow::Result<std::process::Output> {
    let child = command.spawn()?;
    assert!(matches!(child.inner, ChildKind::Native(_)));
    Ok(child.wait_with_output().await?)
}

#[tokio::test]
async fn bare_script_search_matches_child_path_and_preserves_script_spelling() -> anyhow::Result<()>
{
    let root = tempfile::tempdir()?;
    let program = std::ffi::OsString::from("server=é");
    let bin = root.path().join("bin");
    let blocked = root.path().join("blocked");
    fs::create_dir(&bin)?;
    fs::create_dir(&blocked)?;
    fs::write(blocked.join(&program), "not executable")?;
    let script = bin.join(&program);
    fs::write(
        &script,
        "#!/bin/sh\nprintf '%s\\n' \"$0\" \"$1\" \"$MCP_TEST\"; /bin/pwd; printf diagnostic >&2; exit 23\n",
    )?;
    fs::set_permissions(&script, fs::Permissions::from_mode(/*mode*/ 0o755))?;
    symlink(&script, root.path().join(&program))?;
    for path in [
        bin.into_os_string(),
        "bin/".into(),
        "missing:blocked:bin".into(),
        "missing:".into(),
        "".into(),
    ] {
        let mut command = Command::new(&program);
        command
            .current_dir(root.path())
            .process_mode(ProcessMode::NewGroup)
            .env("PATH", path)
            .env("MCP_TEST", "kept")
            .arg("spaces ; literal $arg");
        let expected = command.inner.output().await?;
        assert_eq!(native_output(command).await?, expected);
    }
    Ok(())
}

#[tokio::test]
async fn bare_executable_uses_default_path_and_preserves_argv0() -> anyhow::Result<()> {
    let mut command = Command::new("sh");
    command
        .process_mode(ProcessMode::NewGroup)
        .args(["-c", "printf '%s' \"$0\""]);
    let expected = command.inner.output().await?;
    assert_eq!(native_output(command).await?, expected);
    Ok(())
}

#[tokio::test]
async fn relative_script_preserves_paths_stdio_environment_and_process_group() -> anyhow::Result<()>
{
    let root = tempfile::tempdir()?;
    fs::create_dir_all(root.path().join("actual/bin"))?;
    symlink("actual/bin", root.path().join("link"))?;
    let script = root.path().join("actual/server");
    fs::write(
        &script,
        "#!/bin/sh\nread -r input\nprintf '%s\\n' \"$0\" \"$1\" \"$2\" \"$MCP_TEST\" \"$input\"\nprintf diagnostic >&2\nexit 23\n",
    )?;
    fs::set_permissions(script, fs::Permissions::from_mode(0o755))?;
    let mut command = Command::new("./link/../server");
    command
        .current_dir(root.path())
        .process_mode(ProcessMode::NewGroup)
        .env("MCP_TEST", "kept")
        .arg("spaces ; literal $arg")
        .arg(std::ffi::OsString::from_vec(b"raw-\xff".to_vec()));
    let mut child = NativeChild::spawn(&command)?.expect("native child");
    let mut stdin = child.stdin.take();
    let mut stdout = child.stdout.take().expect("piped stdout");
    let mut stderr = child.stderr.take().expect("piped stderr");
    let pid = child.id().expect("live PID") as libc::pid_t;
    // SAFETY: getpgid only inspects the live child, which waits for input below.
    assert_eq!(unsafe { libc::getpgid(pid) }, pid);
    stdin
        .as_mut()
        .expect("piped stdin")
        .write_all(b"hello\n")
        .await?;
    drop(stdin);
    let mut output = Vec::new();
    let mut diagnostic = String::new();
    let (_, _, status) = tokio::try_join!(
        stdout.read_to_end(&mut output),
        stderr.read_to_string(&mut diagnostic),
        child.wait()
    )?;
    assert_eq!(
        (output.as_slice(), diagnostic.as_str(), status.code()),
        (
            b"./link/../server\nspaces ; literal $arg\nraw-\xff\nkept\nhello\n".as_slice(),
            "diagnostic",
            Some(23)
        )
    );
    assert_eq!(child.wait().await?, status);
    assert_eq!(child.id(), None);
    Ok(())
}

#[tokio::test]
async fn native_executable_preserves_argv0() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    symlink("/bin/sh", root.path().join("shell"))?;
    let program = Path::new("./shell");
    let mut command = Command::new(program);
    command
        .current_dir(root.path())
        .process_mode(ProcessMode::NewGroup)
        .args(["-c", "printf '%s' \"$0\""]);
    let mut child = NativeChild::spawn(&command)?.expect("native child");
    let stdin = child.stdin.take();
    let mut stdout = child.stdout.take().expect("piped stdout");
    drop(stdin);
    let mut output = Vec::new();
    stdout.read_to_end(&mut output).await?;
    assert!(child.wait().await?.success());
    assert_eq!(output, program.as_os_str().as_bytes());
    Ok(())
}

#[tokio::test]
async fn cancelled_wait_can_still_kill_and_reap_child() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    symlink("/bin/cat", root.path().join("server"))?;
    let mut command = Command::new("./server");
    command
        .current_dir(root.path())
        .process_mode(ProcessMode::NewGroup);
    let mut child = NativeChild::spawn(&command)?.expect("native child");
    let _stdin = child.stdin.take();
    assert!(
        tokio::time::timeout(Duration::from_millis(20), child.wait())
            .await
            .is_err()
    );
    child.kill().await?;
    assert_eq!(child.wait().await?.signal(), Some(libc::SIGKILL));
    assert_eq!(child.id(), None);
    Ok(())
}

#[tokio::test]
async fn descriptor_inheritance_matches_command() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    symlink("/bin/sh", root.path().join("shell"))?;
    let file = fs::File::open("/dev/null")?;
    for (operation, expected) in [
        (libc::F_DUPFD, "inherited"),
        (libc::F_DUPFD_CLOEXEC, "closed"),
    ] {
        // SAFETY: Duplicate a harmless descriptor with the requested inheritance flag.
        let fd = unsafe { libc::fcntl(file.as_raw_fd(), operation, 200) };
        assert!(fd >= 0);
        // SAFETY: fcntl returned a new owned descriptor.
        let _sentinel = unsafe { OwnedFd::from_raw_fd(fd) };
        let mut command = Command::new("./shell");
        command
            .current_dir(root.path())
            .process_mode(ProcessMode::NewGroup)
            .env("SENTINEL", fd.to_string())
            .args([
                "-c",
                "if [ -e /dev/fd/\"$SENTINEL\" ]; then printf inherited; else printf closed; fi",
            ]);
        let mut child = NativeChild::spawn(&command)?.expect("native child");
        let stdin = child.stdin.take();
        let mut stdout = child.stdout.take().expect("piped stdout");
        drop(stdin);
        let mut output = String::new();
        stdout.read_to_string(&mut output).await?;
        assert!(child.wait().await?.success());
        assert_eq!(output, expected);
        assert_eq!(
            command.inner.as_std_mut().output()?.stdout,
            output.as_bytes()
        );
    }
    Ok(())
}

#[tokio::test]
async fn launch_failures_preserve_os_errors() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    fs::write(root.path().join("not-executable"), "#!/bin/sh\nexit 0\n")?;
    for (program, cwd, errno) in [
        ("./missing", root.path().to_path_buf(), libc::ENOENT),
        ("./not-executable", root.path().to_path_buf(), libc::EACCES),
        ("missing", root.path().to_path_buf(), libc::ENOENT),
        ("not-executable", root.path().to_path_buf(), libc::EACCES),
        ("", root.path().to_path_buf(), libc::ENOENT),
        ("/bin/sh", root.path().join("missing"), libc::ENOENT),
    ] {
        let mut command = Command::new(program);
        command
            .process_mode(ProcessMode::NewGroup)
            .env("PATH", root.path())
            .current_dir(cwd);
        let error = command.spawn().err().expect("spawn should fail");
        assert_eq!(error.raw_os_error(), Some(errno));
    }
    Ok(())
}

#[tokio::test]
async fn executable_text_without_shebang_retains_command_fallback() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let script = root.path().join("server");
    fs::write(&script, "printf '%s' \"$0\"\n")?;
    fs::set_permissions(script, fs::Permissions::from_mode(0o755))?;
    for program in ["./server", "server"] {
        let mut command = Command::new(program);
        command
            .current_dir(root.path())
            .process_mode(ProcessMode::NewGroup)
            .env("PATH", ".");
        let mut child = command.spawn()?;
        let mut output = Vec::new();
        child
            .stdout
            .take()
            .expect("piped stdout")
            .read_to_end(&mut output)
            .await?;
        assert!(child.wait().await?.success());
        assert_eq!(output, b"./server");
    }
    Ok(())
}

#[test]
fn dropping_after_runtime_shutdown_kills_and_reaps_child() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    symlink("/bin/cat", root.path().join("server"))?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let mut child = runtime
        .block_on(async {
            let mut command = Command::new("./server");
            command
                .current_dir(root.path())
                .process_mode(ProcessMode::NewGroup);
            NativeChild::spawn(&command)
        })?
        .expect("native child");
    let stdin = child.stdin.take();
    let pid = child.id().expect("live PID") as libc::pid_t;
    drop(runtime);
    drop(child);
    drop(stdin);
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        // SAFETY: Signal zero probes existence without changing the process.
        if unsafe { libc::kill(pid, 0) } == -1 {
            assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::ESRCH));
            break;
        }
        anyhow::ensure!(std::time::Instant::now() < deadline, "child was not reaped");
        std::thread::sleep(Duration::from_millis(10));
    }
    // SAFETY: WNOHANG verifies our drop reaper already collected this child.
    assert_eq!(
        unsafe { libc::waitpid(pid, ptr::null_mut(), libc::WNOHANG) },
        -1
    );
    assert_eq!(
        io::Error::last_os_error().raw_os_error(),
        Some(libc::ECHILD)
    );
    Ok(())
}

#[tokio::test]
async fn process_mode_is_preserved_by_both_backends() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    symlink("/bin/cat", root.path().join("server"))?;
    for program in ["./server", "/bin/cat"] {
        for mode in [ProcessMode::Inherit, ProcessMode::NewGroup] {
            let mut command = Command::new(program);
            command.current_dir(root.path()).process_mode(mode);
            let mut child = command.spawn()?;
            let pid = child.id().expect("live PID") as libc::pid_t;
            let expected = match mode {
                // SAFETY: getpgrp only inspects the current process.
                ProcessMode::Inherit => unsafe { libc::getpgrp() },
                ProcessMode::NewGroup => pid,
            };
            // SAFETY: The child is still owned and blocked on its stdin pipe.
            assert_eq!(unsafe { libc::getpgid(pid) }, expected);
            child.kill().await?;
        }
    }
    Ok(())
}
