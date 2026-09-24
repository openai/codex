//! Pipe and PTY compatibility under inherited-descriptor pressure.

use std::collections::HashMap;
use std::io;
use std::io::Seek;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;
use std::path::Path;

use pretty_assertions::assert_eq;

#[tokio::test]
async fn descriptor_capture_pressure_falls_back_to_direct_spawn() -> anyhow::Result<()> {
    if std::env::var_os("CODEX_TEST_PIPE_FD_PRESSURE").is_none() {
        let output = std::process::Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "pipe::unix_tests::descriptor_capture_pressure_falls_back_to_direct_spawn",
                "--nocapture",
            ])
            .env("CODEX_TEST_PIPE_FD_PRESSURE", "1")
            .output()?;
        assert!(output.status.success(), "{output:?}");
        return Ok(());
    }
    let root = tempfile::tempdir()?;
    let path = root.path().join("input");
    std::fs::write(&path, "preserved")?;
    let mut source = std::fs::File::open(path)?;
    let descriptors = (0..17)
        .map(|_| {
            // SAFETY: fcntl duplicates a live source into a newly owned slot.
            let fd = unsafe { libc::fcntl(source.as_raw_fd(), libc::F_DUPFD, 3) };
            anyhow::ensure!(fd >= 3, "{}", io::Error::last_os_error());
            Ok(unsafe { OwnedFd::from_raw_fd(fd) })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let targets = descriptors[..16]
        .iter()
        .map(AsRawFd::as_raw_fd)
        .collect::<Vec<_>>();
    let excluded = descriptors[16].as_raw_fd();
    let script = format!(
        "for fd in \"$@\"; do test -e /dev/fd/$fd || exit 10; done; \
         test ! -e /dev/fd/{excluded} || exit 11; /bin/cat /dev/fd/$1"
    );
    let mut args = vec!["-c".to_owned(), script, "sh".to_owned()];
    args.extend(targets.iter().map(ToString::to_string));
    let mut limit = std::mem::MaybeUninit::<libc::rlimit>::uninit();
    // SAFETY: getrlimit initializes writable storage.
    assert_eq!(
        unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, limit.as_mut_ptr()) },
        0
    );
    let mut limit = unsafe { limit.assume_init() };
    limit.rlim_cur = limit.rlim_cur.min(128);
    // SAFETY: Only this isolated subprocess's soft descriptor limit changes.
    assert_eq!(unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &limit) }, 0);
    enum Transport {
        Pipe(super::PipeStdinMode),
        Pty,
    }
    for (spare_slots, transport) in [
        (12, Transport::Pipe(super::PipeStdinMode::Piped)),
        (12, Transport::Pipe(super::PipeStdinMode::Null)),
        (12, Transport::Pty),
        (20, Transport::Pipe(super::PipeStdinMode::Piped)),
        (20, Transport::Pipe(super::PipeStdinMode::Null)),
        (20, Transport::Pty),
    ] {
        source.rewind()?;
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
        // Twelve slots reject capture itself; twenty allow all sixteen copies
        // but leave too little room for stdio. Both must preserve the old launch.
        anyhow::ensure!(
            occupied.len() >= spare_slots,
            "not enough descriptors to reserve"
        );
        occupied.truncate(occupied.len() - spare_slots);
        let mut spawned = match transport {
            Transport::Pipe(stdin) => {
                super::spawn_process_with_stdin_mode(
                    std::ffi::OsStr::new("/bin/sh"),
                    &args,
                    Path::new("."),
                    &HashMap::new(),
                    &None,
                    stdin,
                    &targets,
                )
                .await?
            }
            Transport::Pty => {
                crate::spawn_pty_process(
                    "/bin/sh",
                    &args,
                    Path::new("."),
                    &HashMap::new(),
                    &None,
                    crate::TerminalSize::default(),
                    crate::ChildFds::Inherited(&targets),
                )
                .await?
            }
        };
        drop(occupied);
        spawned.session.close_stdin();
        let mut stdout = Vec::new();
        while let Some(bytes) = spawned.stdout_rx.recv().await {
            stdout.extend(bytes);
        }
        let mut stderr = Vec::new();
        while let Some(bytes) = spawned.stderr_rx.recv().await {
            stderr.extend(bytes);
        }
        assert_eq!(
            (spawned.exit_rx.await?, stdout, stderr),
            (0, b"preserved".to_vec(), vec![])
        );
        for fd in &descriptors {
            assert_eq!(unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFD) }, 0);
        }
    }
    Ok(())
}
