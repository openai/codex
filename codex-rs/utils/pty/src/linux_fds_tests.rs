//! Child-visible descriptor inheritance, fallback compatibility, and launch errors.

use std::io;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;
use std::os::unix::process::CommandExt;
use std::os::unix::process::ExitStatusExt;
use std::process::Command;
use std::process::ExitStatus;
use std::process::Output;
use std::process::Stdio;

use anyhow::Context;
use pretty_assertions::assert_eq;

// Called only from pre_exec: reject one syscall in the child without changing
// the test runner's permissions or allocating memory after fork.
fn deny_syscall(syscall_number: libc::c_long) -> io::Result<()> {
    let mut filter = [
        libc::sock_filter {
            code: (libc::BPF_LD | libc::BPF_W | libc::BPF_ABS) as _,
            jt: 0,
            jf: 0,
            k: std::mem::offset_of!(libc::seccomp_data, nr) as _,
        },
        libc::sock_filter {
            code: (libc::BPF_JMP | libc::BPF_JEQ | libc::BPF_K) as _,
            jt: 0,
            jf: 1,
            k: syscall_number as _,
        },
        libc::sock_filter {
            code: (libc::BPF_RET | libc::BPF_K) as _,
            jt: 0,
            jf: 0,
            k: libc::SECCOMP_RET_ERRNO | libc::EPERM as u32,
        },
        libc::sock_filter {
            code: (libc::BPF_RET | libc::BPF_K) as _,
            jt: 0,
            jf: 0,
            k: libc::SECCOMP_RET_ALLOW,
        },
    ];
    let program = libc::sock_fprog {
        len: filter.len() as _,
        filter: filter.as_mut_ptr(),
    };
    // SAFETY: The kernel copies the stack-owned filter before prctl returns.
    unsafe {
        if libc::prctl(
            libc::PR_SET_NO_NEW_PRIVS,
            1_usize,
            0_usize,
            0_usize,
            0_usize,
        ) == -1
            || libc::prctl(
                libc::PR_SET_SECCOMP,
                libc::SECCOMP_MODE_FILTER as usize,
                &raw const program,
                0_usize,
                0_usize,
            ) == -1
        {
            return Err(io::Error::last_os_error());
        }
        // Qualify failure injection. Invalid arguments make the probe harmless
        // for close_range and getdents64 even if the filter is ineffective.
        if libc::syscall(syscall_number, -1_isize, 0_usize, 0_usize) != -1
            || io::Error::last_os_error().raw_os_error() != Some(libc::EPERM)
        {
            return Err(io::Error::from_raw_os_error(libc::EIO));
        }
    }
    Ok(())
}

fn output_with_input(command: &mut Command) -> io::Result<Output> {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(b"stdio payload\n")?;
    child.wait_with_output()
}

#[test]
fn cleanup_excludes_unrelated_fds_and_preserves_explicit_fds_and_stdio() -> anyhow::Result<()> {
    let python = crate::tests::find_python().context("descriptor tests require Python")?;
    let file = tempfile::tempfile()?;
    // Model the occupied descriptor table of a parallel test runner.
    let _occupied: Vec<_> = (0..64)
        .map(|_| file.try_clone())
        .collect::<io::Result<_>>()?;
    // SAFETY: Reserve a low descriptor without replacing another owner.
    let raw = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 3) };
    anyhow::ensure!(raw != -1, "{}", io::Error::last_os_error());
    // SAFETY: fcntl returned a new descriptor owned by this test.
    let reservation = unsafe { OwnedFd::from_raw_fd(raw) };
    let reserved_fd = reservation.as_raw_fd();
    // Keep every sentinel above the limit even when the parent's low slots fill.
    let minimum = (reserved_fd + 1).max(/*other*/ 200);
    let mut descriptors = Vec::new();
    for operation in [libc::F_DUPFD, libc::F_DUPFD, libc::F_DUPFD_CLOEXEC] {
        // SAFETY: fcntl reserves a new high descriptor without replacing any existing owner.
        let fd = unsafe { libc::fcntl(file.as_raw_fd(), operation, minimum) };
        anyhow::ensure!(fd != -1, "{}", io::Error::last_os_error());
        // SAFETY: The duplicate is newly owned by this test.
        descriptors.push(unsafe { OwnedFd::from_raw_fd(fd) });
    }
    let fds: Vec<_> = descriptors.iter().map(AsRawFd::as_raw_fd).collect();
    let command = || {
        let mut command = Command::new(&python);
        command
            .arg("-c")
            .arg(
                r#"import errno, os, sys
inherited = []
for fd in map(int, sys.argv[1:]):
    try:
        os.fstat(fd)
        inherited.append(True)
    except OSError as error:
        if error.errno != errno.EBADF: raise
        inherited.append(False)
print(inherited)
print(sys.stdin.read(), end="")
print("stderr preserved", file=sys.stderr)
"#,
            )
            .args(fds.iter().map(ToString::to_string));
        command
    };
    let expected = |inherited| Output {
        status: ExitStatus::from_raw(/*raw*/ 0),
        stdout: format!("{inherited}\nstdio payload\n").into_bytes(),
        stderr: b"stderr preserved\n".to_vec(),
    };
    // Qualify the oracle: the executable itself must not sanitize inherited FDs.
    assert_eq!(
        output_with_input(&mut command())?,
        expected("[True, True, False]")
    );
    for (name, force_fallback) in [("default", false), ("fallback above fd limit", true)] {
        for preserved in [&[][..], &[fds[2], fds[1], fds[2]][..]] {
            // Unsorted and repeated entries must preserve the same inheritance.
            let allowlist = preserved.to_vec();
            let mut command = command();
            // SAFETY: The hook uses only preallocated data and fork-safe syscalls.
            unsafe {
                command.pre_exec(move || {
                    if force_fallback {
                        deny_syscall(libc::SYS_close_range)?;
                        // Leave a free slot for procfs below the lowered limit,
                        // even if parallel tests filled every other low slot.
                        let mut limit = libc::rlimit {
                            rlim_cur: 0,
                            rlim_max: 0,
                        };
                        if libc::close(reserved_fd) == -1
                            || libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) == -1
                        {
                            return Err(io::Error::last_os_error());
                        }
                        limit.rlim_cur = reserved_fd as libc::rlim_t + 1;
                        if libc::setrlimit(libc::RLIMIT_NOFILE, &limit) == -1 {
                            return Err(io::Error::last_os_error());
                        }
                    }
                    crate::pty::close_inherited_fds_except(&allowlist);
                    Ok(())
                });
            }
            let inheritance = if preserved.is_empty() {
                "[False, False, False]"
            } else {
                "[False, True, False]"
            };
            assert_eq!(
                output_with_input(&mut command)
                    .with_context(|| format!("{name}, allowlist {preserved:?}"))?,
                expected(inheritance),
                "{name}, allowlist {preserved:?}"
            );
        }
    }
    // The child must not mutate the parent's descriptors or inheritance flags.
    let flags: Vec<_> = fds
        .iter()
        .map(|&fd| unsafe { libc::fcntl(fd, libc::F_GETFD) })
        .collect();
    assert_eq!(flags, [0, 0, libc::FD_CLOEXEC]);
    Ok(())
}

#[test]
fn cleanup_fallback_preserves_exec_failure_reporting() -> anyhow::Result<()> {
    let temporary = tempfile::tempdir()?;
    let mut command = Command::new(temporary.path().join("does-not-exist"));
    // SAFETY: The hook uses only stack storage and fork-safe syscalls in the child.
    unsafe {
        command.pre_exec(|| {
            deny_syscall(libc::SYS_close_range)?;
            crate::pty::close_inherited_fds_except(&[]);
            Ok(())
        });
    }
    let error = match command.spawn() {
        Err(error) => error,
        Ok(mut child) => {
            child.wait()?;
            anyhow::bail!("exec failure reported as success");
        }
    };
    assert_eq!(error.raw_os_error(), Some(libc::ENOENT));
    Ok(())
}

#[tokio::test]
async fn cleanup_failures_do_not_prevent_explicit_launch() -> anyhow::Result<()> {
    let mut command = crate::Command::new("/bin/sh");
    command
        .args(["-c", "exit 42"])
        .descriptor_policy(crate::DescriptorPolicy::Explicit);
    // SAFETY: The hook only installs child-local syscall filters using stack
    // storage. Command::spawn registers its real cleanup hook after this hook.
    unsafe {
        command.inner.pre_exec(|| {
            deny_syscall(libc::SYS_close_range)?;
            deny_syscall(libc::SYS_getdents64)
        });
    }
    assert_eq!(
        command.spawn()?.wait().await?,
        ExitStatus::from_raw(/*raw*/ 42 << 8)
    );
    Ok(())
}
