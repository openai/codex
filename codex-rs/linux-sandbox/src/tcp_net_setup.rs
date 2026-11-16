//! Pass the private network descriptors and mount the command-local resolver. The inner stage
//! takes ownership of all three received descriptors before any workload exec.

use crate::bwrap::BwrapArgs;
use crate::exec_util::make_files_inheritable;
use crate::proxy_lifecycle::move_fd_above_stdio;
use codex_protocol::protocol::FileSystemSandboxPolicy;
use codex_sandboxing::landlock::CODEX_LINUX_SANDBOX_ARG0;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::fs::File;
use std::io;
use std::io::Seek;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Command;

const SETUP_ARG: &str = "--setup-transparent-tcp";

/// Replace the forked supervisor image before creating user-namespace ID maps.
/// Exec drops the inherited attribution token from memory and restores normal
/// dumpability so an unprivileged caller can write its own UID/GID mappings.
pub(super) fn exec_setup(bwrap: &Path, args: &[String]) -> ! {
    let err = Command::new("/proc/self/exe")
        .arg0(CODEX_LINUX_SANDBOX_ARG0)
        .arg(SETUP_ARG)
        .arg(bwrap)
        .args(args)
        .exec();
    panic!("failed to exec transparent TCP setup: {err}")
}

pub(super) fn dispatch() {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(OsStr::new(SETUP_ARG)) {
        return;
    }
    let program = args
        .next()
        .unwrap_or_else(|| panic!("missing system bubblewrap path"));
    let mut args: Vec<OsString> = args.collect();
    let separator = args
        .iter()
        .position(|arg| arg == "--")
        .filter(|separator| separator + 1 < args.len())
        .unwrap_or_else(|| panic!("missing inner sandbox command"));
    let files = crate::tcp_namespace::create()
        .unwrap_or_else(|err| panic!("failed to create private TCP namespace: {err}"));
    let numbers = files
        .each_ref()
        .map(|fd| fd.as_raw_fd().to_string())
        .join(",");
    args.splice(
        separator + 2..separator + 2,
        [
            "--transparent-tcp".into(),
            "--transparent-tcp-fd".into(),
            numbers.into(),
        ],
    );
    make_files_inheritable(&files);
    let err = Command::new(program).arg0("bwrap").args(args).exec();
    panic!("failed to exec bubblewrap in transparent TCP namespace: {err}")
}

pub(super) fn mount_resolver(
    bwrap: &mut BwrapArgs,
    policy: &FileSystemSandboxPolicy,
    cwd: &Path,
) -> io::Result<()> {
    let path = Path::new("/etc/resolv.conf");
    // Bind onto the existing target so ordinary systemd-resolved symlinks work.
    // Reject policies masking either the logical name or the resolved location.
    let target = path.canonicalize()?;
    if !resolver_readable(policy, cwd, path, &target)? || !target.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "transparent TCP requires readable /etc/resolv.conf",
        ));
    }
    let fd = rustix::fs::memfd_create("codex-resolver", rustix::fs::MemfdFlags::CLOEXEC)?;
    let mut resolv = File::from(move_fd_above_stdio(fd)?);
    resolv.write_all(b"nameserver 127.0.0.53\n")?;
    resolv.rewind()?;
    let separator = bwrap
        .args
        .iter()
        .position(|arg| arg == "--")
        .ok_or_else(|| io::Error::other("missing inner command"))?;
    bwrap.args.splice(
        separator..separator,
        [
            "--perms".into(),
            "0444".into(),
            "--ro-bind-data".into(),
            resolv.as_raw_fd().to_string(),
            target
                .to_str()
                .ok_or_else(|| io::Error::other("non-UTF-8 resolver path"))?
                .into(),
        ],
    );
    bwrap.preserved_files.push(resolv);
    Ok(())
}

fn resolver_readable(
    policy: &FileSystemSandboxPolicy,
    cwd: &Path,
    logical: &Path,
    target: &Path,
) -> io::Result<bool> {
    let denied = codex_protocol::permissions::ReadDenyMatcher::try_new_for_local_paths(policy, cwd)
        .map_err(io::Error::other)?;
    Ok([logical, target].into_iter().all(|path| {
        !denied
            .as_ref()
            .is_some_and(|matcher| matcher.is_local_path_read_denied(path))
            && (policy.can_read_local_path_with_cwd(path, cwd)
                || (policy.include_platform_defaults()
                    && crate::bwrap::LINUX_PLATFORM_DEFAULT_READ_ROOTS
                        .iter()
                        .any(|root| path.starts_with(root))))
    }))
}

pub(super) fn receive(fds: Vec<std::os::fd::RawFd>) -> io::Result<[OwnedFd; 3]> {
    let invalid = || io::Error::other("invalid transparent TCP descriptors");
    let fds: [i32; 3] = fds.try_into().map_err(|_| invalid())?;
    for (i, &fd) in fds.iter().enumerate() {
        if fd <= 2 || fds[..i].contains(&fd) {
            return Err(invalid());
        }
        if unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
            return Err(io::Error::last_os_error());
        }
    }
    // SAFETY: the launcher handed off three distinct, live descriptors above stdio.
    Ok(fds.map(|fd| unsafe { OwnedFd::from_raw_fd(fd) }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::protocol::FileSystemAccessMode;
    use codex_protocol::protocol::FileSystemPath;
    use codex_protocol::protocol::FileSystemSandboxEntry;
    use codex_protocol::protocol::FileSystemSpecialPath;

    #[test]
    fn minimal_resolver_mount_respects_explicit_denials_and_symlink_targets() {
        let mut policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry::new(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Minimal,
            },
            FileSystemAccessMode::Read,
        )]);
        let path = Path::new("/etc/resolv.conf");
        let cwd = Path::new("/");
        assert!(resolver_readable(&policy, cwd, path, path).unwrap());
        assert!(!resolver_readable(&policy, cwd, path, Path::new("/private/resolv.conf")).unwrap());
        policy.entries.push(FileSystemSandboxEntry::new(
            codex_utils_absolute_path::AbsolutePathBuf::try_from(path)
                .unwrap()
                .into(),
            FileSystemAccessMode::Deny,
        ));
        assert!(!resolver_readable(&policy, cwd, path, path).unwrap());
    }
}
