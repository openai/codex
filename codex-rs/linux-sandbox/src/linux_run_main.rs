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
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicI32;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

use crate::bwrap::BwrapNetworkMode;
use crate::bwrap::BwrapOptions;
use crate::bwrap::create_bwrap_command_args;
use crate::landlock::apply_permission_profile_to_current_thread;
use crate::launcher::exec_bwrap;
use crate::launcher::preferred_bwrap_supports_argv0;
use crate::proxy_routing::activate_proxy_routes_in_netns;
use crate::proxy_routing::prepare_host_proxy_route_spec;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::FileSystemSandboxPolicy;
use codex_protocol::protocol::NetworkSandboxPolicy;
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

struct ProtectedCreateMonitor {
    stop: Arc<AtomicBool>,
    violation: Arc<AtomicBool>,
    handle: thread::JoinHandle<()>,
}

struct ProtectedCreateWatcher {
    fd: libc::c_int,
    _watches: Vec<libc::c_int>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProtectedCreateRemoval {
    Directory,
    Other,
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

    /// Canonical runtime permissions for the command.
    #[arg(
        long = "permission-profile",
        hide = true,
        value_parser = parse_permission_profile
    )]
    pub permission_profile: Option<PermissionProfile>,

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
        permission_profile,
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
    let EffectivePermissions {
        permission_profile,
        file_system_sandbox_policy,
        network_sandbox_policy,
    } = resolve_permission_profile(permission_profile).unwrap_or_else(|err| panic!("{err}"));
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
        if let Err(e) = apply_permission_profile_to_current_thread(
            &permission_profile,
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
        if let Err(e) = apply_permission_profile_to_current_thread(
            &permission_profile,
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
            permission_profile: &permission_profile,
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
    if let Err(e) = apply_permission_profile_to_current_thread(
        &permission_profile,
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
struct EffectivePermissions {
    permission_profile: PermissionProfile,
    file_system_sandbox_policy: FileSystemSandboxPolicy,
    network_sandbox_policy: NetworkSandboxPolicy,
}

#[derive(Debug, PartialEq, Eq)]
enum ResolvePermissionProfileError {
    MissingConfiguration,
}

impl fmt::Display for ResolvePermissionProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingConfiguration => write!(f, "missing permission profile configuration"),
        }
    }
}

fn parse_permission_profile(value: &str) -> std::result::Result<PermissionProfile, String> {
    serde_json::from_str(value).map_err(|err| format!("invalid permission profile JSON: {err}"))
}

fn resolve_permission_profile(
    permission_profile: Option<PermissionProfile>,
) -> Result<EffectivePermissions, ResolvePermissionProfileError> {
    let permission_profile =
        permission_profile.ok_or(ResolvePermissionProfileError::MissingConfiguration)?;
    let (file_system_sandbox_policy, network_sandbox_policy) =
        permission_profile.to_runtime_permissions();
    Ok(EffectivePermissions {
        permission_profile,
        file_system_sandbox_policy,
        network_sandbox_policy,
    })
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
            "permission profiles requiring direct runtime enforcement are incompatible with --use-legacy-landlock"
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
    let exec_start_pipe = create_exec_start_pipe(!protected_create_targets.is_empty());
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
        wait_for_parent_exec_start(exec_start_pipe[0], exec_start_pipe[1]);
        exec_bwrap(args, preserved_files);
    }

    close_child_exec_start_read(exec_start_pipe[0]);
    let protected_create_monitor = ProtectedCreateMonitor::start(&protected_create_targets);
    let signal_forwarders = install_bwrap_signal_forwarders(pid);
    release_child_exec_start(exec_start_pipe[1]);
    setup_signal_mask.restore();
    let status = wait_for_bwrap_child(pid);
    let cleanup_signal_mask = ForwardedSignalMask::block();
    BWRAP_CHILD_PID.store(0, Ordering::SeqCst);
    let protected_create_monitor_violation = protected_create_monitor
        .map(ProtectedCreateMonitor::stop)
        .unwrap_or(false);
    cleanup_synthetic_mount_targets(&synthetic_mount_registrations);
    let protected_create_violation = protected_create_monitor_violation
        || cleanup_protected_create_targets(&protected_create_registrations);
    signal_forwarders.restore();
    cleanup_signal_mask.restore();
    exit_with_wait_status_or_policy_violation(status, protected_create_violation);
}

impl ProtectedCreateMonitor {
    fn start(targets: &[crate::bwrap::ProtectedCreateTarget]) -> Option<Self> {
        if targets.is_empty() {
            return None;
        }

        let targets = targets.to_vec();
        let stop = Arc::new(AtomicBool::new(false));
        let violation = Arc::new(AtomicBool::new(false));
        let monitor_stop = Arc::clone(&stop);
        let monitor_violation = Arc::clone(&violation);
        let handle = thread::spawn(move || {
            let watcher = ProtectedCreateWatcher::new(&targets);
            while !monitor_stop.load(Ordering::SeqCst) {
                for target in &targets {
                    if remove_protected_create_target_best_effort(target).is_some() {
                        monitor_violation.store(true, Ordering::SeqCst);
                    }
                }
                if let Some(watcher) = &watcher {
                    watcher.wait_for_create_event(&monitor_stop);
                } else {
                    thread::sleep(Duration::from_millis(1));
                }
            }
        });

        Some(Self {
            stop,
            violation,
            handle,
        })
    }

    fn stop(self) -> bool {
        self.stop.store(true, Ordering::SeqCst);
        self.handle
            .join()
            .unwrap_or_else(|_| panic!("protected create monitor thread panicked"));
        self.violation.load(Ordering::SeqCst)
    }
}

impl ProtectedCreateWatcher {
    fn new(targets: &[crate::bwrap::ProtectedCreateTarget]) -> Option<Self> {
        let fd = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
        if fd < 0 {
            return None;
        }

        let mut watched_parents = Vec::<PathBuf>::new();
        let mut watches = Vec::new();
        for target in targets {
            let Some(parent) = target.path().parent() else {
                continue;
            };
            if watched_parents.iter().any(|watched| watched == parent) {
                continue;
            }
            watched_parents.push(parent.to_path_buf());
            let Ok(parent_cstr) = CString::new(parent.as_os_str().as_bytes()) else {
                continue;
            };
            let mask =
                libc::IN_CREATE | libc::IN_MOVED_TO | libc::IN_DELETE_SELF | libc::IN_MOVE_SELF;
            let watch = unsafe { libc::inotify_add_watch(fd, parent_cstr.as_ptr(), mask) };
            if watch >= 0 {
                watches.push(watch);
            }
        }

        if watches.is_empty() {
            unsafe {
                libc::close(fd);
            }
            return None;
        }

        Some(Self {
            fd,
            _watches: watches,
        })
    }

    fn wait_for_create_event(&self, stop: &AtomicBool) {
        let mut poll_fd = libc::pollfd {
            fd: self.fd,
            events: libc::POLLIN,
            revents: 0,
        };
        while !stop.load(Ordering::SeqCst) {
            let res = unsafe { libc::poll(&mut poll_fd, 1, 10) };
            if res > 0 {
                self.drain_events();
                return;
            }
            if res == 0 {
                return;
            }
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return;
        }
    }

    fn drain_events(&self) {
        let mut buf = [0_u8; 4096];
        loop {
            let read = unsafe { libc::read(self.fd, buf.as_mut_ptr().cast(), buf.len()) };
            if read > 0 {
                continue;
            }
            if read == 0 {
                return;
            }
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return;
        }
    }
}

impl Drop for ProtectedCreateWatcher {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}

fn create_exec_start_pipe(enabled: bool) -> [libc::c_int; 2] {
    if !enabled {
        return [-1, -1];
    }
    let mut pipe = [-1, -1];
    if unsafe { libc::pipe2(pipe.as_mut_ptr(), libc::O_CLOEXEC) } < 0 {
        let err = std::io::Error::last_os_error();
        panic!("failed to create bubblewrap exec start pipe: {err}");
    }
    pipe
}

fn wait_for_parent_exec_start(read_fd: libc::c_int, write_fd: libc::c_int) {
    if write_fd >= 0 {
        unsafe {
            libc::close(write_fd);
        }
    }
    if read_fd < 0 {
        return;
    }

    let mut byte = [0_u8; 1];
    loop {
        let read = unsafe { libc::read(read_fd, byte.as_mut_ptr().cast(), byte.len()) };
        if read >= 0 {
            break;
        }
        let err = std::io::Error::last_os_error();
        if err.kind() != std::io::ErrorKind::Interrupted {
            break;
        }
    }
    unsafe {
        libc::close(read_fd);
    }
}

fn close_child_exec_start_read(read_fd: libc::c_int) {
    if read_fd >= 0 {
        unsafe {
            libc::close(read_fd);
        }
    }
}

fn release_child_exec_start(write_fd: libc::c_int) {
    if write_fd < 0 {
        return;
    }
    let byte = [0_u8; 1];
    unsafe {
        libc::write(write_fd, byte.as_ptr().cast(), byte.len());
        libc::close(write_fd);
    }
}

struct ForwardedSignalMask {
    previous: libc::sigset_t,
}

struct ForwardedSignalHandlers {
    previous: Vec<(libc::c_int, libc::sigaction)>,
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

impl ForwardedSignalHandlers {
    fn restore(self) {
        BWRAP_CHILD_PID.store(0, Ordering::SeqCst);
        PENDING_FORWARDED_SIGNAL.store(0, Ordering::SeqCst);
        for (signal, previous_action) in self.previous {
            unsafe {
                if libc::sigaction(signal, &previous_action, std::ptr::null_mut()) < 0 {
                    let err = std::io::Error::last_os_error();
                    panic!("failed to restore bubblewrap signal handler for {signal}: {err}");
                }
            }
        }
    }
}

fn install_bwrap_signal_forwarders(pid: libc::pid_t) -> ForwardedSignalHandlers {
    BWRAP_CHILD_PID.store(pid, Ordering::SeqCst);
    let mut previous = Vec::with_capacity(FORWARDED_SIGNALS.len());
    for signal in FORWARDED_SIGNALS {
        let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
        let mut previous_action: libc::sigaction = unsafe { std::mem::zeroed() };
        action.sa_sigaction = forward_signal_to_bwrap_child as *const () as libc::sighandler_t;
        unsafe {
            libc::sigemptyset(&mut action.sa_mask);
            if libc::sigaction(*signal, &action, &mut previous_action) < 0 {
                let err = std::io::Error::last_os_error();
                panic!("failed to install bubblewrap signal forwarder for {signal}: {err}");
            }
        }
        previous.push((*signal, previous_action));
    }
    replay_pending_forwarded_signal(pid);
    ForwardedSignalHandlers { previous }
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
    for attempt in 0..100 {
        match try_remove_protected_create_target(target) {
            Ok(removal) => return removal.is_some(),
            Err(err) if err.kind() == std::io::ErrorKind::DirectoryNotEmpty && attempt < 99 => {
                thread::sleep(Duration::from_millis(1));
            }
            Err(err) => {
                panic!(
                    "failed to remove protected create target {}: {err}",
                    target.path().display()
                );
            }
        }
    }
    unreachable!("protected create removal retry loop should return or panic")
}

fn remove_protected_create_target_best_effort(
    target: &crate::bwrap::ProtectedCreateTarget,
) -> Option<ProtectedCreateRemoval> {
    for _ in 0..100 {
        match try_remove_protected_create_target(target) {
            Ok(removal) => return removal,
            Err(err) if err.kind() == std::io::ErrorKind::DirectoryNotEmpty => {
                thread::sleep(Duration::from_millis(1));
            }
            Err(_) => return Some(ProtectedCreateRemoval::Other),
        }
    }
    Some(ProtectedCreateRemoval::Other)
}

fn try_remove_protected_create_target(
    target: &crate::bwrap::ProtectedCreateTarget,
) -> std::io::Result<Option<ProtectedCreateRemoval>> {
    let path = target.path();
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };

    let removal = if metadata.is_dir() {
        ProtectedCreateRemoval::Directory
    } else {
        ProtectedCreateRemoval::Other
    };
    let result = if removal == ProtectedCreateRemoval::Directory {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };
    match result {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    }
    eprintln!(
        "sandbox blocked creation of protected workspace metadata path {}",
        path.display()
    );
    Ok(Some(removal))
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

    let signal_forwarders = install_bwrap_signal_forwarders(pid);
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

    let status = wait_for_bwrap_child(pid);
    let cleanup_signal_mask = ForwardedSignalMask::block();
    BWRAP_CHILD_PID.store(0, Ordering::SeqCst);
    cleanup_synthetic_mount_targets(&synthetic_mount_registrations);
    cleanup_protected_create_targets(&protected_create_registrations);
    signal_forwarders.restore();
    cleanup_signal_mask.restore();
    if libc::WIFSIGNALED(status) {
        exit_with_wait_status(status);
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
    permission_profile: &'a PermissionProfile,
    allow_network_for_proxy: bool,
    proxy_route_spec: Option<String>,
    command: Vec<String>,
}

/// Build the inner command that applies seccomp after bubblewrap.
fn build_inner_seccomp_command(args: InnerSeccompCommandArgs<'_>) -> Vec<String> {
    let InnerSeccompCommandArgs {
        sandbox_policy_cwd,
        command_cwd,
        permission_profile,
        allow_network_for_proxy,
        proxy_route_spec,
        command,
    } = args;
    let current_exe = match std::env::current_exe() {
        Ok(path) => path,
        Err(err) => panic!("failed to resolve current executable path: {err}"),
    };
    let permission_profile_json = match serde_json::to_string(permission_profile) {
        Ok(json) => json,
        Err(err) => panic!("failed to serialize permission profile: {err}"),
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
        "--permission-profile".to_string(),
        permission_profile_json,
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

#[cfg(test)]
#[path = "linux_run_main_tests.rs"]
mod tests;
