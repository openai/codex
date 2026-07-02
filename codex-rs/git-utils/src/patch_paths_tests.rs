use super::*;
use crate::apply::ApplyGitRequest;
use crate::apply::apply_git_patch;
use crate::exact_staging::StagePathsResult;
use crate::exact_staging::update_index_exact_paths;
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

fn init_repo_with_selected_filter(config_key: &str) -> tempfile::TempDir {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join(".gitattributes"), "file.txt filter=selected\n")
        .expect("write attributes");
    std::fs::write(root.join("file.txt"), "old\n").expect("write tracked file");
    assert_eq!(run(root, &["git", "add", "."]).0, 0);
    assert_eq!(run(root, &["git", "commit", "-m", "base"]).0, 0);
    assert_eq!(
        run(
            root,
            &[
                "git",
                "config",
                config_key,
                "codex-definitely-missing-filter-command",
            ],
        )
        .0,
        0
    );
    std::fs::write(root.join("file.txt"), "new\n").expect("modify tracked file");
    repo
}

#[cfg(unix)]
struct RacyFilterFixture {
    repo: tempfile::TempDir,
    marker: PathBuf,
}

#[cfg(unix)]
fn init_racy_filter_fixture(driver: &str, command: &str) -> RacyFilterFixture {
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::fs::symlink;
    use std::time::Duration;
    use std::time::SystemTime;

    let repo = init_repo();
    let root = repo.path();
    std::fs::write(
        root.join(".gitattributes"),
        format!(
            "outside.txt filter={driver}\nlink filter={driver}\nselected.txt filter={driver}\n"
        ),
    )
    .expect("write attributes");
    std::fs::write(root.join("outside.txt"), "outside-racy\n").expect("write racy file");
    std::fs::write(root.join("target.txt"), "old target\n").expect("write target");
    std::fs::write(root.join("selected.txt"), "old selected\n").expect("write selected target");
    symlink("old-target", root.join("link")).expect("create initial symlink");

    let future = SystemTime::UNIX_EPOCH + Duration::from_secs(1_893_484_800);
    let outside = std::fs::File::options()
        .read(true)
        .write(true)
        .open(root.join("outside.txt"))
        .expect("open racy file");
    outside
        .set_times(std::fs::FileTimes::new().set_modified(future))
        .expect("set future racy mtime");
    assert_eq!(run(root, &["git", "add", "."]).0, 0);
    assert_eq!(run(root, &["git", "commit", "-m", "base"]).0, 0);

    let marker = root.join("filter-ran");
    let helper = root.join("filter-helper.sh");
    let helper_body = if command == "clean" {
        format!("#!/bin/sh\n: > '{}'\ncat\n", marker.display())
    } else {
        format!("#!/bin/sh\n: > '{}'\nexit 1\n", marker.display())
    };
    std::fs::write(&helper, helper_body).expect("write filter helper");
    let mut permissions = std::fs::metadata(&helper)
        .expect("filter helper metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&helper, permissions).expect("make filter helper executable");
    let key = format!("filter.{driver}.{command}");
    let helper_arg = helper.to_string_lossy().into_owned();
    assert_eq!(run(root, &["git", "config", &key, &helper_arg]).0, 0);
    let required_key = format!("filter.{driver}.required");
    assert_eq!(run(root, &["git", "config", &required_key, "true"]).0, 0);
    assert!(!marker.exists(), "fixture setup must not run the filter");
    RacyFilterFixture { repo, marker }
}

fn index_entry(root: &Path, path: &str) -> String {
    let (code, stdout, stderr) = run(root, &["git", "ls-files", "--stage", "--", path]);
    assert_eq!(code, 0, "read index entry: {stderr}");
    stdout
}

fn exact_stage(root: &Path, paths: &[&str], content_filter_paths: &[&str]) -> StagePathsResult {
    let git = GitRunner::for_cwd_io(root).expect("trusted Git");
    update_index_exact_paths(
        &git,
        root,
        &paths
            .iter()
            .map(|path| (*path).to_string())
            .collect::<Vec<_>>(),
        &content_filter_paths
            .iter()
            .map(|path| (*path).to_string())
            .collect::<Vec<_>>(),
        &safe_git_config_parts(),
    )
    .expect("run exact staging primitive")
}

fn assert_exact_refused_unchanged(
    root: &Path,
    paths: &[&str],
    content_filter_paths: &[&str],
    message_fragment: &str,
) -> StagePathsResult {
    let before = git_index_bytes(root);
    let result = exact_stage(root, paths, content_filter_paths);
    assert_ne!(result.exit_code, 0, "exact staging unexpectedly succeeded");
    assert!(
        result.stderr.contains(message_fragment),
        "missing {message_fragment:?}: {result:?}"
    );
    assert_eq!(git_index_bytes(root), before, "refusal mutated the index");
    result
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
fn exact_staging_adds_an_untracked_file_and_removes_a_tracked_deletion() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join(".gitignore"), "gone.txt\n").expect("write ignore rule");
    std::fs::write(root.join("gone.txt"), "gone\n").expect("write deletion fixture");
    assert_eq!(run(root, &["git", "add", ".gitignore"]).0, 0);
    assert_eq!(run(root, &["git", "add", "-f", "gone.txt"]).0, 0);
    assert_eq!(run(root, &["git", "commit", "-m", "base"]).0, 0);
    std::fs::remove_file(root.join("gone.txt")).expect("remove tracked file");
    std::fs::write(root.join("fresh.txt"), "fresh\n").expect("write untracked file");

    let result = exact_stage(root, &["gone.txt", "fresh.txt"], &["fresh.txt"]);
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    let (code, staged, stderr) = run(root, &["git", "diff", "--cached", "--name-status"]);
    assert_eq!(code, 0, "read staged changes: {stderr}");
    assert_eq!(staged, "A\tfresh.txt\nD\tgone.txt\n");
}

#[test]
fn exact_staging_preserves_executable_index_mode_when_filemode_is_false() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join("mode.txt"), "old\n").expect("write mode fixture");
    assert_eq!(run(root, &["git", "add", "mode.txt"]).0, 0);
    assert_eq!(
        run(root, &["git", "update-index", "--chmod=+x", "mode.txt"]).0,
        0
    );
    assert_eq!(run(root, &["git", "commit", "-m", "executable"]).0, 0);
    assert_eq!(run(root, &["git", "config", "core.filemode", "false"]).0, 0);
    std::fs::write(root.join("mode.txt"), "new\n").expect("modify mode fixture");

    let result = exact_stage(root, &["mode.txt"], &["mode.txt"]);
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    assert!(
        index_entry(root, "mode.txt").starts_with("100755 "),
        "exact staging must retain the effective index mode"
    );
}

#[test]
fn ignored_untracked_path_returns_nonzero_without_partial_staging() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join(".gitignore"), "ignored.txt\n").expect("write ignore rule");
    assert_eq!(run(root, &["git", "add", ".gitignore"]).0, 0);
    assert_eq!(run(root, &["git", "commit", "-m", "ignore rule"]).0, 0);
    std::fs::write(root.join("ignored.txt"), "ignored\n").expect("write ignored file");
    std::fs::write(root.join("ordinary.txt"), "ordinary\n").expect("write ordinary file");

    let result = exact_stage(
        root,
        &["ignored.txt", "ordinary.txt"],
        &["ignored.txt", "ordinary.txt"],
    );
    assert_ne!(result.exit_code, 0);
    assert!(result.stderr.contains("ignored.txt"), "{}", result.stderr);
    assert!(
        run(root, &["git", "diff", "--cached", "--name-only"])
            .1
            .is_empty(),
        "the ignored-path preflight must prevent partial staging"
    );

    let git = GitRunner::for_cwd_io(root).expect("trusted Git");
    stage_effective_paths(
        &git,
        root,
        &["ignored.txt".to_string(), "ordinary.txt".to_string()],
        &safe_git_config_parts(),
    )
    .expect("public staging remains best effort for ignored paths");
    assert!(
        run(root, &["git", "diff", "--cached", "--name-only"])
            .1
            .is_empty()
    );
}

#[test]
fn ignored_missing_untracked_path_returns_nonzero_without_partial_staging() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join(".gitignore"), "missing-ignored.txt\n").expect("write ignore rule");
    assert_eq!(run(root, &["git", "add", ".gitignore"]).0, 0);
    assert_eq!(run(root, &["git", "commit", "-m", "ignore rule"]).0, 0);
    std::fs::write(root.join("ordinary.txt"), "ordinary\n").expect("write ordinary file");

    let result = exact_stage(
        root,
        &["missing-ignored.txt", "ordinary.txt"],
        &["ordinary.txt"],
    );
    assert_ne!(result.exit_code, 0);
    assert!(
        result.stderr.contains("missing-ignored.txt"),
        "{}",
        result.stderr
    );
    assert!(
        run(root, &["git", "diff", "--cached", "--name-only"])
            .1
            .is_empty(),
        "the missing ignored-path preflight must prevent partial staging"
    );
}

#[test]
fn tracked_path_that_matches_an_ignore_rule_still_stages() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join(".gitignore"), "tracked.txt\n").expect("write ignore rule");
    std::fs::write(root.join("tracked.txt"), "old\n").expect("write tracked file");
    assert_eq!(
        run(root, &["git", "add", "-f", ".gitignore", "tracked.txt"]).0,
        0
    );
    assert_eq!(run(root, &["git", "commit", "-m", "tracked ignored"]).0, 0);
    std::fs::write(root.join("tracked.txt"), "new\n").expect("modify tracked file");

    let result = exact_stage(root, &["tracked.txt"], &["tracked.txt"]);
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    assert_eq!(
        run(root, &["git", "diff", "--cached", "--name-only"]).1,
        "tracked.txt\n"
    );
}

#[test]
fn exact_staging_refuses_skip_and_assume_unchanged_without_mutation() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join("existing.txt"), "old existing\n").expect("write existing fixture");
    std::fs::write(root.join("missing.txt"), "old missing\n").expect("write missing fixture");
    std::fs::write(root.join("assumed.txt"), "old assumed\n").expect("write assumed fixture");
    assert_eq!(
        run(
            root,
            &["git", "add", "assumed.txt", "existing.txt", "missing.txt"]
        )
        .0,
        0
    );
    assert_eq!(run(root, &["git", "commit", "-m", "skip base"]).0, 0);
    assert_eq!(
        run(
            root,
            &[
                "git",
                "update-index",
                "--skip-worktree",
                "--",
                "existing.txt",
                "missing.txt",
            ],
        )
        .0,
        0
    );
    assert_eq!(
        run(
            root,
            &[
                "git",
                "update-index",
                "--assume-unchanged",
                "--",
                "assumed.txt",
            ],
        )
        .0,
        0
    );
    std::fs::write(root.join("existing.txt"), "new existing\n").expect("modify existing fixture");
    std::fs::remove_file(root.join("missing.txt")).expect("remove missing fixture");
    std::fs::remove_file(root.join("assumed.txt")).expect("remove assumed fixture");
    let before = git_index_bytes(root);

    let refused = exact_stage(
        root,
        &["assumed.txt", "existing.txt", "missing.txt"],
        &["existing.txt"],
    );
    assert_ne!(refused.exit_code, 0);
    assert!(refused.stderr.contains("skip-worktree"), "{refused:?}");
    assert!(refused.stderr.contains("assume-unchanged"), "{refused:?}");
    assert!(refused.stderr.contains("assumed.txt"), "{refused:?}");
    assert!(refused.stderr.contains("existing.txt"), "{refused:?}");
    assert!(refused.stderr.contains("missing.txt"), "{refused:?}");
    assert_eq!(git_index_bytes(root), before);

    let git = GitRunner::for_cwd_io(root).expect("trusted Git");
    stage_effective_paths(
        &git,
        root,
        &["existing.txt".to_string()],
        &safe_git_config_parts(),
    )
    .expect("public staging remains best effort for skip-worktree paths");
    assert_eq!(git_index_bytes(root), before);

    assert_eq!(
        run(
            root,
            &[
                "git",
                "update-index",
                "--no-skip-worktree",
                "--",
                "existing.txt",
                "missing.txt",
            ],
        )
        .0,
        0
    );
    assert_eq!(
        run(
            root,
            &[
                "git",
                "update-index",
                "--no-assume-unchanged",
                "--",
                "assumed.txt",
            ],
        )
        .0,
        0
    );
    let staged = exact_stage(
        root,
        &["assumed.txt", "existing.txt", "missing.txt"],
        &["existing.txt"],
    );
    assert_eq!(staged.exit_code, 0, "{}", staged.stderr);
    assert_eq!(
        run(root, &["git", "diff", "--cached", "--name-status"]).1,
        "D\tassumed.txt\nM\texisting.txt\nD\tmissing.txt\n"
    );
}

#[test]
fn exact_staging_refuses_case_mismatched_index_aliases_without_mutation() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join("foo.txt"), "old\n").expect("write case fixture");
    assert_eq!(run(root, &["git", "add", "foo.txt"]).0, 0);
    assert_eq!(run(root, &["git", "commit", "-m", "case base"]).0, 0);
    assert_eq!(
        run(root, &["git", "config", "core.ignoreCase", "true"]).0,
        0
    );
    assert_eq!(
        run(
            root,
            &["git", "update-index", "--skip-worktree", "--", "foo.txt",],
        )
        .0,
        0
    );
    let before = git_index_bytes(root);

    let refused = exact_stage(root, &["FOO.txt"], &["FOO.txt"]);
    assert_ne!(refused.exit_code, 0);
    assert!(refused.stderr.contains("core.ignoreCase"), "{refused:?}");
    assert!(refused.stderr.contains("FOO.txt"), "{refused:?}");
    assert_eq!(git_index_bytes(root), before);
    assert_eq!(run(root, &["git", "ls-files"]).1, "foo.txt\n");

    std::fs::write(root.join("FOO.txt"), "alias\n").expect("write case alias");
    let mapped_flag = exact_stage(root, &["foo.txt", "FOO.txt"], &["foo.txt", "FOO.txt"]);
    assert_ne!(mapped_flag.exit_code, 0);
    assert!(
        mapped_flag.stderr.contains("skip-worktree"),
        "{mapped_flag:?}"
    );
    assert_eq!(git_index_bytes(root), before);

    let exact_spelling = exact_stage(root, &["foo.txt"], &["foo.txt"]);
    assert_ne!(exact_spelling.exit_code, 0);
    assert!(
        exact_spelling.stderr.contains("skip-worktree"),
        "{exact_spelling:?}"
    );
    assert_eq!(git_index_bytes(root), before);

    assert_eq!(
        run(
            root,
            &["git", "update-index", "--no-skip-worktree", "--", "foo.txt",],
        )
        .0,
        0
    );
    std::fs::write(root.join("foo.txt"), "new\n").expect("modify case fixture");
    let staged = exact_stage(root, &["foo.txt"], &["foo.txt"]);
    assert_eq!(staged.exit_code, 0, "{}", staged.stderr);
    assert_eq!(
        run(root, &["git", "diff", "--cached", "--name-only"]).1,
        "foo.txt\n"
    );
}

#[test]
fn exact_staging_maps_only_explicit_present_case_aliases() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join("foo.txt"), "base\n").expect("write base path");
    assert_eq!(run(root, &["git", "add", "foo.txt"]).0, 0);
    assert_eq!(run(root, &["git", "commit", "-m", "case base"]).0, 0);
    assert_eq!(
        run(root, &["git", "config", "core.ignoreCase", "true"]).0,
        0
    );

    std::fs::write(root.join("foo.txt"), "lower\n").expect("modify canonical path");
    std::fs::write(root.join("FOO.txt"), "upper\n").expect("write requested alias");
    let expected = std::fs::read_to_string(root.join("foo.txt")).expect("read canonical path");
    let staged = exact_stage(root, &["foo.txt", "FOO.txt"], &["FOO.txt"]);
    assert_eq!(staged.exit_code, 0, "{}", staged.stderr);
    assert_eq!(run(root, &["git", "ls-files"]).1, "foo.txt\n");
    assert_eq!(run(root, &["git", "show", ":foo.txt"]).1, expected);

    assert_eq!(run(root, &["git", "commit", "-m", "mapped alias"]).0, 0);
    std::fs::remove_file(root.join("FOO.txt")).expect("remove requested alias");
    assert_exact_refused_unchanged(
        root,
        &["foo.txt", "FOO.txt"],
        &["foo.txt"],
        "missing requested case alias",
    );
}

#[test]
fn exact_staging_refuses_case_collisions_within_the_request_set() {
    let repo = init_repo();
    let root = repo.path();
    assert_eq!(
        run(root, &["git", "config", "core.ignoreCase", "true"]).0,
        0
    );
    std::fs::write(root.join("foo.txt"), "lower\n").expect("write lower path");
    std::fs::write(root.join("FOO.txt"), "upper\n").expect("write upper path");
    assert_exact_refused_unchanged(
        root,
        &["foo.txt", "FOO.txt"],
        &["foo.txt", "FOO.txt"],
        "request-set full-path aliases",
    );

    std::fs::create_dir_all(root.join("dir")).expect("create lower directory");
    std::fs::create_dir_all(root.join("DIR")).expect("create upper directory");
    std::fs::write(root.join("dir/lower.txt"), "lower\n").expect("write lower child");
    std::fs::write(root.join("DIR/upper.txt"), "upper\n").expect("write upper child");
    assert_exact_refused_unchanged(
        root,
        &["dir/lower.txt", "DIR/upper.txt"],
        &["dir/lower.txt", "DIR/upper.txt"],
        "request-set directory-prefix aliases",
    );

    assert_exact_refused_unchanged(
        root,
        &["Leaf", "leaf/child.txt"],
        &[],
        "request-set file/directory aliases",
    );
}

#[test]
fn exact_staging_refuses_ambiguous_and_prefix_colliding_index_paths() {
    {
        let repo = init_repo();
        let root = repo.path();
        std::fs::write(root.join("foo.txt"), "base\n").expect("write base path");
        assert_eq!(run(root, &["git", "add", "foo.txt"]).0, 0);
        assert_eq!(run(root, &["git", "commit", "-m", "case base"]).0, 0);
        assert_eq!(
            run(root, &["git", "config", "core.ignoreCase", "true"]).0,
            0
        );
        let oid = run(root, &["git", "rev-parse", ":foo.txt"]).1;
        let cacheinfo = format!("100644,{},FOO.txt", oid.trim());
        assert_eq!(
            run(
                root,
                &[
                    "git",
                    "--literal-pathspecs",
                    "update-index",
                    "--add",
                    "--cacheinfo",
                    &cacheinfo
                ],
            )
            .0,
            0
        );
        assert_eq!(run(root, &["git", "ls-files"]).1, "FOO.txt\nfoo.txt\n");
        assert_exact_refused_unchanged(root, &["foo.txt"], &["foo.txt"], "ambiguous index aliases");
    }

    {
        let repo = init_repo();
        let root = repo.path();
        std::fs::write(root.join("Foo"), "leaf\n").expect("write ancestor leaf");
        assert_eq!(run(root, &["git", "add", "Foo"]).0, 0);
        assert_eq!(run(root, &["git", "commit", "-m", "ancestor base"]).0, 0);
        assert_eq!(
            run(root, &["git", "config", "core.ignoreCase", "true"]).0,
            0
        );
        std::fs::remove_file(root.join("Foo")).expect("remove ancestor leaf");
        std::fs::create_dir(root.join("foo")).expect("create alias directory");
        std::fs::write(root.join("foo/bar.txt"), "child\n").expect("write child");
        assert_exact_refused_unchanged(
            root,
            &["foo/bar.txt"],
            &["foo/bar.txt"],
            "index file/requested-directory alias",
        );
    }

    {
        let repo = init_repo();
        let root = repo.path();
        std::fs::create_dir(root.join("Foo")).expect("create canonical directory");
        std::fs::write(root.join("Foo/base.txt"), "base\n").expect("write indexed sibling");
        assert_eq!(run(root, &["git", "add", "Foo/base.txt"]).0, 0);
        assert_eq!(run(root, &["git", "commit", "-m", "directory base"]).0, 0);
        assert_eq!(
            run(root, &["git", "config", "core.ignoreCase", "true"]).0,
            0
        );
        std::fs::create_dir_all(root.join("foo")).expect("create alias directory");
        std::fs::write(root.join("foo/new.txt"), "new\n").expect("write new sibling");
        assert_exact_refused_unchanged(
            root,
            &["foo/new.txt"],
            &["foo/new.txt"],
            "index directory-prefix alias",
        );
        assert_exact_refused_unchanged(root, &["foo"], &[], "requested file/index-directory alias");
    }
}

#[test]
fn exact_staging_allows_case_distinct_paths_when_ignore_case_is_false() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join("lower.txt"), "lower\n").expect("write lower path");
    std::fs::write(root.join("LOWER.txt"), "upper\n").expect("write upper path");
    if std::fs::read(root.join("lower.txt")).unwrap()
        == std::fs::read(root.join("LOWER.txt")).unwrap()
    {
        return;
    }
    assert_eq!(
        run(root, &["git", "config", "core.ignoreCase", "false"]).0,
        0
    );
    let staged = exact_stage(
        root,
        &["lower.txt", "LOWER.txt"],
        &["lower.txt", "LOWER.txt"],
    );
    assert_eq!(staged.exit_code, 0, "{}", staged.stderr);
    assert_eq!(run(root, &["git", "ls-files"]).1, "LOWER.txt\nlower.txt\n");
}

#[test]
fn exact_staging_handles_non_ascii_filesystem_case_aliases_conservatively() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join("Ä.txt"), "upper base\n").expect("write indexed Unicode path");
    std::fs::write(root.join("Δ.txt"), "delete me\n").expect("write Unicode deletion path");
    assert_eq!(run(root, &["git", "add", "Ä.txt", "Δ.txt"]).0, 0);
    assert_eq!(run(root, &["git", "commit", "-m", "Unicode base"]).0, 0);
    assert_eq!(
        run(root, &["git", "config", "core.ignoreCase", "true"]).0,
        0
    );

    std::fs::write(root.join("Ä.txt"), "upper changed\n").expect("modify exact Unicode path");
    std::fs::remove_file(root.join("Δ.txt")).expect("remove exact Unicode path");
    let exact = exact_stage(root, &["Ä.txt", "Δ.txt"], &["Ä.txt"]);
    assert_eq!(exact.exit_code, 0, "{}", exact.stderr);
    assert_eq!(
        run(root, &["git", "diff", "--cached", "--name-only", "-z"]).1,
        "Ä.txt\0Δ.txt\0"
    );
    assert_eq!(run(root, &["git", "commit", "-m", "exact Unicode"]).0, 0);

    std::fs::write(root.join("ä.txt"), "lower alias\n").expect("write Unicode case alias");
    let has_distinct_lower_entry = std::fs::read_dir(root)
        .expect("read repository entries")
        .any(|entry| entry.expect("read repository entry").file_name() == "ä.txt");
    if has_distinct_lower_entry {
        let staged = exact_stage(root, &["ä.txt"], &["ä.txt"]);
        assert_eq!(staged.exit_code, 0, "{}", staged.stderr);
        assert_eq!(run(root, &["git", "ls-files", "-z"]).1, "Ä.txt\0ä.txt\0");
    } else {
        assert_exact_refused_unchanged(
            root,
            &["ä.txt"],
            &["ä.txt"],
            "filesystem spelling aliases or unresolved paths",
        );
        assert_eq!(run(root, &["git", "ls-files", "-z"]).1, "Ä.txt\0");

        std::fs::rename(root.join("Ä.txt"), root.join("rename.tmp"))
            .expect("begin physical case rename");
        std::fs::rename(root.join("rename.tmp"), root.join("ä.txt"))
            .expect("finish physical case rename");
        let renamed = exact_stage(root, &["ä.txt"], &["ä.txt"]);
        assert_eq!(renamed.exit_code, 0, "{}", renamed.stderr);
        assert_eq!(run(root, &["git", "ls-files", "-z"]).1, "Ä.txt\0ä.txt\0");
    }
}

#[test]
fn exact_staging_refuses_alias_only_filesystem_lookups_without_an_index_entry() {
    let repo = init_repo();
    let root = repo.path();
    assert_eq!(
        run(root, &["git", "config", "core.ignoreCase", "true"]).0,
        0
    );
    std::fs::write(root.join("Foo.txt"), "stored spelling\n").expect("write stored spelling");
    assert_exact_refused_unchanged(
        root,
        &["foo.txt"],
        &["foo.txt"],
        "filesystem spelling aliases or unresolved paths",
    );
}

#[test]
fn exact_staging_checks_unicode_aliases_in_both_directions() {
    for (indexed, requested) in [
        ("KELVIN.txt", "kelvin.txt"),
        ("kelvin.txt", "KELVIN.txt"),
        ("ſAFE.txt", "safe.txt"),
        ("safe.txt", "ſAFE.txt"),
        ("é.txt", "e\u{301}.txt"),
        ("e\u{301}.txt", "é.txt"),
    ] {
        let repo = init_repo();
        let root = repo.path();
        std::fs::write(root.join(indexed), "indexed\n").expect("write indexed spelling");
        assert_eq!(run(root, &["git", "add", indexed]).0, 0);
        assert_eq!(run(root, &["git", "commit", "-m", "spelling base"]).0, 0);
        assert_eq!(
            run(root, &["git", "config", "core.ignoreCase", "true"]).0,
            0
        );
        std::fs::write(root.join(requested), "requested\n").expect("write requested spelling");
        let requested_is_physically_exact = std::fs::read_dir(root)
            .expect("read repository entries")
            .any(|entry| entry.expect("read repository entry").file_name() == requested);
        let requested_is_exact_index = run(root, &["git", "ls-files", "-z"])
            .1
            .split('\0')
            .any(|path| path == requested);
        if requested_is_physically_exact || requested_is_exact_index {
            let staged = exact_stage(root, &[requested], &[requested]);
            assert_eq!(
                staged.exit_code, 0,
                "{indexed:?} -> {requested:?}: {staged:?}"
            );
            assert!(!index_entry(root, requested).is_empty());
        } else {
            let before = git_index_bytes(root);
            let refused = exact_stage(root, &[requested], &[requested]);
            assert_ne!(
                refused.exit_code, 0,
                "{indexed:?} -> {requested:?} unexpectedly succeeded: {refused:?}"
            );
            assert!(
                refused
                    .stderr
                    .contains("filesystem spelling aliases or unresolved paths"),
                "{indexed:?} -> {requested:?}: {refused:?}"
            );
            assert_eq!(git_index_bytes(root), before);
        }
    }
}

#[test]
fn exact_staging_allows_a_new_child_under_an_exact_unicode_directory() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::create_dir(root.join("Ärea")).expect("create Unicode directory");
    std::fs::write(root.join("Ärea/base.txt"), "base\n").expect("write indexed child");
    assert_eq!(run(root, &["git", "add", "Ärea/base.txt"]).0, 0);
    assert_eq!(run(root, &["git", "commit", "-m", "directory base"]).0, 0);
    assert_eq!(
        run(root, &["git", "config", "core.ignoreCase", "true"]).0,
        0
    );
    std::fs::write(root.join("Ärea/new.txt"), "new\n").expect("write new child");

    let staged = exact_stage(root, &["Ärea/new.txt"], &["Ärea/new.txt"]);
    assert_eq!(staged.exit_code, 0, "{}", staged.stderr);
    assert!(!index_entry(root, "Ärea/new.txt").is_empty());
}

#[cfg(unix)]
#[test]
fn exact_index_scan_allows_an_unrelated_non_utf8_path() {
    use std::io::Write as _;

    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join("blob-source"), "unrelated\n").expect("write blob source");
    let oid = run(root, &["git", "hash-object", "-w", "blob-source"]).1;
    let mut command = std::process::Command::new("git");
    crate::safe_git::isolate_git_command_environment(&mut command);
    let mut child = command
        .args(["update-index", "-z", "--index-info"])
        .current_dir(root)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .expect("spawn raw index update");
    let mut record = format!("100644 {}\t", oid.trim()).into_bytes();
    record.extend_from_slice(b"unrelated-\xff\0");
    child
        .stdin
        .take()
        .expect("raw index stdin")
        .write_all(&record)
        .expect("write raw index entry");
    let status = child.wait().expect("wait for raw index update");
    assert!(status.success(), "raw index update failed: {status}");
    assert_eq!(run(root, &["git", "commit", "-m", "non-UTF-8 base"]).0, 0);
    assert_eq!(
        run(root, &["git", "config", "core.ignoreCase", "true"]).0,
        0
    );
    std::fs::write(root.join("ordinary.txt"), "ordinary\n").expect("write ordinary path");

    let staged = exact_stage(root, &["ordinary.txt"], &["ordinary.txt"]);
    assert_eq!(staged.exit_code, 0, "{}", staged.stderr);
    assert!(!index_entry(root, "ordinary.txt").is_empty());
}

#[test]
fn exact_staging_preserves_sparse_checkout_policy() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::create_dir_all(root.join("inside")).expect("create included directory");
    std::fs::create_dir_all(root.join("outside")).expect("create excluded directory");
    std::fs::write(root.join("inside/tracked.txt"), "inside\n").expect("write included file");
    std::fs::write(root.join("outside/tracked.txt"), "old\n").expect("write excluded file");
    assert_eq!(run(root, &["git", "add", "."]).0, 0);
    assert_eq!(run(root, &["git", "commit", "-m", "sparse base"]).0, 0);
    assert_eq!(
        run(root, &["git", "sparse-checkout", "init", "--cone"]).0,
        0
    );
    assert_eq!(run(root, &["git", "sparse-checkout", "set", "inside"]).0, 0);

    std::fs::create_dir_all(root.join("outside")).expect("recreate excluded directory");
    std::fs::write(root.join("outside/tracked.txt"), "new\n")
        .expect("modify excluded tracked file");
    std::fs::write(root.join("outside/untracked.txt"), "new\n")
        .expect("write excluded untracked file");
    let outside_before = index_entry(root, "outside/tracked.txt");
    let result = exact_stage(
        root,
        &["outside/tracked.txt", "outside/untracked.txt"],
        &["outside/tracked.txt", "outside/untracked.txt"],
    );
    assert_ne!(result.exit_code, 0);
    assert!(
        result.stderr.contains("sparse-checkout"),
        "{}",
        result.stderr
    );
    assert_eq!(index_entry(root, "outside/tracked.txt"), outside_before);
    assert!(index_entry(root, "outside/untracked.txt").is_empty());
    assert!(
        run(root, &["git", "diff", "--cached", "--name-only"])
            .1
            .is_empty()
    );

    std::fs::write(root.join("inside/untracked.txt"), "new\n")
        .expect("write included untracked file");
    let included = exact_stage(root, &["inside/untracked.txt"], &["inside/untracked.txt"]);
    assert_eq!(included.exit_code, 0, "{}", included.stderr);
    assert!(!index_entry(root, "inside/untracked.txt").is_empty());
}

#[cfg(unix)]
#[test]
fn sparse_rule_probe_failure_is_closed_only_for_sparse_repositories() {
    use std::os::unix::fs::PermissionsExt;

    const TEST_NAME: &str =
        "patch_paths::tests::sparse_rule_probe_failure_is_closed_only_for_sparse_repositories";
    if let Some(mode) = std::env::var_os("CODEX_GIT_UTILS_FAKE_SPARSE_MODE") {
        let root = PathBuf::from(
            std::env::var_os("CODEX_GIT_UTILS_TARGET_REPO").expect("target repository"),
        );
        let result = exact_stage(&root, &["file.txt"], &["file.txt"]);
        if mode == "sparse" {
            assert_ne!(result.exit_code, 0);
            assert!(result.stderr.contains("fake unsupported check-rules"));
            assert!(index_entry(&root, "file.txt").is_empty());
        } else if mode == "legacy" {
            assert_ne!(result.exit_code, 0);
            assert!(result.stderr.contains("built-in Git command"));
            assert!(index_entry(&root, "file.txt").is_empty());
            assert!(
                !PathBuf::from(
                    std::env::var_os("CODEX_GIT_UTILS_SPARSE_HELPER_MARKER")
                        .expect("helper marker"),
                )
                .exists(),
                "legacy Git must not dispatch a PATH-resolved sparse helper"
            );
        } else {
            assert_eq!(result.exit_code, 0, "{}", result.stderr);
            assert!(!index_entry(&root, "file.txt").is_empty());
        }
        return;
    }

    let wrapper_dir = tempfile::tempdir().expect("wrapper directory");
    let wrapper = wrapper_dir.path().join("git");
    std::fs::write(
        &wrapper,
        "#!/bin/sh\nprevious=\nfor argument in \"$@\"; do\n  if [ \"$CODEX_GIT_UTILS_FAKE_SPARSE_MODE\" = legacy ] && [ \"$argument\" = --list-cmds=builtins ]; then\n    echo add\n    echo config\n    exit 0\n  fi\n  if [ \"$previous\" = sparse-checkout ] && [ \"$argument\" = check-rules ]; then\n    echo fake unsupported check-rules >&2\n    exit 129\n  fi\n  previous=$argument\ndone\nexec \"$CODEX_GIT_UTILS_REAL_GIT\" \"$@\"\n",
    )
    .expect("write Git wrapper");
    let mut permissions = std::fs::metadata(&wrapper).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&wrapper, permissions).expect("make wrapper executable");
    let sparse_helper = wrapper_dir.path().join("git-sparse-checkout");
    std::fs::write(
        &sparse_helper,
        "#!/bin/sh\n: > \"$CODEX_GIT_UTILS_SPARSE_HELPER_MARKER\"\nexit 0\n",
    )
    .expect("write malicious sparse helper");
    let mut permissions = std::fs::metadata(&sparse_helper).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&sparse_helper, permissions).expect("make sparse helper executable");
    let real_git = [
        "/usr/bin/git",
        "/opt/homebrew/bin/git",
        "/usr/local/bin/git",
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|path| path.is_file())
    .expect("native Git outside the test wrapper");
    let wrapper_path = std::env::join_paths([wrapper_dir.path()]).expect("wrapper PATH");
    let helper_marker = wrapper_dir.path().join("sparse-helper-ran");

    for mode in ["ordinary", "sparse", "legacy"] {
        let repo = init_repo();
        let root = repo.path();
        if matches!(mode, "sparse" | "legacy") {
            assert_eq!(
                run(root, &["git", "config", "core.sparseCheckout", "true"]).0,
                0
            );
        }
        std::fs::write(root.join("file.txt"), "new\n").expect("write staging file");
        run_isolated_test(
            TEST_NAME,
            &[
                ("CODEX_GIT_UTILS_FAKE_SPARSE_MODE", OsStr::new(mode)),
                ("CODEX_GIT_UTILS_TARGET_REPO", root.as_os_str()),
                ("CODEX_GIT_UTILS_REAL_GIT", real_git.as_os_str()),
                (
                    "CODEX_GIT_UTILS_SPARSE_HELPER_MARKER",
                    helper_marker.as_os_str(),
                ),
                ("PATH", wrapper_path.as_os_str()),
            ],
        );
    }
}

#[cfg(unix)]
#[test]
fn exact_staging_treats_pathspec_magic_and_newline_as_literal_bytes() {
    let repo = init_repo();
    let root = repo.path();
    commit_seed(root);
    assert_eq!(
        run(root, &["git", "sparse-checkout", "init", "--cone"]).0,
        0
    );
    assert_eq!(run(root, &["git", "sparse-checkout", "set", "."]).0, 0);
    assert_eq!(
        run(root, &["git", "config", "core.ignoreCase", "true"]).0,
        0
    );
    let path = ":(glob)*[literal]\nname.txt";
    std::fs::write(root.join(path), "literal\n").expect("write unusual path");

    let result = exact_stage(root, &[path], &[path]);
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    assert!(String::from_utf8_lossy(&git_index_bytes(root)).contains(path));
}

#[test]
fn exact_staging_does_not_recurse_when_a_leaf_becomes_a_directory() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join("victim"), "old\n").expect("write tracked leaf");
    assert_eq!(run(root, &["git", "add", "victim"]).0, 0);
    assert_eq!(run(root, &["git", "commit", "-m", "tracked leaf"]).0, 0);
    std::fs::remove_file(root.join("victim")).expect("remove tracked leaf");
    std::fs::create_dir(root.join("victim")).expect("replace leaf with directory");
    std::fs::write(root.join("victim/child.txt"), "child\n").expect("write descendant");

    // Passing `victim` as filterable content models a classification made
    // immediately before the file-to-directory swap.
    let result = exact_stage(root, &["victim"], &["victim"]);
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    assert!(index_entry(root, "victim/child.txt").is_empty());
    assert!(index_entry(root, "victim").is_empty());
}

#[cfg(unix)]
#[test]
fn raced_directory_descendant_filter_is_not_run() {
    use std::os::unix::fs::PermissionsExt;

    let repo = init_repo();
    let root = repo.path();
    std::fs::write(
        root.join(".gitattributes"),
        "victim/child.txt filter=descendant\n",
    )
    .expect("write descendant attribute");
    std::fs::write(root.join("victim"), "old\n").expect("write tracked leaf");
    assert_eq!(run(root, &["git", "add", ".gitattributes", "victim"]).0, 0);
    assert_eq!(run(root, &["git", "commit", "-m", "tracked leaf"]).0, 0);

    let marker = root.join("descendant-filter-ran");
    let helper = root.join("descendant-filter.sh");
    std::fs::write(
        &helper,
        format!("#!/bin/sh\n: > '{}'\ncat\n", marker.display()),
    )
    .expect("write filter helper");
    let mut permissions = std::fs::metadata(&helper).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&helper, permissions).expect("make filter executable");
    assert_eq!(
        run(
            root,
            &[
                "git",
                "config",
                "filter.descendant.clean",
                &helper.to_string_lossy(),
            ],
        )
        .0,
        0
    );
    assert_eq!(
        run(
            root,
            &["git", "config", "filter.descendant.required", "true"],
        )
        .0,
        0
    );
    std::fs::remove_file(root.join("victim")).expect("remove tracked leaf");
    std::fs::create_dir(root.join("victim")).expect("replace leaf with directory");
    std::fs::write(root.join("victim/child.txt"), "child\n").expect("write descendant");

    let hash = run(
        root,
        &[
            "git",
            "hash-object",
            "--path",
            "victim/child.txt",
            "--",
            "victim/child.txt",
        ],
    );
    assert_eq!(hash.0, 0, "filter control failed: {}", hash.2);
    assert!(
        marker.exists(),
        "control must prove the helper is executable"
    );
    std::fs::remove_file(&marker).expect("reset filter marker");

    let result = exact_stage(root, &["victim"], &["victim"]);
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    assert!(
        !marker.exists(),
        "exact staging must not run the descendant filter"
    );
    assert!(index_entry(root, "victim/child.txt").is_empty());
}

#[cfg(unix)]
#[test]
fn exact_staging_records_a_symlink_as_a_mode_120000_blob() {
    use std::os::unix::fs::symlink;

    let repo = init_repo();
    let root = repo.path();
    symlink("literal-target", root.join("link")).expect("create symlink");

    let result = exact_stage(root, &["link"], &[]);
    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    assert!(index_entry(root, "link").starts_with("120000 "));
    let (code, blob, stderr) = run(root, &["git", "show", ":link"]);
    assert_eq!(code, 0, "read staged symlink: {stderr}");
    assert_eq!(blob, "literal-target");
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
        let error = stage_effective_paths(
            &git,
            &root,
            &["file.txt".to_string()],
            &safe_git_config_parts(),
        )
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

#[test]
fn staging_rejects_selected_process_but_allows_smudge_only_filter() {
    let process_repo = init_repo_with_selected_filter("filter.selected.process");
    let process_root = process_repo.path();
    let process_git = GitRunner::for_cwd_io(process_root).expect("trusted Git");
    let error = stage_effective_paths(
        &process_git,
        process_root,
        &["file.txt".to_string()],
        &safe_git_config_parts(),
    )
    .expect_err("reject selected process filter");
    assert_eq!(error.kind(), io::ErrorKind::Unsupported);

    let smudge_repo = init_repo_with_selected_filter("filter.selected.smudge");
    let smudge_root = smudge_repo.path();
    let smudge_git = GitRunner::for_cwd_io(smudge_root).expect("trusted Git");
    stage_effective_paths(
        &smudge_git,
        smudge_root,
        &["file.txt".to_string()],
        &safe_git_config_parts(),
    )
    .expect("smudge-only filters do not run during staging");
    let (code, staged, stderr) = run(smudge_root, &["git", "diff", "--cached", "--name-only"]);
    assert_eq!(code, 0, "read staged paths: {stderr}");
    assert_eq!(staged, "file.txt\n");
}

#[cfg(unix)]
#[test]
fn staging_neutralizes_off_path_racy_filters_without_changing_their_index_entry() {
    for (driver, command) in [
        ("selected", "clean"),
        ("selected", "process"),
        ("", "clean"),
        ("x=y", "clean"),
    ] {
        let control = init_racy_filter_fixture(driver, command);
        let control_root = control.repo.path();
        std::fs::write(control_root.join("target.txt"), "new target\n")
            .expect("modify control target");
        let _ = run(
            control_root,
            &[
                "git",
                "--literal-pathspecs",
                "update-index",
                "--add",
                "--remove",
                "--",
                "target.txt",
            ],
        );
        assert!(
            control.marker.exists(),
            "raw update-index should execute the off-path racy {driver:?} {command} filter"
        );

        let guarded = init_racy_filter_fixture(driver, command);
        let guarded_root = guarded.repo.path();
        let outside_before = index_entry(guarded_root, "outside.txt");
        std::fs::write(guarded_root.join("target.txt"), "new target\n")
            .expect("modify guarded target");
        let git = GitRunner::for_cwd_io(guarded_root).expect("trusted Git");
        stage_effective_paths(
            &git,
            guarded_root,
            &["target.txt".to_string()],
            &safe_git_config_parts(),
        )
        .expect("stage with off-path filters neutralized");

        assert!(
            !guarded.marker.exists(),
            "guarded staging must not execute the off-path racy {driver:?} {command} filter"
        );
        assert_eq!(
            index_entry(guarded_root, "outside.txt"),
            outside_before,
            "staging must preserve the unrelated index entry for {driver:?} {command}"
        );
        let (code, staged, stderr) = run(guarded_root, &["git", "diff", "--cached", "--name-only"]);
        assert_eq!(code, 0, "read staged paths: {stderr}");
        assert_eq!(
            staged, "target.txt\n",
            "driver={driver:?} command={command}"
        );
    }
}

#[cfg(unix)]
#[test]
fn staging_allows_selected_symlink_while_neutralizing_off_path_filters() {
    use std::os::unix::fs::symlink;

    for (command, core_symlinks) in [("clean", true), ("process", false)] {
        let fixture = init_racy_filter_fixture("selected", command);
        let root = fixture.repo.path();
        assert_eq!(
            run(
                root,
                &[
                    "git",
                    "config",
                    "core.symlinks",
                    if core_symlinks { "true" } else { "false" },
                ],
            )
            .0,
            0
        );
        std::fs::remove_file(root.join("link")).expect("remove old symlink");
        symlink("new-target", root.join("link")).expect("create updated symlink");
        let outside_before = index_entry(root, "outside.txt");

        let git = GitRunner::for_cwd_io(root).expect("trusted Git");
        stage_effective_paths(&git, root, &["link".to_string()], &safe_git_config_parts())
            .expect("stage selected symlink");

        assert!(
            !fixture.marker.exists(),
            "staging a symlink must not run {command} filters"
        );
        assert_eq!(index_entry(root, "outside.txt"), outside_before);
        let link_entry = index_entry(root, "link");
        assert!(
            link_entry.starts_with("120000 "),
            "symlink index mode with core.symlinks={core_symlinks}: {link_entry:?}"
        );
        let (code, blob, stderr) = run(root, &["git", "show", ":link"]);
        assert_eq!(code, 0, "read staged link: {stderr}");
        assert_eq!(blob, "new-target");
        let (code, staged, stderr) = run(root, &["git", "diff", "--cached", "--name-only"]);
        assert_eq!(code, 0, "read staged paths: {stderr}");
        assert_eq!(staged, "link\n");
    }
}

#[cfg(unix)]
#[test]
fn staging_allows_optional_smudge_only_but_refuses_required_or_malformed_smudge() {
    use std::io::Write as _;

    for (required, accepted) in [
        ("absent", true),
        ("empty", true),
        ("false", true),
        ("yes", false),
        ("implicit", false),
        ("not-a-bool", false),
    ] {
        let fixture = init_racy_filter_fixture("selected", "smudge");
        let root = fixture.repo.path();
        if matches!(required, "absent" | "implicit") {
            assert_eq!(
                run(
                    root,
                    &["git", "config", "--unset-all", "filter.selected.required"],
                )
                .0,
                0
            );
        }
        match required {
            "absent" => {}
            "implicit" => {
                let mut config = std::fs::OpenOptions::new()
                    .append(true)
                    .open(root.join(".git/config"))
                    .expect("open repository config");
                writeln!(config, "[filter \"selected\"]\n\trequired")
                    .expect("write implicit true required value");
            }
            value => {
                let value = if value == "empty" { "" } else { value };
                assert_eq!(
                    run(root, &["git", "config", "filter.selected.required", value],).0,
                    0
                );
            }
        }
        std::fs::write(root.join("selected.txt"), "new selected\n")
            .expect("modify selected target");
        let before = git_index_bytes(root);
        let git = GitRunner::for_cwd_io(root).expect("trusted Git");
        let result = stage_effective_paths(
            &git,
            root,
            &["selected.txt".to_string()],
            &safe_git_config_parts(),
        );

        assert!(
            !fixture.marker.exists(),
            "smudge helper must not run for required={required}"
        );
        if accepted {
            result.expect("stage optional smudge-only target");
            let (code, staged, stderr) = run(root, &["git", "diff", "--cached", "--name-only"]);
            assert_eq!(code, 0, "read staged paths: {stderr}");
            assert_eq!(staged, "selected.txt\n");
        } else {
            let error = result.expect_err("refuse required or malformed smudge-only target");
            assert_eq!(error.kind(), io::ErrorKind::Unsupported);
            assert_eq!(git_index_bytes(root), before);
        }
    }
}

#[test]
fn optional_smudge_target_does_not_mask_a_later_clean_target() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(
        root.join(".gitattributes"),
        "a.txt filter=smudger\nz.txt filter=cleaner\n",
    )
    .expect("write attributes");
    std::fs::write(root.join("a.txt"), "old a\n").expect("write smudge target");
    std::fs::write(root.join("z.txt"), "old z\n").expect("write clean target");
    assert_eq!(run(root, &["git", "add", "."]).0, 0);
    assert_eq!(run(root, &["git", "commit", "-m", "base"]).0, 0);
    assert_eq!(
        run(
            root,
            &[
                "git",
                "config",
                "filter.smudger.smudge",
                "codex-definitely-missing-smudge-command",
            ],
        )
        .0,
        0
    );
    assert_eq!(
        run(
            root,
            &[
                "git",
                "config",
                "filter.cleaner.clean",
                "codex-definitely-missing-clean-command",
            ],
        )
        .0,
        0
    );
    std::fs::write(root.join("a.txt"), "new a\n").expect("modify smudge target");
    std::fs::write(root.join("z.txt"), "new z\n").expect("modify clean target");
    let before = git_index_bytes(root);

    let git = GitRunner::for_cwd_io(root).expect("trusted Git");
    let error = stage_effective_paths(
        &git,
        root,
        &["a.txt".to_string(), "z.txt".to_string()],
        &safe_git_config_parts(),
    )
    .expect_err("reject the selected clean target after optional smudge");
    assert_eq!(error.kind(), io::ErrorKind::Unsupported);
    assert_eq!(git_index_bytes(root), before);
}

#[test]
fn staging_filter_probe_uses_only_existing_non_directory_leaves() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(
        root.join(".gitattributes"),
        "missing.txt filter=selected\ndeleted.txt filter=selected\n",
    )
    .expect("write attributes");
    std::fs::write(root.join("deleted.txt"), "old\n").expect("write deleted fixture");
    std::fs::write(root.join("kept.txt"), "old\n").expect("write kept fixture");
    assert_eq!(run(root, &["git", "add", "."]).0, 0);
    assert_eq!(run(root, &["git", "commit", "-m", "base"]).0, 0);
    assert_eq!(
        run(
            root,
            &[
                "git",
                "config",
                "filter.selected.clean",
                "codex-definitely-missing-filter-command",
            ],
        )
        .0,
        0
    );
    std::fs::remove_file(root.join("deleted.txt")).expect("delete tracked file");
    std::fs::write(root.join("kept.txt"), "new\n").expect("modify kept fixture");

    let git = GitRunner::for_cwd_io(root).expect("trusted Git");
    stage_effective_paths(
        &git,
        root,
        &[
            "missing.txt".to_string(),
            "deleted.txt".to_string(),
            "kept.txt".to_string(),
        ],
        &safe_git_config_parts(),
    )
    .expect("probe only the surviving staging leaf");

    let (code, staged, stderr) = run(root, &["git", "diff", "--cached", "--name-only"]);
    assert_eq!(code, 0, "read staged paths: {stderr}");
    assert_eq!(staged, "kept.txt\n");
    assert!(!root.join("missing.txt").exists());
    assert!(!root.join("deleted.txt").exists());
}

#[test]
fn staging_skips_filter_probe_when_no_leaf_will_be_staged() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(
        root.join(".gitattributes"),
        "missing.txt filter=selected\ndeleted.txt filter=selected\n",
    )
    .expect("write attributes");
    std::fs::write(root.join("deleted.txt"), "old\n").expect("write deleted fixture");
    assert_eq!(run(root, &["git", "add", "."]).0, 0);
    assert_eq!(run(root, &["git", "commit", "-m", "base"]).0, 0);
    assert_eq!(
        run(
            root,
            &[
                "git",
                "config",
                "filter.selected.clean",
                "codex-definitely-missing-filter-command",
            ],
        )
        .0,
        0
    );
    std::fs::remove_file(root.join("deleted.txt")).expect("delete tracked file");
    let before = git_index_bytes(root);

    let git = GitRunner::for_cwd_io(root).expect("trusted Git");
    stage_effective_paths(
        &git,
        root,
        &["missing.txt".to_string(), "deleted.txt".to_string()],
        &safe_git_config_parts(),
    )
    .expect("skip staging when every effective leaf is absent");

    assert_eq!(git_index_bytes(root), before);
}
