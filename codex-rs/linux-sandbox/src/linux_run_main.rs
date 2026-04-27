use clap::Parser;
use std::ffi::CString;
use std::fmt;
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicI32;
use std::sync::atomic::Ordering;

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
use codex_sandboxing::landlock::CODEX_LINUX_SANDBOX_ARG0;

static BWRAP_CHILD_PID: AtomicI32 = AtomicI32::new(0);
static PENDING_FORWARDED_SIGNAL: AtomicI32 = AtomicI32::new(0);

const FORWARDED_SIGNALS: &[libc::c_int] =
    &[libc::SIGHUP, libc::SIGINT, libc::SIGQUIT, libc::SIGTERM];
const SYNTHETIC_MOUNT_MARKER_SYNTHETIC: &[u8] = b"synthetic\n";
const SYNTHETIC_MOUNT_MARKER_EXISTING: &[u8] = b"existing\n";
const PROTECTED_CREATE_MARKER: &[u8] = b"protected-create\n";

#[derive(Debug)]
struct SyntheticMountTargetRegistration {
    target: crate::bwrap::SyntheticMountTarget,
    marker_file: PathBuf,
    marker_dir: PathBuf,
}

#[derive(Debug)]
struct ProtectedCreateTargetRegistration {
    target: crate::bwrap::ProtectedCreateTarget,
    marker_file: PathBuf,
    marker_dir: PathBuf,
}

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
            file_system_sandbox_policy: FileSystemSandboxPolicy::from_legacy_sandbox_policy_for_cwd(
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
            &FileSystemSandboxPolicy::from_legacy_sandbox_policy_for_cwd(
                provided,
                sandbox_policy_cwd,
            ),
            &FileSystemSandboxPolicy::from_legacy_sandbox_policy_for_cwd(
                derived,
                sandbox_policy_cwd,
            ),
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
        ..Default::default()
    };
    let mut bwrap_args = build_bwrap_argv(
        inner,
        file_system_sandbox_policy,
        sandbox_policy_cwd,
        command_cwd,
        options,
    );
    apply_inner_command_argv0(&mut bwrap_args.args);
    run_or_exec_bwrap(bwrap_args);
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
        synthetic_mount_targets: bwrap_args.synthetic_mount_targets,
        protected_create_targets: bwrap_args.protected_create_targets,
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
    let command_separator_index = argv
        .iter()
        .position(|arg| arg == "--")
        .unwrap_or_else(|| panic!("bubblewrap argv is missing command separator '--'"));

    if supports_argv0 {
        argv.splice(
            command_separator_index..command_separator_index,
            ["--argv0".to_string(), CODEX_LINUX_SANDBOX_ARG0.to_string()],
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
            ..Default::default()
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

fn run_or_exec_bwrap(bwrap_args: crate::bwrap::BwrapArgs) -> ! {
    if bwrap_args.synthetic_mount_targets.is_empty()
        && bwrap_args.protected_create_targets.is_empty()
    {
        exec_bwrap(bwrap_args.args, bwrap_args.preserved_files);
    }
    run_bwrap_in_child_with_synthetic_mount_cleanup(bwrap_args);
}

fn run_bwrap_in_child_with_synthetic_mount_cleanup(bwrap_args: crate::bwrap::BwrapArgs) -> ! {
    let crate::bwrap::BwrapArgs {
        args,
        preserved_files,
        synthetic_mount_targets,
        protected_create_targets,
    } = bwrap_args;
    let setup_signal_mask = ForwardedSignalMask::block();
    let synthetic_mount_registrations = register_synthetic_mount_targets(&synthetic_mount_targets);
    let protected_create_registrations =
        register_protected_create_targets(&protected_create_targets);
    let parent_pid = unsafe { libc::getpid() };
    let pid = unsafe { libc::fork() };
    if pid < 0 {
        let err = std::io::Error::last_os_error();
        panic!("failed to fork for bubblewrap: {err}");
    }

    if pid == 0 {
        reset_forwarded_signal_handlers_to_default();
        setup_signal_mask.restore();
        let setpgid_res = unsafe { libc::setpgid(0, 0) };
        if setpgid_res < 0 {
            let err = std::io::Error::last_os_error();
            panic!("failed to place bubblewrap child in its own process group: {err}");
        }
        terminate_with_parent(parent_pid);
        exec_bwrap(args, preserved_files);
    }

    install_bwrap_signal_forwarders(pid);
    setup_signal_mask.restore();
    let status = wait_for_bwrap_child(pid);
    let cleanup_signal_mask = ForwardedSignalMask::block();
    BWRAP_CHILD_PID.store(0, Ordering::SeqCst);
    cleanup_synthetic_mount_targets(&synthetic_mount_registrations);
    let protected_create_violation =
        cleanup_protected_create_targets(&protected_create_registrations);
    cleanup_signal_mask.restore();
    exit_with_wait_status_or_policy_violation(status, protected_create_violation);
}

struct ForwardedSignalMask {
    previous: libc::sigset_t,
}

impl ForwardedSignalMask {
    fn block() -> Self {
        let mut blocked: libc::sigset_t = unsafe { std::mem::zeroed() };
        let mut previous: libc::sigset_t = unsafe { std::mem::zeroed() };
        unsafe {
            libc::sigemptyset(&mut blocked);
            for signal in FORWARDED_SIGNALS {
                libc::sigaddset(&mut blocked, *signal);
            }
            if libc::sigprocmask(libc::SIG_BLOCK, &blocked, &mut previous) < 0 {
                let err = std::io::Error::last_os_error();
                panic!("failed to block bubblewrap forwarded signals: {err}");
            }
        }
        Self { previous }
    }

    fn restore(&self) {
        let mut restored = self.previous;
        unsafe {
            for signal in FORWARDED_SIGNALS {
                libc::sigdelset(&mut restored, *signal);
            }
            if libc::sigprocmask(libc::SIG_SETMASK, &restored, std::ptr::null_mut()) < 0 {
                let err = std::io::Error::last_os_error();
                panic!("failed to restore bubblewrap forwarded signals: {err}");
            }
        }
    }
}

fn terminate_with_parent(parent_pid: libc::pid_t) {
    let res = unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) };
    if res < 0 {
        let err = std::io::Error::last_os_error();
        panic!("failed to set bubblewrap child parent-death signal: {err}");
    }
    if unsafe { libc::getppid() } != parent_pid {
        unsafe {
            libc::raise(libc::SIGTERM);
        }
    }
}

fn install_bwrap_signal_forwarders(pid: libc::pid_t) {
    BWRAP_CHILD_PID.store(pid, Ordering::SeqCst);
    for signal in FORWARDED_SIGNALS {
        let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
        action.sa_sigaction = forward_signal_to_bwrap_child as *const () as libc::sighandler_t;
        unsafe {
            libc::sigemptyset(&mut action.sa_mask);
            if libc::sigaction(*signal, &action, std::ptr::null_mut()) < 0 {
                let err = std::io::Error::last_os_error();
                panic!("failed to install bubblewrap signal forwarder for {signal}: {err}");
            }
        }
    }
    replay_pending_forwarded_signal(pid);
}

extern "C" fn forward_signal_to_bwrap_child(signal: libc::c_int) {
    PENDING_FORWARDED_SIGNAL.store(signal, Ordering::SeqCst);
    let pid = BWRAP_CHILD_PID.load(Ordering::SeqCst);
    if pid > 0 {
        send_signal_to_bwrap_child(pid, signal);
    }
}

fn replay_pending_forwarded_signal(pid: libc::pid_t) {
    let signal = PENDING_FORWARDED_SIGNAL.swap(0, Ordering::SeqCst);
    if signal > 0 {
        send_signal_to_bwrap_child(pid, signal);
    }
}

fn send_signal_to_bwrap_child(pid: libc::pid_t, signal: libc::c_int) {
    unsafe {
        libc::kill(-pid, signal);
        libc::kill(pid, signal);
    }
}

fn reset_forwarded_signal_handlers_to_default() {
    for signal in FORWARDED_SIGNALS {
        unsafe {
            if libc::signal(*signal, libc::SIG_DFL) == libc::SIG_ERR {
                let err = std::io::Error::last_os_error();
                panic!("failed to reset bubblewrap signal handler for {signal}: {err}");
            }
        }
    }
}

fn wait_for_bwrap_child(pid: libc::pid_t) -> libc::c_int {
    loop {
        let mut status: libc::c_int = 0;
        let wait_res = unsafe { libc::waitpid(pid, &mut status as *mut libc::c_int, 0) };
        if wait_res >= 0 {
            return status;
        }
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::EINTR) {
            continue;
        }
        panic!("waitpid failed for bubblewrap child: {err}");
    }
}

fn register_synthetic_mount_targets(
    targets: &[crate::bwrap::SyntheticMountTarget],
) -> Vec<SyntheticMountTargetRegistration> {
    with_synthetic_mount_registry_lock(|| {
        targets
            .iter()
            .map(|target| {
                let marker_dir = synthetic_mount_marker_dir(target.path());
                fs::create_dir_all(&marker_dir).unwrap_or_else(|err| {
                    panic!(
                        "failed to create synthetic bubblewrap mount marker directory {}: {err}",
                        marker_dir.display()
                    )
                });
                let target = if target.preserves_pre_existing_path()
                    && synthetic_mount_marker_dir_has_active_synthetic_owner(&marker_dir)
                {
                    match target.kind() {
                        crate::bwrap::SyntheticMountTargetKind::EmptyFile => {
                            crate::bwrap::SyntheticMountTarget::missing(target.path())
                        }
                        crate::bwrap::SyntheticMountTargetKind::EmptyDirectory => {
                            crate::bwrap::SyntheticMountTarget::missing_empty_directory(
                                target.path(),
                            )
                        }
                    }
                } else {
                    target.clone()
                };
                let marker_file = marker_dir.join(std::process::id().to_string());
                fs::write(&marker_file, synthetic_mount_marker_contents(&target)).unwrap_or_else(
                    |err| {
                        panic!(
                            "failed to register synthetic bubblewrap mount target {}: {err}",
                            target.path().display()
                        )
                    },
                );
                SyntheticMountTargetRegistration {
                    target,
                    marker_file,
                    marker_dir,
                }
            })
            .collect()
    })
}

fn register_protected_create_targets(
    targets: &[crate::bwrap::ProtectedCreateTarget],
) -> Vec<ProtectedCreateTargetRegistration> {
    with_synthetic_mount_registry_lock(|| {
        targets
            .iter()
            .map(|target| {
                let marker_dir = synthetic_mount_marker_dir(target.path());
                fs::create_dir_all(&marker_dir).unwrap_or_else(|err| {
                    panic!(
                        "failed to create protected create marker directory {}: {err}",
                        marker_dir.display()
                    )
                });
                let marker_file = marker_dir.join(std::process::id().to_string());
                fs::write(&marker_file, PROTECTED_CREATE_MARKER).unwrap_or_else(|err| {
                    panic!(
                        "failed to register protected create target {}: {err}",
                        target.path().display()
                    )
                });
                ProtectedCreateTargetRegistration {
                    target: target.clone(),
                    marker_file,
                    marker_dir,
                }
            })
            .collect()
    })
}

fn synthetic_mount_marker_contents(target: &crate::bwrap::SyntheticMountTarget) -> &'static [u8] {
    if target.preserves_pre_existing_path() {
        SYNTHETIC_MOUNT_MARKER_EXISTING
    } else {
        SYNTHETIC_MOUNT_MARKER_SYNTHETIC
    }
}

fn synthetic_mount_marker_dir_has_active_synthetic_owner(marker_dir: &Path) -> bool {
    synthetic_mount_marker_dir_has_active_process_matching(marker_dir, |path| {
        match fs::read(path) {
            Ok(contents) => contents == SYNTHETIC_MOUNT_MARKER_SYNTHETIC,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => false,
            Err(err) => panic!(
                "failed to read synthetic bubblewrap mount marker {}: {err}",
                path.display()
            ),
        }
    })
}

fn synthetic_mount_marker_dir_has_active_process(marker_dir: &Path) -> bool {
    synthetic_mount_marker_dir_has_active_process_matching(marker_dir, |_| true)
}

fn synthetic_mount_marker_dir_has_active_process_matching(
    marker_dir: &Path,
    matches_marker: impl Fn(&Path) -> bool,
) -> bool {
    let entries = match fs::read_dir(marker_dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return false,
        Err(err) => panic!(
            "failed to read synthetic bubblewrap mount marker directory {}: {err}",
            marker_dir.display()
        ),
    };
    for entry in entries {
        let entry = entry.unwrap_or_else(|err| {
            panic!(
                "failed to read synthetic bubblewrap mount marker in {}: {err}",
                marker_dir.display()
            )
        });
        let path = entry.path();
        let Some(pid) = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.parse::<libc::pid_t>().ok())
        else {
            continue;
        };
        if !process_is_active(pid) {
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => panic!(
                    "failed to remove stale synthetic bubblewrap mount marker {}: {err}",
                    path.display()
                ),
            }
            continue;
        }
        let matches_marker = matches_marker(&path);
        if matches_marker {
            return true;
        }
    }
    false
}

fn cleanup_synthetic_mount_targets(targets: &[SyntheticMountTargetRegistration]) {
    with_synthetic_mount_registry_lock(|| {
        for target in targets.iter().rev() {
            match fs::remove_file(&target.marker_file) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => panic!(
                    "failed to unregister synthetic bubblewrap mount target {}: {err}",
                    target.target.path().display()
                ),
            }
        }

        for target in targets.iter().rev() {
            if synthetic_mount_marker_dir_has_active_process(&target.marker_dir) {
                continue;
            }
            remove_synthetic_mount_target(&target.target);
            match fs::remove_dir(&target.marker_dir) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) if err.kind() == std::io::ErrorKind::DirectoryNotEmpty => {}
                Err(err) => panic!(
                    "failed to remove synthetic bubblewrap mount marker directory {}: {err}",
                    target.marker_dir.display()
                ),
            }
        }
    });
}

fn cleanup_protected_create_targets(targets: &[ProtectedCreateTargetRegistration]) -> bool {
    with_synthetic_mount_registry_lock(|| {
        for target in targets.iter().rev() {
            match fs::remove_file(&target.marker_file) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => panic!(
                    "failed to unregister protected create target {}: {err}",
                    target.target.path().display()
                ),
            }
        }

        let mut violation = false;
        for target in targets.iter().rev() {
            if synthetic_mount_marker_dir_has_active_process(&target.marker_dir) {
                if target.target.path().exists() {
                    violation = true;
                }
                continue;
            }
            violation |= remove_protected_create_target(&target.target);
            match fs::remove_dir(&target.marker_dir) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) if err.kind() == std::io::ErrorKind::DirectoryNotEmpty => {}
                Err(err) => panic!(
                    "failed to remove protected create marker directory {}: {err}",
                    target.marker_dir.display()
                ),
            }
        }
        violation
    })
}

fn remove_protected_create_target(target: &crate::bwrap::ProtectedCreateTarget) -> bool {
    let path = target.path();
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return false,
        Err(err) => panic!(
            "failed to inspect protected create target {}: {err}",
            path.display()
        ),
    };

    if metadata.is_dir() {
        fs::remove_dir_all(path).unwrap_or_else(|err| {
            panic!(
                "failed to remove protected create target directory {}: {err}",
                path.display()
            )
        });
    } else {
        fs::remove_file(path).unwrap_or_else(|err| {
            panic!(
                "failed to remove protected create target file {}: {err}",
                path.display()
            )
        });
    }
    eprintln!(
        "sandbox blocked creation of preserved workspace metadata path {}",
        path.display()
    );
    true
}

fn remove_synthetic_mount_target(target: &crate::bwrap::SyntheticMountTarget) {
    let path = target.path();
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return,
        Err(err) => panic!(
            "failed to inspect synthetic bubblewrap mount target {}: {err}",
            path.display()
        ),
    };
    if !target.should_remove_after_bwrap(&metadata) {
        return;
    }
    match target.kind() {
        crate::bwrap::SyntheticMountTargetKind::EmptyFile => match fs::remove_file(path) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => panic!(
                "failed to remove synthetic bubblewrap mount target {}: {err}",
                path.display()
            ),
        },
        crate::bwrap::SyntheticMountTargetKind::EmptyDirectory => match fs::remove_dir(path) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) if err.kind() == std::io::ErrorKind::DirectoryNotEmpty => {}
            Err(err) => panic!(
                "failed to remove synthetic bubblewrap mount target {}: {err}",
                path.display()
            ),
        },
    }
}

fn process_is_active(pid: libc::pid_t) -> bool {
    let result = unsafe { libc::kill(pid, 0) };
    if result == 0 {
        return true;
    }
    let err = std::io::Error::last_os_error();
    !matches!(err.raw_os_error(), Some(libc::ESRCH))
}

fn with_synthetic_mount_registry_lock<T>(f: impl FnOnce() -> T) -> T {
    let registry_root = synthetic_mount_registry_root();
    fs::create_dir_all(&registry_root).unwrap_or_else(|err| {
        panic!(
            "failed to create synthetic bubblewrap mount registry {}: {err}",
            registry_root.display()
        )
    });
    let lock_path = registry_root.join("lock");
    let lock_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .unwrap_or_else(|err| {
            panic!(
                "failed to open synthetic bubblewrap mount registry lock {}: {err}",
                lock_path.display()
            )
        });
    if unsafe { libc::flock(lock_file.as_raw_fd(), libc::LOCK_EX) } < 0 {
        let err = std::io::Error::last_os_error();
        panic!(
            "failed to lock synthetic bubblewrap mount registry {}: {err}",
            lock_path.display()
        );
    }
    let result = f();
    if unsafe { libc::flock(lock_file.as_raw_fd(), libc::LOCK_UN) } < 0 {
        let err = std::io::Error::last_os_error();
        panic!(
            "failed to unlock synthetic bubblewrap mount registry {}: {err}",
            lock_path.display()
        );
    }
    result
}

fn synthetic_mount_marker_dir(path: &Path) -> PathBuf {
    synthetic_mount_registry_root().join(format!("{:016x}", hash_path(path)))
}

fn synthetic_mount_registry_root() -> PathBuf {
    std::env::temp_dir().join("codex-bwrap-synthetic-mount-targets")
}

fn hash_path(path: &Path) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in path.as_os_str().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn exit_with_wait_status(status: libc::c_int) -> ! {
    if libc::WIFEXITED(status) {
        std::process::exit(libc::WEXITSTATUS(status));
    }

    if libc::WIFSIGNALED(status) {
        let signal = libc::WTERMSIG(status);
        unsafe {
            libc::signal(signal, libc::SIG_DFL);
            libc::kill(libc::getpid(), signal);
        }
        std::process::exit(128 + signal);
    }

    std::process::exit(1);
}

fn exit_with_wait_status_or_policy_violation(
    status: libc::c_int,
    protected_create_violation: bool,
) -> ! {
    if protected_create_violation && libc::WIFEXITED(status) && libc::WEXITSTATUS(status) == 0 {
        std::process::exit(1);
    }

    exit_with_wait_status(status);
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
    let crate::bwrap::BwrapArgs {
        args,
        preserved_files,
        synthetic_mount_targets,
        protected_create_targets,
    } = bwrap_args;
    let setup_signal_mask = ForwardedSignalMask::block();
    let synthetic_mount_registrations = register_synthetic_mount_targets(&synthetic_mount_targets);
    let protected_create_registrations =
        register_protected_create_targets(&protected_create_targets);

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
        reset_forwarded_signal_handlers_to_default();
        setup_signal_mask.restore();
        // Child: redirect stderr to the pipe, then run bubblewrap.
        unsafe {
            close_fd_or_panic(read_fd, "close read end in bubblewrap child");
            if libc::dup2(write_fd, libc::STDERR_FILENO) < 0 {
                let err = std::io::Error::last_os_error();
                panic!("failed to redirect stderr for bubblewrap: {err}");
            }
            close_fd_or_panic(write_fd, "close write end in bubblewrap child");
        }

        exec_bwrap(args, preserved_files);
    }

    install_bwrap_signal_forwarders(pid);
    setup_signal_mask.restore();
    // Parent: close the write end and read stderr while the child runs.
    close_fd_or_panic(write_fd, "close write end in bubblewrap parent");

    // SAFETY: `read_fd` is a valid owned fd in the parent.
    let mut read_file = unsafe { File::from_raw_fd(read_fd) };
    let mut stderr_bytes = Vec::new();
    let mut limited_reader = (&mut read_file).take(MAX_PREFLIGHT_STDERR_BYTES);
    if let Err(err) = limited_reader.read_to_end(&mut stderr_bytes) {
        panic!("failed to read bubblewrap stderr: {err}");
    }

    wait_for_bwrap_child(pid);
    let cleanup_signal_mask = ForwardedSignalMask::block();
    BWRAP_CHILD_PID.store(0, Ordering::SeqCst);
    cleanup_synthetic_mount_targets(&synthetic_mount_registrations);
    cleanup_protected_create_targets(&protected_create_registrations);
    cleanup_signal_mask.restore();

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
