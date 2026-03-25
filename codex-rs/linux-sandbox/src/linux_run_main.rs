use clap::Parser;
use std::ffi::CString;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::os::fd::FromRawFd;
use std::path::Path;
use std::path::PathBuf;

use crate::bwrap::BwrapNetworkMode;
use crate::bwrap::BwrapOptions;
use crate::bwrap::create_bwrap_command_args;
use crate::landlock::apply_sandbox_policy_to_current_thread;
use crate::launcher::exec_bwrap;
use crate::launcher::preferred_bwrap_supports_argv0;
use crate::proxy_routing::activate_proxy_routes_in_netns;
use crate::proxy_routing::prepare_host_proxy_route_spec;
use codex_protocol::protocol::FileSystemSandboxPolicy;
use codex_protocol::protocol::NetworkSandboxPolicy;
use codex_protocol::protocol::SandboxPolicy;

#[derive(Debug, Parser)]
/// CLI surface for the Linux sandbox helper.
///
/// The type name remains `LandlockCommand` for compatibility with existing
/// wiring, but bubblewrap is now the default filesystem sandbox and Landlock
/// is the legacy fallback.
pub struct LandlockCommand {
    /// It is possible that the cwd used in the context of the sandbox policy
    /// is different from the cwd of the process to spawn.
    #[arg(long = "sandbox-policy-cwd")]
    pub sandbox_policy_cwd: PathBuf,

    /// The logical working directory for the command being sandboxed.
    ///
    /// This can intentionally differ from `sandbox_policy_cwd` when the
    /// command runs from a symlinked alias of the policy workspace. Keep it
    /// explicit so bubblewrap can preserve the caller's logical cwd when that
    /// alias would otherwise disappear inside the sandbox namespace.
    #[arg(long = "command-cwd", hide = true)]
    pub command_cwd: Option<PathBuf>,

    /// Legacy compatibility policy.
    ///
    /// Newer callers pass split filesystem/network policies as well so the
    /// helper can migrate incrementally without breaking older invocations.
    #[arg(long = "sandbox-policy", hide = true)]
    pub sandbox_policy: Option<SandboxPolicy>,

    #[arg(long = "file-system-sandbox-policy", hide = true)]
    pub file_system_sandbox_policy: Option<FileSystemSandboxPolicy>,

    #[arg(long = "network-sandbox-policy", hide = true)]
    pub network_sandbox_policy: Option<NetworkSandboxPolicy>,

    /// Opt-in: use the legacy Landlock Linux sandbox fallback.
    ///
    /// When not set, the helper uses the default bubblewrap pipeline.
    #[arg(long = "use-legacy-landlock", hide = true, default_value_t = false)]
    pub use_legacy_landlock: bool,

    /// Internal: apply seccomp and `no_new_privs` in the already-sandboxed
    /// process, then exec the user command.
    ///
    /// This exists so we can run bubblewrap first (which may rely on setuid)
    /// and only tighten with seccomp after the filesystem view is established.
    #[arg(long = "apply-seccomp-then-exec", hide = true, default_value_t = false)]
    pub apply_seccomp_then_exec: bool,

    /// Internal compatibility flag.
    ///
    /// By default, restricted-network sandboxing uses isolated networking.
    /// If set, sandbox setup switches to proxy-only network mode with
    /// managed routing bridges.
    #[arg(long = "allow-network-for-proxy", hide = true, default_value_t = false)]
    pub allow_network_for_proxy: bool,

    /// Internal route spec used for managed proxy routing in bwrap mode.
    #[arg(long = "proxy-route-spec", hide = true)]
    pub proxy_route_spec: Option<String>,

    /// When set, skip mounting a fresh `/proc` even though PID isolation is
    /// still enabled. This is primarily intended for restrictive container
    /// environments that deny `--proc /proc`.
    #[arg(long = "no-proc", default_value_t = false)]
    pub no_proc: bool,

    /// Full command args to run under the Linux sandbox helper.
    #[arg(trailing_var_arg = true)]
    pub command: Vec<String>,
}

/// Entry point for the Linux sandbox helper.
///
/// The sequence is:
/// 1. When needed, wrap the command with bubblewrap to construct the
///    filesystem view.
/// 2. Apply in-process restrictions (no_new_privs + seccomp).
/// 3. `execvp` into the final command.
pub fn run_main() -> ! {
    let LandlockCommand {
        sandbox_policy_cwd,
        command_cwd,
        sandbox_policy,
        file_system_sandbox_policy,
        network_sandbox_policy,
        use_legacy_landlock,
        apply_seccomp_then_exec,
        allow_network_for_proxy,
        proxy_route_spec,
        no_proc,
        command,
    } = LandlockCommand::parse();

    if command.is_empty() {
        panic!("No command specified to execute.");
    }
    ensure_inner_stage_mode_is_valid(apply_seccomp_then_exec, use_legacy_landlock);
    let EffectiveSandboxPolicies {
        sandbox_policy,
        file_system_sandbox_policy,
        network_sandbox_policy,
    } = resolve_sandbox_policies(
        sandbox_policy_cwd.as_path(),
        sandbox_policy,
        file_system_sandbox_policy,
        network_sandbox_policy,
    )
    .unwrap_or_else(|err| panic!("{err}"));
    ensure_legacy_landlock_mode_supports_policy(
        use_legacy_landlock,
        &file_system_sandbox_policy,
        network_sandbox_policy,
        &sandbox_policy_cwd,
    );

    // Inner stage: apply seccomp/no_new_privs after bubblewrap has already
    // established the filesystem view.
    if apply_seccomp_then_exec {
        if allow_network_for_proxy {
            let spec = proxy_route_spec
                .as_deref()
                .unwrap_or_else(|| panic!("managed proxy mode requires --proxy-route-spec"));
            if let Err(err) = activate_proxy_routes_in_netns(spec) {
                panic!("error activating Linux proxy routing bridge: {err}");
            }
        }
        let proxy_routing_active = allow_network_for_proxy;
        if let Err(e) = apply_sandbox_policy_to_current_thread(
            &sandbox_policy,
            network_sandbox_policy,
            &sandbox_policy_cwd,
            /*apply_landlock_fs*/ false,
            allow_network_for_proxy,
            proxy_routing_active,
        ) {
            panic!("error applying Linux sandbox restrictions: {e:?}");
        }
        exec_or_panic(command);
    }

    if file_system_sandbox_policy.has_full_disk_write_access() && !allow_network_for_proxy {
        if let Err(e) = apply_sandbox_policy_to_current_thread(
            &sandbox_policy,
            network_sandbox_policy,
            &sandbox_policy_cwd,
            /*apply_landlock_fs*/ false,
            allow_network_for_proxy,
            /*proxy_routed_network*/ false,
        ) {
            panic!("error applying Linux sandbox restrictions: {e:?}");
        }
        exec_or_panic(command);
    }

    if !use_legacy_landlock {
        // Outer stage: bubblewrap first, then re-enter this binary in the
        // sandboxed environment to apply seccomp. This path never falls back
        // to legacy Landlock on failure.
        let proxy_route_spec =
            if allow_network_for_proxy {
                Some(prepare_host_proxy_route_spec().unwrap_or_else(|err| {
                    panic!("failed to prepare host proxy routing bridge: {err}")
                }))
            } else {
                None
            };
        let inner = build_inner_seccomp_command(InnerSeccompCommandArgs {
            sandbox_policy_cwd: &sandbox_policy_cwd,
            command_cwd: command_cwd.as_deref(),
            sandbox_policy: &sandbox_policy,
            file_system_sandbox_policy: &file_system_sandbox_policy,
            network_sandbox_policy,
            allow_network_for_proxy,
            proxy_route_spec,
            command,
        });
        run_bwrap_with_proc_fallback(
            &sandbox_policy_cwd,
            command_cwd.as_deref(),
            &file_system_sandbox_policy,
            network_sandbox_policy,
            inner,
            !no_proc,
            allow_network_for_proxy,
        );
    }

    // Legacy path: Landlock enforcement only, when bwrap sandboxing is not enabled.
    if let Err(e) = apply_sandbox_policy_to_current_thread(
        &sandbox_policy,
        network_sandbox_policy,
        &sandbox_policy_cwd,
        /*apply_landlock_fs*/ true,
        allow_network_for_proxy,
        /*proxy_routed_network*/ false,
    ) {
        panic!("error applying legacy Linux sandbox restrictions: {e:?}");
    }
    exec_or_panic(command);
}

#[derive(Debug, Clone)]
struct EffectiveSandboxPolicies {
    sandbox_policy: SandboxPolicy,
    file_system_sandbox_policy: FileSystemSandboxPolicy,
    network_sandbox_policy: NetworkSandboxPolicy,
}

#[derive(Debug, PartialEq, Eq)]
enum ResolveSandboxPoliciesError {
    PartialSplitPolicies,
    SplitPoliciesRequireDirectRuntimeEnforcement(String),
    FailedToDeriveLegacyPolicy(String),
    MismatchedLegacyPolicy {
        provided: SandboxPolicy,
        derived: SandboxPolicy,
    },
    MissingConfiguration,
}

impl fmt::Display for ResolveSandboxPoliciesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PartialSplitPolicies => {
                write!(
                    f,
                    "file-system and network sandbox policies must be provided together"
                )
            }
            Self::SplitPoliciesRequireDirectRuntimeEnforcement(err) => {
                write!(
                    f,
                    "split sandbox policies require direct runtime enforcement and cannot be paired with legacy sandbox policy: {err}"
                )
            }
            Self::FailedToDeriveLegacyPolicy(err) => {
                write!(
                    f,
                    "failed to derive legacy sandbox policy from split policies: {err}"
                )
            }
            Self::MismatchedLegacyPolicy { provided, derived } => {
                write!(
                    f,
                    "legacy sandbox policy must match split sandbox policies: provided={provided:?}, derived={derived:?}"
                )
            }
            Self::MissingConfiguration => write!(f, "missing sandbox policy configuration"),
        }
    }
}

fn resolve_sandbox_policies(
    sandbox_policy_cwd: &Path,
    sandbox_policy: Option<SandboxPolicy>,
    file_system_sandbox_policy: Option<FileSystemSandboxPolicy>,
    network_sandbox_policy: Option<NetworkSandboxPolicy>,
) -> Result<EffectiveSandboxPolicies, ResolveSandboxPoliciesError> {
    // Accept either a fully legacy policy, a fully split policy pair, or all
    // three views together. Reject partial split-policy input so the helper
    // never runs with mismatched filesystem/network state.
    let split_policies = match (file_system_sandbox_policy, network_sandbox_policy) {
        (Some(file_system_sandbox_policy), Some(network_sandbox_policy)) => {
            Some((file_system_sandbox_policy, network_sandbox_policy))
        }
        (None, None) => None,
        _ => return Err(ResolveSandboxPoliciesError::PartialSplitPolicies),
    };

    match (sandbox_policy, split_policies) {
        (Some(sandbox_policy), Some((file_system_sandbox_policy, network_sandbox_policy))) => {
            if file_system_sandbox_policy
                .needs_direct_runtime_enforcement(network_sandbox_policy, sandbox_policy_cwd)
            {
                return Ok(EffectiveSandboxPolicies {
                    sandbox_policy,
                    file_system_sandbox_policy,
                    network_sandbox_policy,
                });
            }
            let derived_legacy_policy = file_system_sandbox_policy
                .to_legacy_sandbox_policy(network_sandbox_policy, sandbox_policy_cwd)
                .map_err(|err| {
                    ResolveSandboxPoliciesError::SplitPoliciesRequireDirectRuntimeEnforcement(
                        err.to_string(),
                    )
                })?;
            if !legacy_sandbox_policies_match_semantics(
                &sandbox_policy,
                &derived_legacy_policy,
                sandbox_policy_cwd,
            ) {
                return Err(ResolveSandboxPoliciesError::MismatchedLegacyPolicy {
                    provided: sandbox_policy,
                    derived: derived_legacy_policy,
                });
            }
            Ok(EffectiveSandboxPolicies {
                sandbox_policy,
                file_system_sandbox_policy,
                network_sandbox_policy,
            })
        }
        (Some(sandbox_policy), None) => Ok(EffectiveSandboxPolicies {
            file_system_sandbox_policy: FileSystemSandboxPolicy::from_legacy_sandbox_policy(
                &sandbox_policy,
                sandbox_policy_cwd,
            ),
            network_sandbox_policy: NetworkSandboxPolicy::from(&sandbox_policy),
            sandbox_policy,
        }),
        (None, Some((file_system_sandbox_policy, network_sandbox_policy))) => {
            let sandbox_policy = file_system_sandbox_policy
                .to_legacy_sandbox_policy(network_sandbox_policy, sandbox_policy_cwd)
                .map_err(|err| {
                    ResolveSandboxPoliciesError::FailedToDeriveLegacyPolicy(err.to_string())
                })?;
            Ok(EffectiveSandboxPolicies {
                sandbox_policy,
                file_system_sandbox_policy,
                network_sandbox_policy,
            })
        }
        (None, None) => Err(ResolveSandboxPoliciesError::MissingConfiguration),
    }
}

fn legacy_sandbox_policies_match_semantics(
    provided: &SandboxPolicy,
    derived: &SandboxPolicy,
    sandbox_policy_cwd: &Path,
) -> bool {
    NetworkSandboxPolicy::from(provided) == NetworkSandboxPolicy::from(derived)
        && file_system_sandbox_policies_match_semantics(
            &FileSystemSandboxPolicy::from_legacy_sandbox_policy(provided, sandbox_policy_cwd),
            &FileSystemSandboxPolicy::from_legacy_sandbox_policy(derived, sandbox_policy_cwd),
            sandbox_policy_cwd,
        )
}

fn file_system_sandbox_policies_match_semantics(
    provided: &FileSystemSandboxPolicy,
    derived: &FileSystemSandboxPolicy,
    sandbox_policy_cwd: &Path,
) -> bool {
    provided.has_full_disk_read_access() == derived.has_full_disk_read_access()
        && provided.has_full_disk_write_access() == derived.has_full_disk_write_access()
        && provided.include_platform_defaults() == derived.include_platform_defaults()
        && provided.get_readable_roots_with_cwd(sandbox_policy_cwd)
            == derived.get_readable_roots_with_cwd(sandbox_policy_cwd)
        && provided.get_writable_roots_with_cwd(sandbox_policy_cwd)
            == derived.get_writable_roots_with_cwd(sandbox_policy_cwd)
        && provided.get_unreadable_roots_with_cwd(sandbox_policy_cwd)
            == derived.get_unreadable_roots_with_cwd(sandbox_policy_cwd)
}

fn ensure_inner_stage_mode_is_valid(apply_seccomp_then_exec: bool, use_legacy_landlock: bool) {
    if apply_seccomp_then_exec && use_legacy_landlock {
        panic!("--apply-seccomp-then-exec is incompatible with --use-legacy-landlock");
    }
}

fn ensure_legacy_landlock_mode_supports_policy(
    use_legacy_landlock: bool,
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    network_sandbox_policy: NetworkSandboxPolicy,
    sandbox_policy_cwd: &Path,
) {
    if use_legacy_landlock
        && file_system_sandbox_policy
            .needs_direct_runtime_enforcement(network_sandbox_policy, sandbox_policy_cwd)
    {
        panic!(
            "split sandbox policies requiring direct runtime enforcement are incompatible with --use-legacy-landlock"
        );
    }
}

fn run_bwrap_with_proc_fallback(
    sandbox_policy_cwd: &Path,
    command_cwd: Option<&Path>,
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    network_sandbox_policy: NetworkSandboxPolicy,
    inner: Vec<String>,
    mount_proc: bool,
    allow_network_for_proxy: bool,
) -> ! {
    let network_mode = bwrap_network_mode(network_sandbox_policy, allow_network_for_proxy);
    let mut mount_proc = mount_proc;
    let command_cwd = command_cwd.unwrap_or(sandbox_policy_cwd);

    if mount_proc
        && !preflight_proc_mount_support(
            sandbox_policy_cwd,
            command_cwd,
            file_system_sandbox_policy,
            network_mode,
        )
    {
        // Keep the retry silent so sandbox-internal diagnostics do not leak into the
        // child process stderr stream.
        mount_proc = false;
    }

    let options = BwrapOptions {
        mount_proc,
        network_mode,
    };
    let mut bwrap_args = build_bwrap_argv(
        inner,
        file_system_sandbox_policy,
        sandbox_policy_cwd,
        command_cwd,
        options,
    );
    apply_inner_command_argv0(&mut bwrap_args.args);
    exec_bwrap(bwrap_args.args, bwrap_args.preserved_files);
}

fn bwrap_network_mode(
    network_sandbox_policy: NetworkSandboxPolicy,
    allow_network_for_proxy: bool,
) -> BwrapNetworkMode {
    if allow_network_for_proxy {
        BwrapNetworkMode::ProxyOnly
    } else if network_sandbox_policy.is_enabled() {
        BwrapNetworkMode::FullAccess
    } else {
        BwrapNetworkMode::Isolated
    }
}

fn build_bwrap_argv(
    inner: Vec<String>,
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    sandbox_policy_cwd: &Path,
    command_cwd: &Path,
    options: BwrapOptions,
) -> crate::bwrap::BwrapArgs {
    let bwrap_args = create_bwrap_command_args(
        inner,
        file_system_sandbox_policy,
        sandbox_policy_cwd,
        command_cwd,
        options,
    )
    .unwrap_or_else(|err| panic!("error building bubblewrap command: {err:?}"));

    let mut argv = vec!["bwrap".to_string()];
    argv.extend(bwrap_args.args);
    crate::bwrap::BwrapArgs {
        args: argv,
        preserved_files: bwrap_args.preserved_files,
    }
}

fn apply_inner_command_argv0(argv: &mut Vec<String>) {
    apply_inner_command_argv0_for_launcher(
        argv,
        preferred_bwrap_supports_argv0(),
        current_process_argv0(),
    );
}

fn apply_inner_command_argv0_for_launcher(
    argv: &mut Vec<String>,
    supports_argv0: bool,
    argv0_fallback_command: String,
) {
    if !supports_argv0 && let Ok(current_exe) = std::env::current_exe() {
        let current_exe = current_exe.to_string_lossy();
        if let Some(command) = argv.iter_mut().find(|arg| arg.as_str() == current_exe) {
            *command = argv0_fallback_command;
            return;
        }
    }

    let command_separator_index = argv
        .iter()
        .position(|arg| arg == "--")
        .unwrap_or_else(|| panic!("bubblewrap argv is missing command separator '--'"));

    if supports_argv0 {
        argv.splice(
            command_separator_index..command_separator_index,
            ["--argv0".to_string(), "codex-linux-sandbox".to_string()],
        );
        return;
    }

    let command_index = command_separator_index + 1;
    let Some(command) = argv.get_mut(command_index) else {
        panic!("bubblewrap argv is missing inner command after '--'");
    };
    *command = argv0_fallback_command;
}

fn current_process_argv0() -> String {
    match std::env::args_os().next() {
        Some(argv0) => argv0.to_string_lossy().into_owned(),
        None => panic!("failed to resolve current process argv[0]"),
    }
}

fn preflight_proc_mount_support(
    sandbox_policy_cwd: &Path,
    command_cwd: &Path,
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    network_mode: BwrapNetworkMode,
) -> bool {
    let preflight_argv = build_preflight_bwrap_argv(
        sandbox_policy_cwd,
        command_cwd,
        file_system_sandbox_policy,
        network_mode,
    );
    let stderr = run_bwrap_in_child_capture_stderr(preflight_argv);
    !is_proc_mount_failure(stderr.as_str())
}

fn build_preflight_bwrap_argv(
    sandbox_policy_cwd: &Path,
    command_cwd: &Path,
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    network_mode: BwrapNetworkMode,
) -> crate::bwrap::BwrapArgs {
    let preflight_command = vec![resolve_true_command()];
    build_bwrap_argv(
        preflight_command,
        file_system_sandbox_policy,
        sandbox_policy_cwd,
        command_cwd,
        BwrapOptions {
            mount_proc: true,
            network_mode,
        },
    )
}

fn resolve_true_command() -> String {
    for candidate in ["/usr/bin/true", "/bin/true"] {
        if Path::new(candidate).exists() {
            return candidate.to_string();
        }
    }
    "true".to_string()
}

/// Run a short-lived bubblewrap preflight in a child process and capture stderr.
///
/// Strategy:
/// - This is used only by `preflight_proc_mount_support`, which runs `/bin/true`
///   under bubblewrap with `--proc /proc`.
/// - The goal is to detect environments where mounting `/proc` fails (for
///   example, restricted containers), so we can retry the real run with
///   `--no-proc`.
/// - We capture stderr from that preflight to match known mount-failure text.
///   We do not stream it because this is a one-shot probe with a trivial
///   command, and reads are bounded to a fixed max size.
fn run_bwrap_in_child_capture_stderr(bwrap_args: crate::bwrap::BwrapArgs) -> String {
    const MAX_PREFLIGHT_STDERR_BYTES: u64 = 64 * 1024;

    let mut pipe_fds = [0; 2];
    let pipe_res = unsafe { libc::pipe2(pipe_fds.as_mut_ptr(), libc::O_CLOEXEC) };
    if pipe_res < 0 {
        let err = std::io::Error::last_os_error();
        panic!("failed to create stderr pipe for bubblewrap: {err}");
    }
    let read_fd = pipe_fds[0];
    let write_fd = pipe_fds[1];

    let pid = unsafe { libc::fork() };
    if pid < 0 {
        let err = std::io::Error::last_os_error();
        panic!("failed to fork for bubblewrap: {err}");
    }

    if pid == 0 {
        // Child: redirect stderr to the pipe, then run bubblewrap.
        unsafe {
            close_fd_or_panic(read_fd, "close read end in bubblewrap child");
            if libc::dup2(write_fd, libc::STDERR_FILENO) < 0 {
                let err = std::io::Error::last_os_error();
                panic!("failed to redirect stderr for bubblewrap: {err}");
            }
            close_fd_or_panic(write_fd, "close write end in bubblewrap child");
        }

        exec_bwrap(bwrap_args.args, bwrap_args.preserved_files);
    }

    // Parent: close the write end and read stderr while the child runs.
    close_fd_or_panic(write_fd, "close write end in bubblewrap parent");

    // SAFETY: `read_fd` is a valid owned fd in the parent.
    let mut read_file = unsafe { File::from_raw_fd(read_fd) };
    let mut stderr_bytes = Vec::new();
    let mut limited_reader = (&mut read_file).take(MAX_PREFLIGHT_STDERR_BYTES);
    if let Err(err) = limited_reader.read_to_end(&mut stderr_bytes) {
        panic!("failed to read bubblewrap stderr: {err}");
    }

    let mut status: libc::c_int = 0;
    let wait_res = unsafe { libc::waitpid(pid, &mut status as *mut libc::c_int, 0) };
    if wait_res < 0 {
        let err = std::io::Error::last_os_error();
        panic!("waitpid failed for bubblewrap child: {err}");
    }

    String::from_utf8_lossy(&stderr_bytes).into_owned()
}

/// Close an owned file descriptor and panic with context on failure.
///
/// We use explicit close() checks here (instead of ignoring return codes)
/// because this code runs in low-level sandbox setup paths where fd leaks or
/// close errors can mask the root cause of later failures.
fn close_fd_or_panic(fd: libc::c_int, context: &str) {
    let close_res = unsafe { libc::close(fd) };
    if close_res < 0 {
        let err = std::io::Error::last_os_error();
        panic!("{context}: {err}");
    }
}

fn is_proc_mount_failure(stderr: &str) -> bool {
    stderr.contains("Can't mount proc")
        && stderr.contains("/newroot/proc")
        && (stderr.contains("Invalid argument")
            || stderr.contains("Operation not permitted")
            || stderr.contains("Permission denied"))
}

struct InnerSeccompCommandArgs<'a> {
    sandbox_policy_cwd: &'a Path,
    command_cwd: Option<&'a Path>,
    sandbox_policy: &'a SandboxPolicy,
    file_system_sandbox_policy: &'a FileSystemSandboxPolicy,
    network_sandbox_policy: NetworkSandboxPolicy,
    allow_network_for_proxy: bool,
    proxy_route_spec: Option<String>,
    command: Vec<String>,
}

/// Build the inner command that applies seccomp after bubblewrap.
fn build_inner_seccomp_command(args: InnerSeccompCommandArgs<'_>) -> Vec<String> {
    let InnerSeccompCommandArgs {
        sandbox_policy_cwd,
        command_cwd,
        sandbox_policy,
        file_system_sandbox_policy,
        network_sandbox_policy,
        allow_network_for_proxy,
        proxy_route_spec,
        command,
    } = args;
    let current_exe = match std::env::current_exe() {
        Ok(path) => path,
        Err(err) => panic!("failed to resolve current executable path: {err}"),
    };
    let policy_json = match serde_json::to_string(sandbox_policy) {
        Ok(json) => json,
        Err(err) => panic!("failed to serialize sandbox policy: {err}"),
    };
    let file_system_policy_json = match serde_json::to_string(file_system_sandbox_policy) {
        Ok(json) => json,
        Err(err) => panic!("failed to serialize filesystem sandbox policy: {err}"),
    };
    let network_policy_json = match serde_json::to_string(&network_sandbox_policy) {
        Ok(json) => json,
        Err(err) => panic!("failed to serialize network sandbox policy: {err}"),
    };

    let mut inner = vec![
        current_exe.to_string_lossy().to_string(),
        "--sandbox-policy-cwd".to_string(),
        sandbox_policy_cwd.to_string_lossy().to_string(),
    ];
    if let Some(command_cwd) = command_cwd {
        inner.push("--command-cwd".to_string());
        inner.push(command_cwd.to_string_lossy().to_string());
    }
    inner.extend([
        "--sandbox-policy".to_string(),
        policy_json,
        "--file-system-sandbox-policy".to_string(),
        file_system_policy_json,
        "--network-sandbox-policy".to_string(),
        network_policy_json,
        "--apply-seccomp-then-exec".to_string(),
    ]);
    if allow_network_for_proxy {
        inner.push("--allow-network-for-proxy".to_string());
        let proxy_route_spec = proxy_route_spec
            .unwrap_or_else(|| panic!("managed proxy mode requires a proxy route spec"));
        inner.push("--proxy-route-spec".to_string());
        inner.push(proxy_route_spec);
    }
    inner.push("--".to_string());
    inner.extend(command);
    inner
}

/// Exec the provided argv, panicking with context if it fails.
fn exec_or_panic(command: Vec<String>) -> ! {
    #[expect(clippy::expect_used)]
    let c_command =
        CString::new(command[0].as_str()).expect("Failed to convert command to CString");
    #[expect(clippy::expect_used)]
    let c_args: Vec<CString> = command
        .iter()
        .map(|arg| CString::new(arg.as_str()).expect("Failed to convert arg to CString"))
        .collect();

    let mut c_args_ptrs: Vec<*const libc::c_char> = c_args.iter().map(|arg| arg.as_ptr()).collect();
    c_args_ptrs.push(std::ptr::null());

    unsafe {
        libc::execvp(c_command.as_ptr(), c_args_ptrs.as_ptr());
    }

    // If execvp returns, there was an error.
    let err = std::io::Error::last_os_error();
    panic!("Failed to execvp {}: {err}", command[0].as_str());
}

#[path = "linux_run_main_tests.rs"]
mod tests;
