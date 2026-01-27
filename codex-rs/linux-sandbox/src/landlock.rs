use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Once;

use codex_core::error::CodexErr;
use codex_core::error::Result;
use codex_core::error::SandboxErr;
use codex_core::protocol::SandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;

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
    if !sandbox_policy.has_full_disk_write_access() || !sandbox_policy.has_full_network_access() {
        set_no_new_privs()?;
    }

    if !sandbox_policy.has_full_network_access() {
        install_network_seccomp_filter_on_current_thread()?;
    }

    if !sandbox_policy.has_full_disk_write_access() {
        let writable_roots = sandbox_policy
            .get_writable_roots_with_cwd(cwd)
            .into_iter()
            .map(|writable_root| writable_root.root)
            .collect();
        let allow_unenforced = allow_unenforced_landlock(sandbox_policy);
        let landlock_status = install_filesystem_landlock_rules_on_current_thread(writable_roots)?;
        ensure_landlock_enforced(&landlock_status, allow_unenforced)?;
        if landlock_status == landlock::RulesetStatus::NotEnforced {
            warn_unenforced_landlock();
        }
    }

    // TODO(ragona): Add appropriate restrictions if
    // `sandbox_policy.has_full_disk_read_access()` is `false`.

    Ok(())
}

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
/// Returns the Landlock enforcement status for the process.
///
/// # Errors
/// Returns [`CodexErr`] variants when the ruleset fails to apply.
fn install_filesystem_landlock_rules_on_current_thread(
    writable_roots: Vec<AbsolutePathBuf>,
) -> Result<landlock::RulesetStatus> {
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

    Ok(status.ruleset)
}

fn allow_unenforced_landlock(sandbox_policy: &SandboxPolicy) -> bool {
    matches!(
        sandbox_policy,
        SandboxPolicy::WorkspaceWrite {
            network_access: false,
            ..
        }
    )
}

#[allow(clippy::print_stderr)]
fn warn_unenforced_landlock() {
    static WARNED: Once = Once::new();
    WARNED.call_once(|| {
        eprintln!(
            "codex-linux-sandbox: warning: Landlock is not enforced; filesystem sandbox disabled (network remains restricted)"
        );
    });
}

fn ensure_landlock_enforced(
    status: &landlock::RulesetStatus,
    allow_unenforced: bool,
) -> Result<()> {
    if *status == landlock::RulesetStatus::NotEnforced && !allow_unenforced {
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
    deny_syscall(libc::SYS_sendmmsg);
    // NOTE: allowing recvfrom allows some tools like: `cargo clippy` to run
    // with their socketpair + child processes for sub-proc management
    // deny_syscall(libc::SYS_recvfrom);
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
    use super::*;

    #[test]
    fn allows_unenforced_landlock_for_workspace_write_without_network_access() {
        assert!(allow_unenforced_landlock(&SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            network_access: false,
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
        }));
    }

    #[test]
    fn rejects_unenforced_landlock_for_workspace_write_with_network_access() {
        assert!(!allow_unenforced_landlock(&SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            network_access: true,
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
        }));
    }

    #[test]
    fn rejects_unenforced_landlock_for_read_only() {
        let result = ensure_landlock_enforced(&landlock::RulesetStatus::NotEnforced, false);
        assert!(matches!(
            result,
            Err(CodexErr::Sandbox(SandboxErr::LandlockRestrict))
        ));
    }

    #[test]
    fn allows_unenforced_landlock_when_allowed() {
        ensure_landlock_enforced(&landlock::RulesetStatus::NotEnforced, true).unwrap();
    }

    #[test]
    fn allows_partially_enforced_landlock() {
        ensure_landlock_enforced(&landlock::RulesetStatus::PartiallyEnforced, false).unwrap();
    }

    #[test]
    fn allows_fully_enforced_landlock() {
        ensure_landlock_enforced(&landlock::RulesetStatus::FullyEnforced, false).unwrap();
    }
}
