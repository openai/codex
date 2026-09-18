use super::resolve_windows_deny_read_paths;
use super::walker::DirectoryScanMode;
use super::walker::collect_existing_glob_directory_matches;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::ReadDenyMatcher;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use tempfile::TempDir;

fn policy() -> FileSystemSandboxPolicy {
    FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
        path: FileSystemPath::GlobPattern {
            pattern: "**/*.env".to_string(),
        },
        access: FileSystemAccessMode::Deny,
        missing_path_behavior: None,
    }])
}

struct ProtectedDirectory {
    path: PathBuf,
    #[cfg(unix)]
    permissions: std::fs::Permissions,
}

impl ProtectedDirectory {
    fn new(path: &Path) -> Self {
        #[cfg(unix)]
        let permissions = {
            use std::os::unix::fs::PermissionsExt;

            let permissions = path.metadata().expect("directory metadata").permissions();
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(/*mode*/ 0o000))
                .expect("protect directory");
            permissions
        };
        #[cfg(windows)]
        assert!(
            std::process::Command::new("icacls.exe")
                .arg(path)
                .args(["/deny", "*S-1-1-0:(OI)(CI)R"])
                .output()
                .expect("protect directory for every account")
                .status
                .success()
        );
        Self {
            path: path.to_path_buf(),
            #[cfg(unix)]
            permissions,
        }
    }
}

impl Drop for ProtectedDirectory {
    fn drop(&mut self) {
        #[cfg(unix)]
        std::fs::set_permissions(&self.path, self.permissions.clone())
            .expect("restore directory permissions");
        #[cfg(windows)]
        assert!(
            std::process::Command::new("icacls.exe")
                .arg(&self.path)
                .args(["/remove:d", "*S-1-1-0"])
                .output()
                .expect("restore directory permissions")
                .status
                .success()
        );
    }
}

#[test]
fn protected_directory_does_not_discard_accessible_deny_matches() {
    #[cfg(windows)]
    if let Some(root) = std::env::var_os("CODEX_TEST_DENY_READ_RESTRICTED_ROOT") {
        assert_accessible_deny_matches(Path::new(&root));
        return;
    }

    let temp = TempDir::new().expect("tempdir");
    let protected = temp.path().join("protected");
    let readable = temp.path().join("readable");
    std::fs::create_dir(&protected).expect("protected directory");
    std::fs::create_dir(&readable).expect("readable directory");
    std::fs::write(protected.join("hidden.env"), "protected").expect("protected file");
    std::fs::write(readable.join("secret.env"), "secret").expect("readable secret");
    std::fs::create_dir(readable.join("empty.env")).expect("matching empty directory");
    std::fs::write(readable.join("public.txt"), "public").expect("public file");
    let guard = ProtectedDirectory::new(&protected);

    #[cfg(windows)]
    {
        // CI enables backup privileges, which let directory enumeration bypass
        // this fixture's ACL. Restrict a separate process, including its rg child,
        // rather than changing the shared test process's privileges. The parent
        // owns ACL setup and cleanup, which the restricted child cannot perform.
        let codex_home = TempDir::new().expect("restricted codex home");
        let cwd = AbsolutePathBuf::from_absolute_path(temp.path()).expect("workspace");
        let mut env = std::collections::HashMap::from([
            (
                "CODEX_TEST_DENY_READ_RESTRICTED_ROOT".to_string(),
                cwd.display().to_string(),
            ),
            ("TEMP".to_string(), cwd.display().to_string()),
            ("TMP".to_string(), cwd.display().to_string()),
        ]);
        for name in ["PATH", "SystemRoot", "COMSPEC", "USERPROFILE"] {
            if let Ok(value) = std::env::var(name) {
                env.insert(name.to_string(), value);
            }
        }
        let result = crate::run_windows_sandbox_capture(
            &codex_protocol::models::PermissionProfile::workspace_write(),
            std::slice::from_ref(&cwd),
            codex_home.path(),
            vec![
                std::env::current_exe()
                    .expect("test binary")
                    .display()
                    .to_string(),
                "--exact".to_string(),
                std::thread::current()
                    .name()
                    .expect("test name")
                    .to_string(),
            ],
            cwd.as_path(),
            env,
            /*timeout_ms*/ Some(30_000),
            /*cancellation*/ None,
        )
        .expect("run protected-directory regression without backup privileges");
        let stdout = String::from_utf8_lossy(&result.stdout);
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert_eq!(
            (result.exit_code, result.timed_out),
            (0, false),
            "{stdout}\n{stderr}"
        );
        assert!(stdout.contains("test result: ok. 1 passed"), "{stdout}");
    }

    #[cfg(not(windows))]
    assert_accessible_deny_matches(temp.path());
    drop(guard);
}

fn assert_accessible_deny_matches(root: &Path) {
    let cwd = AbsolutePathBuf::from_absolute_path(root).expect("cwd");
    let protected = root.join("protected");
    let enumeration = std::fs::read_dir(&protected)
        .and_then(std::iter::Iterator::collect::<std::io::Result<Vec<_>>>);
    assert_eq!(
        enumeration
            .expect_err("fixture must deny directory enumeration")
            .kind(),
        std::io::ErrorKind::PermissionDenied
    );

    let actual = resolve_windows_deny_read_paths(&policy(), &cwd)
        .expect("protected branches must not abort expansion of accessible paths")
        .into_iter()
        .map(AbsolutePathBuf::into_path_buf)
        .collect::<HashSet<_>>();
    assert_eq!(
        actual,
        HashSet::from([
            root.join("readable/secret.env"),
            root.join("readable/empty.env"),
        ])
    );
}

#[test]
fn native_walk_preserves_canonical_matches_through_directory_links_and_cycles() {
    let temp = TempDir::new().expect("tempdir");
    let cwd = AbsolutePathBuf::from_absolute_path(temp.path()).expect("cwd");
    let walk = temp.path().join("walk");
    let target = temp.path().join("target");
    std::fs::create_dir(&walk).expect("walk root");
    std::fs::create_dir(&target).expect("link target");
    std::fs::write(target.join("secret.env"), "secret").expect("secret");
    std::fs::write(target.join("public.txt"), "public").expect("public");
    let alias = walk.join("alias");
    let cycle = target.join("cycle");
    for (link, destination) in [(&alias, &target), (&cycle, &walk)] {
        #[cfg(unix)]
        std::os::unix::fs::symlink(destination, link).expect("directory symlink");
        #[cfg(windows)]
        assert!(
            std::process::Command::new("cmd.exe")
                .args(["/d", "/c", "mklink", "/J"])
                .arg(link)
                .arg(destination)
                .output()
                .expect("directory junction")
                .status
                .success()
        );
    }
    let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
        path: FileSystemPath::GlobPattern {
            pattern: target
                .canonicalize()
                .expect("canonical target")
                .join("**")
                .join("*.env")
                .display()
                .to_string(),
        },
        access: FileSystemAccessMode::Deny,
        missing_path_behavior: None,
    }]);
    let matcher = ReadDenyMatcher::try_new_for_local_paths(&policy, cwd.as_path())
        .expect("valid matcher")
        .expect("deny restriction");
    assert!(
        matcher.is_local_path_read_denied_with_canonical_path(
            &alias.join("secret.env"),
            &target
                .canonicalize()
                .expect("canonical target")
                .join("secret.env"),
        )
    );
    let mut paths = Vec::new();
    collect_existing_glob_directory_matches(
        &walk,
        &matcher,
        &mut paths,
        &mut HashSet::new(),
        /*max_depth*/ None,
        DirectoryScanMode::IncludeAllFiles,
    )
    .expect("walk through directory alias and cycle");
    assert_eq!(
        paths,
        vec![
            AbsolutePathBuf::from_absolute_path(alias.join("secret.env")).expect("lexical target")
        ]
    );
}
