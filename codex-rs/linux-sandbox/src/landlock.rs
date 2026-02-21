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
    proxy_routed_network: bool,
) -> Result<()> {
    let network_seccomp_mode = network_seccomp_mode(
        sandbox_policy,
        allow_network_for_proxy,
        proxy_routed_network,
    );

    // `PR_SET_NO_NEW_PRIVS` is required for seccomp, but it also prevents
    // setuid privilege elevation. Many `bwrap` deployments rely on setuid, so
    // we avoid this unless we need seccomp or we are explicitly using the
    // legacy Landlock filesystem pipeline.
    if network_seccomp_mode.is_some()
        || (apply_landlock_fs && !sandbox_policy.has_full_disk_write_access())
    {
        set_no_new_privs()?;
    }

    if let Some(mode) = network_seccomp_mode {
        install_network_seccomp_filter_on_current_thread(mode)?;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NetworkSeccompMode {
    Restricted,
    ProxyRouted,
}

fn should_install_network_seccomp(
    sandbox_policy: &SandboxPolicy,
    allow_network_for_proxy: bool,
) -> bool {
    // Managed-network sessions should remain fail-closed even for policies that
    // would normally grant full network access (for example, DangerFullAccess).
    !sandbox_policy.has_full_network_access() || allow_network_for_proxy
}

fn network_seccomp_mode(
    sandbox_policy: &SandboxPolicy,
    allow_network_for_proxy: bool,
    proxy_routed_network: bool,
) -> Option<NetworkSeccompMode> {
    if !should_install_network_seccomp(sandbox_policy, allow_network_for_proxy) {
        None
    } else if proxy_routed_network {
        Some(NetworkSeccompMode::ProxyRouted)
    } else {
        Some(NetworkSeccompMode::Restricted)
    }
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

/// Installs a seccomp filter for Linux network sandboxing.
///
/// The filter is applied to the current thread so only the sandboxed child
/// inherits it.
fn install_network_seccomp_filter_on_current_thread(
    mode: NetworkSeccompMode,
) -> std::result::Result<(), SandboxErr> {
    fn deny_syscall(rules: &mut BTreeMap<i64, Vec<SeccompRule>>, nr: i64) {
        rules.insert(nr, vec![]); // empty rule vec = unconditional match
    }

    // Build rule map.
    let mut rules: BTreeMap<i64, Vec<SeccompRule>> = BTreeMap::new();

    deny_syscall(&mut rules, libc::SYS_ptrace);
    deny_syscall(&mut rules, libc::SYS_io_uring_setup);
    deny_syscall(&mut rules, libc::SYS_io_uring_enter);
    deny_syscall(&mut rules, libc::SYS_io_uring_register);

    match mode {
        NetworkSeccompMode::Restricted => {
            deny_syscall(&mut rules, libc::SYS_connect);
            deny_syscall(&mut rules, libc::SYS_accept);
            deny_syscall(&mut rules, libc::SYS_accept4);
            deny_syscall(&mut rules, libc::SYS_bind);
            deny_syscall(&mut rules, libc::SYS_listen);
            deny_syscall(&mut rules, libc::SYS_getpeername);
            deny_syscall(&mut rules, libc::SYS_getsockname);
            deny_syscall(&mut rules, libc::SYS_shutdown);
            deny_syscall(&mut rules, libc::SYS_sendto);
            deny_syscall(&mut rules, libc::SYS_sendmmsg);
            // NOTE: allowing recvfrom allows some tools like: `cargo clippy`
            // to run with their socketpair + child processes for sub-proc
            // management.
            // deny_syscall(&mut rules, libc::SYS_recvfrom);
            deny_syscall(&mut rules, libc::SYS_recvmmsg);
            deny_syscall(&mut rules, libc::SYS_getsockopt);
            deny_syscall(&mut rules, libc::SYS_setsockopt);

            // For `socket` we allow AF_UNIX (arg0 == AF_UNIX) and deny
            // everything else.
            let unix_only_rule = SeccompRule::new(vec![SeccompCondition::new(
                0, // first argument (domain)
                SeccompCmpArgLen::Dword,
                SeccompCmpOp::Ne,
                libc::AF_UNIX as u64,
            )?])?;

            rules.insert(libc::SYS_socket, vec![unix_only_rule.clone()]);
            rules.insert(libc::SYS_socketpair, vec![unix_only_rule]);
        }
        NetworkSeccompMode::ProxyRouted => {
            // In proxy-routed mode we allow IP sockets in the isolated
            // namespace (used to reach the local TCP bridge) but deny all
            // other socket families, including AF_UNIX. This prevents
            // bypassing the routed bridge via new Unix sockets and narrows the
            // socket surface in proxy-only mode.
            let deny_non_ip_socket = SeccompRule::new(vec![
                SeccompCondition::new(
                    0,
                    SeccompCmpArgLen::Dword,
                    SeccompCmpOp::Ne,
                    libc::AF_INET as u64,
                )?,
                SeccompCondition::new(
                    0,
                    SeccompCmpArgLen::Dword,
                    SeccompCmpOp::Ne,
                    libc::AF_INET6 as u64,
                )?,
            ])?;
            let deny_unix_socketpair = SeccompRule::new(vec![SeccompCondition::new(
                0,
                SeccompCmpArgLen::Dword,
                SeccompCmpOp::Eq,
                libc::AF_UNIX as u64,
            )?])?;
            rules.insert(libc::SYS_socket, vec![deny_non_ip_socket]);
            rules.insert(libc::SYS_socketpair, vec![deny_unix_socketpair]);
        }
    }

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
    use super::NetworkSeccompMode;
    use super::network_seccomp_mode;
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

    #[test]
    fn managed_proxy_routes_use_proxy_routed_seccomp_mode() {
        assert_eq!(
            network_seccomp_mode(&SandboxPolicy::DangerFullAccess, true, true),
            Some(NetworkSeccompMode::ProxyRouted)
        );
    }

    #[test]
    fn restricted_network_without_proxy_routing_uses_restricted_mode() {
        assert_eq!(
            network_seccomp_mode(&SandboxPolicy::new_read_only_policy(), false, false),
            Some(NetworkSeccompMode::Restricted)
        );
    }

    #[test]
    fn full_network_without_managed_proxy_skips_network_seccomp_mode() {
        assert_eq!(
            network_seccomp_mode(&SandboxPolicy::DangerFullAccess, false, false),
            None
        );
    }
}
