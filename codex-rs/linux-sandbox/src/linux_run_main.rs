use clap::Parser;
use std::ffi::CString;
use std::fs::File;
use std::io::Read;
use std::os::fd::FromRawFd;
use std::path::Path;
use std::path::PathBuf;

use crate::bwrap::BwrapNetworkMode;
use crate::bwrap::BwrapOptions;
use crate::bwrap::create_bwrap_command_args;
use crate::landlock::apply_sandbox_policy_to_current_thread;
use crate::proxy_routing::activate_proxy_routes_in_netns;
use crate::proxy_routing::prepare_host_proxy_route_spec;
use crate::vendored_bwrap::exec_vendored_bwrap;
use crate::vendored_bwrap::run_vendored_bwrap_main;
use codex_protocol::protocol::FileSystemSandboxPolicy;
use codex_protocol::protocol::NetworkSandboxPolicy;
use codex_protocol::protocol::SandboxPolicy;

#[derive(Debug, Parser)]
/// CLI surface for the Linux sandbox helper.
///
/// The type name remains `LandlockCommand` for compatibility with existing
/// wiring, but the filesystem sandbox now uses bubblewrap.
pub struct LandlockCommand {
    /// It is possible that the cwd used in the context of the sandbox policy
    /// is different from the cwd of the process to spawn.
    #[arg(long = "sandbox-policy-cwd")]
    pub sandbox_policy_cwd: PathBuf,

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

    /// Opt-in: use the bubblewrap-based Linux sandbox pipeline.
    ///
    /// When not set, we fall back to the legacy Landlock + mount pipeline.
    #[arg(long = "use-bwrap-sandbox", hide = true, default_value_t = false)]
    pub use_bwrap_sandbox: bool,

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
        sandbox_policy,
        file_system_sandbox_policy,
        network_sandbox_policy,
        use_bwrap_sandbox,
        apply_seccomp_then_exec,
        allow_network_for_proxy,
        proxy_route_spec,
        no_proc,
        command,
    } = LandlockCommand::parse();

    if command.is_empty() {
        panic!("No command specified to execute.");
    }
    ensure_inner_stage_mode_is_valid(apply_seccomp_then_exec, use_bwrap_sandbox);
    let EffectiveSandboxPolicies {
        sandbox_policy,
        file_system_sandbox_policy,
        network_sandbox_policy,
    } = resolve_sandbox_policies(
        sandbox_policy_cwd.as_path(),
        sandbox_policy,
        file_system_sandbox_policy,
        network_sandbox_policy,
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
            false,
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
            false,
            allow_network_for_proxy,
            false,
        ) {
            panic!("error applying Linux sandbox restrictions: {e:?}");
        }
        exec_or_panic(command);
    }

    if use_bwrap_sandbox {
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
            sandbox_policy: &sandbox_policy,
            file_system_sandbox_policy: &file_system_sandbox_policy,
            network_sandbox_policy,
            use_bwrap_sandbox,
            allow_network_for_proxy,
            proxy_route_spec,
            command,
        });
        run_bwrap_with_proc_fallback(
            &sandbox_policy_cwd,
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
        true,
        allow_network_for_proxy,
        false,
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

fn resolve_sandbox_policies(
    sandbox_policy_cwd: &Path,
    sandbox_policy: Option<SandboxPolicy>,
    file_system_sandbox_policy: Option<FileSystemSandboxPolicy>,
    network_sandbox_policy: Option<NetworkSandboxPolicy>,
) -> EffectiveSandboxPolicies {
    // Accept either a fully legacy policy, a fully split policy pair, or all
    // three views together. Reject partial split-policy input so the helper
    // never runs with mismatched filesystem/network state.
    let split_policies = match (file_system_sandbox_policy, network_sandbox_policy) {
        (Some(file_system_sandbox_policy), Some(network_sandbox_policy)) => {
            Some((file_system_sandbox_policy, network_sandbox_policy))
        }
        (None, None) => None,
        _ => panic!("file-system and network sandbox policies must be provided together"),
    };

    match (sandbox_policy, split_policies) {
        (Some(sandbox_policy), Some((file_system_sandbox_policy, network_sandbox_policy))) => {
            EffectiveSandboxPolicies {
                sandbox_policy,
                file_system_sandbox_policy,
                network_sandbox_policy,
            }
        }
        (Some(sandbox_policy), None) => EffectiveSandboxPolicies {
            file_system_sandbox_policy: FileSystemSandboxPolicy::from_legacy_sandbox_policy(
                &sandbox_policy,
                sandbox_policy_cwd,
            ),
            network_sandbox_policy: NetworkSandboxPolicy::from(&sandbox_policy),
            sandbox_policy,
        },
        (None, Some((file_system_sandbox_policy, network_sandbox_policy))) => {
            let sandbox_policy = file_system_sandbox_policy
                .to_legacy_sandbox_policy(network_sandbox_policy, sandbox_policy_cwd)
                .unwrap_or_else(|err| {
                    panic!("failed to derive legacy sandbox policy from split policies: {err}")
                });
            EffectiveSandboxPolicies {
                sandbox_policy,
                file_system_sandbox_policy,
                network_sandbox_policy,
            }
        }
        (None, None) => panic!("missing sandbox policy configuration"),
    }
}

fn ensure_inner_stage_mode_is_valid(apply_seccomp_then_exec: bool, use_bwrap_sandbox: bool) {
    if apply_seccomp_then_exec && !use_bwrap_sandbox {
        panic!("--apply-seccomp-then-exec requires --use-bwrap-sandbox");
    }
}

fn run_bwrap_with_proc_fallback(
    sandbox_policy_cwd: &Path,
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    network_sandbox_policy: NetworkSandboxPolicy,
    inner: Vec<String>,
    mount_proc: bool,
    allow_network_for_proxy: bool,
) -> ! {
    let network_mode = bwrap_network_mode(network_sandbox_policy, allow_network_for_proxy);
    let mut mount_proc = mount_proc;

    if mount_proc
        && !preflight_proc_mount_support(
            sandbox_policy_cwd,
            file_system_sandbox_policy,
            network_mode,
        )
    {
        eprintln!("codex-linux-sandbox: bwrap could not mount /proc; retrying with --no-proc");
        mount_proc = false;
    }

    let options = BwrapOptions {
        mount_proc,
        network_mode,
    };
    let bwrap_args = build_bwrap_argv(
        inner,
        file_system_sandbox_policy,
        sandbox_policy_cwd,
        options,
    );
    exec_vendored_bwrap(bwrap_args.args, bwrap_args.preserved_files);
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
    options: BwrapOptions,
) -> crate::bwrap::BwrapArgs {
    let mut bwrap_args = create_bwrap_command_args(
        inner,
        file_system_sandbox_policy,
        sandbox_policy_cwd,
        options,
    )
    .unwrap_or_else(|err| panic!("error building bubblewrap command: {err:?}"));

    let command_separator_index = bwrap_args
        .args
        .iter()
        .position(|arg| arg == "--")
        .unwrap_or_else(|| panic!("bubblewrap argv is missing command separator '--'"));
    bwrap_args.args.splice(
        command_separator_index..command_separator_index,
        ["--argv0".to_string(), "codex-linux-sandbox".to_string()],
    );

    let mut argv = vec!["bwrap".to_string()];
    argv.extend(bwrap_args.args);
    crate::bwrap::BwrapArgs {
        args: argv,
        preserved_files: bwrap_args.preserved_files,
    }
}

fn preflight_proc_mount_support(
    sandbox_policy_cwd: &Path,
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    network_mode: BwrapNetworkMode,
) -> bool {
    let preflight_argv =
        build_preflight_bwrap_argv(sandbox_policy_cwd, file_system_sandbox_policy, network_mode);
    let stderr = run_bwrap_in_child_capture_stderr(preflight_argv);
    !is_proc_mount_failure(stderr.as_str())
}

fn build_preflight_bwrap_argv(
    sandbox_policy_cwd: &Path,
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    network_mode: BwrapNetworkMode,
) -> crate::bwrap::BwrapArgs {
    let preflight_command = vec![resolve_true_command()];
    build_bwrap_argv(
        preflight_command,
        file_system_sandbox_policy,
        sandbox_policy_cwd,
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

        let exit_code = run_vendored_bwrap_main(&bwrap_args.args, &bwrap_args.preserved_files);
        std::process::exit(exit_code);
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
    sandbox_policy: &'a SandboxPolicy,
    file_system_sandbox_policy: &'a FileSystemSandboxPolicy,
    network_sandbox_policy: NetworkSandboxPolicy,
    use_bwrap_sandbox: bool,
    allow_network_for_proxy: bool,
    proxy_route_spec: Option<String>,
    command: Vec<String>,
}

/// Build the inner command that applies seccomp after bubblewrap.
fn build_inner_seccomp_command(args: InnerSeccompCommandArgs<'_>) -> Vec<String> {
    let InnerSeccompCommandArgs {
        sandbox_policy_cwd,
        sandbox_policy,
        file_system_sandbox_policy,
        network_sandbox_policy,
        use_bwrap_sandbox,
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
        "--sandbox-policy".to_string(),
        policy_json,
        "--file-system-sandbox-policy".to_string(),
        file_system_policy_json,
        "--network-sandbox-policy".to_string(),
        network_policy_json,
    ];
    if use_bwrap_sandbox {
        inner.push("--use-bwrap-sandbox".to_string());
        inner.push("--apply-seccomp-then-exec".to_string());
    }
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
