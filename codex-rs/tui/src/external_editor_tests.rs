use super::editor_directory;
#[cfg(unix)]
use super::run_editor;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use tempfile::TempDir;

struct EditorPaths {
    _root: TempDir,
    codex_home: PathBuf,
    cwd: PathBuf,
}

impl EditorPaths {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("create editor test root");
        let canonical_root =
            dunce::canonicalize(root.path()).expect("canonicalize editor test root");
        let codex_home = canonical_root.join("codex-home");
        let cwd = canonical_root.join("workspace");
        fs::create_dir(&codex_home).expect("create Codex home");
        fs::create_dir(&cwd).expect("create workspace");

        Self {
            _root: root,
            codex_home,
            cwd,
        }
    }
}

fn workspace_write_policy(writable_roots: &[&Path]) -> FileSystemSandboxPolicy {
    let writable_roots = writable_roots
        .iter()
        .map(|root| AbsolutePathBuf::from_absolute_path(root).expect("absolute writable root"))
        .collect::<Vec<_>>();

    FileSystemSandboxPolicy::workspace_write(
        &writable_roots,
        /*exclude_tmpdir_env_var*/ true,
        /*exclude_slash_tmp*/ true,
    )
}

#[test]
fn editor_directory_is_inside_isolated_codex_home() {
    let paths = EditorPaths::new();
    let policy = workspace_write_policy(&[]);

    let directory = editor_directory(&[&paths.codex_home], &policy, &paths.cwd)
        .expect("create isolated editor directory");

    assert_eq!(directory, paths.codex_home.join("editor"));
    assert!(directory.is_dir());
}

#[test]
fn editor_directory_rejects_writable_home_editor_and_parent() {
    let paths = EditorPaths::new();
    let editor = paths.codex_home.join("editor");
    fs::create_dir(&editor).expect("create editor directory");
    let parent = paths.codex_home.parent().expect("Codex home parent");

    for writable_root in [paths.codex_home.as_path(), editor.as_path(), parent] {
        let policy = workspace_write_policy(&[writable_root]);

        assert!(
            editor_directory(&[&paths.codex_home], &policy, &paths.cwd).is_err(),
            "writable root {} must not expose editor buffers",
            writable_root.display()
        );
    }
}

#[test]
fn editor_directory_rejects_writable_descendant() {
    let paths = EditorPaths::new();
    let writable_descendant = paths.codex_home.join("editor").join("nested");
    fs::create_dir_all(&writable_descendant).expect("create writable editor descendant");
    let policy = workspace_write_policy(&[&writable_descendant]);

    assert!(editor_directory(&[&paths.codex_home], &policy, &paths.cwd).is_err());
}

#[test]
fn editor_directory_rejects_read_only_carveout_with_writable_parent() {
    let paths = EditorPaths::new();
    let editor = paths.codex_home.join("editor");
    fs::create_dir(&editor).expect("create editor directory");
    let policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry::new(
            AbsolutePathBuf::from_absolute_path(&paths.codex_home)
                .expect("absolute Codex home")
                .into(),
            FileSystemAccessMode::Write,
        ),
        FileSystemSandboxEntry::new(
            AbsolutePathBuf::from_absolute_path(&editor)
                .expect("absolute editor directory")
                .into(),
            FileSystemAccessMode::Read,
        ),
    ]);

    assert!(editor_directory(&[&paths.codex_home], &policy, &paths.cwd).is_err());
}

#[test]
#[cfg(unix)]
fn editor_directory_rejects_preexisting_symlink() {
    use std::os::unix::fs::symlink;

    let paths = EditorPaths::new();
    let outside = paths.cwd.join("outside");
    fs::create_dir(&outside).expect("create editor symlink target");
    symlink(&outside, paths.codex_home.join("editor")).expect("create editor directory symlink");
    let policy = FileSystemSandboxPolicy::read_only();

    assert!(editor_directory(&[&paths.codex_home], &policy, &paths.cwd).is_err());
}

#[test]
#[cfg(unix)]
fn editor_directory_rejects_writable_codex_home_alias() {
    use std::os::unix::fs::symlink;

    let paths = EditorPaths::new();
    let aliased_home = paths.cwd.join("codex-home-link");
    symlink(&paths.codex_home, &aliased_home).expect("create Codex home symlink");
    let policy = workspace_write_policy(&[]);

    assert!(policy.can_write_path_with_cwd(&aliased_home.join("editor"), &paths.cwd));
    assert!(!policy.can_write_path_with_cwd(&paths.codex_home.join("editor"), &paths.cwd));
    assert!(editor_directory(&[&aliased_home], &policy, &paths.cwd).is_err());
}

#[test]
#[cfg(unix)]
fn editor_directory_rejects_writable_codex_home_alias_target() {
    use std::os::unix::fs::symlink;

    let paths = EditorPaths::new();
    let alias_parent = paths
        .codex_home
        .parent()
        .expect("Codex home parent")
        .join("aliases");
    fs::create_dir(&alias_parent).expect("create protected alias parent");
    let aliased_home = alias_parent.join("codex-home-link");
    symlink(&paths.codex_home, &aliased_home).expect("create Codex home symlink");
    let policy = workspace_write_policy(&[&paths.codex_home]);

    assert!(!policy.can_write_path_with_cwd(&aliased_home.join("editor"), &paths.cwd));
    assert!(policy.can_write_path_with_cwd(&paths.codex_home.join("editor"), &paths.cwd));
    assert!(editor_directory(&[&aliased_home], &policy, &paths.cwd).is_err());
}

#[test]
#[cfg(unix)]
fn editor_directory_uses_protected_workspace_fallback_with_default_temporary_grants() {
    let root = tempfile::tempdir().expect("create editor test root");
    let codex_home = root.path().join("codex-home");
    let cwd = root.path().join("workspace");
    fs::create_dir(&codex_home).expect("create Codex home");
    fs::create_dir(&cwd).expect("create workspace");
    let workspace_codex_home = cwd.join(".codex");
    let policy = FileSystemSandboxPolicy::workspace_write(
        &[],
        /*exclude_tmpdir_env_var*/ false,
        /*exclude_slash_tmp*/ false,
    );

    assert!(!workspace_codex_home.exists());
    assert!(policy.can_write_path_with_cwd(&codex_home, &cwd));
    assert!(!policy.can_write_path_with_cwd(&workspace_codex_home, &cwd));
    assert!(!policy.can_write_path_with_cwd(&workspace_codex_home.join("editor"), &cwd));

    let directory = editor_directory(&[&codex_home, &workspace_codex_home], &policy, &cwd)
        .expect("use protected workspace metadata directory");

    assert_eq!(
        directory,
        dunce::canonicalize(&workspace_codex_home)
            .expect("canonicalize workspace metadata directory")
            .join("editor")
    );
    assert!(directory.is_dir());
}

#[test]
#[cfg(unix)]
fn editor_directory_rejects_explicitly_writable_workspace_fallback() {
    let root = tempfile::tempdir().expect("create editor test root");
    let codex_home = root.path().join("codex-home");
    let cwd = root.path().join("workspace");
    fs::create_dir(&codex_home).expect("create Codex home");
    fs::create_dir(&cwd).expect("create workspace");
    let workspace_codex_home = cwd.join(".codex");
    let writable_workspace_codex_home = AbsolutePathBuf::from_absolute_path(&workspace_codex_home)
        .expect("absolute workspace metadata directory");
    let policy = FileSystemSandboxPolicy::workspace_write(
        &[writable_workspace_codex_home],
        /*exclude_tmpdir_env_var*/ false,
        /*exclude_slash_tmp*/ false,
    );

    assert!(policy.can_write_path_with_cwd(&codex_home, &cwd));
    assert!(policy.can_write_path_with_cwd(&workspace_codex_home, &cwd));
    assert!(
        editor_directory(&[&codex_home, &workspace_codex_home], &policy, &cwd).is_err(),
        "explicitly writable metadata must not be used for editor buffers"
    );
    assert!(!workspace_codex_home.exists());
}

#[test]
#[cfg(unix)]
fn editor_directory_rejects_workspace_fallback_symlink_to_writable_target() {
    use std::os::unix::fs::symlink;

    let paths = EditorPaths::new();
    let workspace_codex_home = paths.cwd.join(".codex");
    symlink(&paths.codex_home, &workspace_codex_home).expect("create workspace metadata symlink");
    let policy = workspace_write_policy(&[&paths.codex_home]);

    assert!(!policy.can_write_path_with_cwd(&workspace_codex_home, &paths.cwd));
    assert!(policy.can_write_path_with_cwd(&paths.codex_home, &paths.cwd));
    assert!(
        editor_directory(
            &[&paths.codex_home, &workspace_codex_home],
            &policy,
            &paths.cwd,
        )
        .is_err()
    );
}

#[test]
fn editor_directory_uses_next_protected_candidate_after_creation_error() {
    let paths = EditorPaths::new();
    let unavailable_home = paths
        .codex_home
        .parent()
        .expect("Codex home parent")
        .join("unavailable-home");
    fs::write(&unavailable_home, "not a directory").expect("create unavailable Codex home");
    let policy = workspace_write_policy(&[]);

    let directory = editor_directory(&[&unavailable_home, &paths.codex_home], &policy, &paths.cwd)
        .expect("use next protected candidate after directory creation fails");

    assert_eq!(directory, paths.codex_home.join("editor"));
}

#[test]
#[cfg(windows)]
fn editor_directory_rejects_windows_temporary_directory_outside_tmpdir_policy_root() {
    let paths = EditorPaths::new();
    let policy = FileSystemSandboxPolicy::workspace_write(
        &[],
        /*exclude_tmpdir_env_var*/ false,
        /*exclude_slash_tmp*/ true,
    );

    assert!(
        editor_directory(&[&paths.codex_home], &policy, &paths.cwd).is_err(),
        "effective Windows temporary directories must not contain editor buffers"
    );
    assert!(!paths.codex_home.join("editor").exists());
}

#[test]
fn editor_directory_allows_full_disk_write_policies() {
    let paths = EditorPaths::new();

    for policy in [
        FileSystemSandboxPolicy::unrestricted(),
        FileSystemSandboxPolicy::external_sandbox(),
    ] {
        let directory = editor_directory(&[&paths.codex_home], &policy, &paths.cwd)
            .expect("full-disk-write policies should preserve external editor support");

        assert_eq!(directory, paths.codex_home.join("editor"));
    }
}

#[tokio::test]
#[cfg(unix)]
async fn editor_process_receives_buffer_in_isolated_codex_home() {
    let paths = EditorPaths::new();
    let policy = workspace_write_policy(&[]);
    let editor_directory = paths.codex_home.join("editor");
    let editor_command = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        "case \"$2\" in \"$1\"/*) printf edited > \"$2\" ;; *) exit 88 ;; esac".to_string(),
        "editor".to_string(),
        editor_directory.to_string_lossy().into_owned(),
    ];

    let content = run_editor(
        "seed",
        &editor_command,
        &paths.codex_home,
        &policy,
        &paths.cwd,
    )
    .await
    .expect("run editor with isolated buffer");

    assert_eq!(content, "edited");
}

#[tokio::test]
#[cfg(any(target_os = "macos", target_os = "linux"))]
async fn editor_process_uses_protected_workspace_fallback_with_default_temporary_grants() {
    let root = tempfile::tempdir().expect("create editor test root");
    let codex_home = root.path().join("codex-home");
    let cwd = root.path().join("workspace");
    fs::create_dir(&codex_home).expect("create Codex home");
    fs::create_dir(&cwd).expect("create workspace");
    let default_codex_home = dirs::home_dir().expect("home directory").join(".codex");
    let writable_default_codex_home = AbsolutePathBuf::from_absolute_path(&default_codex_home)
        .expect("absolute default Codex home");
    let policy = FileSystemSandboxPolicy::workspace_write(
        &[writable_default_codex_home],
        /*exclude_tmpdir_env_var*/ false,
        /*exclude_slash_tmp*/ false,
    );
    let workspace_codex_home = cwd.join(".codex");
    let editor_directory = dunce::canonicalize(&cwd)
        .expect("canonicalize workspace")
        .join(".codex")
        .join("editor");
    let editor_command = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        "case \"$2\" in \"$1\"/*) printf edited > \"$2\" ;; *) exit 88 ;; esac".to_string(),
        "editor".to_string(),
        editor_directory.to_string_lossy().into_owned(),
    ];

    assert!(!workspace_codex_home.exists());
    assert!(policy.can_write_path_with_cwd(&codex_home, &cwd));
    assert!(policy.can_write_path_with_cwd(&default_codex_home, &cwd));
    assert!(!policy.can_write_path_with_cwd(&workspace_codex_home, &cwd));

    let content = run_editor("seed", &editor_command, &codex_home, &policy, &cwd)
        .await
        .expect("run editor with protected workspace fallback");

    assert_eq!(content, "edited");
    assert!(editor_directory.is_dir());
}
