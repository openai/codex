//! In-process Linux sandbox primitives: `no_new_privs` and seccomp.
//!
//! Filesystem restrictions are enforced by bubblewrap in `linux_run_main`.
//! Landlock helpers remain available here as legacy/backup utilities.
use std::collections::BTreeMap;
use std::path::Path;

use codex_core::error::CodexErr;
use codex_core::error::Result;
use codex_core::error::SandboxErr;
use codex_protocol::protocol::SandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;

use landlock::ABI;
#[allow(unused_imports)]
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
///
/// This function is responsible for:
/// - enabling `PR_SET_NO_NEW_PRIVS` when restrictions apply, and
/// - installing the network seccomp filter when network access is disabled.
///
/// Filesystem restrictions are intentionally handled by bubblewrap.
pub(crate) fn apply_sandbox_policy_to_current_thread(
    sandbox_policy: &SandboxPolicy,
    cwd: &Path,
    apply_landlock_fs: bool,
    allow_network_for_proxy: bool,
) -> Result<()> {
    let install_network_seccomp =
        should_install_network_seccomp(sandbox_policy, allow_network_for_proxy);

    // `PR_SET_NO_NEW_PRIVS` is required for seccomp, but it also prevents
    // setuid privilege elevation. Many `bwrap` deployments rely on setuid, so
    // we avoid this unless we need seccomp or we are explicitly using the
    // legacy Landlock filesystem pipeline.
    if install_network_seccomp
        || (apply_landlock_fs && !sandbox_policy.has_full_disk_write_access())
    {
        set_no_new_privs()?;
    }

    if install_network_seccomp {
        install_network_seccomp_filter_on_current_thread()?;
    }

    if apply_landlock_fs && !sandbox_policy.has_full_disk_write_access() {
        if !sandbox_policy.has_full_disk_read_access() {
            return Err(CodexErr::UnsupportedOperation(
                "Restricted read-only access is not supported by the legacy Linux Landlock filesystem backend."
                    .to_string(),
            ));
        }

        let writable_roots = sandbox_policy
            .get_writable_roots_with_cwd(cwd)
            .into_iter()
            .map(|writable_root| writable_root.root)
            .collect();
        install_filesystem_landlock_rules_on_current_thread(writable_roots)?;
    }

    Ok(())
}

fn should_install_network_seccomp(
    sandbox_policy: &SandboxPolicy,
    allow_network_for_proxy: bool,
) -> bool {
    // Managed-network sessions should remain fail-closed even for policies that
    // would normally grant full network access (for example, DangerFullAccess).
    !sandbox_policy.has_full_network_access() || allow_network_for_proxy
}

/// Enable `PR_SET_NO_NEW_PRIVS` so seccomp can be applied safely.
fn set_no_new_privs() -> Result<()> {
    let result = unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) };
    if result != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

/// Installs Landlock file-system rules on the current thread allowing read
/// access to the entire file-system while restricting write access to
/// `/dev/null` and the provided list of `writable_roots`.
///
/// # Errors
/// Returns [`CodexErr::Sandbox`] variants when the ruleset fails to apply.
///
/// Note: this is currently unused because filesystem sandboxing is performed
/// via bubblewrap. It is kept for reference and potential fallback use.
fn install_filesystem_landlock_rules_on_current_thread(
    writable_roots: Vec<AbsolutePathBuf>,
) -> Result<()> {
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
///
/// The filter is applied to the current thread so only the sandboxed child
/// inherits it.
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
    deny_syscall(libc::SYS_sendmmsg);
    // NOTE: allowing recvfrom allows some tools like: `cargo clippy` to run
    // with their socketpair + child processes for sub-proc management
    // deny_syscall(libc::SYS_recvfrom);
    deny_syscall(libc::SYS_recvmmsg);
    deny_syscall(libc::SYS_getsockopt);
    deny_syscall(libc::SYS_setsockopt);
    deny_syscall(libc::SYS_ptrace);
    deny_syscall(libc::SYS_io_uring_setup);
    deny_syscall(libc::SYS_io_uring_enter);
    deny_syscall(libc::SYS_io_uring_register);

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
    use super::should_install_network_seccomp;
    use codex_protocol::protocol::SandboxPolicy;
    use pretty_assertions::assert_eq;

    #[test]
    fn managed_network_enforces_seccomp_even_for_full_network_policy() {
        assert_eq!(
            should_install_network_seccomp(&SandboxPolicy::DangerFullAccess, true),
            true
        );
    }

    #[test]
    fn full_network_policy_without_managed_network_skips_seccomp() {
        assert_eq!(
            should_install_network_seccomp(&SandboxPolicy::DangerFullAccess, false),
            false
        );
    }

    #[test]
    fn restricted_network_policy_always_installs_seccomp() {
        assert!(should_install_network_seccomp(
            &SandboxPolicy::new_read_only_policy(),
            false
        ));
        assert!(should_install_network_seccomp(
            &SandboxPolicy::new_read_only_policy(),
            true
        ));
    }
}
