use std::path::Path;

use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::PermissionIntersectionError;
use super::intersect_effective_permission_profiles;
use crate::models::PermissionProfile;
use crate::permissions::FileSystemAccessMode;
use crate::permissions::FileSystemAccessMode::Deny;
use crate::permissions::FileSystemAccessMode::Read;
use crate::permissions::FileSystemAccessMode::Write;
use crate::permissions::FileSystemPath;
use crate::permissions::FileSystemSandboxEntry;
use crate::permissions::FileSystemSandboxPolicy;
use crate::permissions::FileSystemSpecialPath;
use crate::permissions::FileSystemSpecialPath::Minimal;
use crate::permissions::FileSystemSpecialPath::Tmpdir;
use crate::permissions::NetworkSandboxPolicy;
use crate::permissions::NetworkSandboxPolicy::Enabled;
use crate::permissions::NetworkSandboxPolicy::Restricted;
use crate::permissions::ReadDenyMatcher;
use crate::permissions::project_roots_glob_pattern;

fn absolute(path: &Path) -> AbsolutePathBuf {
    AbsolutePathBuf::from_absolute_path(path).expect("absolute test path")
}

fn canonical(temp: &TempDir) -> AbsolutePathBuf {
    absolute(temp.path())
        .canonicalize()
        .expect("canonical root")
}

fn managed(entries: Vec<FileSystemSandboxEntry>) -> PermissionProfile {
    PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(entries),
        Restricted,
    )
}

fn entry(path: &Path, access: FileSystemAccessMode) -> FileSystemSandboxEntry {
    FileSystemSandboxEntry::new(absolute(path).into(), access)
}

fn special(value: FileSystemSpecialPath, access: FileSystemAccessMode) -> FileSystemSandboxEntry {
    FileSystemSandboxEntry::new(FileSystemPath::Special { value }, access)
}

fn skipped(path: FileSystemPath, access: FileSystemAccessMode) -> FileSystemSandboxEntry {
    FileSystemSandboxEntry::skip_missing_path(path, access)
}

fn deny_glob(pattern: &str) -> FileSystemSandboxEntry {
    FileSystemSandboxEntry::new(
        FileSystemPath::GlobPattern {
            pattern: pattern.into(),
        },
        Deny,
    )
}

fn root_read() -> FileSystemSandboxEntry {
    FileSystemSandboxPolicy::read_only().entries[0].clone()
}

fn rooted(
    root: &AbsolutePathBuf,
    access: FileSystemAccessMode,
    entries: impl IntoIterator<Item = FileSystemSandboxEntry>,
) -> PermissionProfile {
    let mut all_entries = vec![root_read(), entry(root.as_path(), access)];
    all_entries.extend(entries);
    managed(all_entries)
}

fn workspace(root: &AbsolutePathBuf, network: NetworkSandboxPolicy) -> PermissionProfile {
    PermissionProfile::workspace_write_with(
        &[],
        network,
        /*exclude_tmpdir_env_var*/ false,
        /*exclude_slash_tmp*/ true,
    )
    .materialize_project_roots_with_workspace_roots(std::slice::from_ref(root))
}

fn intersection(
    left: &PermissionProfile,
    right: &PermissionProfile,
    cwd: &AbsolutePathBuf,
) -> PermissionProfile {
    intersect_effective_permission_profiles(left, right, cwd.as_path()).expect("safe intersection")
}

#[test]
fn effective_workspace_intersection_preserves_network_metadata_and_temp() {
    let temp = TempDir::new().expect("workspace");
    let root = canonical(&temp);
    let project = root.join("project");
    std::fs::create_dir(project.as_path()).expect("project directory");
    let gitdir = root.join("external-gitdir");
    std::fs::create_dir(gitdir.as_path()).expect("worktree gitdir");
    std::fs::write(
        project.join(".git").as_path(),
        format!("gitdir: {}", gitdir.as_path().display()),
    )
    .expect("worktree git pointer");
    let authority = workspace(&root, Enabled);
    let mut requested_policy = workspace(&project, Restricted).file_system_sandbox_policy();
    requested_policy
        .entries
        .push(skipped(gitdir.clone().into(), Read));
    let requested = PermissionProfile::from_runtime_permissions(&requested_policy, Restricted);
    let result = intersection(&authority, &requested, &project);
    let policy = result.file_system_sandbox_policy();

    assert_eq!(
        [&root, &project]
            .map(|path| policy.resolve_access_with_cwd(path.as_path(), root.as_path())),
        [Read, Write]
    );
    assert_eq!(result.network_sandbox_policy(), Restricted);
    assert!(policy.entries.contains(&special(Tmpdir, Write)));
    for name in [".git", ".agents", ".codex"] {
        let protected = project.join(name);
        assert!(!policy.can_write_path_with_cwd(protected.as_path(), root.as_path()));
        assert!(policy.entries.contains(&skipped(protected.into(), Read)));
    }
    assert!(policy.entries.contains(&skipped(gitdir.into(), Read)));
    let strict = rooted(
        &project,
        Write,
        [entry(project.join(".git").as_path(), Read)],
    );
    let strict = intersection(&requested, &strict, &project).file_system_sandbox_policy();
    let git_protection = entry(project.join(".git").as_path(), Read);
    assert!(strict.entries.contains(&git_protection));
}

#[test]
fn exact_denies_nested_read_carveouts_and_reopened_writes_are_preserved() {
    let temp = TempDir::new().expect("workspace");
    let root = canonical(&temp);
    let shared = root.join("shared");
    let editable = shared.join("editable");
    let left_secret = root.join("left-secret.env");
    let right_secret = root.join("right-secret.token");
    for directory in [&editable, &left_secret, &right_secret] {
        std::fs::create_dir_all(directory.as_path()).expect("policy directory");
    }
    let left = rooted(
        &root,
        Write,
        [
            entry(shared.as_path(), Read),
            entry(editable.as_path(), Write),
            entry(left_secret.as_path(), Deny),
            deny_glob("**/*.env"),
        ],
    );
    let mut left_policy = left.file_system_sandbox_policy();
    left_policy.glob_scan_max_depth = Some(2);
    let left = PermissionProfile::from_runtime_permissions(&left_policy, Restricted);
    let right = rooted(
        &root,
        Write,
        [
            entry(shared.as_path(), Write),
            entry(editable.as_path(), Write),
            entry(right_secret.as_path(), Deny),
            deny_glob("**/*.token"),
        ],
    );
    let mut right_policy = right.file_system_sandbox_policy();
    right_policy.glob_scan_max_depth = Some(4);
    let right = PermissionProfile::from_runtime_permissions(&right_policy, Restricted);
    let result = intersection(&left, &right, &root);
    let policy = result.file_system_sandbox_policy();
    let denies = ReadDenyMatcher::new(&policy, root.as_path()).expect("merged denies");
    assert_eq!(
        [&root, &shared, &editable]
            .map(|path| policy.resolve_access_with_cwd(path.as_path(), root.as_path())),
        [Write, Read, Write]
    );
    assert_eq!(
        [
            &left_secret,
            &right_secret,
            &root.join("credentials.env"),
            &root.join("credentials.token"),
        ]
        .map(|path| denies.is_read_denied(path.as_path())),
        [true, true, true, true]
    );
    assert_eq!(policy.glob_scan_max_depth, Some(4));
    assert!(policy.entries.contains(&entry(left_secret.as_path(), Deny)));
    assert_eq!(result, intersection(&right, &left, &root));
    let unbounded = rooted(&root, Write, [deny_glob("**/*.key")]);
    assert_eq!(
        intersection(&left, &unbounded, &root)
            .file_system_sandbox_policy()
            .glob_scan_max_depth,
        None
    );
}

#[test]
fn temporary_directory_restrictions_fail_closed() {
    let temp = TempDir::new().expect("workspace");
    let root = canonical(&temp);
    let writable = workspace(&root, Restricted);
    for access in [Read, Deny] {
        let restricted = rooted(&root, Write, [special(Tmpdir, access)]);
        assert!(matches!(
            intersect_effective_permission_profiles(&restricted, &writable, root.as_path()),
            Err(PermissionIntersectionError::UnsupportedPath(_))
        ));
    }
}

#[test]
fn identical_unrestricted_and_disabled_profiles_keep_existing_enforcement() {
    let temp = TempDir::new().expect("workspace");
    let root = absolute(temp.path());
    let readonly = PermissionProfile::read_only();
    let disabled = PermissionProfile::Disabled;
    assert_eq!(intersection(&disabled, &readonly, &root), readonly);
    assert_eq!(intersection(&readonly, &disabled, &root), readonly);
    assert_eq!(intersection(&disabled, &disabled, &root), disabled);
    let unrestricted = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::unrestricted(),
        Enabled,
    );
    assert_eq!(intersection(&unrestricted, &readonly, &root), readonly);

    let minimal = managed(vec![special(Minimal, Read)]);
    assert_eq!(intersection(&minimal, &minimal, &root), minimal);
    let filesystem_root = root.as_path().ancestors().last().unwrap();
    let explicit = managed(vec![entry(filesystem_root, Read)]);
    assert!(
        !intersection(&explicit, &explicit, &root)
            .file_system_sandbox_policy()
            .has_full_disk_read_access()
    );
}

#[test]
fn unsupported_unresolved_and_optional_policy_shapes_fail_closed() {
    let temp = TempDir::new().expect("workspace");
    let root = canonical(&temp);
    let readonly = PermissionProfile::read_only();
    let external = PermissionProfile::External { network: Enabled };
    for (left, right) in [(&external, &readonly), (&readonly, &external)] {
        assert_eq!(
            intersect_effective_permission_profiles(left, right, root.as_path()),
            Err(PermissionIntersectionError::ExternalSandbox)
        );
    }
    let unmaterialized = PermissionProfile::workspace_write();
    assert!(matches!(
        intersect_effective_permission_profiles(&unmaterialized, &readonly, root.as_path()),
        Err(PermissionIntersectionError::UnsupportedPath(_))
    ));
    let minimal = managed(vec![special(Minimal, Read)]);
    assert_eq!(
        intersect_effective_permission_profiles(&minimal, &readonly, root.as_path()),
        Err(PermissionIntersectionError::PlatformDefaults)
    );

    let optional_write = rooted(&root, Write, [skipped(root.clone().into(), Write)]);
    let invalid_glob = rooted(&root, Write, [deny_glob("**/[z-a]")]);
    let optional_glob = rooted(&root, Write, [skipped(deny_glob("**/*.env").path, Deny)]);
    let unresolved_pattern = project_roots_glob_pattern(Path::new("**/*.env"));
    let unresolved_glob = rooted(&root, Write, [deny_glob(&unresolved_pattern)]);
    let duplicate_tmpdir = managed(vec![
        root_read(),
        special(Tmpdir, Read),
        skipped(FileSystemPath::Special { value: Tmpdir }, Write),
    ]);
    for unsupported in [
        &optional_write,
        &invalid_glob,
        &optional_glob,
        &unresolved_glob,
        &duplicate_tmpdir,
    ] {
        assert!(matches!(
            intersect_effective_permission_profiles(unsupported, &readonly, root.as_path()),
            Err(PermissionIntersectionError::UnsupportedPath(_))
        ));
    }
    assert!(matches!(
        intersect_effective_permission_profiles(
            &readonly,
            &readonly,
            Path::new("relative/workspace")
        ),
        Err(PermissionIntersectionError::UnsupportedPath(_))
    ));
}

#[cfg(unix)]
#[test]
fn canonical_grants_cannot_escape_through_readable_or_writable_symlinks() {
    let temp = TempDir::new().expect("workspace");
    let root = canonical(&temp);
    let outside_temp = TempDir::new().expect("outside workspace");
    let outside = canonical(&outside_temp);
    let escaped = root.join("escaped");
    let inside = root.join("inside");
    let internal_alias = root.join("internal-alias");
    std::fs::create_dir(inside.as_path()).expect("inside directory");
    std::os::unix::fs::symlink(outside.as_path(), escaped.as_path()).expect("outside symlink");
    std::os::unix::fs::symlink(inside.as_path(), internal_alias.as_path()).expect("inside symlink");

    let parent = rooted(&root, Write, []);
    let child = rooted(&escaped, Write, []);
    let policy = intersection(&parent, &child, &root).file_system_sandbox_policy();
    assert!(!policy.can_write_path_with_cwd(outside.as_path(), root.as_path()));

    let parent = managed(vec![entry(root.as_path(), Read)]);
    let child = managed(vec![entry(escaped.as_path(), Read)]);
    assert!(matches!(
        intersect_effective_permission_profiles(&parent, &child, root.as_path()),
        Err(PermissionIntersectionError::UnsupportedPath(_))
    ));

    let parent = rooted(&root, Write, []);
    let child = rooted(&internal_alias, Write, []);
    let policy = intersection(&parent, &child, &root).file_system_sandbox_policy();
    assert!(policy.can_write_path_with_cwd(inside.as_path(), root.as_path()));
    assert!(
        !policy
            .entries
            .contains(&entry(internal_alias.as_path(), Write))
    );
}

#[cfg(target_os = "macos")]
#[test]
fn macos_system_path_aliases_share_the_same_physical_permissions() {
    let temp = TempDir::new_in("/tmp").expect("workspace under system path alias");
    let canonical = canonical(&temp);
    let alias = absolute(
        &Path::new("/").join(
            canonical
                .as_path()
                .strip_prefix("/private")
                .expect("macOS alias"),
        ),
    );
    for access in [Read, Write] {
        let left = managed(vec![entry(alias.as_path(), access)]);
        let right = managed(vec![entry(canonical.as_path(), access)]);
        let policy = intersection(&left, &right, &canonical).file_system_sandbox_policy();
        assert_eq!(
            policy.resolve_access_with_cwd(canonical.as_path(), canonical.as_path()),
            access
        );
    }
    let policy = intersection(
        &workspace(&alias, Enabled),
        &workspace(&canonical, Restricted),
        &canonical,
    )
    .file_system_sandbox_policy();
    assert!(policy.can_write_path_with_cwd(canonical.as_path(), canonical.as_path()));
}
