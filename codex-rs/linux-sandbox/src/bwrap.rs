//! Bubblewrap-based filesystem sandboxing for Linux.
//!
//! This module mirrors the semantics used by the macOS Seatbelt sandbox:
//! - the filesystem is read-only by default,
//! - explicit writable roots are layered on top, and
//! - sensitive subpaths such as `.git` and `.codex` remain read-only even when
//!   their parent root is writable.
//!
//! The overall Linux sandbox is composed of:
//! - seccomp + `PR_SET_NO_NEW_PRIVS` applied in-process, and
//! - bubblewrap used to construct the filesystem view before exec.
use std::collections::BTreeSet;
use std::path::Path;
use std::path::PathBuf;

use codex_core::error::CodexErr;
use codex_core::error::Result;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::WritableRoot;

/// Linux "platform defaults" that keep common system binaries and dynamic
/// libraries readable when `ReadOnlyAccess::Restricted` requests them.
///
/// These are intentionally system-level paths only (plus Nix store roots) so
/// `include_platform_defaults` does not silently widen access to user data.
const LINUX_PLATFORM_DEFAULT_READ_ROOTS: &[&str] = &[
    "/bin",
    "/sbin",
    "/usr",
    "/etc",
    "/lib",
    "/lib64",
    "/nix/store",
    "/run/current-system/sw",
];

/// Options that control how bubblewrap is invoked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BwrapOptions {
    /// Whether to mount a fresh `/proc` inside the PID namespace.
    ///
    /// This is the secure default, but some restrictive container environments
    /// deny `--proc /proc` even when PID namespaces are available.
    pub mount_proc: bool,
    /// How networking should be configured inside the bubblewrap sandbox.
    pub network_mode: BwrapNetworkMode,
}

impl Default for BwrapOptions {
    fn default() -> Self {
        Self {
            mount_proc: true,
            network_mode: BwrapNetworkMode::FullAccess,
        }
    }
}

/// Network policy modes for bubblewrap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum BwrapNetworkMode {
    /// Keep access to the host network namespace.
    #[default]
    FullAccess,
    /// Remove access to the host network namespace.
    Isolated,
    /// Intended proxy-only mode.
    ///
    /// Bubblewrap enforces this by unsharing the network namespace. The
    /// proxy-routing bridge is established by the helper process after startup.
    ProxyOnly,
}

impl BwrapNetworkMode {
    fn should_unshare_network(self) -> bool {
        !matches!(self, Self::FullAccess)
    }
}

/// Wrap a command with bubblewrap so the filesystem is read-only by default,
/// with explicit writable roots and read-only subpaths layered afterward.
///
/// When the policy grants full disk write access and full network access, this
/// returns `command` unchanged so we avoid unnecessary sandboxing overhead.
/// If network isolation is requested, we still wrap with bubblewrap so network
/// namespace restrictions apply while preserving full filesystem access.
pub(crate) fn create_bwrap_command_args(
    command: Vec<String>,
    sandbox_policy: &SandboxPolicy,
    cwd: &Path,
    options: BwrapOptions,
) -> Result<Vec<String>> {
    if sandbox_policy.has_full_disk_write_access() {
        return if options.network_mode == BwrapNetworkMode::FullAccess {
            Ok(command)
        } else {
            Ok(create_bwrap_flags_full_filesystem(command, options))
        };
    }

    create_bwrap_flags(command, sandbox_policy, cwd, options)
}

fn create_bwrap_flags_full_filesystem(command: Vec<String>, options: BwrapOptions) -> Vec<String> {
    let mut args = vec![
        "--new-session".to_string(),
        "--die-with-parent".to_string(),
        "--bind".to_string(),
        "/".to_string(),
        "/".to_string(),
        "--unshare-pid".to_string(),
    ];
    if options.network_mode.should_unshare_network() {
        args.push("--unshare-net".to_string());
    }
    if options.mount_proc {
        args.push("--proc".to_string());
        args.push("/proc".to_string());
    }
    args.push("--".to_string());
    args.extend(command);
    args
}

/// Build the bubblewrap flags (everything after `argv[0]`).
fn create_bwrap_flags(
    command: Vec<String>,
    sandbox_policy: &SandboxPolicy,
    cwd: &Path,
    options: BwrapOptions,
) -> Result<Vec<String>> {
    let mut args = Vec::new();
    args.push("--new-session".to_string());
    args.push("--die-with-parent".to_string());
    args.extend(create_filesystem_args(sandbox_policy, cwd)?);
    // Isolate the PID namespace.
    args.push("--unshare-pid".to_string());
    if options.network_mode.should_unshare_network() {
        args.push("--unshare-net".to_string());
    }
    // Mount a fresh /proc unless the caller explicitly disables it.
    if options.mount_proc {
        args.push("--proc".to_string());
        args.push("/proc".to_string());
    }
    args.push("--".to_string());
    args.extend(command);
    Ok(args)
}

/// Build the bubblewrap filesystem mounts for a given sandbox policy.
///
/// The mount order is important:
/// 1. Full-read policies use `--ro-bind / /`; restricted-read policies start
///    from `--tmpfs /` and layer scoped `--ro-bind` mounts.
/// 2. `--dev /dev` mounts a minimal writable `/dev` with standard device nodes
///    (including `/dev/urandom`) even under a read-only root.
/// 3. `--bind <root> <root>` re-enables writes for allowed roots, including
///    writable subpaths under `/dev` (for example, `/dev/shm`).
/// 4. `--ro-bind <subpath> <subpath>` re-applies read-only protections under
///    those writable roots so protected subpaths win.
fn create_filesystem_args(sandbox_policy: &SandboxPolicy, cwd: &Path) -> Result<Vec<String>> {
    let writable_roots = sandbox_policy.get_writable_roots_with_cwd(cwd);
    ensure_mount_targets_exist(&writable_roots)?;

    let mut args = if sandbox_policy.has_full_disk_read_access() {
        // Read-only root, then mount a minimal device tree.
        // In bubblewrap (`bubblewrap.c`, `SETUP_MOUNT_DEV`), `--dev /dev`
        // creates the standard minimal nodes: null, zero, full, random,
        // urandom, and tty. `/dev` must be mounted before writable roots so
        // explicit `/dev/*` writable binds remain visible.
        vec![
            "--ro-bind".to_string(),
            "/".to_string(),
            "/".to_string(),
            "--dev".to_string(),
            "/dev".to_string(),
        ]
    } else {
        // Start from an empty filesystem and add only the approved readable
        // roots plus a minimal `/dev`.
        let mut args = vec![
            "--tmpfs".to_string(),
            "/".to_string(),
            "--dev".to_string(),
            "/dev".to_string(),
        ];

        let mut readable_roots: BTreeSet<PathBuf> = sandbox_policy
            .get_readable_roots_with_cwd(cwd)
            .into_iter()
            .map(PathBuf::from)
            .collect();
        if sandbox_policy.include_platform_defaults() {
            readable_roots.extend(
                LINUX_PLATFORM_DEFAULT_READ_ROOTS
                    .iter()
                    .map(|path| PathBuf::from(*path))
                    .filter(|path| path.exists()),
            );
        }

        // A restricted policy can still explicitly request `/`, which is
        // semantically equivalent to broad read access.
        if readable_roots.iter().any(|root| root == Path::new("/")) {
            args = vec![
                "--ro-bind".to_string(),
                "/".to_string(),
                "/".to_string(),
                "--dev".to_string(),
                "/dev".to_string(),
            ];
        } else {
            for root in readable_roots {
                if !root.exists() {
                    continue;
                }
                args.push("--ro-bind".to_string());
                args.push(path_to_string(&root));
                args.push(path_to_string(&root));
            }
        }

        args
    };

    for writable_root in &writable_roots {
        let root = writable_root.root.as_path();
        args.push("--bind".to_string());
        args.push(path_to_string(root));
        args.push(path_to_string(root));
    }

    // Re-apply read-only subpaths after the writable binds so they win.
    let allowed_write_paths: Vec<PathBuf> = writable_roots
        .iter()
        .map(|writable_root| writable_root.root.as_path().to_path_buf())
        .collect();

    for subpath in collect_read_only_subpaths(&writable_roots) {
        if let Some(symlink_path) = find_symlink_in_path(&subpath, &allowed_write_paths) {
            args.push("--ro-bind".to_string());
            args.push("/dev/null".to_string());
            args.push(path_to_string(&symlink_path));
            continue;
        }

        if !subpath.exists() {
            // Keep this in the per-subpath loop: each protected subpath can have
            // a different first missing component that must be blocked
            // independently (for example, `/repo/.git` vs `/repo/.codex`).
            if let Some(first_missing_component) = find_first_non_existent_component(&subpath)
                && is_within_allowed_write_paths(&first_missing_component, &allowed_write_paths)
            {
                args.push("--ro-bind".to_string());
                args.push("/dev/null".to_string());
                args.push(path_to_string(&first_missing_component));
            }
            continue;
        }

        if is_within_allowed_write_paths(&subpath, &allowed_write_paths) {
            args.push("--ro-bind".to_string());
            args.push(path_to_string(&subpath));
            args.push(path_to_string(&subpath));
        }
    }

    Ok(args)
}

/// Collect unique read-only subpaths across all writable roots.
fn collect_read_only_subpaths(writable_roots: &[WritableRoot]) -> Vec<PathBuf> {
    let mut subpaths: BTreeSet<PathBuf> = BTreeSet::new();
    for writable_root in writable_roots {
        for subpath in &writable_root.read_only_subpaths {
            subpaths.insert(subpath.as_path().to_path_buf());
        }
    }
    subpaths.into_iter().collect()
}

/// Validate that writable roots exist before constructing mounts.
///
/// Bubblewrap requires bind mount targets to exist. We fail fast with a clear
/// error so callers can present an actionable message.
fn ensure_mount_targets_exist(writable_roots: &[WritableRoot]) -> Result<()> {
    for writable_root in writable_roots {
        let root = writable_root.root.as_path();
        if !root.exists() {
            return Err(CodexErr::UnsupportedOperation(format!(
                "Sandbox expected writable root {root}, but it does not exist.",
                root = root.display()
            )));
        }
    }
    Ok(())
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

/// Returns true when `path` is under any allowed writable root.
fn is_within_allowed_write_paths(path: &Path, allowed_write_paths: &[PathBuf]) -> bool {
    allowed_write_paths
        .iter()
        .any(|root| path.starts_with(root))
}

/// Find the first symlink along `target_path` that is also under a writable root.
///
/// This blocks symlink replacement attacks where a protected path is a symlink
/// inside a writable root (e.g., `.codex -> ./decoy`). In that case we mount
/// `/dev/null` on the symlink itself to prevent rewiring it.
fn find_symlink_in_path(target_path: &Path, allowed_write_paths: &[PathBuf]) -> Option<PathBuf> {
    let mut current = PathBuf::new();

    for component in target_path.components() {
        use std::path::Component;
        match component {
            Component::RootDir => {
                current.push(Path::new("/"));
                continue;
            }
            Component::CurDir => continue,
            Component::ParentDir => {
                current.pop();
                continue;
            }
            Component::Normal(part) => current.push(part),
            Component::Prefix(_) => continue,
        }

        let metadata = match std::fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(_) => break,
        };

        if metadata.file_type().is_symlink()
            && is_within_allowed_write_paths(&current, allowed_write_paths)
        {
            return Some(current);
        }
    }

    None
}

/// Find the first missing path component while walking `target_path`.
///
/// Mounting `/dev/null` on the first missing component prevents the sandboxed
/// process from creating the protected path hierarchy.
fn find_first_non_existent_component(target_path: &Path) -> Option<PathBuf> {
    let mut current = PathBuf::new();

    for component in target_path.components() {
        use std::path::Component;
        match component {
            Component::RootDir => {
                current.push(Path::new("/"));
                continue;
            }
            Component::CurDir => continue,
            Component::ParentDir => {
                current.pop();
                continue;
            }
            Component::Normal(part) => current.push(part),
            Component::Prefix(_) => continue,
        }

        if !current.exists() {
            return Some(current);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::protocol::ReadOnlyAccess;
    use codex_protocol::protocol::SandboxPolicy;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    #[test]
    fn full_disk_write_full_network_returns_unwrapped_command() {
        let command = vec!["/bin/true".to_string()];
        let args = create_bwrap_command_args(
            command.clone(),
            &SandboxPolicy::DangerFullAccess,
            Path::new("/"),
            BwrapOptions {
                mount_proc: true,
                network_mode: BwrapNetworkMode::FullAccess,
            },
        )
        .expect("create bwrap args");

        assert_eq!(args, command);
    }

    #[test]
    fn full_disk_write_proxy_only_keeps_full_filesystem_but_unshares_network() {
        let command = vec!["/bin/true".to_string()];
        let args = create_bwrap_command_args(
            command,
            &SandboxPolicy::DangerFullAccess,
            Path::new("/"),
            BwrapOptions {
                mount_proc: true,
                network_mode: BwrapNetworkMode::ProxyOnly,
            },
        )
        .expect("create bwrap args");

        assert_eq!(
            args,
            vec![
                "--new-session".to_string(),
                "--die-with-parent".to_string(),
                "--bind".to_string(),
                "/".to_string(),
                "/".to_string(),
                "--unshare-pid".to_string(),
                "--unshare-net".to_string(),
                "--proc".to_string(),
                "/proc".to_string(),
                "--".to_string(),
                "/bin/true".to_string(),
            ]
        );
    }

    #[test]
    fn mounts_dev_before_writable_dev_binds() {
        let sandbox_policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![AbsolutePathBuf::try_from(Path::new("/dev")).expect("/dev path")],
            read_only_access: Default::default(),
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        };

        let args = create_filesystem_args(&sandbox_policy, Path::new("/")).expect("bwrap fs args");
        assert_eq!(
            args,
            vec![
                "--ro-bind".to_string(),
                "/".to_string(),
                "/".to_string(),
                "--dev".to_string(),
                "/dev".to_string(),
                "--bind".to_string(),
                "/dev".to_string(),
                "/dev".to_string(),
                "--bind".to_string(),
                "/".to_string(),
                "/".to_string(),
            ]
        );
    }

    #[test]
    fn restricted_read_only_uses_scoped_read_roots_instead_of_erroring() {
        let temp_dir = TempDir::new().expect("temp dir");
        let readable_root = temp_dir.path().join("readable");
        std::fs::create_dir(&readable_root).expect("create readable root");

        let policy = SandboxPolicy::ReadOnly {
            access: ReadOnlyAccess::Restricted {
                include_platform_defaults: false,
                readable_roots: vec![
                    AbsolutePathBuf::try_from(readable_root.as_path())
                        .expect("absolute readable root"),
                ],
            },
        };

        let args = create_filesystem_args(&policy, temp_dir.path()).expect("filesystem args");

        assert_eq!(args[0..4], ["--tmpfs", "/", "--dev", "/dev"]);

        let readable_root_str = path_to_string(&readable_root);
        assert!(args.windows(3).any(|window| {
            window
                == [
                    "--ro-bind",
                    readable_root_str.as_str(),
                    readable_root_str.as_str(),
                ]
        }));
    }

    #[test]
    fn restricted_read_only_with_platform_defaults_includes_usr_when_present() {
        let temp_dir = TempDir::new().expect("temp dir");
        let policy = SandboxPolicy::ReadOnly {
            access: ReadOnlyAccess::Restricted {
                include_platform_defaults: true,
                readable_roots: Vec::new(),
            },
        };

        // `ReadOnlyAccess::Restricted` always includes `cwd` as a readable
        // root. Using `"/"` here would intentionally collapse to broad read
        // access, so use a non-root cwd to exercise the restricted path.
        let args = create_filesystem_args(&policy, temp_dir.path()).expect("filesystem args");

        assert!(args.starts_with(&["--tmpfs".to_string(), "/".to_string()]));

        if Path::new("/usr").exists() {
            assert!(
                args.windows(3)
                    .any(|window| window == ["--ro-bind", "/usr", "/usr"])
            );
        }
    }
}
