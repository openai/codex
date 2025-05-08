use std::collections::BTreeMap;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use crate::error::CodexErr;
use crate::error::Result;
use crate::error::SandboxErr;
use crate::exec::ExecParams;
use crate::exec::RawExecToolCallOutput;
use crate::exec::exec;
use crate::protocol::SandboxPolicy;

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
use tokio::sync::Notify;

pub async fn exec_linux(
    params: ExecParams,
    ctrl_c: Arc<Notify>,
    sandbox_policy: &SandboxPolicy,
) -> Result<RawExecToolCallOutput> {
    // Allow READ on /
    // Allow WRITE on /dev/null
    let ctrl_c_copy = ctrl_c.clone();
    let sandbox_policy = sandbox_policy.clone();

    // Isolate thread to run the sandbox from
    let tool_call_output = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to create runtime");

        rt.block_on(async {
            apply_sandbox_policy_to_current_thread(sandbox_policy, &params.cwd)?;
            exec(params, ctrl_c_copy).await
        })
    })
    .join();

    match tool_call_output {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(e)) => Err(e),
        Err(e) => Err(CodexErr::Io(io::Error::new(
            io::ErrorKind::Other,
            format!("thread join failed: {e:?}"),
        ))),
    }
}

/// Apply sandbox policies inside this thread so only the child inherits
/// them, not the entire CLI process.
pub fn apply_sandbox_policy_to_current_thread(
    sandbox_policy: SandboxPolicy,
    cwd: &Path,
) -> Result<()> {
    if !sandbox_policy.has_full_network_access() {
        install_network_seccomp_filter_on_current_thread()?;
    }

    if !sandbox_policy.has_full_disk_write_access() {
        let writable_roots = sandbox_policy.get_writable_roots_with_cwd(cwd);
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
    deny_syscall(libc::SYS_recvfrom);
    deny_syscall(libc::SYS_recvmsg);
    deny_syscall(libc::SYS_recvmmsg);
    deny_syscall(libc::SYS_getsockopt);
    deny_syscall(libc::SYS_setsockopt);
    deny_syscall(libc::SYS_ptrace);

    // For `socket` we allow AF_UNIX (arg0 == AF_UNIX) and deny everything else.
    let unix_only_rule = SeccompRule::new(vec![SeccompCondition::new(
        0, // first argument (domain)
        SeccompCmpArgLen::Dword,
        SeccompCmpOp::Eq,
        libc::AF_UNIX as u64,
    )?])?;

    rules.insert(libc::SYS_socket, vec![unix_only_rule]);
    rules.insert(libc::SYS_socketpair, vec![]); // always deny (Unix can use socketpair but fine, keep open?)

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
mod tests_linux {
    use super::*;
    use crate::exec::ExecParams;
    use crate::exec::SandboxType;
    use crate::exec::process_exec_tool_call;
    use crate::protocol::SandboxPolicy;
    use std::sync::Arc;
    use tempfile::NamedTempFile;
    use tokio::sync::Notify;

    #[allow(clippy::print_stdout)]
    async fn run_cmd(cmd: &[&str], writable_roots: &[PathBuf], timeout_ms: u64) {
        let params = ExecParams {
            command: cmd.iter().map(|elm| elm.to_string()).collect(),
            cwd: std::env::current_dir().expect("cwd should exist"),
            timeout_ms: Some(timeout_ms),
        };

        let sandbox_policy =
            SandboxPolicy::new_read_only_policy_with_writable_roots(writable_roots);
        let ctrl_c = Arc::new(Notify::new());
        let res =
            process_exec_tool_call(params, SandboxType::LinuxSeccomp, ctrl_c, &sandbox_policy)
                .await
                .unwrap();

        if res.exit_code != 0 {
            println!("stdout:\n{}", res.stdout);
            println!("stderr:\n{}", res.stderr);
            panic!("exit code: {}", res.exit_code);
        }
    }

    #[tokio::test]
    async fn test_root_read() {
        run_cmd(&["ls", "-l", "/bin"], &[], 200).await;
    }

    #[tokio::test]
    #[should_panic]
    async fn test_root_write() {
        let tmpfile = NamedTempFile::new().unwrap();
        let tmpfile_path = tmpfile.path().to_string_lossy();
        run_cmd(
            &["bash", "-lc", &format!("echo blah > {}", tmpfile_path)],
            &[],
            200,
        )
        .await;
    }

    #[tokio::test]
    async fn test_dev_null_write() {
        run_cmd(&["echo", "blah", ">", "/dev/null"], &[], 200).await;
    }

    #[tokio::test]
    async fn test_writable_root() {
        let tmpdir = tempfile::tempdir().unwrap();
        let file_path = tmpdir.path().join("test");
        run_cmd(
            &[
                "bash",
                "-lc",
                &format!("echo blah > {}", file_path.to_string_lossy()),
            ],
            &[tmpdir.path().to_path_buf()],
            // We have seen timeouts when running this test in CI on GitHub,
            // so we are using a generous timeout until we can diagnose further.
            1_000,
        )
        .await;
    }

    #[tokio::test]
    #[should_panic(expected = "Sandbox(Timeout)")]
    async fn test_timeout() {
        run_cmd(&["sleep", "2"], &[], 50).await;
    }

    /// Helper that runs `cmd` under the Linux sandbox and asserts that the command
    /// does NOT succeed (i.e. returns a non‑zero exit code) **unless** the binary
    /// is missing in which case we silently treat it as an accepted skip so the
    /// suite remains green on leaner CI images.
    async fn assert_network_blocked(cmd: &[&str]) {
        let params = ExecParams {
            command: cmd.iter().map(|s| s.to_string()).collect(),
            cwd: std::env::current_dir().expect("cwd should exist"),
            // Give the tool a generous 2‑second timeout so even slow DNS timeouts
            // do not stall the suite.
            timeout_ms: Some(2_000),
        };

        let sandbox_policy = SandboxPolicy::new_read_only_policy();
        let ctrl_c = Arc::new(Notify::new());
        let result =
            process_exec_tool_call(params, SandboxType::LinuxSeccomp, ctrl_c, &sandbox_policy)
                .await;

        let (exit_code, stdout, stderr) = match result {
            Ok(output) => (output.exit_code, output.stdout, output.stderr),
            Err(CodexErr::Sandbox(SandboxErr::Denied(exit_code, stdout, stderr))) => {
                (exit_code, stdout, stderr)
            }
            _ => {
                panic!("expected sandbox denied error, got: {:?}", result);
            }
        };

        dbg!(&stderr);
        dbg!(&stdout);
        dbg!(&exit_code);

        // A completely missing binary exits with 127.  Anything else should also
        // be non‑zero (EPERM from seccomp will usually bubble up as 1, 2, 13…)
        // If—*and only if*—the command exits 0 we consider the sandbox breached.

        if exit_code == 0 {
            panic!(
                "Network sandbox FAILED - {:?} exited 0\nstdout:\n{}\nstderr:\n{}",
                cmd, stdout, stderr
            );
        }
    }

    #[tokio::test]
    async fn sandbox_blocks_curl() {
        assert_network_blocked(&["curl", "-I", "http://openai.com"]).await;
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn sandbox_blocks_wget() {
        assert_network_blocked(&["wget", "-qO-", "http://openai.com"]).await;
    }

    #[tokio::test]
    async fn sandbox_blocks_ping() {
        // ICMP requires raw socket – should be denied quickly with EPERM.
        assert_network_blocked(&["ping", "-c", "1", "8.8.8.8"]).await;
    }

    #[tokio::test]
    async fn sandbox_blocks_nc() {
        // Zero‑length connection attempt to localhost.
        assert_network_blocked(&["nc", "-z", "127.0.0.1", "80"]).await;
    }

    #[tokio::test]
    async fn sandbox_blocks_ssh() {
        // Force ssh to attempt a real TCP connection but fail quickly.  `BatchMode`
        // avoids password prompts, and `ConnectTimeout` keeps the hang time low.
        assert_network_blocked(&[
            "ssh",
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=1",
            "github.com",
        ])
        .await;
    }

    #[tokio::test]
    async fn sandbox_blocks_getent() {
        assert_network_blocked(&["getent", "ahosts", "openai.com"]).await;
    }

    #[tokio::test]
    async fn sandbox_blocks_dev_tcp_redirection() {
        // This syntax is only supported by bash and zsh. We try bash first.
        // Fallback generic socket attempt using /bin/sh with bash‑style /dev/tcp.  Not
        // all images ship bash, so we guard against 127 as well.
        assert_network_blocked(&["bash", "-c", "echo hi > /dev/tcp/127.0.0.1/80"]).await;
    }
}
