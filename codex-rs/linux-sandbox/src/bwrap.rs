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
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashSet;
use std::ffi::OsString;
use std::fs;
use std::fs::File;
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStringExt;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use codex_protocol::error::CodexErr;
use codex_protocol::error::Result;
use codex_protocol::protocol::FileSystemSandboxPolicy;
use codex_protocol::protocol::WritableRoot;
use codex_utils_absolute_path::AbsolutePathBuf;
use globset::GlobBuilder;
use globset::GlobSet;
use globset::GlobSetBuilder;

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

const MAX_UNREADABLE_GLOB_MATCHES: usize = 8192;

/// Options that control how bubblewrap is invoked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BwrapOptions {
    /// Whether to mount a fresh `/proc` inside the sandbox.
    ///
    /// This is the secure default, but some restrictive container environments
    /// deny `--proc /proc`.
    pub mount_proc: bool,
    /// How networking should be configured inside the bubblewrap sandbox.
    pub network_mode: BwrapNetworkMode,
    /// Optional maximum depth for expanding unreadable glob patterns with ripgrep.
    ///
    /// Keep this uncapped by default so existing nested deny-read matches are
    /// masked before the sandboxed command starts.
    pub glob_scan_max_depth: Option<usize>,
}

impl Default for BwrapOptions {
    fn default() -> Self {
        Self {
            mount_proc: true,
            network_mode: BwrapNetworkMode::FullAccess,
            glob_scan_max_depth: None,
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

#[derive(Debug)]
pub(crate) struct BwrapArgs {
    pub args: Vec<String>,
    pub preserved_files: Vec<File>,
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
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    sandbox_policy_cwd: &Path,
    command_cwd: &Path,
    options: BwrapOptions,
) -> Result<BwrapArgs> {
    let unreadable_globs =
        file_system_sandbox_policy.get_unreadable_globs_with_cwd(sandbox_policy_cwd);
    // Full disk write normally skips bwrap, but unreadable glob patterns still
    // need concrete bwrap masks for the matches expanded below.
    if file_system_sandbox_policy.has_full_disk_write_access() && unreadable_globs.is_empty() {
        return if options.network_mode == BwrapNetworkMode::FullAccess {
            Ok(BwrapArgs {
                args: command,
                preserved_files: Vec::new(),
            })
        } else {
            Ok(create_bwrap_flags_full_filesystem(command, options))
        };
    }

    create_bwrap_flags(
        command,
        file_system_sandbox_policy,
        sandbox_policy_cwd,
        command_cwd,
        options,
    )
}

fn create_bwrap_flags_full_filesystem(command: Vec<String>, options: BwrapOptions) -> BwrapArgs {
    let mut args = vec![
        "--new-session".to_string(),
        "--die-with-parent".to_string(),
        "--bind".to_string(),
        "/".to_string(),
        "/".to_string(),
        // Always enter a fresh user namespace so root inside a container does
        // not need ambient CAP_SYS_ADMIN to create the remaining namespaces.
        "--unshare-user".to_string(),
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
    BwrapArgs {
        args,
        preserved_files: Vec::new(),
    }
}

/// Build the bubblewrap flags (everything after `argv[0]`).
fn create_bwrap_flags(
    command: Vec<String>,
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    sandbox_policy_cwd: &Path,
    command_cwd: &Path,
    options: BwrapOptions,
) -> Result<BwrapArgs> {
    let BwrapArgs {
        args: filesystem_args,
        preserved_files,
    } = create_filesystem_args(
        file_system_sandbox_policy,
        sandbox_policy_cwd,
        options
            .glob_scan_max_depth
            .or(file_system_sandbox_policy.glob_scan_max_depth),
    )?;
    let normalized_command_cwd = normalize_command_cwd_for_bwrap(command_cwd);
    let mut args = Vec::new();
    args.push("--new-session".to_string());
    args.push("--die-with-parent".to_string());
    args.extend(filesystem_args);
    // Request a user namespace explicitly rather than relying on bubblewrap's
    // auto-enable behavior, which is skipped when the caller runs as uid 0.
    args.push("--unshare-user".to_string());
    args.push("--unshare-pid".to_string());
    if options.network_mode.should_unshare_network() {
        args.push("--unshare-net".to_string());
    }
    // Mount a fresh /proc unless the caller explicitly disables it.
    if options.mount_proc {
        args.push("--proc".to_string());
        args.push("/proc".to_string());
    }
    if normalized_command_cwd.as_path() != command_cwd {
        // Bubblewrap otherwise inherits the helper's logical cwd, which can be
        // a symlink alias that disappears once the sandbox only mounts
        // canonical roots. Enter the canonical command cwd explicitly so
        // relative paths stay aligned with the mounted filesystem view.
        args.push("--chdir".to_string());
        args.push(path_to_string(normalized_command_cwd.as_path()));
    }
    args.push("--".to_string());
    args.extend(command);
    Ok(BwrapArgs {
        args,
        preserved_files,
    })
}

/// Build the bubblewrap filesystem mounts for a given filesystem policy.
///
/// The mount order is important:
/// 1. Full-read policies, and restricted policies that explicitly read `/`,
///    use `--ro-bind / /`; other restricted-read policies start from
///    `--tmpfs /` and layer scoped `--ro-bind` mounts.
/// 2. `--dev /dev` mounts a minimal writable `/dev` with standard device nodes
///    (including `/dev/urandom`) even under a read-only root.
/// 3. Unreadable ancestors of writable roots are masked before their child
///    mounts are rebound so nested writable carveouts can be reopened safely.
/// 4. `--bind <root> <root>` re-enables writes for allowed roots, including
///    writable subpaths under `/dev` (for example, `/dev/shm`).
/// 5. `--ro-bind <subpath> <subpath>` re-applies read-only protections under
///    those writable roots so protected subpaths win.
/// 6. Nested unreadable carveouts under a writable root are masked after that
///    root is bound, and unrelated unreadable roots are masked afterward.
fn create_filesystem_args(
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    cwd: &Path,
    glob_scan_max_depth: Option<usize>,
) -> Result<BwrapArgs> {
    let unreadable_globs = file_system_sandbox_policy.get_unreadable_globs_with_cwd(cwd);
    // Bubblewrap requires bind mount targets to exist. Skip missing writable
    // roots so mixed-platform configs can keep harmless paths for other
    // environments without breaking Linux command startup.
    let mut writable_roots = file_system_sandbox_policy
        .get_writable_roots_with_cwd(cwd)
        .into_iter()
        .filter(|writable_root| writable_root.root.as_path().exists())
        .collect::<Vec<_>>();
    if writable_roots.is_empty()
        && file_system_sandbox_policy.has_full_disk_write_access()
        && !unreadable_globs.is_empty()
    {
        writable_roots.push(WritableRoot {
            root: AbsolutePathBuf::from_absolute_path("/")?,
            read_only_subpaths: Vec::new(),
        });
    }
    let mut unreadable_roots = file_system_sandbox_policy
        .get_unreadable_roots_with_cwd(cwd)
        .into_iter()
        .map(AbsolutePathBuf::into_path_buf)
        .collect::<Vec<_>>();
    // Bubblewrap can only mask concrete paths. Expand unreadable glob patterns
    // to the existing matches we can see before constructing the mount overlay;
    // core tool helpers still evaluate the original patterns directly at read time.
    unreadable_roots.extend(
        expand_unreadable_globs_with_ripgrep(&unreadable_globs, cwd, glob_scan_max_depth)?
            .into_iter()
            .map(AbsolutePathBuf::into_path_buf),
    );
    unreadable_roots.sort();
    unreadable_roots.dedup();

    let mut args = if file_system_sandbox_policy.has_full_disk_read_access() {
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

        let mut readable_roots: BTreeSet<PathBuf> = file_system_sandbox_policy
            .get_readable_roots_with_cwd(cwd)
            .into_iter()
            .map(PathBuf::from)
            .collect();
        if file_system_sandbox_policy.include_platform_defaults() {
            readable_roots.extend(
                LINUX_PLATFORM_DEFAULT_READ_ROOTS
                    .iter()
                    .map(|path| PathBuf::from(*path))
                    .filter(|path| path.exists()),
            );
        }

        // A restricted policy can still explicitly request `/`, which is
        // the broad read baseline. Explicit unreadable carveouts are
        // re-applied later.
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
                // Writable roots are rebound by real target below; mirror that
                // for their restricted-read bootstrap mount. Plain read-only
                // roots must stay logical because callers may execute those
                // paths inside bwrap, such as Bazel runfiles helper binaries.
                let mount_root = if writable_roots
                    .iter()
                    .any(|writable_root| root.starts_with(writable_root.root.as_path()))
                {
                    canonical_target_if_symlinked_path(&root).unwrap_or(root)
                } else {
                    root
                };
                args.push("--ro-bind".to_string());
                args.push(path_to_string(&mount_root));
                args.push(path_to_string(&mount_root));
            }
        }

        args
    };
    let mut preserved_files = Vec::new();
    let mut allowed_write_paths = Vec::with_capacity(writable_roots.len());
    for writable_root in &writable_roots {
        let root = writable_root.root.as_path();
        allowed_write_paths.push(root.to_path_buf());
        if let Some(target) = canonical_target_if_symlinked_path(root) {
            allowed_write_paths.push(target);
        }
    }
    let unreadable_paths: HashSet<PathBuf> = unreadable_roots.iter().cloned().collect();
    let mut sorted_writable_roots = writable_roots;
    sorted_writable_roots.sort_by_key(|writable_root| path_depth(writable_root.root.as_path()));
    // Mask only the unreadable ancestors that sit outside every writable root.
    // Unreadable paths nested under a broader writable root are applied after
    // that broader root is bound, then reopened by any deeper writable child.
    let mut unreadable_ancestors_of_writable_roots: Vec<PathBuf> = unreadable_roots
        .iter()
        .filter(|path| {
            let unreadable_root = path.as_path();
            !allowed_write_paths
                .iter()
                .any(|root| unreadable_root.starts_with(root))
                && allowed_write_paths
                    .iter()
                    .any(|root| root.starts_with(unreadable_root))
        })
        .cloned()
        .collect();
    unreadable_ancestors_of_writable_roots.sort_by_key(|path| path_depth(path));

    for unreadable_root in &unreadable_ancestors_of_writable_roots {
        append_unreadable_root_args(
            &mut args,
            &mut preserved_files,
            unreadable_root,
            &allowed_write_paths,
        )?;
    }

    for writable_root in &sorted_writable_roots {
        let root = writable_root.root.as_path();
        let symlink_target = canonical_target_if_symlinked_path(root);
        // If a denied ancestor was already masked, recreate any missing mount
        // target parents before binding the narrower writable descendant.
        if let Some(masking_root) = unreadable_roots
            .iter()
            .map(PathBuf::as_path)
            .filter(|unreadable_root| root.starts_with(unreadable_root))
            .max_by_key(|unreadable_root| path_depth(unreadable_root))
        {
            append_mount_target_parent_dir_args(&mut args, root, masking_root);
        }

        let mount_root = symlink_target.as_deref().unwrap_or(root);
        args.push("--bind".to_string());
        args.push(path_to_string(mount_root));
        args.push(path_to_string(mount_root));

        let mut read_only_subpaths: Vec<PathBuf> = writable_root
            .read_only_subpaths
            .iter()
            .map(|path| path.as_path().to_path_buf())
            .filter(|path| !unreadable_paths.contains(path))
            .collect();
        if let Some(target) = &symlink_target {
            read_only_subpaths = remap_paths_for_symlink_target(read_only_subpaths, root, target);
        }
        read_only_subpaths.sort_by_key(|path| path_depth(path));
        for subpath in read_only_subpaths {
            append_read_only_subpath_args(&mut args, &subpath, &allowed_write_paths)?;
        }
        let mut nested_unreadable_roots: Vec<PathBuf> = unreadable_roots
            .iter()
            .filter(|path| path.starts_with(root))
            .cloned()
            .collect();
        if let Some(target) = &symlink_target {
            nested_unreadable_roots =
                remap_paths_for_symlink_target(nested_unreadable_roots, root, target);
        }
        nested_unreadable_roots.sort_by_key(|path| path_depth(path));
        for unreadable_root in nested_unreadable_roots {
            append_unreadable_root_args(
                &mut args,
                &mut preserved_files,
                &unreadable_root,
                &allowed_write_paths,
            )?;
        }
    }

    let mut rootless_unreadable_roots: Vec<PathBuf> = unreadable_roots
        .iter()
        .filter(|path| {
            let unreadable_root = path.as_path();
            !allowed_write_paths
                .iter()
                .any(|root| unreadable_root.starts_with(root) || root.starts_with(unreadable_root))
        })
        .cloned()
        .collect();
    rootless_unreadable_roots.sort_by_key(|path| path_depth(path));
    for unreadable_root in rootless_unreadable_roots {
        append_unreadable_root_args(
            &mut args,
            &mut preserved_files,
            &unreadable_root,
            &allowed_write_paths,
        )?;
    }

    Ok(BwrapArgs {
        args,
        preserved_files,
    })
}

fn expand_unreadable_globs_with_ripgrep(
    patterns: &[String],
    cwd: &Path,
    max_depth: Option<usize>,
) -> Result<Vec<AbsolutePathBuf>> {
    if patterns.is_empty() || max_depth == Some(0) {
        return Ok(Vec::new());
    }

    // Group each pattern by the static path prefix before its first glob
    // metacharacter. That keeps scans narrow, avoids searching from `/`, and
    // lets one `rg --files` call handle all patterns under the same root.
    let mut patterns_by_search_root: BTreeMap<AbsolutePathBuf, Vec<String>> = BTreeMap::new();
    for pattern in patterns {
        if let Some((search_root, glob)) = split_pattern_for_ripgrep(pattern, cwd)
            && search_root.as_path().is_dir()
        {
            patterns_by_search_root
                .entry(search_root)
                .or_default()
                .push(glob);
        }
    }

    // Record both the logical match and any canonical symlink target. The bwrap
    // overlay needs the resolved target to prevent a readable symlink path from
    // bypassing an unreadable glob match.
    let mut expanded_paths = BTreeSet::new();
    for (search_root, globs) in patterns_by_search_root {
        for path in ripgrep_files(search_root.as_path(), &globs, max_depth)? {
            if let Some(target) = canonical_target_if_symlinked_path(path.as_path()) {
                expanded_paths.insert(AbsolutePathBuf::from_absolute_path_checked(target)?);
            }
            expanded_paths.insert(path);
            if expanded_paths.len() > MAX_UNREADABLE_GLOB_MATCHES {
                return Err(CodexErr::Fatal(format!(
                    "unreadable glob expansion for {} matched more than {MAX_UNREADABLE_GLOB_MATCHES} paths",
                    search_root.display()
                )));
            }
        }
    }

    Ok(expanded_paths.into_iter().collect())
}

fn split_pattern_for_ripgrep(pattern: &str, cwd: &Path) -> Option<(AbsolutePathBuf, String)> {
    // Resolve relative patterns once, then split at the first glob
    // metacharacter. The prefix becomes the search root and the suffix stays as
    // the ripgrep glob. Root-level glob scans are intentionally skipped because
    // they are too broad for startup-time sandbox construction.
    let absolute_pattern = AbsolutePathBuf::resolve_path_against_base(pattern, cwd);
    let pattern = absolute_pattern.to_string_lossy();
    let first_glob_index = pattern
        .char_indices()
        .find_map(|(index, ch)| matches!(ch, '*' | '?' | '[' | ']').then_some(index))?;
    let static_prefix = &pattern[..first_glob_index];
    if static_prefix.is_empty() || static_prefix == "/" {
        return None;
    }
    let search_root_end = if static_prefix.ends_with('/') {
        static_prefix.len() - 1
    } else {
        static_prefix.rfind('/').unwrap_or(0)
    };
    let search_root = if search_root_end == 0 {
        PathBuf::from("/")
    } else {
        PathBuf::from(&pattern[..search_root_end])
    };
    let search_root = AbsolutePathBuf::from_absolute_path_checked(search_root).ok()?;
    let glob = escape_unclosed_glob_classes(&pattern[search_root_end + 1..]);
    (!glob.is_empty()).then_some((search_root, glob))
}

fn escape_unclosed_glob_classes(glob: &str) -> String {
    // The filesystem policy accepts an unclosed `[` as a literal. Ripgrep treats
    // that as invalid glob syntax, so escape only the unclosed class opener.
    let mut escaped = String::with_capacity(glob.len());
    let mut chars = glob.chars();

    while let Some(ch) = chars.next() {
        if ch != '[' {
            escaped.push(ch);
            continue;
        }

        let mut class = String::new();
        let mut closed = false;
        for class_ch in chars.by_ref() {
            if class_ch == ']' {
                closed = true;
                break;
            }
            class.push(class_ch);
        }

        if closed {
            escaped.push('[');
            escaped.push_str(&class);
            escaped.push(']');
        } else {
            escaped.push_str(r"\[");
            escaped.push_str(&class);
        }
    }

    escaped
}

fn ripgrep_files(
    search_root: &Path,
    globs: &[String],
    max_depth: Option<usize>,
) -> Result<Vec<AbsolutePathBuf>> {
    // Use `rg --files` rather than shell expansion so dotfiles and ignored files
    // are still considered. A status 1 with no stderr is ripgrep's "no matches"
    // case, not a sandbox construction error.
    let mut command = Command::new("rg");
    command
        .arg("--files")
        .arg("--hidden")
        .arg("--no-ignore")
        .arg("--null");
    if let Some(max_depth) = max_depth {
        command.arg("--max-depth").arg(max_depth.to_string());
    }
    for glob in globs {
        command.arg("--glob").arg(glob);
    }
    command.arg("--").arg(search_root);

    /*
     * Prefer ripgrep for unreadable glob expansion because it is fast and
     * already implements the file-walking semantics we want here: include
     * dotfiles, ignore ignore files, and do not recurse through symlinked
     * directories. If `rg` is not installed in the runtime environment, fall
     * back to the internal globset walker so sandbox construction still masks
     * matching paths. Other ripgrep failures stay fatal so deny-read does not
     * silently weaken.
     */
    let output = match command.output() {
        Ok(output) => output,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return glob_files(search_root, globs, max_depth);
        }
        Err(err) => return Err(err.into()),
    };
    if !output.status.success() {
        if output.status.code() == Some(1) && output.stderr.is_empty() {
            return Ok(Vec::new());
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CodexErr::Fatal(format!(
            "ripgrep unreadable glob scan failed for {}: {stderr}",
            search_root.display()
        )));
    }

    let paths = output
        .stdout
        .split(|byte| *byte == b'\0')
        .filter(|path| !path.is_empty())
        .map(|path| {
            let path = PathBuf::from(OsString::from_vec(path.to_vec()));
            if path.is_absolute() {
                path
            } else {
                search_root.join(path)
            }
        })
        .map(AbsolutePathBuf::from_absolute_path_checked)
        .collect::<io::Result<Vec<_>>>()?;
    Ok(paths)
}

fn glob_files(
    search_root: &Path,
    globs: &[String],
    max_depth: Option<usize>,
) -> Result<Vec<AbsolutePathBuf>> {
    let mut builder = GlobSetBuilder::new();
    for glob in globs {
        let glob = GlobBuilder::new(glob)
            .literal_separator(true)
            .allow_unclosed_class(true)
            .build()
            .map_err(|err| {
                CodexErr::Fatal(format!(
                    "unreadable glob pattern is invalid for {}: {err}",
                    search_root.display()
                ))
            })?;
        builder.add(glob);
    }
    let glob_set = builder.build().map_err(|err| {
        CodexErr::Fatal(format!(
            "unreadable glob matcher failed for {}: {err}",
            search_root.display()
        ))
    })?;

    let mut paths = Vec::new();
    collect_glob_files(search_root, search_root, &glob_set, max_depth, &mut paths)?;
    Ok(paths)
}

fn collect_glob_files(
    search_root: &Path,
    dir: &Path,
    glob_set: &GlobSet,
    remaining_depth: Option<usize>,
    paths: &mut Vec<AbsolutePathBuf>,
) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        let relative = path.strip_prefix(search_root).unwrap_or(path.as_path());

        if (file_type.is_file() || file_type.is_symlink()) && glob_set.is_match(relative) {
            paths.push(AbsolutePathBuf::from_absolute_path_checked(&path)?);
        }

        if !file_type.is_dir() {
            continue;
        }
        let remaining_depth = match remaining_depth {
            Some(0 | 1) => continue,
            Some(depth) => Some(depth - 1),
            None => None,
        };
        collect_glob_files(search_root, &path, glob_set, remaining_depth, paths)?;
    }
    Ok(())
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn path_depth(path: &Path) -> usize {
    path.components().count()
}

fn canonical_target_if_symlinked_path(path: &Path) -> Option<PathBuf> {
    // Return the fully resolved target only when some path component is a
    // symlink. Callers use this to bind/mask the real filesystem location while
    // leaving ordinary paths in their logical form.
    let mut current = PathBuf::new();
    for component in path.components() {
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

        let metadata = match fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(_) => return None,
        };
        if metadata.file_type().is_symlink() {
            let target = fs::canonicalize(path).ok()?;
            if target.as_path() == path {
                return None;
            }
            return Some(target);
        }
    }
    None
}

fn remap_paths_for_symlink_target(paths: Vec<PathBuf>, root: &Path, target: &Path) -> Vec<PathBuf> {
    paths
        .into_iter()
        .map(|path| {
            if let Ok(relative) = path.strip_prefix(root) {
                target.join(relative)
            } else {
                path
            }
        })
        .collect()
}

fn normalize_command_cwd_for_bwrap(command_cwd: &Path) -> PathBuf {
    command_cwd
        .canonicalize()
        .unwrap_or_else(|_| command_cwd.to_path_buf())
}

fn append_mount_target_parent_dir_args(args: &mut Vec<String>, mount_target: &Path, anchor: &Path) {
    let mount_target_dir = if mount_target.is_dir() {
        mount_target
    } else if let Some(parent) = mount_target.parent() {
        parent
    } else {
        return;
    };
    let mut mount_target_dirs: Vec<PathBuf> = mount_target_dir
        .ancestors()
        .take_while(|path| *path != anchor)
        .map(Path::to_path_buf)
        .collect();
    mount_target_dirs.reverse();
    for mount_target_dir in mount_target_dirs {
        args.push("--dir".to_string());
        args.push(path_to_string(&mount_target_dir));
    }
}

fn append_read_only_subpath_args(
    args: &mut Vec<String>,
    subpath: &Path,
    allowed_write_paths: &[PathBuf],
) -> Result<()> {
    if let Some(symlink) = first_writable_symlink_component_in_path(subpath, allowed_write_paths) {
        /*
         * A read-only carveout under a writable symlink cannot be made reliable
         * with bwrap path arguments. Binding the symlink's current target would
         * only protect a startup-time snapshot; the sandboxed process could
         * replace the writable symlink before it reads through the logical path.
         */
        return Err(CodexErr::Fatal(format!(
            "cannot enforce sandbox read-only path {} because it crosses writable symlink {}",
            subpath.display(),
            symlink.display()
        )));
    }

    if !subpath.exists() {
        if let Some(first_missing_component) = find_first_non_existent_component(subpath)
            && is_within_allowed_write_paths(&first_missing_component, allowed_write_paths)
        {
            args.push("--ro-bind".to_string());
            args.push("/dev/null".to_string());
            args.push(path_to_string(&first_missing_component));
        }
        return Ok(());
    }

    if is_within_allowed_write_paths(subpath, allowed_write_paths) {
        args.push("--ro-bind".to_string());
        args.push(path_to_string(subpath));
        args.push(path_to_string(subpath));
    }
    Ok(())
}

fn append_unreadable_root_args(
    args: &mut Vec<String>,
    preserved_files: &mut Vec<File>,
    unreadable_root: &Path,
    allowed_write_paths: &[PathBuf],
) -> Result<()> {
    if let Some(symlink) =
        first_writable_symlink_component_in_path(unreadable_root, allowed_write_paths)
    {
        /*
         * Deny-read masks must fail closed when the protected path crosses a
         * symlink that remains writable to the sandboxed process. Resolving and
         * masking the symlink's current target is a TOCTTOU snapshot: bwrap would
         * protect the old target while the logical path could later point
         * somewhere else.
         */
        return Err(CodexErr::Fatal(format!(
            "cannot enforce sandbox deny-read path {} because it crosses writable symlink {}",
            unreadable_root.display(),
            symlink.display()
        )));
    }

    if !unreadable_root.exists() {
        if let Some(first_missing_component) = find_first_non_existent_component(unreadable_root)
            && is_within_allowed_write_paths(&first_missing_component, allowed_write_paths)
        {
            args.push("--ro-bind".to_string());
            args.push("/dev/null".to_string());
            args.push(path_to_string(&first_missing_component));
        }
        return Ok(());
    }

    append_existing_unreadable_path_args(
        args,
        preserved_files,
        unreadable_root,
        allowed_write_paths,
    )
}

fn append_existing_unreadable_path_args(
    args: &mut Vec<String>,
    preserved_files: &mut Vec<File>,
    unreadable_root: &Path,
    allowed_write_paths: &[PathBuf],
) -> Result<()> {
    if unreadable_root.is_dir() {
        let mut writable_descendants: Vec<&Path> = allowed_write_paths
            .iter()
            .map(PathBuf::as_path)
            .filter(|path| *path != unreadable_root && path.starts_with(unreadable_root))
            .collect();
        args.push("--perms".to_string());
        // Execute-only perms let the process traverse into explicitly
        // re-opened writable descendants while still hiding the denied
        // directory contents. Plain denied directories with no writable child
        // mounts stay at `000`.
        args.push(if writable_descendants.is_empty() {
            "000".to_string()
        } else {
            "111".to_string()
        });
        args.push("--tmpfs".to_string());
        args.push(path_to_string(unreadable_root));
        // Recreate any writable descendants inside the tmpfs before remounting
        // the denied parent read-only. Otherwise bubblewrap cannot mkdir the
        // nested mount targets after the parent has been frozen.
        writable_descendants.sort_by_key(|path| path_depth(path));
        for writable_descendant in writable_descendants {
            append_mount_target_parent_dir_args(args, writable_descendant, unreadable_root);
        }
        args.push("--remount-ro".to_string());
        args.push(path_to_string(unreadable_root));
        return Ok(());
    }

    if preserved_files.is_empty() {
        preserved_files.push(File::open("/dev/null")?);
    }
    let null_fd = preserved_files[0].as_raw_fd().to_string();
    args.push("--perms".to_string());
    args.push("000".to_string());
    args.push("--ro-bind-data".to_string());
    args.push(null_fd);
    args.push(path_to_string(unreadable_root));
    Ok(())
}

/// Returns true when `path` is under any allowed writable root.
fn is_within_allowed_write_paths(path: &Path, allowed_write_paths: &[PathBuf]) -> bool {
    allowed_write_paths
        .iter()
        .any(|root| path.starts_with(root))
}

fn first_writable_symlink_component_in_path(
    target_path: &Path,
    allowed_write_paths: &[PathBuf],
) -> Option<PathBuf> {
    /*
     * Walk the logical path and report the first symlink component that lives
     * under a writable root. These symlinks are mutable from inside the sandbox,
     * so any mount or mask based on their resolved target would be racing a path
     * the sandboxed process can change.
     */
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
    use codex_protocol::protocol::FileSystemAccessMode;
    use codex_protocol::protocol::FileSystemPath;
    use codex_protocol::protocol::FileSystemSandboxEntry;
    use codex_protocol::protocol::FileSystemSandboxPolicy;
    use codex_protocol::protocol::FileSystemSpecialPath;
    use codex_protocol::protocol::ReadOnlyAccess;
    use codex_protocol::protocol::SandboxPolicy;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    const NO_UNREADABLE_GLOB_SCAN_MAX_DEPTH: Option<usize> = None;

    #[test]
    fn default_unreadable_glob_scan_has_no_depth_cap() {
        assert_eq!(BwrapOptions::default().glob_scan_max_depth, None);
    }

    fn unreadable_glob_entry(pattern: String) -> FileSystemSandboxEntry {
        FileSystemSandboxEntry {
            path: FileSystemPath::GlobPattern { pattern },
            access: FileSystemAccessMode::None,
        }
    }

    fn default_policy_with_unreadable_glob(pattern: String) -> FileSystemSandboxPolicy {
        let mut policy = FileSystemSandboxPolicy::default();
        policy.entries.push(unreadable_glob_entry(pattern));
        policy
    }

    #[test]
    fn full_disk_write_full_network_returns_unwrapped_command() {
        let command = vec!["/bin/true".to_string()];
        let args = create_bwrap_command_args(
            command.clone(),
            &FileSystemSandboxPolicy::from(&SandboxPolicy::DangerFullAccess),
            Path::new("/"),
            Path::new("/"),
            BwrapOptions {
                mount_proc: true,
                network_mode: BwrapNetworkMode::FullAccess,
                ..Default::default()
            },
        )
        .expect("create bwrap args");

        assert_eq!(args.args, command);
    }

    #[test]
    fn full_disk_write_proxy_only_keeps_full_filesystem_but_unshares_network() {
        let command = vec!["/bin/true".to_string()];
        let args = create_bwrap_command_args(
            command,
            &FileSystemSandboxPolicy::from(&SandboxPolicy::DangerFullAccess),
            Path::new("/"),
            Path::new("/"),
            BwrapOptions {
                mount_proc: true,
                network_mode: BwrapNetworkMode::ProxyOnly,
                ..Default::default()
            },
        )
        .expect("create bwrap args");

        assert_eq!(
            args.args,
            vec![
                "--new-session".to_string(),
                "--die-with-parent".to_string(),
                "--bind".to_string(),
                "/".to_string(),
                "/".to_string(),
                "--unshare-user".to_string(),
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
    fn full_disk_write_with_unreadable_glob_still_wraps_and_masks_match() {
        if !ripgrep_available() {
            return;
        }

        let temp_dir = TempDir::new().expect("temp dir");
        let root_env = temp_dir.path().join(".env");
        std::fs::write(&root_env, "secret").expect("write env");
        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                access: FileSystemAccessMode::Write,
            },
            unreadable_glob_entry(format!("{}/**/*.env", temp_dir.path().display())),
        ]);
        let command = vec!["/bin/true".to_string()];

        let args = create_bwrap_command_args(
            command.clone(),
            &policy,
            temp_dir.path(),
            temp_dir.path(),
            BwrapOptions::default(),
        )
        .expect("create bwrap args");

        assert_ne!(
            args.args, command,
            "full-write policy with unreadable globs must still use bwrap"
        );
        assert_file_masked(&args.args, &root_env);
    }

    #[cfg(unix)]
    #[test]
    fn restricted_policy_chdirs_to_canonical_command_cwd() {
        let temp_dir = TempDir::new().expect("temp dir");
        let real_root = temp_dir.path().join("real");
        let real_subdir = real_root.join("subdir");
        let link_root = temp_dir.path().join("link");
        std::fs::create_dir_all(&real_subdir).expect("create real subdir");
        std::os::unix::fs::symlink(&real_root, &link_root).expect("create symlinked root");

        let sandbox_policy_cwd = AbsolutePathBuf::from_absolute_path(&link_root)
            .expect("absolute symlinked root")
            .to_path_buf();
        let command_cwd = link_root.join("subdir");
        let canonical_command_cwd = real_subdir
            .canonicalize()
            .expect("canonicalize command cwd");
        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::Minimal,
                },
                access: FileSystemAccessMode::Read,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::CurrentWorkingDirectory,
                },
                access: FileSystemAccessMode::Write,
            },
        ]);

        let args = create_bwrap_command_args(
            vec!["/bin/true".to_string()],
            &policy,
            sandbox_policy_cwd.as_path(),
            &command_cwd,
            BwrapOptions::default(),
        )
        .expect("create bwrap args");
        let canonical_sandbox_cwd = path_to_string(
            &real_root
                .canonicalize()
                .expect("canonicalize sandbox policy cwd"),
        );
        let canonical_command_cwd = path_to_string(&canonical_command_cwd);
        let link_sandbox_cwd = path_to_string(&link_root);
        let link_command_cwd = path_to_string(&command_cwd);

        assert!(
            args.args
                .windows(2)
                .any(|window| { window == ["--chdir", canonical_command_cwd.as_str()] })
        );
        assert!(args.args.windows(3).any(|window| {
            window
                == [
                    "--ro-bind",
                    canonical_sandbox_cwd.as_str(),
                    canonical_sandbox_cwd.as_str(),
                ]
        }));
        assert!(
            !args
                .args
                .windows(2)
                .any(|window| { window == ["--chdir", link_command_cwd.as_str()] })
        );
        assert!(!args.args.windows(3).any(|window| {
            window
                == [
                    "--ro-bind",
                    link_sandbox_cwd.as_str(),
                    link_sandbox_cwd.as_str(),
                ]
        }));
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_writable_roots_bind_real_target_and_remap_carveouts() {
        let temp_dir = TempDir::new().expect("temp dir");
        let real_root = temp_dir.path().join("real");
        let link_root = temp_dir.path().join("link");
        let blocked = real_root.join("blocked");
        std::fs::create_dir_all(&blocked).expect("create blocked dir");
        std::os::unix::fs::symlink(&real_root, &link_root).expect("create symlinked root");

        let link_root =
            AbsolutePathBuf::from_absolute_path(&link_root).expect("absolute symlinked root");
        let link_blocked = link_root.join("blocked");
        let real_root_str = path_to_string(&real_root);
        let real_blocked_str = path_to_string(&blocked);
        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Path { path: link_root },
                access: FileSystemAccessMode::Write,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Path { path: link_blocked },
                access: FileSystemAccessMode::None,
            },
        ]);

        let args =
            create_filesystem_args(&policy, temp_dir.path(), NO_UNREADABLE_GLOB_SCAN_MAX_DEPTH)
                .expect("filesystem args");

        assert!(args.args.windows(3).any(|window| {
            window == ["--bind", real_root_str.as_str(), real_root_str.as_str()]
        }));
        assert!(args.args.windows(6).any(|window| {
            window
                == [
                    "--perms",
                    "000",
                    "--tmpfs",
                    real_blocked_str.as_str(),
                    "--remount-ro",
                    real_blocked_str.as_str(),
                ]
        }));
    }

    #[cfg(unix)]
    #[test]
    fn writable_roots_under_symlinked_ancestors_bind_real_target() {
        let temp_dir = TempDir::new().expect("temp dir");
        let logical_home = temp_dir.path().join("home");
        let real_codex = temp_dir.path().join("real-codex");
        let logical_codex = logical_home.join(".codex");
        let real_memories = real_codex.join("memories");
        let logical_memories = logical_codex.join("memories");
        std::fs::create_dir_all(&logical_home).expect("create logical home");
        std::fs::create_dir_all(&real_memories).expect("create memories dir");
        std::os::unix::fs::symlink(&real_codex, &logical_codex)
            .expect("create symlinked codex home");

        let logical_memories_root =
            AbsolutePathBuf::from_absolute_path(&logical_memories).expect("absolute memories");
        let real_memories_str = path_to_string(&real_memories);
        let logical_memories_str = path_to_string(&logical_memories);
        let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: logical_memories_root,
            },
            access: FileSystemAccessMode::Write,
        }]);

        let args =
            create_filesystem_args(&policy, temp_dir.path(), NO_UNREADABLE_GLOB_SCAN_MAX_DEPTH)
                .expect("filesystem args");

        assert!(args.args.windows(3).any(|window| {
            window
                == [
                    "--bind",
                    real_memories_str.as_str(),
                    real_memories_str.as_str(),
                ]
        }));
        assert!(!args.args.windows(3).any(|window| {
            window
                == [
                    "--bind",
                    logical_memories_str.as_str(),
                    logical_memories_str.as_str(),
                ]
        }));
    }

    #[cfg(unix)]
    #[test]
    fn protected_symlinked_directory_subpaths_fail_closed() {
        let temp_dir = TempDir::new().expect("temp dir");
        let root = temp_dir.path().join("root");
        let agents_target = root.join("agents-target");
        let agents_link = root.join(".agents");
        std::fs::create_dir_all(&agents_target).expect("create agents target");
        std::os::unix::fs::symlink(&agents_target, &agents_link).expect("create symlinked .agents");

        let root = AbsolutePathBuf::from_absolute_path(&root).expect("absolute root");
        let agents_link_str = path_to_string(&agents_link);
        let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Path { path: root },
            access: FileSystemAccessMode::Write,
        }]);

        let err =
            create_filesystem_args(&policy, temp_dir.path(), NO_UNREADABLE_GLOB_SCAN_MAX_DEPTH)
                .expect_err("protected symlinked subpath should fail closed");
        let message = err.to_string();

        assert!(
            message.contains("cannot enforce sandbox read-only path"),
            "{message}"
        );
        assert!(message.contains(&agents_link_str), "{message}");
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_writable_roots_nested_symlink_escape_paths_fail_closed() {
        let temp_dir = TempDir::new().expect("temp dir");
        let real_root = temp_dir.path().join("real");
        let link_root = temp_dir.path().join("link");
        let outside = temp_dir.path().join("outside-private");
        let linked_private = real_root.join("linked-private");
        std::fs::create_dir_all(&real_root).expect("create real root");
        std::fs::create_dir_all(&outside).expect("create outside dir");
        std::os::unix::fs::symlink(&real_root, &link_root).expect("create symlinked root");
        std::os::unix::fs::symlink(&outside, &linked_private)
            .expect("create nested escape symlink");

        let link_root =
            AbsolutePathBuf::from_absolute_path(&link_root).expect("absolute symlinked root");
        let link_private = link_root.join("linked-private");
        let real_linked_private_str = path_to_string(&linked_private);
        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Path { path: link_root },
                access: FileSystemAccessMode::Write,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Path { path: link_private },
                access: FileSystemAccessMode::None,
            },
        ]);

        let err =
            create_filesystem_args(&policy, temp_dir.path(), NO_UNREADABLE_GLOB_SCAN_MAX_DEPTH)
                .expect_err("deny-read path crossing writable symlink should fail closed");
        let message = err.to_string();

        assert!(
            message.contains("cannot enforce sandbox deny-read path"),
            "{message}"
        );
        assert!(message.contains(&real_linked_private_str), "{message}");
    }

    #[test]
    fn ignores_missing_writable_roots() {
        let temp_dir = TempDir::new().expect("temp dir");
        let existing_root = temp_dir.path().join("existing");
        let missing_root = temp_dir.path().join("missing");
        std::fs::create_dir(&existing_root).expect("create existing root");

        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![
                AbsolutePathBuf::try_from(existing_root.as_path()).expect("absolute existing root"),
                AbsolutePathBuf::try_from(missing_root.as_path()).expect("absolute missing root"),
            ],
            read_only_access: Default::default(),
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        };

        let args = create_filesystem_args(
            &FileSystemSandboxPolicy::from(&policy),
            temp_dir.path(),
            NO_UNREADABLE_GLOB_SCAN_MAX_DEPTH,
        )
        .expect("filesystem args");
        let existing_root = path_to_string(&existing_root);
        let missing_root = path_to_string(&missing_root);

        assert!(
            args.args.windows(3).any(|window| {
                window == ["--bind", existing_root.as_str(), existing_root.as_str()]
            }),
            "existing writable root should be rebound writable",
        );
        assert!(
            !args.args.iter().any(|arg| arg == &missing_root),
            "missing writable root should be skipped",
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

        let args = create_filesystem_args(
            &FileSystemSandboxPolicy::from(&sandbox_policy),
            Path::new("/"),
            NO_UNREADABLE_GLOB_SCAN_MAX_DEPTH,
        )
        .expect("bwrap fs args");
        assert_eq!(
            args.args,
            vec![
                // Start from a read-only view of the full filesystem.
                "--ro-bind".to_string(),
                "/".to_string(),
                "/".to_string(),
                // Recreate a writable /dev inside the sandbox.
                "--dev".to_string(),
                "/dev".to_string(),
                // Make the writable root itself writable again.
                "--bind".to_string(),
                "/".to_string(),
                "/".to_string(),
                // Mask the default protected .codex subpath under that writable
                // root. Because the root is `/` in this test, the carveout path
                // appears as `/.codex`.
                "--ro-bind".to_string(),
                "/dev/null".to_string(),
                "/.codex".to_string(),
                // Rebind /dev after the root bind so device nodes remain
                // writable/usable inside the writable root.
                "--bind".to_string(),
                "/dev".to_string(),
                "/dev".to_string(),
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
            network_access: false,
        };

        let args = create_filesystem_args(
            &FileSystemSandboxPolicy::from(&policy),
            temp_dir.path(),
            NO_UNREADABLE_GLOB_SCAN_MAX_DEPTH,
        )
        .expect("filesystem args");

        assert_eq!(args.args[0..4], ["--tmpfs", "/", "--dev", "/dev"]);

        let readable_root_str = path_to_string(&readable_root);
        assert!(args.args.windows(3).any(|window| {
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
            network_access: false,
        };

        // `ReadOnlyAccess::Restricted` always includes `cwd` as a readable
        // root. Using `"/"` here would intentionally collapse to broad read
        // access, so use a non-root cwd to exercise the restricted path.
        let args = create_filesystem_args(
            &FileSystemSandboxPolicy::from(&policy),
            temp_dir.path(),
            NO_UNREADABLE_GLOB_SCAN_MAX_DEPTH,
        )
        .expect("filesystem args");

        assert!(
            args.args
                .starts_with(&["--tmpfs".to_string(), "/".to_string()])
        );

        if Path::new("/usr").exists() {
            assert!(
                args.args
                    .windows(3)
                    .any(|window| window == ["--ro-bind", "/usr", "/usr"])
            );
        }
    }

    #[test]
    fn split_policy_reapplies_unreadable_carveouts_after_writable_binds() {
        let temp_dir = TempDir::new().expect("temp dir");
        let writable_root = temp_dir.path().join("workspace");
        let blocked = writable_root.join("blocked");
        std::fs::create_dir_all(&blocked).expect("create blocked dir");
        let writable_root =
            AbsolutePathBuf::from_absolute_path(&writable_root).expect("absolute writable root");
        let blocked = AbsolutePathBuf::from_absolute_path(&blocked).expect("absolute blocked dir");
        let writable_root_str = path_to_string(writable_root.as_path());
        let blocked_str = path_to_string(blocked.as_path());
        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Path {
                    path: writable_root,
                },
                access: FileSystemAccessMode::Write,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Path { path: blocked },
                access: FileSystemAccessMode::None,
            },
        ]);

        let args =
            create_filesystem_args(&policy, temp_dir.path(), NO_UNREADABLE_GLOB_SCAN_MAX_DEPTH)
                .expect("filesystem args");

        assert!(args.args.windows(3).any(|window| {
            window
                == [
                    "--bind",
                    writable_root_str.as_str(),
                    writable_root_str.as_str(),
                ]
        }));
        let blocked_mask_index = args
            .args
            .windows(6)
            .position(|window| {
                window
                    == [
                        "--perms",
                        "000",
                        "--tmpfs",
                        blocked_str.as_str(),
                        "--remount-ro",
                        blocked_str.as_str(),
                    ]
            })
            .expect("blocked directory should be remounted unreadable");

        let writable_root_bind_index = args
            .args
            .windows(3)
            .position(|window| {
                window
                    == [
                        "--bind",
                        writable_root_str.as_str(),
                        writable_root_str.as_str(),
                    ]
            })
            .expect("writable root should be rebound writable");

        assert!(
            writable_root_bind_index < blocked_mask_index,
            "expected unreadable carveout to be re-applied after writable bind: {:#?}",
            args.args
        );
    }

    #[cfg(unix)]
    #[test]
    fn split_policy_masks_symlinked_project_config_file() {
        let temp_dir = TempDir::new().expect("temp dir");
        let writable_root = temp_dir.path().join("workspace");
        let dot_codex = writable_root.join(".codex");
        let payload = writable_root.join("payload.toml");
        let config = dot_codex.join("config.toml");
        std::fs::create_dir_all(&dot_codex).expect("create .codex");
        std::fs::write(&payload, "sandbox_mode = \"danger-full-access\"").expect("write payload");
        std::os::unix::fs::symlink(&payload, &config).expect("create config symlink");
        let writable_root =
            AbsolutePathBuf::from_absolute_path(&writable_root).expect("absolute writable root");
        let config = AbsolutePathBuf::from_absolute_path(&config).expect("absolute config");
        let config_str = path_to_string(config.as_path());
        let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: writable_root,
            },
            access: FileSystemAccessMode::Write,
        }]);

        let args = create_filesystem_args(&policy, temp_dir.path()).expect("filesystem args");

        assert!(
            args.args
                .windows(3)
                .any(|window| window == ["--ro-bind", "/dev/null", config_str.as_str()]),
            "expected symlinked config.toml to be masked: {:#?}",
            args.args
        );
    }

    #[test]
    fn split_policy_reenables_nested_writable_subpaths_after_read_only_parent() {
        let temp_dir = TempDir::new().expect("temp dir");
        let writable_root = temp_dir.path().join("workspace");
        let docs = writable_root.join("docs");
        let docs_public = docs.join("public");
        std::fs::create_dir_all(&docs_public).expect("create docs/public");
        let writable_root =
            AbsolutePathBuf::from_absolute_path(&writable_root).expect("absolute writable root");
        let docs = AbsolutePathBuf::from_absolute_path(&docs).expect("absolute docs");
        let docs_public =
            AbsolutePathBuf::from_absolute_path(&docs_public).expect("absolute docs/public");
        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Path {
                    path: writable_root,
                },
                access: FileSystemAccessMode::Write,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Path { path: docs.clone() },
                access: FileSystemAccessMode::Read,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Path {
                    path: docs_public.clone(),
                },
                access: FileSystemAccessMode::Write,
            },
        ]);

        let args =
            create_filesystem_args(&policy, temp_dir.path(), NO_UNREADABLE_GLOB_SCAN_MAX_DEPTH)
                .expect("filesystem args");
        let docs_str = path_to_string(docs.as_path());
        let docs_public_str = path_to_string(docs_public.as_path());
        let docs_ro_index = args
            .args
            .windows(3)
            .position(|window| window == ["--ro-bind", docs_str.as_str(), docs_str.as_str()])
            .expect("docs should be remounted read-only");
        let docs_public_rw_index = args
            .args
            .windows(3)
            .position(|window| {
                window == ["--bind", docs_public_str.as_str(), docs_public_str.as_str()]
            })
            .expect("docs/public should be rebound writable");

        assert!(
            docs_ro_index < docs_public_rw_index,
            "expected read-only parent remount before nested writable bind: {:#?}",
            args.args
        );
    }

    #[test]
    fn split_policy_reenables_writable_subpaths_after_unreadable_parent() {
        let temp_dir = TempDir::new().expect("temp dir");
        let blocked = temp_dir.path().join("blocked");
        let allowed = blocked.join("allowed");
        std::fs::create_dir_all(&allowed).expect("create blocked/allowed");
        let blocked = AbsolutePathBuf::from_absolute_path(&blocked).expect("absolute blocked");
        let allowed = AbsolutePathBuf::from_absolute_path(&allowed).expect("absolute allowed");
        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                access: FileSystemAccessMode::Read,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Path {
                    path: blocked.clone(),
                },
                access: FileSystemAccessMode::None,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Path {
                    path: allowed.clone(),
                },
                access: FileSystemAccessMode::Write,
            },
        ]);

        let args =
            create_filesystem_args(&policy, temp_dir.path(), NO_UNREADABLE_GLOB_SCAN_MAX_DEPTH)
                .expect("filesystem args");
        let blocked_str = path_to_string(blocked.as_path());
        let allowed_str = path_to_string(allowed.as_path());
        let blocked_none_index = args
            .args
            .windows(4)
            .position(|window| window == ["--perms", "111", "--tmpfs", blocked_str.as_str()])
            .expect("blocked should be masked first");
        let allowed_dir_index = args
            .args
            .windows(2)
            .position(|window| window == ["--dir", allowed_str.as_str()])
            .expect("allowed mount target should be recreated");
        let blocked_remount_ro_index = args
            .args
            .windows(2)
            .position(|window| window == ["--remount-ro", blocked_str.as_str()])
            .expect("blocked directory should be remounted read-only");
        let allowed_bind_index = args
            .args
            .windows(3)
            .position(|window| window == ["--bind", allowed_str.as_str(), allowed_str.as_str()])
            .expect("allowed path should be rebound writable");

        assert!(
            blocked_none_index < allowed_dir_index
                && allowed_dir_index < blocked_remount_ro_index
                && blocked_remount_ro_index < allowed_bind_index,
            "expected writable child target recreation before remounting and rebinding under unreadable parent: {:#?}",
            args.args
        );
    }

    #[test]
    fn split_policy_reenables_writable_files_after_unreadable_parent() {
        let temp_dir = TempDir::new().expect("temp dir");
        let blocked = temp_dir.path().join("blocked");
        let allowed_dir = blocked.join("allowed");
        let allowed_file = allowed_dir.join("note.txt");
        std::fs::create_dir_all(&allowed_dir).expect("create blocked/allowed");
        std::fs::write(&allowed_file, "ok").expect("create note");
        let blocked = AbsolutePathBuf::from_absolute_path(&blocked).expect("absolute blocked");
        let allowed_dir =
            AbsolutePathBuf::from_absolute_path(&allowed_dir).expect("absolute allowed dir");
        let allowed_file =
            AbsolutePathBuf::from_absolute_path(&allowed_file).expect("absolute allowed file");
        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                access: FileSystemAccessMode::Read,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Path {
                    path: blocked.clone(),
                },
                access: FileSystemAccessMode::None,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Path {
                    path: allowed_file.clone(),
                },
                access: FileSystemAccessMode::Write,
            },
        ]);

        let args =
            create_filesystem_args(&policy, temp_dir.path(), NO_UNREADABLE_GLOB_SCAN_MAX_DEPTH)
                .expect("filesystem args");
        let blocked_str = path_to_string(blocked.as_path());
        let allowed_dir_str = path_to_string(allowed_dir.as_path());
        let allowed_file_str = path_to_string(allowed_file.as_path());

        assert!(
            args.args
                .windows(2)
                .any(|window| window == ["--dir", allowed_dir_str.as_str()]),
            "expected ancestor directory to be recreated: {:#?}",
            args.args
        );
        assert!(
            !args
                .args
                .windows(2)
                .any(|window| window == ["--dir", allowed_file_str.as_str()]),
            "writable file target should not be converted into a directory: {:#?}",
            args.args
        );
        let blocked_none_index = args
            .args
            .windows(4)
            .position(|window| window == ["--perms", "111", "--tmpfs", blocked_str.as_str()])
            .expect("blocked should be masked first");
        let allowed_bind_index = args
            .args
            .windows(3)
            .position(|window| {
                window
                    == [
                        "--bind",
                        allowed_file_str.as_str(),
                        allowed_file_str.as_str(),
                    ]
            })
            .expect("allowed file should be rebound writable");

        assert!(
            blocked_none_index < allowed_bind_index,
            "expected unreadable parent mask before rebinding writable file child: {:#?}",
            args.args
        );
    }

    #[test]
    fn split_policy_reenables_nested_writable_roots_after_unreadable_parent() {
        let temp_dir = TempDir::new().expect("temp dir");
        let writable_root = temp_dir.path().join("workspace");
        let blocked = writable_root.join("blocked");
        let allowed = blocked.join("allowed");
        std::fs::create_dir_all(&allowed).expect("create blocked/allowed dir");
        let writable_root =
            AbsolutePathBuf::from_absolute_path(&writable_root).expect("absolute writable root");
        let blocked = AbsolutePathBuf::from_absolute_path(&blocked).expect("absolute blocked dir");
        let allowed = AbsolutePathBuf::from_absolute_path(&allowed).expect("absolute allowed dir");
        let blocked_str = path_to_string(blocked.as_path());
        let allowed_str = path_to_string(allowed.as_path());
        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Path {
                    path: writable_root,
                },
                access: FileSystemAccessMode::Write,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Path { path: blocked },
                access: FileSystemAccessMode::None,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Path { path: allowed },
                access: FileSystemAccessMode::Write,
            },
        ]);

        let args =
            create_filesystem_args(&policy, temp_dir.path(), NO_UNREADABLE_GLOB_SCAN_MAX_DEPTH)
                .expect("filesystem args");
        let blocked_none_index = args
            .args
            .windows(4)
            .position(|window| window == ["--perms", "111", "--tmpfs", blocked_str.as_str()])
            .expect("blocked should be masked first");
        let allowed_dir_index = args
            .args
            .windows(2)
            .position(|window| window == ["--dir", allowed_str.as_str()])
            .expect("allowed mount target should be recreated");
        let allowed_bind_index = args
            .args
            .windows(3)
            .position(|window| window == ["--bind", allowed_str.as_str(), allowed_str.as_str()])
            .expect("allowed path should be rebound writable");

        assert!(
            blocked_none_index < allowed_dir_index && allowed_dir_index < allowed_bind_index,
            "expected unreadable parent mask before recreating and rebinding writable child: {:#?}",
            args.args
        );
    }

    #[test]
    fn split_policy_masks_root_read_directory_carveouts() {
        let temp_dir = TempDir::new().expect("temp dir");
        let blocked = temp_dir.path().join("blocked");
        std::fs::create_dir_all(&blocked).expect("create blocked dir");
        let blocked = AbsolutePathBuf::from_absolute_path(&blocked).expect("absolute blocked dir");
        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                access: FileSystemAccessMode::Read,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Path {
                    path: blocked.clone(),
                },
                access: FileSystemAccessMode::None,
            },
        ]);

        let args =
            create_filesystem_args(&policy, temp_dir.path(), NO_UNREADABLE_GLOB_SCAN_MAX_DEPTH)
                .expect("filesystem args");
        let blocked_str = path_to_string(blocked.as_path());

        assert!(
            args.args
                .windows(3)
                .any(|window| window == ["--ro-bind", "/", "/"])
        );
        assert!(
            args.args
                .windows(4)
                .any(|window| { window == ["--perms", "000", "--tmpfs", blocked_str.as_str()] })
        );
        assert!(
            args.args
                .windows(2)
                .any(|window| window == ["--remount-ro", blocked_str.as_str()])
        );
    }

    #[test]
    fn split_policy_masks_root_read_file_carveouts() {
        let temp_dir = TempDir::new().expect("temp dir");
        let blocked_file = temp_dir.path().join("blocked.txt");
        std::fs::write(&blocked_file, "secret").expect("create blocked file");
        let blocked_file =
            AbsolutePathBuf::from_absolute_path(&blocked_file).expect("absolute blocked file");
        let policy = FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                access: FileSystemAccessMode::Read,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Path {
                    path: blocked_file.clone(),
                },
                access: FileSystemAccessMode::None,
            },
        ]);

        let args =
            create_filesystem_args(&policy, temp_dir.path(), NO_UNREADABLE_GLOB_SCAN_MAX_DEPTH)
                .expect("filesystem args");
        let blocked_file_str = path_to_string(blocked_file.as_path());

        assert_eq!(args.preserved_files.len(), 1);
        assert!(args.args.windows(5).any(|window| {
            window[0] == "--perms"
                && window[1] == "000"
                && window[2] == "--ro-bind-data"
                && window[4] == blocked_file_str
        }));
    }

    #[test]
    fn unreadable_globs_expand_existing_matches_with_configured_depth() {
        if !ripgrep_available() {
            return;
        }

        let temp_dir = TempDir::new().expect("temp dir");
        let root_env = temp_dir.path().join(".env");
        let nested_env = temp_dir.path().join("app").join(".env");
        let too_deep_env = temp_dir.path().join("app").join("deep").join(".env");
        std::fs::create_dir_all(too_deep_env.parent().expect("parent")).expect("create parent");
        std::fs::write(temp_dir.path().join(".gitignore"), ".env\n").expect("write gitignore");
        std::fs::write(&root_env, "secret").expect("write root env");
        std::fs::write(&nested_env, "secret").expect("write nested env");
        std::fs::write(&too_deep_env, "secret").expect("write deep env");
        let policy =
            default_policy_with_unreadable_glob(format!("{}/**/*.env", temp_dir.path().display()));

        let args =
            create_filesystem_args(&policy, temp_dir.path(), Some(2)).expect("filesystem args");

        assert_file_masked(&args.args, &root_env);
        assert_file_masked(&args.args, &nested_env);
        assert!(
            !args
                .args
                .iter()
                .any(|arg| arg == &path_to_string(&too_deep_env)),
            "max depth should keep deeper matches out of bwrap args: {:#?}",
            args.args
        );
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_globs_add_canonical_targets_for_symlink_matches() {
        if !ripgrep_available() {
            return;
        }

        let temp_dir = TempDir::new().expect("temp dir");
        let real_root = temp_dir.path().join("real");
        let link_root = temp_dir.path().join("link");
        let real_secret = real_root.join("secret.env");
        std::fs::create_dir_all(&real_root).expect("create real root");
        std::fs::write(&real_secret, "secret").expect("write real secret");
        std::os::unix::fs::symlink(&real_root, &link_root).expect("create symlink");
        let policy =
            default_policy_with_unreadable_glob(format!("{}/**/*.env", link_root.display()));

        let args =
            create_filesystem_args(&policy, temp_dir.path(), Some(2)).expect("filesystem args");

        assert_file_masked(&args.args, &real_secret);
    }

    #[test]
    fn root_prefix_unreadable_globs_are_too_broad_for_linux_expansion() {
        assert_eq!(
            split_pattern_for_ripgrep("/**/*.env", Path::new("/tmp")),
            None
        );
    }

    #[test]
    fn unclosed_character_classes_are_escaped_for_ripgrep() {
        let (search_root, glob) =
            split_pattern_for_ripgrep("/tmp/[*.env", Path::new("/")).expect("split pattern");

        assert_eq!(search_root.as_path(), Path::new("/tmp"));
        assert_eq!(glob, r"\[*.env");
    }

    fn ripgrep_available() -> bool {
        Command::new("rg")
            .arg("--version")
            .output()
            .is_ok_and(|output| output.status.success())
    }

    /// Assert that `path` is masked due to a bwrap arg sequence like:
    ///
    /// `bwrap ... --perms 000 --ro-bind-data FD PATH`
    fn assert_file_masked(args: &[String], path: &Path) {
        let path = path_to_string(path);
        assert!(
            args.windows(5).any(|window| {
                window[0] == "--perms"
                    && window[1] == "000"
                    && window[2] == "--ro-bind-data"
                    && window[4] == path
            }),
            "expected file mask for {path}: {args:#?}"
        );
    }
}
