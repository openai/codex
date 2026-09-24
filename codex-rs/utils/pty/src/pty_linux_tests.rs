//! Portable PTY fallback must release the unsuccessful native launch's resources.

use std::collections::HashMap;
use std::io;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;
use std::path::Path;

use pretty_assertions::assert_eq;

#[tokio::test]
async fn portable_fallback_releases_native_pty_descriptors() -> anyhow::Result<()> {
    if std::env::var_os("CODEX_TEST_PTY_FALLBACK_PRESSURE").is_none() {
        let output = std::process::Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "pty::linux_tests::portable_fallback_releases_native_pty_descriptors",
                "--nocapture",
            ])
            .env("CODEX_TEST_PTY_FALLBACK_PRESSURE", "1")
            .output()?;
        assert!(output.status.success(), "{output:?}");
        return Ok(());
    }
    let source = std::fs::File::open("/dev/null")?;
    let mut limit = std::mem::MaybeUninit::<libc::rlimit>::uninit();
    // SAFETY: This isolated subprocess owns its descriptor limit and reservations.
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
    // The initial PTY fits, but helper setup cannot. The portable replacement
    // fits only after the initial PTY and its stdio copies have been closed.
    anyhow::ensure!(occupied.len() >= 10, "not enough descriptors to reserve");
    occupied.truncate(occupied.len() - 10);
    let mut child = crate::spawn_pty_process(
        "/bin/sh",
        &["-c".to_owned(), "printf fallback".to_owned()],
        Path::new("/"),
        &HashMap::from([("SHELL".to_owned(), "/bin/sh".to_owned())]),
        &None,
        crate::TerminalSize::default(),
        crate::ChildFds::Inherited(&[]),
    )
    .await?;
    drop(occupied);
    let mut output = Vec::new();
    while let Some(bytes) = child.stdout_rx.recv().await {
        output.extend(bytes);
    }
    assert_eq!((child.exit_rx.await?, output), (0, b"fallback".to_vec()));
    Ok(())
}
