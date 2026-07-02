use super::*;
use crate::apply::ApplyGitRequest;
use crate::apply::apply_git_patch;
use std::io;
use std::path::Path;
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
