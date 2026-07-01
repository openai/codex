use super::*;
use crate::apply::ApplyGitRequest;
use crate::apply::apply_git_patch;
use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::OnceLock;

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn run(cwd: &Path, args: &[&str]) -> (i32, String, String) {
    let mut command = std::process::Command::new(args[0]);
    crate::safe_git::isolate_git_command_environment(&mut command);
    let out = command
        .args(&args[1..])
        .current_dir(cwd)
        .output()
        .expect("spawn ok");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn run_isolated_test(test_name: &str, env: &[(&str, &OsStr)]) {
    let mut command = std::process::Command::new(std::env::current_exe().expect("test binary"));
    crate::safe_git::isolate_git_command_environment(&mut command);
    let output = command
        .args([test_name, "--exact", "--nocapture"])
        .env("CODEX_GIT_UTILS_PATH_ENV_CHILD", "1")
        .env("RUST_TEST_THREADS", "1")
        .envs(env.iter().copied())
        .output()
        .expect("run isolated test process");
    assert!(
        output.status.success(),
        "isolated test {test_name} failed: {output:?}"
    );
}

fn init_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    // git init and minimal identity
    let _ = run(root, &["git", "init"]);
    let _ = run(root, &["git", "config", "user.email", "codex@example.com"]);
    let _ = run(root, &["git", "config", "user.name", "Codex"]);
    dir
}

fn read_file_normalized(path: &Path) -> String {
    std::fs::read_to_string(path)
        .expect("read file")
        .replace("\r\n", "\n")
}

fn effective_paths(diff: &str, revert: bool) -> io::Result<Vec<String>> {
    let (tmpdir, patch_path) = write_temp_patch(diff)?;
    let cwd = std::env::current_dir()?;
    let git = GitRunner::for_cwd_io(&cwd)?;
    let paths = extract_effective_paths_from_patch(&git, &patch_path, revert)?;
    drop(tmpdir);
    Ok(paths)
}

#[test]
fn effective_paths_cover_supported_patch_headers() {
    let cases = [
        (
            "quoted new file",
            "diff --git \"a/hello world.txt\" \"b/hello world.txt\"\nnew file mode 100644\n--- /dev/null\n+++ b/hello world.txt\n@@ -0,0 +1 @@\n+hi\n",
            vec!["hello world.txt"],
        ),
        (
            "unquoted spaced path",
            "diff --git a/space name.txt b/space name.txt\n--- a/space name.txt\n+++ b/space name.txt\n@@ -1 +1 @@\n-old\n+new\n",
            vec!["space name.txt"],
        ),
        (
            "headerless p0 inference",
            "--- headerless-p0.txt\n+++ headerless-p0.txt\n@@ -1 +1 @@\n-old\n+new\n",
            vec!["headerless-p0.txt"],
        ),
        (
            "headerless unified diff",
            "--- old/headerless.txt\n+++ new/headerless.txt\n@@ -1 +1 @@\n-old\n+new\n",
            vec!["headerless.txt"],
        ),
        (
            "arbitrary prefixes",
            "diff --git left/file.txt right/file.txt\n--- before/file.txt\n+++ after/file.txt\n@@ -1 +1 @@\n-old\n+new\n",
            vec!["file.txt"],
        ),
        (
            "deleted file",
            "diff --git a/gone.txt b/gone.txt\ndeleted file mode 100644\n--- a/gone.txt\n+++ /dev/null\n@@ -1 +0,0 @@\n-old\n",
            vec!["gone.txt"],
        ),
        (
            "literal dev/null path",
            "diff --git a/dev/null b/dev/null\n--- a/dev/null\n+++ b/dev/null\n@@ -1 +1 @@\n-old\n+new\n",
            vec!["dev/null"],
        ),
        (
            "rename",
            "diff --git a/rename-old.txt b/rename-new.txt\nsimilarity index 100%\nrename from rename-old.txt\nrename to rename-new.txt\n",
            vec!["rename-new.txt", "rename-old.txt"],
        ),
        (
            "copy",
            "diff --git a/copy-old.txt b/copy-new.txt\nsimilarity index 100%\ncopy from copy-old.txt\ncopy to copy-new.txt\n",
            vec!["copy-new.txt", "copy-old.txt"],
        ),
    ];

    for (name, diff, expected) in cases {
        for revert in [false, true] {
            assert_eq!(
                effective_paths(diff, revert).unwrap_or_else(|error| panic!("{name}: {error}")),
                expected,
                "{name}, revert={revert}"
            );
        }
        assert_eq!(extract_paths_from_patch(diff), expected, "{name}");
    }

    let nul_rename_paths = parse_numstat_paths(b"0\t0\t\0old name.txt\0new name.txt\0")
        .expect("parse NUL-delimited rename paths");
    assert_eq!(
        nul_rename_paths,
        vec!["old name.txt".to_string(), "new name.txt".to_string()]
    );
}

#[test]
fn effective_paths_follow_git_for_mismatched_headers() {
    let mismatch = "diff --git a/safe.txt b/safe.txt\n--- a/nested/file.txt\n+++ b/nested/file.txt\n@@ -1 +1 @@\n-old\n+new\n";
    let expected = vec!["nested/file.txt".to_string()];
    assert_eq!(
        effective_paths(mismatch, /*revert*/ false).unwrap(),
        expected
    );
    assert_eq!(
        effective_paths(mismatch, /*revert*/ true).unwrap(),
        expected
    );
    assert_eq!(extract_paths_from_patch(mismatch), expected);
}

#[test]
fn effective_paths_reject_platform_ambiguous_paths() {
    let error = effective_paths("", /*revert*/ false).expect_err("reject empty patch paths");
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    #[cfg(windows)]
    {
        let error = validate_patch_path("..\\nested\\file.txt".to_string())
            .expect_err("reject Windows path separators");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        let error =
            validate_patch_path("C:/outside.txt".to_string()).expect_err("reject drive prefix");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }
    #[cfg(unix)]
    {
        assert_eq!(
            validate_patch_path("back\\slash.txt".to_string()).expect("valid Unix filename"),
            "back\\slash.txt"
        );
        assert_eq!(
            validate_patch_path("a:file.txt".to_string()).expect("valid Unix filename"),
            "a:file.txt"
        );
    }
}

#[test]
fn windows_namespace_validator_rejects_aliases_and_reserved_names() {
    let rejected_paths = [
        "file.txt:stream",
        "dir/file.txt::$DATA",
        "dir:stream/file.txt",
        "file.",
        "file ",
        "dir./file.txt",
        "dir /file.txt",
        "dir/CoM¹ .log",
        "dir/lPt² .log",
        "dir/CoM³ .log",
    ];
    for path in rejected_paths {
        assert!(invalid_windows_patch_path(path), "must reject {path:?}");
    }

    for punctuation in ['\\', '<', '>', ':', '"', '|', '?', '*'] {
        let path = format!("dir/file{punctuation}name.txt");
        assert!(invalid_windows_patch_path(&path), "must reject {path:?}");
    }
    for control in ['\0', '\u{0001}', '\u{001f}'] {
        let path = format!("dir/file{control}name.txt");
        assert!(invalid_windows_patch_path(&path), "must reject {path:?}");
    }
    for family in ["AUX", "CON", "CONIN$", "CONOUT$", "NUL", "PRN"] {
        for path in [
            family.to_string(),
            format!("{}.txt", family.to_ascii_lowercase()),
            format!("{family} .log"),
        ] {
            assert!(invalid_windows_patch_path(&path), "must reject {path:?}");
        }
    }
    for digit in "123456789¹²³".chars() {
        for family in ["CoM", "LpT"] {
            for suffix in ["", ".txt", " .log"] {
                let path = format!("{family}{digit}{suffix}");
                assert!(invalid_windows_patch_path(&path), "must reject {path:?}");
            }
        }
    }

    for path in "COM0 COM10 LPT0 LPT10 COM⁴ LPT⁴ NULx contest.txt auxiliary.txt printer.txt conin$x conout$x ordinary.file"
        .split_ascii_whitespace()
    {
        assert!(
            !invalid_windows_patch_path(path),
            "must allow near miss {path:?}"
        );
    }
}

#[cfg(windows)]
#[test]
fn apply_rejects_win32_namespace_aliases_with_protect_ntfs_disabled() {
    let _g = env_lock().lock().unwrap();
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join("file.txt"), "original\n").expect("write fixture");
    let (add_code, _, add_err) = run(root, &["git", "add", "file.txt"]);
    assert_eq!(add_code, 0, "add fixture: {add_err}");
    let (commit_code, _, commit_err) = run(root, &["git", "commit", "-m", "fixture"]);
    assert_eq!(commit_code, 0, "commit fixture: {commit_err}");
    let (config_code, _, config_err) = run(root, &["git", "config", "core.protectNTFS", "false"]);
    assert_eq!(config_code, 0, "disable core.protectNTFS: {config_err}");

    let cases = [
        (
            "ADS",
            "diff --git a/file.txt:stream b/file.txt:stream\nnew file mode 100644\n--- /dev/null\n+++ b/file.txt:stream\n@@ -0,0 +1 @@\n+stream\n",
        ),
        (
            "trailing dot",
            "diff --git a/file.txt. b/file.txt.\n--- a/file.txt.\n+++ b/file.txt.\n@@ -1 +1 @@\n-original\n+mutated\n",
        ),
        (
            "trailing space",
            "diff --git \"a/file.txt \" \"b/file.txt \"\n--- \"a/file.txt \"\n+++ \"b/file.txt \"\n@@ -1 +1 @@\n-original\n+mutated\n",
        ),
        (
            "device name",
            "diff --git a/NUL.txt b/NUL.txt\nnew file mode 100644\n--- /dev/null\n+++ b/NUL.txt\n@@ -0,0 +1 @@\n+device\n",
        ),
        (
            "superscript device name",
            "diff --git a/COM¹.txt b/COM¹.txt\nnew file mode 100644\n--- /dev/null\n+++ b/COM¹.txt\n@@ -0,0 +1 @@\n+device\n",
        ),
    ];

    for (name, diff) in cases {
        for revert in [false, true] {
            let error = apply_git_patch(&ApplyGitRequest {
                cwd: root.to_path_buf(),
                diff: diff.to_string(),
                revert,
                preflight: false,
            })
            .expect_err("reject Win32 namespace alias before mutation");
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{name}");
            assert_eq!(
                error.to_string(),
                "patch path is not a normalized repository-relative path",
                "{name}, revert={revert}"
            );
            assert_eq!(
                read_file_normalized(&root.join("file.txt")),
                "original\n",
                "{name}, revert={revert}"
            );
            let (status_code, status, status_err) = run(root, &["git", "status", "--porcelain=v1"]);
            assert_eq!(status_code, 0, "status after {name}: {status_err}");
            assert!(status.is_empty(), "{name}, revert={revert}: {status:?}");
            assert!(
                std::fs::metadata(root.join("file.txt:stream")).is_err(),
                "{name}, revert={revert}: ADS must not be created"
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn patch_applies_valid_unix_colon_and_backslash_paths() {
    let _g = env_lock().lock().unwrap();
    let repo = init_repo();
    let root = repo.path();
    for (path, contents) in [
        ("a:file.txt", "old colon\n"),
        ("back\\slash.txt", "old slash\n"),
    ] {
        std::fs::write(root.join(path), contents).expect("write fixture");
    }
    let (add_code, _, add_err) = run(
        root,
        &[
            "git",
            "--literal-pathspecs",
            "add",
            "--",
            "a:file.txt",
            "back\\slash.txt",
        ],
    );
    assert_eq!(add_code, 0, "add fixture: {add_err}");
    let (commit_code, _, commit_err) = run(root, &["git", "commit", "-m", "fixture"]);
    assert_eq!(commit_code, 0, "commit fixture: {commit_err}");

    std::fs::write(root.join("a:file.txt"), "new colon\n").expect("modify colon path");
    std::fs::write(root.join("back\\slash.txt"), "new slash\n").expect("modify backslash path");
    let (diff_code, diff, diff_err) = run(
        root,
        &[
            "git",
            "--literal-pathspecs",
            "diff",
            "--full-index",
            "--binary",
            "--",
            "a:file.txt",
            "back\\slash.txt",
        ],
    );
    assert_eq!(diff_code, 0, "create patch: {diff_err}");
    assert!(!diff.is_empty(), "fixture patch must not be empty");
    let (restore_code, _, restore_err) = run(
        root,
        &[
            "git",
            "--literal-pathspecs",
            "checkout",
            "--",
            "a:file.txt",
            "back\\slash.txt",
        ],
    );
    assert_eq!(restore_code, 0, "restore fixture: {restore_err}");

    for preflight in [true, false] {
        let result = apply_git_patch(&ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: diff.clone(),
            revert: false,
            preflight,
        })
        .unwrap_or_else(|error| panic!("forward preflight={preflight}: {error}"));
        assert_eq!(result.exit_code, 0, "forward preflight={preflight}");
    }
    assert_eq!(
        read_file_normalized(&root.join("a:file.txt")),
        "new colon\n"
    );
    assert_eq!(
        read_file_normalized(&root.join("back\\slash.txt")),
        "new slash\n"
    );

    for preflight in [true, false] {
        let result = apply_git_patch(&ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: diff.clone(),
            revert: true,
            preflight,
        })
        .unwrap_or_else(|error| panic!("reverse preflight={preflight}: {error}"));
        assert_eq!(result.exit_code, 0, "reverse preflight={preflight}");
    }
    assert_eq!(
        read_file_normalized(&root.join("a:file.txt")),
        "old colon\n"
    );
    assert_eq!(
        read_file_normalized(&root.join("back\\slash.txt")),
        "old slash\n"
    );
}

fn new_file_diff(path: &str) -> String {
    format!(
        "diff --git a/{path} b/{path}\nnew file mode 100644\n--- /dev/null\n+++ b/{path}\n@@ -0,0 +1 @@\n+new\n"
    )
}

fn git_index_bytes(root: &Path) -> Vec<u8> {
    let mut command = std::process::Command::new("git");
    crate::safe_git::isolate_git_command_environment(&mut command);
    let output = command
        .args(["ls-files", "--stage", "-z"])
        .current_dir(root)
        .output()
        .expect("inspect index");
    assert!(output.status.success(), "{:?}", output.status);
    output.stdout
}

fn assert_stage_refused(root: &Path, path: &str, before: &[u8]) {
    let error = stage_paths(root, &new_file_diff(path)).expect_err("reject path alias");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(git_index_bytes(root), before);
}

fn commit_seed(root: &Path) {
    std::fs::write(root.join("seed.txt"), "seed\n").expect("write seed");
    assert_eq!(run(root, &["git", "add", "seed.txt"]).0, 0);
    assert_eq!(run(root, &["git", "commit", "-m", "seed"]).0, 0);
}

#[cfg(any(unix, windows))]
fn create_dir_alias(target: &Path, alias: &Path) -> DirectoryAlias {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, alias).expect("create directory symlink");
    #[cfg(windows)]
    {
        // Bazel's GNU Windows runner can surface temporary paths with `/`
        // separators. `mklink` treats those separators as option prefixes, so
        // pass native separators to the cmd.exe built-in.
        let alias = alias.as_os_str().to_string_lossy().replace('/', "\\");
        let target = target.as_os_str().to_string_lossy().replace('/', "\\");
        let output = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(alias)
            .arg(target)
            .output()
            .expect("spawn mklink");
        assert!(output.status.success(), "mklink /J failed: {output:?}");
    }
    DirectoryAlias(alias.to_path_buf())
}

#[cfg(any(unix, windows))]
struct DirectoryAlias(PathBuf);

#[cfg(any(unix, windows))]
impl Drop for DirectoryAlias {
    fn drop(&mut self) {
        #[cfg(unix)]
        let _ = std::fs::remove_file(&self.0);
        #[cfg(windows)]
        let _ = std::fs::remove_dir(&self.0);
    }
}

#[cfg(any(unix, windows))]
fn assert_longest_existing_alias_prefix_carries_unresolved_suffix() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::create_dir_all(root.join("container/realdir")).expect("create alias target");
    let alias = root.join("alias");
    let _alias = create_dir_alias(&root.join("container/realdir"), &alias);

    let git = GitRunner::for_cwd_io(root).expect("git runner");
    let paths = [
        "alias/nested/file.txt".to_string(),
        "missing/path.txt".to_string(),
        "alias/nested/file.txt".to_string(),
    ];
    let confined = confine_patch_paths(&git, root, &paths).expect("confine");
    assert_eq!(
        confine_patch_paths(&git, root, &paths)
            .expect("confine exact leaves")
            .into_exact_leaves()
            .expect("extract exact leaves"),
        paths
    );
    assert!(
        confine_patch_paths(&git, root, &[])
            .unwrap()
            .into_exact_leaves()
            .expect("extract no exact leaves")
            .is_empty()
    );
    use ConfinedPathOrigin::*;
    use ConfinedPathRole::*;
    let candidate = |path: &str, origin: ConfinedPathOrigin, role: ConfinedPathRole| {
        ConfinedPathCandidate::new(path.to_string(), origin, role)
    };
    assert_eq!(
        confined.entries[0].candidates,
        vec![
            candidate("alias", Raw, StrictAncestor),
            candidate("alias/nested", Raw, StrictAncestor),
            candidate("alias/nested/file.txt", Raw, Leaf),
            candidate("container", Canonical, StrictAncestor),
            candidate("container/realdir", Canonical, StrictAncestor),
            candidate("container/realdir/nested", Canonical, StrictAncestor),
            candidate("container/realdir/nested/file.txt", Canonical, Leaf),
        ]
    );
    assert_eq!(
        confined.entries[0]
            .candidates
            .iter()
            .map(|candidate| candidate.depth)
            .collect::<Vec<_>>(),
        [1, 2, 3, 1, 2, 3, 4]
    );
    let missing = &confined.entries[1].candidates;
    assert_eq!(
        (
            &missing[0].path,
            missing[0].origin,
            &missing[2].path,
            missing[2].origin
        ),
        (&missing[2].path, Raw, &missing[0].path, Canonical)
    );
}

#[cfg(unix)]
#[test]
fn longest_existing_symlink_prefix_carries_the_unresolved_suffix() {
    assert_longest_existing_alias_prefix_carries_unresolved_suffix();
}

#[cfg(windows)]
#[test]
fn longest_existing_junction_prefix_carries_the_unresolved_suffix() {
    assert_longest_existing_alias_prefix_carries_unresolved_suffix();
}

#[cfg(any(unix, windows))]
#[test]
fn stage_rejects_strict_ancestor_alias_outside_or_to_worktree_root_without_mutation() {
    let _g = env_lock().lock().unwrap();
    let repo = init_repo();
    let root = repo.path();
    let outside = tempfile::tempdir().expect("outside directory");
    std::fs::write(outside.path().join("marker.txt"), "outside\n").expect("outside marker");
    let before = git_index_bytes(root);

    for (name, target) in [("outside-alias", outside.path()), ("root-alias", root)] {
        let alias = root.join(name);
        let _alias = create_dir_alias(target, &alias);
        assert_stage_refused(root, &format!("{name}/marker.txt"), &before);
        #[cfg(windows)]
        assert_stage_refused(root, name, &before);
    }
    assert_eq!(
        std::fs::read(outside.path().join("marker.txt")).unwrap(),
        b"outside\n"
    );

    std::fs::create_dir(root.join("inside")).expect("create in-tree return target");
    let escape = root.join("escape");
    let _escape = create_dir_alias(outside.path(), &escape);
    let _return = create_dir_alias(&root.join("inside"), &outside.path().join("return"));
    assert_stage_refused(root, "escape/return/file.txt", &before);
}

#[cfg(any(unix, windows))]
#[test]
fn stage_rejects_aliases_into_private_and_common_git_metadata() {
    let _g = env_lock().lock().unwrap();
    let repo = init_repo();
    commit_seed(repo.path());
    let linked_holder = tempfile::tempdir().expect("linked holder");
    let linked = linked_holder.path().join("linked");
    let linked_arg = linked.to_string_lossy().into_owned();
    assert_eq!(
        run(
            repo.path(),
            &["git", "worktree", "add", "-b", "s3a-linked", &linked_arg]
        )
        .0,
        0
    );

    let resolve = |arg| {
        let output = run(&linked, &["git", "rev-parse", arg]);
        assert_eq!(output.0, 0, "resolve {arg}: {}", output.2);
        let path = PathBuf::from(output.1.trim());
        std::fs::canonicalize(if path.is_absolute() {
            path
        } else {
            linked.join(path)
        })
        .unwrap()
    };
    let targets = [resolve("--absolute-git-dir"), resolve("--git-common-dir")];
    assert_ne!(targets[0], targets[1]);
    let before = git_index_bytes(&linked);

    for (index, target) in targets.into_iter().enumerate() {
        let name = format!("metadata-alias-{index}");
        let alias = linked.join(&name);
        let _alias = create_dir_alias(&target, &alias);
        assert_stage_refused(&linked, &format!("{name}/probe"), &before);
        #[cfg(windows)]
        assert_stage_refused(&linked, &name, &before);
    }
}

#[cfg(unix)]
#[test]
fn stage_allows_outside_pointing_leaf_symlink_without_touching_target() {
    let _g = env_lock().lock().unwrap();
    let repo = init_repo();
    let outside = tempfile::tempdir().expect("outside directory");
    let target = outside.path().join("target.txt");
    std::fs::write(&target, "outside\n").expect("write target");
    std::os::unix::fs::symlink(&target, repo.path().join("leaf")).expect("create leaf symlink");

    stage_paths(repo.path(), &new_file_diff("leaf")).expect("stage leaf symlink");
    assert_eq!(
        std::fs::read_to_string(target).expect("read target"),
        "outside\n"
    );
    assert!(String::from_utf8_lossy(&git_index_bytes(repo.path())).contains("\tleaf\0"));
}

#[test]
fn stage_allows_ordinary_missing_and_unrelated_unicode_paths() {
    let _g = env_lock().lock().unwrap();
    let repo = init_repo();
    let root = repo.path();
    std::fs::create_dir(root.join("résumé")).expect("create Unicode directory");
    std::fs::write(root.join("résumé/file.txt"), "new\n").expect("write Unicode file");

    stage_paths(root, &new_file_diff("résumé/file.txt")).expect("stage Unicode path");
    assert!(String::from_utf8_lossy(&git_index_bytes(root)).contains("résumé/file.txt"));

    let before = git_index_bytes(root);
    stage_paths(root, &new_file_diff("absent/nested.txt")).expect("allow missing path");
    assert_eq!(git_index_bytes(root), before);
    assert!(
        !root.join("absent").exists(),
        "containment must be observational"
    );

    std::fs::create_dir(root.join("directory")).unwrap();
    std::fs::write(root.join("directory/unrelated.txt"), "unrelated\n").unwrap();
    assert_stage_refused(root, "directory", &before);
}

#[test]
fn containment_does_not_claim_uninitialized_gitlinks() {
    let _g = env_lock().lock().unwrap();
    let repo = init_repo();
    let root = repo.path();
    commit_seed(root);
    let oid = run(root, &["git", "rev-parse", "HEAD"]);
    assert_eq!(oid.0, 0, "{}", oid.2);
    let cacheinfo = format!("160000,{},nested", oid.1.trim());
    assert_eq!(
        run(
            root,
            &["git", "update-index", "--add", "--cacheinfo", &cacheinfo]
        )
        .0,
        0
    );
    assert!(!root.join("nested").exists());

    let git = GitRunner::for_cwd_io(root).expect("git runner");
    let paths = ["nested/file.txt".to_string()];
    let confined = confine_patch_paths(&git, root, &paths).expect("record candidates only");
    let candidate = &confined.entries[0].candidates[0];
    assert_eq!(
        (candidate.path.as_str(), candidate.origin),
        ("nested", ConfinedPathOrigin::Raw)
    );
}

#[cfg(any(unix, windows))]
#[test]
fn containment_metadata_queries_ignore_inherited_git_selection_environment() {
    let _g = env_lock().lock().unwrap();
    if std::env::var_os("CODEX_GIT_UTILS_PATH_ENV_CHILD").is_some() {
        let root = PathBuf::from(
            std::env::var_os("CODEX_GIT_UTILS_TARGET_REPO").expect("target repository"),
        );
        let git = GitRunner::for_cwd_io(&root).expect("target git runner");
        let paths = ["metadata-alias/probe".to_string()];
        let error =
            confine_patch_paths(&git, &root, &paths).expect_err("use target repository metadata");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        return;
    }

    let target = init_repo();
    let metadata_alias = target.path().join("metadata-alias");
    let _metadata_alias = create_dir_alias(&target.path().join(".git"), &metadata_alias);
    let alternate = init_repo();
    let alternate_git_dir = alternate.path().join(".git");
    let target_env = ("CODEX_GIT_UTILS_TARGET_REPO", target.path().as_os_str());
    for (name, value) in [
        ("GIT_DIR", alternate_git_dir.as_os_str()),
        ("GIT_WORK_TREE", alternate.path().as_os_str()),
        ("GIT_COMMON_DIR", alternate_git_dir.as_os_str()),
        ("GIT_PREFIX", OsStr::new("elsewhere/")),
    ] {
        run_isolated_test(
            "patch_paths::tests::containment_metadata_queries_ignore_inherited_git_selection_environment",
            &[target_env, (name, value)],
        );
    }
}

#[cfg(unix)]
#[test]
fn staging_rejects_global_lfs_filter_without_running_it() {
    let _g = env_lock().lock().unwrap();
    if std::env::var_os("CODEX_GIT_UTILS_PATH_ENV_CHILD").is_some() {
        let root = PathBuf::from(
            std::env::var_os("CODEX_GIT_UTILS_TARGET_REPO").expect("target repository"),
        );
        let git = GitRunner::for_cwd_io(&root).expect("trusted Git");
        let error = stage_effective_paths(&git, &root, &["file.txt".to_string()])
            .expect_err("reject global Git LFS filter");
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
        return;
    }

    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join(".gitattributes"), "file.txt filter=lfs\n").expect("write attributes");
    std::fs::write(root.join("file.txt"), "old\n").expect("write file");
    let (add_code, _, add_err) = run(root, &["git", "add", "."]);
    assert_eq!(add_code, 0, "add base files: {add_err}");
    let (commit_code, _, commit_err) = run(root, &["git", "commit", "-m", "base"]);
    assert_eq!(commit_code, 0, "commit base files: {commit_err}");
    std::fs::write(root.join("file.txt"), "new\n").expect("modify file");

    let config_dir = tempfile::tempdir().expect("config tempdir");
    let global_config = config_dir.path().join("global.gitconfig");
    let system_config = config_dir.path().join("system.gitconfig");
    let filter_marker = config_dir.path().join("repo-lfs-ran");
    let repo_git_lfs = root.join("git-lfs");
    std::fs::write(
        &repo_git_lfs,
        "#!/bin/sh\n: > \"$CODEX_GIT_UTILS_LFS_MARKER\"\ncat\n",
    )
    .expect("write repository git-lfs");
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = std::fs::metadata(&repo_git_lfs)
            .expect("repository git-lfs metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&repo_git_lfs, permissions)
            .expect("make repository git-lfs executable");
    }
    std::fs::write(
        &global_config,
        "[filter \"lfs\"]\n\tclean = git-lfs clean -- %f\n\trequired = true\n",
    )
    .expect("write global config");
    std::fs::write(&system_config, "").expect("write system config");
    run_isolated_test(
        "patch_paths::tests::staging_rejects_global_lfs_filter_without_running_it",
        &[
            ("CODEX_GIT_UTILS_TARGET_REPO", root.as_os_str()),
            ("CODEX_GIT_UTILS_LFS_MARKER", filter_marker.as_os_str()),
            ("GIT_CONFIG_GLOBAL", global_config.as_os_str()),
            ("GIT_CONFIG_SYSTEM", system_config.as_os_str()),
            ("GIT_EXEC_PATH", root.as_os_str()),
            ("GIT_GLOB_PATHSPECS", OsStr::new("1")),
            ("GIT_ICASE_PATHSPECS", OsStr::new("1")),
        ],
    );

    assert!(!filter_marker.exists(), "Git LFS filter must not run");
    let (diff_code, staged, diff_err) = run(root, &["git", "diff", "--cached", "--name-only"]);
    assert_eq!(diff_code, 0, "read staged paths: {diff_err}");
    assert!(staged.is_empty(), "staging changed the index: {staged}");
}
