//! Exercise the exec-helper boundary with real children, including descriptors,
//! startup errors, cancellation cleanup, and concurrent exit notification.
//! Procfs-dependent fixtures are skipped when procfs is unavailable.

use std::collections::HashMap;
use std::io;
use std::os::fd::AsFd;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;
use std::path::Path;
use std::time::Duration;

use pretty_assertions::assert_eq;

use crate::spawn_pipe_process;
use crate::spawn_pipe_process_no_stdin;

const PARENT_FIXTURE: &str = "--codex-spawn-test-parent";
const CHILD_FIXTURE: &str = "--codex-spawn-test-child";

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
    let Ok(arguments) = process_arguments() else {
        return;
    };
    let mut arguments = arguments.into_iter().skip(1);
    let argument = arguments.next();
    if argument.as_deref() == Some(std::ffi::OsStr::new(CHILD_FIXTURE)) {
        let Some(cleanup_fd) = arguments
            .next()
            .and_then(|argument| argument.to_str()?.parse().ok())
        else {
            std::process::exit(1);
        };
        // SAFETY: The parent passed this owned read end as an inherited descriptor.
        let mut cleanup = unsafe { std::fs::File::from_raw_fd(cleanup_fd) };
        // Preserve a final empty argument through musl's constructor argv fallback.
        assert_eq!(arguments.collect::<Vec<_>>(), [std::ffi::OsString::new()]);
        let mut signal = 0;
        assert_eq!(
            unsafe { libc::prctl(libc::PR_GET_PDEATHSIG, &mut signal) },
            0
        );
        assert_eq!(signal, libc::SIGTERM);
        println!("{}", std::process::id());
        // Only the test owns the write end. EOF provides cleanup if the
        // parent-death signal fails, without a timer that could mask that failure.
        match std::io::copy(&mut cleanup, &mut std::io::sink()) {
            Ok(_) => std::process::exit(0),
            Err(_) => std::process::exit(1),
        }
    }
    if argument.as_deref() == Some(std::ffi::OsStr::new(PARENT_FIXTURE)) {
        let result = (|| -> anyhow::Result<()> {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            runtime.block_on(async {
                let cleanup = std::io::stdin().as_fd().try_clone_to_owned()?;
                // The shared launcher only preserves explicitly inheritable fds.
                assert_eq!(
                    unsafe { libc::fcntl(cleanup.as_raw_fd(), libc::F_SETFD, 0) },
                    0
                );
                let mut child = spawn_pipe_process_no_stdin(
                    "/proc/self/exe",
                    &[
                        CHILD_FIXTURE.to_string(),
                        cleanup.as_raw_fd().to_string(),
                        String::new(),
                    ],
                    Path::new("/"),
                    &HashMap::new(),
                    &None,
                    &[cleanup.as_raw_fd()],
                )
                .await?;
                let ready = child
                    .stdout_rx
                    .recv()
                    .await
                    .ok_or_else(|| io::Error::other("fixture child did not start"))?;
                std::io::Write::write_all(&mut std::io::stdout(), &ready)?;
                // Kill this parent without destructors so session cleanup
                // cannot mask the kernel's parent-death signal behavior.
                std::future::pending::<()>().await;
                anyhow::Ok(())
            })
        })();
        if let Err(error) = result {
            eprintln!("spawn fixture failed: {error}");
            std::process::exit(1);
        }
        std::process::exit(0);
    }
}

/// Pin a process identity so exit observation and cleanup cannot follow PID reuse.
fn open_pidfd(pid: libc::pid_t) -> io::Result<OwnedFd> {
    // SAFETY: pidfd_open takes a PID and flags, and returns a new owned descriptor.
    let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0u32) };
    if fd == -1 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: The successful syscall returned a fresh descriptor owned by this call.
    Ok(unsafe { OwnedFd::from_raw_fd(fd as _) })
}

#[tokio::test]
async fn helper_target_dies_when_parent_is_killed() -> anyhow::Result<()> {
    use tokio::io::AsyncBufReadExt;
    if !Path::new("/proc/self/exe").exists() || process_arguments().is_err() {
        eprintln!("skipping parent-death test: procfs is unavailable");
        return Ok(());
    }
    // Check support and sandbox permissions before creating fixtures.
    match open_pidfd(std::process::id() as _) {
        Ok(_) => {}
        Err(error)
            if matches!(
                error.raw_os_error(),
                Some(libc::ENOSYS | libc::EPERM | libc::EACCES)
            ) =>
        {
            eprintln!("skipping parent-death test: pidfd_open is unavailable");
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    }
    let mut parent = tokio::process::Command::new(std::env::current_exe()?)
        .arg(PARENT_FIXTURE)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    // Keep the cleanup pipe open independently of the parent's lifetime.
    let cleanup = parent.stdin.take().expect("parent fixture stdin is piped");
    let mut output = tokio::io::BufReader::new(parent.stdout.take().unwrap()).lines();
    let pid = tokio::time::timeout(Duration::from_secs(5), output.next_line())
        .await??
        .expect("child reports its PID after verifying PDEATHSIG")
        .parse::<libc::pid_t>()?;
    // The fixture child waits on the cleanup pipe while its parent is alive.
    let child = tokio::io::unix::AsyncFd::new(open_pidfd(pid)?)?;
    parent.kill().await?;
    let stopped = tokio::time::timeout(Duration::from_secs(5), child.readable()).await;
    drop(cleanup);
    if stopped.is_err() {
        drop(tokio::time::timeout(Duration::from_secs(5), child.readable()).await??);
    }
    drop(stopped??);
    Ok(())
}

/// Probe an actual helper launch before requiring the native handshake.
async fn native_helper_available() -> anyhow::Result<bool> {
    if !Path::new("/proc/self/exe").exists() || process_arguments().is_err() {
        return Ok(false);
    }
    let command = crate::Command::new("/bin/true");
    let Some(child) =
        crate::spawn_helper::spawn(&command, crate::spawn_helper::Setup::Pipe).await?
    else {
        return Ok(false);
    };
    Ok(child.wait_with_output().await?.status.success())
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
    if native_helper_available().await? {
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
    let Err(error) = crate::spawn_helper::spawn(&command, crate::spawn_helper::Setup::Pipe).await
    else {
        panic!("helper must reach target exec despite loader settings");
    };
    assert_eq!(error.raw_os_error(), Some(libc::ENOENT));
    Ok(())
}

#[tokio::test]
async fn large_environment_can_cross_the_control_socket() -> anyhow::Result<()> {
    if !Path::new("/proc/self/exe").exists() || process_arguments().is_err() {
        eprintln!("skipping control-socket test: procfs is unavailable");
        return Ok(());
    }
    // Stay below Linux's minimum ARG_MAX; cancellation tests force socket backpressure.
    let mut command = crate::Command::new("/bin/sh");
    command
        .args([
            "-c",
            "test ${#LARGE_0} -eq 8192 && test ${#LARGE_7} -eq 8192",
        ])
        .current_dir("/")
        .envs((0..8).map(|index| (format!("LARGE_{index}"), "x".repeat(8_192))));
    let child = tokio::time::timeout(
        Duration::from_secs(5),
        crate::spawn_helper::spawn(&command, crate::spawn_helper::Setup::Pipe),
    )
    .await??
    .expect("native helper must transfer the environment");
    assert!(child.wait_with_output().await?.status.success());
    Ok(())
}

#[tokio::test]
async fn helper_preserves_inheritable_fds_and_closes_unrequested_fds() -> anyhow::Result<()> {
    let file = std::fs::File::open("/dev/null")?;
    if std::fs::metadata(format!("/proc/self/fd/{}", file.as_raw_fd())).is_err() {
        eprintln!("skipping descriptor inspection test: procfs is unavailable");
        return Ok(());
    }
    let descriptors = [file.try_clone()?, file.try_clone()?];
    let preserved = descriptors[0].as_raw_fd();
    let unwanted = descriptors[1].as_raw_fd();
    assert_eq!(unsafe { libc::fcntl(unwanted, libc::F_SETFD, 0) }, 0);
    assert_eq!(unsafe { libc::fcntl(preserved, libc::F_SETFD, 0) }, 0);
    let script = format!("test -e /proc/self/fd/{preserved} && test ! -e /proc/self/fd/{unwanted}");
    let mut command = crate::Command::new("/bin/sh");
    command
        .args(["-c", &script])
        .current_dir("/")
        .descriptor_policy(crate::DescriptorPolicy::Explicit)
        .preserve_fds(&[preserved]);
    let child = crate::spawn_helper::spawn(&command, crate::spawn_helper::Setup::Pipe)
        .await?
        .expect("native helper must preserve requested descriptors");
    assert!(child.wait_with_output().await?.status.success());
    assert_eq!(unsafe { libc::fcntl(preserved, libc::F_GETFD) }, 0);
    assert_eq!(unsafe { libc::fcntl(unwanted, libc::F_GETFD) }, 0);
    Ok(())
}

// Test-only helper synchronization travels as target arguments and inherited
// descriptors. Production dispatch has no environment switches or pause points.
pub(crate) fn pause_handshake(phase: &str) -> io::Result<()> {
    let arguments = process_arguments()?;
    let marker = format!("--codex-spawn-test-pause={phase}");
    let Some(index) = arguments
        .iter()
        .position(|argument| argument == std::ffi::OsStr::new(&marker))
    else {
        return Ok(());
    };
    let fd = arguments
        .get(index + 1)
        .and_then(|argument| argument.to_str())
        .and_then(|argument| argument.parse::<i32>().ok())
        .ok_or_else(|| io::Error::other("invalid handshake notification fd"))?;
    let pid = std::process::id().to_le_bytes();
    assert_eq!(
        unsafe { libc::write(fd, pid.as_ptr().cast(), pid.len()) },
        4
    );
    loop {
        unsafe { libc::pause() };
    }
}

// Keep the inherited notification socket alive inside the future, so the
// cancellation tests exercise the same fd ownership as a real spawn caller.
fn paused_spawn(
    phase: &str,
) -> io::Result<(
    impl std::future::Future<Output = anyhow::Result<crate::SpawnedProcess>>,
    tokio::net::UnixStream,
)> {
    let (notify, helper_notify) = std::os::unix::net::UnixStream::pair()?;
    notify.set_nonblocking(true)?;
    let notify = tokio::net::UnixStream::from_std(notify)?;
    assert_eq!(
        unsafe { libc::fcntl(helper_notify.as_raw_fd(), libc::F_SETFD, 0) },
        0
    );
    let arguments = vec![
        "-c".into(),
        "printf fallback".into(),
        "sh".into(),
        format!("--codex-spawn-test-pause={phase}"),
        helper_notify.as_raw_fd().to_string(),
    ];
    let env = if phase == "environment" {
        // Exceed socket capacity to test cancellation of an environment write.
        HashMap::from([("LARGE".into(), "x".repeat(2 * 1024 * 1024))])
    } else {
        HashMap::new()
    };
    let spawn = async move {
        spawn_pipe_process_no_stdin(
            "/bin/sh",
            &arguments,
            Path::new("/"),
            &env,
            &None,
            &[helper_notify.as_raw_fd()],
        )
        .await
    };
    Ok((spawn, notify))
}

async fn paused_pid(
    spawn: std::pin::Pin<
        &mut impl std::future::Future<Output = anyhow::Result<crate::SpawnedProcess>>,
    >,
    notify: &mut tokio::net::UnixStream,
) -> anyhow::Result<libc::pid_t> {
    use tokio::io::AsyncReadExt;
    let mut pid = [0; 4];
    tokio::time::timeout(Duration::from_secs(5), async {
        tokio::select! {
            biased;
            result = spawn => panic!("helper returned before cancellation: {result:?}"),
            result = notify.read_exact(&mut pid) => result,
        }
    })
    .await??;
    Ok(u32::from_le_bytes(pid) as libc::pid_t)
}

fn is_reaped(pid: libc::pid_t) -> bool {
    let mut info = std::mem::MaybeUninit::<libc::siginfo_t>::zeroed();
    // SAFETY: Observe our child without reaping it or probing an unrelated reused PID.
    if unsafe {
        libc::waitid(
            libc::P_PID,
            pid as libc::id_t,
            info.as_mut_ptr(),
            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
        )
    } == 0
    {
        return false;
    }
    assert_eq!(
        io::Error::last_os_error().raw_os_error(),
        Some(libc::ECHILD)
    );
    true
}

fn wait_for_fallback_reaping(pid: libc::pid_t) -> anyhow::Result<()> {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !is_reaped(pid) {
        anyhow::ensure!(std::time::Instant::now() < deadline, "child was not reaped");
        std::thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}

#[tokio::test]
async fn cancelling_real_helper_handshakes_kills_and_reaps() -> anyhow::Result<()> {
    if !native_helper_available().await? {
        eprintln!("skipping helper handshake test: native spawning is unavailable");
        return Ok(());
    }
    for phase in ["environment", "report", "exec"] {
        let (spawn, mut notify) = paused_spawn(phase)?;
        let mut spawn = Box::pin(spawn);
        let pid = paused_pid(spawn.as_mut(), &mut notify).await?;
        drop(spawn);
        tokio::time::timeout(Duration::from_secs(5), async {
            while !is_reaped(pid) {
                tokio::task::yield_now().await;
            }
        })
        .await?;
    }
    Ok(())
}

#[test]
fn runtime_shutdown_retains_reaping_ownership_without_head_of_line_blocking() -> anyhow::Result<()>
{
    let mut children = Vec::new();
    for _ in 0..2 {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let child = runtime.block_on(async {
            let mut child = spawn_pipe_process_no_stdin(
                "/bin/sh",
                &["-c".into(), "echo $$; exec sleep 60".into()],
                Path::new("/"),
                &HashMap::new(),
                &None,
                &[],
            )
            .await?;
            let output = tokio::time::timeout(Duration::from_secs(5), child.stdout_rx.recv())
                .await?
                .expect("target reports pid");
            let pid = std::str::from_utf8(&output)?
                .trim()
                .parse::<libc::pid_t>()?;
            anyhow::Ok((child, pid))
        })?;
        // Shutdown completes the first child's transfer to the shared reaper
        // before the second can arrive, while both children remain alive.
        drop(runtime);
        children.push(child);
    }
    for index in [1, 0] {
        children[index].0.session.terminate();
        wait_for_fallback_reaping(children[index].1)?;
        if index == 1 {
            assert_eq!(unsafe { libc::kill(children[0].1, 0) }, 0);
        }
    }
    Ok(())
}

#[test]
fn cancelling_startup_after_runtime_shutdown_reaps_helper() -> anyhow::Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    if !runtime.block_on(native_helper_available())? {
        eprintln!("skipping helper handshake test: native spawning is unavailable");
        return Ok(());
    }
    let (spawn, mut notify) = {
        let _entered = runtime.enter();
        paused_spawn("environment")?
    };
    let mut spawn = Box::pin(spawn);
    let pid = runtime.block_on(paused_pid(spawn.as_mut(), &mut notify))?;
    drop(runtime);
    drop(spawn);
    wait_for_fallback_reaping(pid)
}

/// Read argv before Rust's musl startup has initialized std::env::args_os.
fn process_arguments() -> io::Result<Vec<std::ffi::OsString>> {
    use std::os::unix::ffi::OsStringExt;
    let command_line = std::fs::read("/proc/self/cmdline")?;
    Ok(command_line
        .strip_suffix(&[0])
        .unwrap_or(&command_line)
        .split(|byte| *byte == 0)
        .map(|arg| std::ffi::OsString::from_vec(arg.to_vec()))
        .collect())
}

#[tokio::test]
async fn helper_exit_before_acknowledgement_falls_back() -> anyhow::Result<()> {
    if !native_helper_available().await? {
        eprintln!("skipping helper handshake test: native spawning is unavailable");
        return Ok(());
    }
    let (spawn, mut notify) = paused_spawn("report")?;
    let mut spawn = Box::pin(spawn);
    let pid = paused_pid(spawn.as_mut(), &mut notify).await?;
    assert_eq!(unsafe { libc::kill(pid, libc::SIGKILL) }, 0);
    let mut child = tokio::time::timeout(Duration::from_secs(5), spawn).await??;
    let mut stdout = Vec::new();
    while let Some(chunk) = child.stdout_rx.recv().await {
        stdout.extend(chunk);
    }
    assert_eq!((child.exit_rx.await?, stdout), (0, b"fallback".to_vec()));
    wait_for_fallback_reaping(pid)
}

#[tokio::test]
async fn pty_helper_establishes_a_controlling_terminal_without_forking() -> anyhow::Result<()> {
    use std::cell::Cell;
    thread_local! { static FORKS: Cell<usize> = const { Cell::new(0) }; }
    extern "C" fn after_fork() {
        FORKS.with(|count| count.set(count.get() + 1));
    }
    FORKS.with(|count| count.set(0));
    // SAFETY: The parent callback only updates initialized thread-local storage.
    assert_eq!(
        unsafe {
            libc::pthread_atfork(/*prepare*/ None, Some(after_fork), /*child*/ None)
        },
        0
    );
    let mut spawned = crate::spawn_pty_process(
        "/bin/sh",
        &[
            "-c".to_owned(),
            "test -t 0 && test -t 1 && test -t 2 && printf ready >/dev/tty".to_owned(),
        ],
        std::path::Path::new("."),
        &std::env::vars().collect(),
        /*arg0*/ &None,
        crate::TerminalSize::default(),
        crate::ChildFds::Inherited(&[]),
    )
    .await?;
    let mut output = Vec::new();
    while let Some(chunk) = spawned.stdout_rx.recv().await {
        output.extend(chunk);
    }
    assert_eq!((spawned.exit_rx.await?, output), (0, b"ready".to_vec()));
    if native_helper_available().await? {
        assert_eq!(FORKS.with(Cell::get), 0);
    }
    Ok(())
}

#[tokio::test]
async fn pty_helper_preserves_portable_signal_exit_status() -> anyhow::Result<()> {
    let env = std::env::vars().collect();
    for program in ["/bin/sh", "sh"] {
        let spawned = crate::spawn_pty_process(
            program,
            &["-c".to_owned(), "kill -TERM $$".to_owned()],
            std::path::Path::new("."),
            &env,
            /*arg0*/ &None,
            crate::TerminalSize::default(),
            crate::ChildFds::Inherited(&[]),
        )
        .await?;
        assert_eq!(spawned.exit_rx.await?, 1);
    }
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
    let Some(probe) =
        crate::spawn_helper::spawn(&command, crate::spawn_helper::Setup::Pipe).await?
    else {
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
    let result = crate::spawn_helper::spawn(&command, crate::spawn_helper::Setup::Pipe).await;
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
    assert!(
        crate::spawn_helper::spawn(&command, crate::spawn_helper::Setup::Pipe)
            .await?
            .is_none()
    );
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
    let Err(error) = crate::spawn_helper::spawn(&command, crate::spawn_helper::Setup::Pipe).await
    else {
        panic!("helper must reach target exec despite parent loader settings");
    };
    assert_eq!(error.raw_os_error(), Some(libc::ENOENT));
    Ok(())
}
