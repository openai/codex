use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;

use codex_core::error::CodexErr;
use codex_core::error::Result;
use codex_core::error::SandboxErr;
use codex_core::protocol::SandboxPolicy;

use landlock::ABI;
use landlock::Access;
use landlock::AccessFs;
use landlock::CompatLevel;
use landlock::Compatible;
use landlock::Ruleset;
use landlock::RulesetAttr;
use landlock::RulesetCreatedAttr;
use seccompiler::BpfProgram;
use seccompiler::SeccompAction;
use seccompiler::SeccompCmpArgLen;
use seccompiler::SeccompCmpOp;
use seccompiler::SeccompCondition;
use seccompiler::SeccompFilter;
use seccompiler::SeccompRule;
use seccompiler::TargetArch;
use seccompiler::apply_filter;

/// Apply sandbox policies inside this thread so only the child inherits
/// them, not the entire CLI process.
pub(crate) fn apply_sandbox_policy_to_current_thread(
    sandbox_policy: &SandboxPolicy,
    cwd: &Path,
) -> Result<()> {
    if !sandbox_policy.has_full_network_access() {
        install_network_seccomp_filter_on_current_thread()?;
    }

    if !sandbox_policy.has_full_disk_write_access() {
        let writable_roots = sandbox_policy
            .get_writable_roots_with_cwd(cwd)
            .into_iter()
            .map(|writable_root| writable_root.root)
            .collect();
        install_filesystem_landlock_rules_on_current_thread(writable_roots)?;
    }

    // TODO(ragona): Add appropriate restrictions if
    // `sandbox_policy.has_full_disk_read_access()` is `false`.

    Ok(())
}

/// Installs Landlock file-system rules on the current thread allowing read
/// access to the entire file-system while restricting write access to
/// `/dev/null` and the provided list of `writable_roots`.
///
/// # Errors
/// Returns [`CodexErr::Sandbox`] variants when the ruleset fails to apply.
fn install_filesystem_landlock_rules_on_current_thread(writable_roots: Vec<PathBuf>) -> Result<()> {
    let abi = ABI::V5;
    let access_rw = AccessFs::from_all(abi);
    let access_ro = AccessFs::from_read(abi);

    let mut ruleset = Ruleset::default()
        .set_compatibility(CompatLevel::BestEffort)
        .handle_access(access_rw)?
        .create()?
        .add_rules(landlock::path_beneath_rules(&["/"], access_ro))?
        .add_rules(landlock::path_beneath_rules(&["/dev/null"], access_rw))?
        .set_no_new_privs(true);

    if !writable_roots.is_empty() {
        ruleset = ruleset.add_rules(landlock::path_beneath_rules(&writable_roots, access_rw))?;
    }

    let status = ruleset.restrict_self()?;

    if status.ruleset == landlock::RulesetStatus::NotEnforced {
        return Err(CodexErr::Sandbox(SandboxErr::LandlockRestrict));
    }

    Ok(())
}

/// Installs a seccomp filter that blocks outbound network access except for
/// AF_UNIX domain sockets.
fn install_network_seccomp_filter_on_current_thread() -> std::result::Result<(), SandboxErr> {
    // Build rule map.
    let mut rules: BTreeMap<i64, Vec<SeccompRule>> = BTreeMap::new();

    // Helper – insert unconditional deny rule for syscall number.
    let mut deny_syscall = |nr: i64| {
        rules.insert(nr, vec![]); // empty rule vec = unconditional match
    };

    deny_syscall(libc::SYS_connect);
    deny_syscall(libc::SYS_accept);
    deny_syscall(libc::SYS_accept4);
    deny_syscall(libc::SYS_bind);
    deny_syscall(libc::SYS_listen);
    deny_syscall(libc::SYS_getpeername);
    deny_syscall(libc::SYS_getsockname);
    deny_syscall(libc::SYS_shutdown);
    deny_syscall(libc::SYS_sendto);
    deny_syscall(libc::SYS_sendmsg);
    deny_syscall(libc::SYS_sendmmsg);
    // NOTE: allowing recvfrom allows some tools like: `cargo clippy` to run
    // with their socketpair + child processes for sub-proc management
    // deny_syscall(libc::SYS_recvfrom);
    deny_syscall(libc::SYS_recvmsg);
    deny_syscall(libc::SYS_recvmmsg);
    deny_syscall(libc::SYS_getsockopt);
    deny_syscall(libc::SYS_setsockopt);
    deny_syscall(libc::SYS_ptrace);

    // For `socket` we allow AF_UNIX (arg0 == AF_UNIX) and deny everything else.
    let unix_only_rule = SeccompRule::new(vec![SeccompCondition::new(
        0, // first argument (domain)
        SeccompCmpArgLen::Dword,
        SeccompCmpOp::Ne,
        libc::AF_UNIX as u64,
    )?])?;

    rules.insert(libc::SYS_socket, vec![unix_only_rule.clone()]);
    rules.insert(libc::SYS_socketpair, vec![unix_only_rule]); // always deny (Unix can use socketpair but fine, keep open?)

    let filter = SeccompFilter::new(
        rules,
        SeccompAction::Allow,                     // default – allow
        SeccompAction::Errno(libc::EPERM as u32), // when rule matches – return EPERM
        if cfg!(target_arch = "x86_64") {
            TargetArch::x86_64
        } else if cfg!(target_arch = "aarch64") {
            TargetArch::aarch64
        } else {
            unimplemented!("unsupported architecture for seccomp filter");
        },
    )?;

    let prog: BpfProgram = filter.try_into()?;

    apply_filter(&prog)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    #![expect(clippy::expect_used)]
    use super::*;
    use std::io;

    #[test]
    pub fn allow_unix_socketpair_recvfrom() {
        let sandbox_policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            network_access: false,
            // Exclude tmp-related folders from writable roots because we need a
            // folder that is writable by tests but that we intentionally disallow
            // writing to in the sandbox.
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        };
        let cwd = std::env::current_dir().expect("cwd should exist");

        apply_sandbox_policy_to_current_thread(&sandbox_policy, &cwd)
            .expect("Failed to apply sandbox policy");

        // SAFETY: we call libc to create an AF_UNIX datagram socketpair and use
        // it entirely within this function.
        unsafe {
            let mut fds = [0i32; 2];
            let r = libc::socketpair(libc::AF_UNIX, libc::SOCK_DGRAM, 0, fds.as_mut_ptr());
            assert_eq!(
                r,
                0,
                "socketpair(AF_UNIX, SOCK_DGRAM) failed: {}",
                io::Error::last_os_error()
            );

            let msg = b"hello_unix";
            // write() from one end (generic write is allowed)
            let sent = libc::write(fds[0], msg.as_ptr() as *const libc::c_void, msg.len());
            assert!(sent >= 0, "write() failed: {}", io::Error::last_os_error());

            // recvfrom() on the other end. We don’t need the address for socketpair,
            // so we pass null pointers for src address.
            let mut buf = [0u8; 64];
            let recvd = libc::recvfrom(
                fds[1],
                buf.as_mut_ptr() as *mut libc::c_void,
                buf.len(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            assert!(
                recvd >= 0,
                "recvfrom() failed: {}",
                io::Error::last_os_error()
            );

            let recvd_slice = &buf[..(recvd as usize)];
            assert_eq!(
                recvd_slice,
                &msg[..],
                "payload mismatch: sent {} bytes, got {} bytes",
                msg.len(),
                recvd
            );

            // Also exercise AF_UNIX stream socketpair quickly to ensure AF_UNIX in general works.
            let mut sfds = [0i32; 2];
            let sr = libc::socketpair(libc::AF_UNIX, libc::SOCK_STREAM, 0, sfds.as_mut_ptr());
            assert_eq!(
                sr,
                0,
                "socketpair(AF_UNIX, SOCK_STREAM) failed: {}",
                io::Error::last_os_error()
            );
            let snt2 = libc::write(sfds[0], msg.as_ptr() as *const libc::c_void, msg.len());
            assert!(
                snt2 >= 0,
                "write(stream) failed: {}",
                io::Error::last_os_error()
            );
            let mut b2 = [0u8; 64];
            let rcv2 = libc::recv(sfds[1], b2.as_mut_ptr() as *mut libc::c_void, b2.len(), 0);
            assert!(
                rcv2 >= 0,
                "recv(stream) failed: {}",
                io::Error::last_os_error()
            );

            // Clean up
            let _ = libc::close(sfds[0]);
            let _ = libc::close(sfds[1]);
            let _ = libc::close(fds[0]);
            let _ = libc::close(fds[1]);
        }
    }
}
