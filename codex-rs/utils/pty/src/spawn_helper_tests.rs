//! Verify Linux helper dispatch, launch errors, and target environment isolation.

use std::collections::HashMap;
use std::io;
use std::path::Path;

use pretty_assertions::assert_eq;

use crate::spawn_pipe_process;
use crate::spawn_pipe_process_no_stdin;

#[ctor::ctor]
fn initialize_spawn_helper() {
    if let Ok(arguments) = std::fs::read("/proc/self/cmdline") {
        let mut arguments = arguments.split(|byte| *byte == 0).skip(1);
        if arguments.next() == Some(crate::spawn_helper::HELPER_ARG.as_bytes())
            && arguments.any(|arg| arg == b"--codex-test-fail-helper-bootstrap")
        {
            // Simulate a loader failure before the helper can report setup.
            std::process::exit(127);
        }
    }
    #[cfg(target_env = "gnu")]
    if std::env::var_os("CODEX_TEST_PARENT_LOADER_ENV").is_some() {
        // SAFETY: This isolated fixture is still single-threaded, before the test
        // harness starts. Simulate .env loading after the dynamic loader ran.
        unsafe { std::env::set_var("LD_TRACE_LOADED_OBJECTS", "1") };
    }
    if std::env::var_os("CODEX_TEST_EARLY_HELPER_ARGV").is_some() {
        crate::init_spawn_helper(std::iter::empty());
    } else {
        crate::init_spawn_helper(std::env::args_os());
    }
}

#[tokio::test]
async fn helper_preserves_cwd_env_arg0_and_streams() -> anyhow::Result<()> {
    thread_local! { static FORKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
    extern "C" fn after_fork() {
        FORKS.with(|count| count.set(count.get() + 1));
    }
    FORKS.with(|count| count.set(0));
    assert_eq!(
        unsafe { libc::pthread_atfork(None, Some(after_fork), None) },
        0
    );
    let mut child = spawn_pipe_process(
        "sh",
        &["-c".into(), "read value; printf '%s|%s|%s|%s' \"$0\" \"$MARKER\" \"$PWD\" \"$value\"; printf error >&2; exit 23".into()],
        Path::new("/tmp"),
        &HashMap::from([("PATH".into(), "/bin".into()), ("MARKER".into(), "a value".into())]),
        &Some("custom-arg0".into()),
        &[],
    ).await?;
    #[cfg(target_env = "gnu")]
    let native_available = unsafe {
        !libc::dlsym(
            libc::RTLD_DEFAULT,
            c"posix_spawn_file_actions_addchdir_np".as_ptr(),
        )
        .is_null()
    };
    #[cfg(not(target_env = "gnu"))]
    let native_available = true;
    if native_available {
        assert_eq!(FORKS.with(std::cell::Cell::get), 0);
    }
    child
        .session
        .writer_sender()
        .send(b"input\n".to_vec())
        .await?;
    child.session.close_stdin();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    while let Some(chunk) = child.stdout_rx.recv().await {
        stdout.extend(chunk);
    }
    while let Some(chunk) = child.stderr_rx.recv().await {
        stderr.extend(chunk);
    }
    assert_eq!(
        (stdout, stderr, child.exit_rx.await?),
        (
            b"custom-arg0|a value|/tmp|input".to_vec(),
            b"error".to_vec(),
            23
        )
    );
    Ok(())
}

#[tokio::test]
async fn helper_reports_target_exec_and_cwd_errno() {
    for (program, cwd, errno) in [
        ("/codex-missing-spawn-target", "/", libc::ENOENT),
        ("/bin/sh", "/codex-missing-spawn-directory", libc::ENOENT),
        ("/", "/", libc::EACCES),
    ] {
        let result =
            spawn_pipe_process_no_stdin(program, &[], Path::new(cwd), &HashMap::new(), &None, &[])
                .await;
        let error = result.expect_err("invalid target must fail at spawn");
        assert_eq!(
            error
                .downcast_ref::<io::Error>()
                .and_then(io::Error::raw_os_error),
            Some(errno)
        );
    }
}

#[tokio::test]
async fn target_loader_environment_does_not_disable_helper_dispatch() -> anyhow::Result<()> {
    if !Path::new("/proc/self/exe").exists() {
        eprintln!("skipping loader isolation test: procfs is unavailable");
        return Ok(());
    }
    // glibc's loader exits before main when this is set. It must affect the
    // requested executable only, after the helper has completed its setup.
    let mut command = crate::Command::new("/codex-missing-loader-env-target");
    command.current_dir("/").env("LD_TRACE_LOADED_OBJECTS", "1");
    // A failed helper bootstrap returns None; require the helper's target error.
    let Err(error) = crate::spawn_helper::spawn(&command).await else {
        panic!("helper must reach target exec despite loader settings");
    };
    assert_eq!(error.raw_os_error(), Some(libc::ENOENT));
    Ok(())
}

#[test]
fn helper_dispatch_before_rust_initializes_argv() -> anyhow::Result<()> {
    let output = std::process::Command::new(std::env::current_exe()?)
        .args([
            "--exact",
            "spawn_helper_tests::helper_preserves_cwd_env_arg0_and_streams",
            "--nocapture",
        ])
        .env("CODEX_TEST_EARLY_HELPER_ARGV", "1")
        .output()?;
    assert!(output.status.success(), "{output:?}");
    Ok(())
}

#[tokio::test]
async fn helper_bootstrap_failure_falls_back_to_direct_spawn() -> anyhow::Result<()> {
    let mut child = spawn_pipe_process_no_stdin(
        "/bin/echo",
        &["--codex-test-fail-helper-bootstrap".into()],
        Path::new("/"),
        &HashMap::new(),
        &None,
        &[],
    )
    .await?;
    let mut stdout = Vec::new();
    while let Some(chunk) = child.stdout_rx.recv().await {
        stdout.extend(chunk);
    }
    assert_eq!(
        (child.exit_rx.await?, stdout),
        (0, b"--codex-test-fail-helper-bootstrap\n".to_vec())
    );
    Ok(())
}

#[tokio::test]
async fn helper_control_socket_survives_closed_stdio() -> anyhow::Result<()> {
    use std::os::fd::AsRawFd;
    use std::os::fd::BorrowedFd;

    if std::env::var_os("CODEX_TEST_HELPER_CLOSED_STDIO").is_none() {
        let output = std::process::Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "spawn_helper_tests::helper_control_socket_survives_closed_stdio",
                "--nocapture",
            ])
            .env("CODEX_TEST_HELPER_CLOSED_STDIO", "1")
            .output()?;
        assert!(output.status.success(), "{output:?}");
        return Ok(());
    }
    let mut command = crate::Command::new("/bin/echo");
    command.arg("closed-stdio").stdin(crate::ChildStdin::Null);
    let Some(probe) = crate::spawn_helper::spawn(&command).await? else {
        return Ok(());
    };
    assert!(probe.wait_with_output().await?.status.success());
    // This isolated subprocess has an initialized runtime. Close stdio only
    // after saving it, so the helper socket really receives descriptor 0 or 1.
    let saved = [0, 1]
        .map(|fd| unsafe { BorrowedFd::borrow_raw(fd) }.try_clone_to_owned())
        .into_iter()
        .collect::<io::Result<Vec<_>>>()?;
    for fd in [0, 1] {
        assert_eq!(unsafe { libc::close(fd) }, 0);
    }
    let result = crate::spawn_helper::spawn(&command).await;
    for (fd, saved) in [0, 1].into_iter().zip(saved) {
        assert_eq!(unsafe { libc::dup2(saved.as_raw_fd(), fd) }, fd);
    }
    let output = result
        .expect("closed stdio must not break the helper handshake")
        .expect("closed stdio must not force a fallback")
        .wait_with_output()
        .await?;
    assert_eq!(
        (output.status.code(), output.stdout, output.stderr),
        (Some(0), b"closed-stdio\n".to_vec(), vec![])
    );
    Ok(())
}

#[tokio::test]
async fn helper_descriptor_pressure_falls_back_to_direct_spawn() -> anyhow::Result<()> {
    use std::os::fd::AsRawFd;
    use std::os::fd::FromRawFd;
    use std::os::fd::OwnedFd;
    if std::env::var_os("CODEX_TEST_HELPER_FD_PRESSURE").is_none() {
        let output = std::process::Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "spawn_helper_tests::helper_descriptor_pressure_falls_back_to_direct_spawn",
                "--nocapture",
            ])
            .env("CODEX_TEST_HELPER_FD_PRESSURE", "1")
            .output()?;
        assert!(output.status.success(), "{output:?}");
        return Ok(());
    }
    let source = std::fs::File::open("/dev/null")?;
    let originals = (0..16)
        .map(|_| {
            // SAFETY: fcntl duplicates a live descriptor into a newly owned slot.
            let fd = unsafe { libc::fcntl(source.as_raw_fd(), libc::F_DUPFD, 3) };
            anyhow::ensure!(fd >= 3, "{}", io::Error::last_os_error());
            Ok(unsafe { OwnedFd::from_raw_fd(fd) })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let targets = originals.iter().map(AsRawFd::as_raw_fd).collect::<Vec<_>>();
    let mut command = crate::Command::new("/bin/echo");
    command
        .arg("fallback")
        .stdin(crate::ChildStdin::File(source.try_clone()?.into()))
        .process_mode(crate::ProcessMode::NewSession)
        .terminate_on_parent_death()
        .preserve_fds(&targets);
    // Restrict only this subprocess, after constructing the target's stdio.
    let mut limit = std::mem::MaybeUninit::<libc::rlimit>::uninit();
    assert_eq!(
        unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, limit.as_mut_ptr()) },
        0
    );
    let mut limit = unsafe { limit.assume_init() };
    limit.rlim_cur = limit.rlim_cur.min(128);
    assert_eq!(unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &limit) }, 0);
    let mut occupied = Vec::new();
    loop {
        let fd = unsafe { libc::fcntl(source.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 3) };
        if fd == -1 {
            assert_eq!(
                io::Error::last_os_error().raw_os_error(),
                Some(libc::EMFILE)
            );
            break;
        }
        occupied.push(unsafe { OwnedFd::from_raw_fd(fd) });
    }
    // Direct spawning fits, but the helper also clones stdin and opens its control socket.
    anyhow::ensure!(occupied.len() >= 7, "not enough descriptors to reserve");
    occupied.truncate(occupied.len() - 7);
    assert!(crate::spawn_helper::spawn(&command).await?.is_none());
    let output = command.spawn()?.wait_with_output().await?;
    assert_eq!(
        (output.status.code(), output.stdout, output.stderr),
        (Some(0), b"fallback\n".to_vec(), vec![])
    );
    Ok(())
}

#[cfg(target_env = "gnu")]
#[tokio::test]
async fn parent_loader_environment_does_not_disable_helper_dispatch() -> anyhow::Result<()> {
    if !Path::new("/proc/self/exe").exists() {
        eprintln!("skipping loader isolation test: procfs is unavailable");
        return Ok(());
    }
    if std::env::var_os("CODEX_TEST_PARENT_LOADER_ENV").is_none() {
        let output = std::process::Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "spawn_helper_tests::parent_loader_environment_does_not_disable_helper_dispatch",
                "--nocapture",
            ])
            .env("CODEX_TEST_PARENT_LOADER_ENV", "1")
            .env_remove("LD_TRACE_LOADED_OBJECTS")
            .output()?;
        assert!(output.status.success(), "{output:?}");
        return Ok(());
    }
    let mut command = crate::Command::new("/codex-missing-parent-loader-env-target");
    command.current_dir("/");
    let Err(error) = crate::spawn_helper::spawn(&command).await else {
        panic!("helper must reach target exec despite parent loader settings");
    };
    assert_eq!(error.raw_os_error(), Some(libc::ENOENT));
    Ok(())
}
