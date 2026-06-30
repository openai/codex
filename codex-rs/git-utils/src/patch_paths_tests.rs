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
    command
        .arg(test_name)
        .arg("--exact")
        .arg("--nocapture")
        .env("CODEX_GIT_UTILS_PATH_ENV_CHILD", "1")
        .env("RUST_TEST_THREADS", "1");
    for (name, value) in env {
        command.env(name, value);
    }
    let output = command.output().expect("run isolated test process");
    assert!(
        output.status.success(),
        "isolated test {test_name} failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
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

fn configured_filter_ran(root: &Path) -> bool {
    let (code, _, _) = run(root, &["git", "config", "--get", "codex.filterran"]);
    code == 0
}

fn init_submodule_with_clean_filter(parent: &Path) {
    let source = tempfile::tempdir().expect("submodule source");
    let source_root = source.path();
    let _ = run(source_root, &["git", "init"]);
    let _ = run(
        source_root,
        &["git", "config", "user.email", "codex@example.com"],
    );
    let _ = run(source_root, &["git", "config", "user.name", "Codex"]);
    std::fs::write(source_root.join("file.txt"), "original\n").expect("write submodule file");
    std::fs::write(
        source_root.join(".gitattributes"),
        "file.txt filter=codex-test\n",
    )
    .expect("write submodule attributes");
    let _ = run(source_root, &["git", "add", "."]);
    let _ = run(source_root, &["git", "commit", "-m", "seed"]);

    let source_path = source_root.to_string_lossy().into_owned();
    let (add_code, _, add_err) = run(
        parent,
        &[
            "git",
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            &source_path,
            "nested",
        ],
    );
    assert_eq!(add_code, 0, "add submodule: {add_err}");
    let _ = run(parent, &["git", "commit", "-m", "add submodule"]);

    let nested = parent.join("nested");
    let (config_code, _, config_err) = run(
        &nested,
        &[
            "git",
            "config",
            "filter.codex-test.clean",
            "git config codex.filterran true && git hash-object --stdin",
        ],
    );
    assert_eq!(config_code, 0, "configure submodule filter: {config_err}");
    std::fs::write(nested.join("file.txt"), "modified\n").expect("dirty submodule file");
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

#[test]
fn path_prefix_sets_include_leaf_only_for_index_candidates() {
    let mut index_candidates = std::collections::BTreeSet::new();
    insert_path_prefixes("a/b/c", &mut index_candidates, /*include_leaf*/ true);
    assert_eq!(
        index_candidates,
        ["a", "a/b", "a/b/c"]
            .into_iter()
            .map(str::to_string)
            .collect()
    );

    let mut traversed_ancestors = std::collections::BTreeSet::new();
    insert_path_prefixes(
        "a/b/c",
        &mut traversed_ancestors,
        /*include_leaf*/ false,
    );
    assert_eq!(
        traversed_ancestors,
        ["a", "a/b"].into_iter().map(str::to_string).collect()
    );
}

#[test]
fn stage_paths_rejects_gitlink_before_entering_submodule() {
    let _g = env_lock().lock().unwrap();
    let repo = init_repo();
    let root = repo.path();
    init_submodule_with_clean_filter(root);

    let diff = "diff --git a/nested b/nested\nindex 1111111..2222222 160000\n--- a/nested\n+++ b/nested\n@@ -1 +1 @@\n-Subproject commit 1111111111111111111111111111111111111111\n+Subproject commit 2222222222222222222222222222222222222222\n";
    let error = stage_paths(root, diff).expect_err("reject gitlink staging");
    assert_eq!(error.kind(), io::ErrorKind::Unsupported);
    assert!(!configured_filter_ran(&root.join("nested")));
}

#[test]
fn patch_allows_exact_gitlink_updates_without_entering_submodule() {
    let _g = env_lock().lock().unwrap();
    let repo = init_repo();
    let root = repo.path();
    init_submodule_with_clean_filter(root);
    let nested = root.join("nested");

    let (old_code, old_sha, old_err) = run(root, &["git", "rev-parse", "HEAD:nested"]);
    assert_eq!(old_code, 0, "read old gitlink: {old_err}");
    let old_sha = old_sha.trim().to_string();
    let _ = run(
        &nested,
        &["git", "config", "user.email", "codex@example.com"],
    );
    let _ = run(&nested, &["git", "config", "user.name", "Codex"]);
    std::fs::write(nested.join("second.txt"), "second\n").expect("write child commit");
    let (add_code, _, add_err) = run(&nested, &["git", "add", "second.txt"]);
    assert_eq!(add_code, 0, "add child commit: {add_err}");
    let (commit_code, _, commit_err) = run(&nested, &["git", "commit", "-m", "second"]);
    assert_eq!(commit_code, 0, "commit child: {commit_err}");
    let (new_code, new_sha, new_err) = run(&nested, &["git", "rev-parse", "HEAD"]);
    assert_eq!(new_code, 0, "read new child commit: {new_err}");
    let new_sha = new_sha.trim().to_string();

    let (update_code, _, update_err) = run(
        root,
        &[
            "git",
            "update-index",
            "--cacheinfo",
            "160000",
            &new_sha,
            "nested",
        ],
    );
    assert_eq!(update_code, 0, "update parent gitlink: {update_err}");
    let (diff_code, diff, diff_err) = run(
        root,
        &[
            "git",
            "diff",
            "--cached",
            "--full-index",
            "--binary",
            "--",
            "nested",
        ],
    );
    assert_eq!(diff_code, 0, "create gitlink patch: {diff_err}");
    assert!(!diff.is_empty(), "gitlink patch must not be empty");
    let (restore_code, _, restore_err) = run(
        root,
        &[
            "git",
            "update-index",
            "--cacheinfo",
            "160000",
            &old_sha,
            "nested",
        ],
    );
    assert_eq!(restore_code, 0, "restore parent gitlink: {restore_err}");
    let (reset_code, _, reset_err) = run(&nested, &["git", "reset", "--soft", &old_sha]);
    assert_eq!(reset_code, 0, "restore child HEAD: {reset_err}");
    let _ = run(&nested, &["git", "config", "--unset", "codex.filterran"]);
    assert!(!configured_filter_ran(&nested));

    for preflight in [true, false] {
        let result = apply_git_patch(&ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: diff.clone(),
            revert: false,
            preflight,
        })
        .unwrap_or_else(|error| panic!("forward gitlink preflight={preflight}: {error}"));
        assert_eq!(result.exit_code, 0, "forward preflight={preflight}");
        assert!(!configured_filter_ran(&nested));
    }
    let (staged_code, staged, staged_err) = run(root, &["git", "ls-files", "--stage", "nested"]);
    assert_eq!(staged_code, 0, "read updated gitlink: {staged_err}");
    assert!(
        staged.contains(&new_sha),
        "expected new gitlink, got {staged:?}"
    );

    let (reset_code, _, reset_err) = run(&nested, &["git", "reset", "--soft", &new_sha]);
    assert_eq!(reset_code, 0, "advance child HEAD: {reset_err}");
    for preflight in [true, false] {
        let result = apply_git_patch(&ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: diff.clone(),
            revert: true,
            preflight,
        })
        .unwrap_or_else(|error| panic!("reverse gitlink preflight={preflight}: {error}"));
        assert_eq!(result.exit_code, 0, "reverse preflight={preflight}");
        assert!(!configured_filter_ran(&nested));
    }
    let (staged_code, staged, staged_err) = run(root, &["git", "ls-files", "--stage", "nested"]);
    assert_eq!(staged_code, 0, "read restored gitlink: {staged_err}");
    assert!(
        staged.contains(&old_sha),
        "expected old gitlink, got {staged:?}"
    );

    std::fs::remove_dir_all(&nested).expect("remove initialized child checkout");
    for (revert, expected_sha) in [(false, &new_sha), (true, &old_sha)] {
        let preflight = apply_git_patch(&ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: diff.clone(),
            revert,
            preflight: true,
        })
        .unwrap_or_else(|error| panic!("uninitialized preflight revert={revert}: {error}"));
        assert_eq!(
            preflight.exit_code, 0,
            "uninitialized preflight revert={revert}"
        );
        let applied = apply_git_patch(&ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: diff.clone(),
            revert,
            preflight: false,
        })
        .unwrap_or_else(|error| panic!("uninitialized apply revert={revert}: {error}"));
        assert_eq!(applied.exit_code, 0, "uninitialized apply revert={revert}");
        let (stage_code, stage, stage_err) = run(root, &["git", "ls-files", "--stage", "nested"]);
        assert_eq!(stage_code, 0, "read uninitialized gitlink: {stage_err}");
        assert!(
            stage.contains(expected_sha.as_str()),
            "unexpected gitlink: {stage:?}"
        );
        assert!(
            !nested.exists(),
            "Codex must not initialize the child worktree"
        );
    }
}

#[test]
fn patch_rejects_gitlink_add_delete_rename_and_mode_shapes_without_mutation() {
    let _g = env_lock().lock().unwrap();
    let repo = init_repo();
    let root = repo.path();
    init_submodule_with_clean_filter(root);
    let nested = root.join("nested");
    std::fs::write(root.join("ordinary.txt"), "ordinary\n").expect("ordinary file");
    let (ordinary_add_code, _, ordinary_add_err) = run(root, &["git", "add", "ordinary.txt"]);
    assert_eq!(
        ordinary_add_code, 0,
        "add ordinary file: {ordinary_add_err}"
    );
    let (ordinary_commit_code, _, ordinary_commit_err) =
        run(root, &["git", "commit", "-m", "ordinary file"]);
    assert_eq!(
        ordinary_commit_code, 0,
        "commit ordinary file: {ordinary_commit_err}"
    );
    let (sha_code, gitlink_sha, sha_err) = run(root, &["git", "rev-parse", "HEAD:nested"]);
    assert_eq!(sha_code, 0, "read gitlink: {sha_err}");
    let gitlink_sha = gitlink_sha.trim().to_string();
    let (blob_code, ordinary_blob, blob_err) =
        run(root, &["git", "rev-parse", "HEAD:ordinary.txt"]);
    assert_eq!(blob_code, 0, "read ordinary blob: {blob_err}");
    let ordinary_blob = ordinary_blob.trim().to_string();
    let (before_code, before_index, before_err) = run(root, &["git", "ls-files", "--stage"]);
    assert_eq!(before_code, 0, "snapshot index: {before_err}");

    let (remove_code, _, remove_err) =
        run(root, &["git", "update-index", "--force-remove", "nested"]);
    assert_eq!(remove_code, 0, "prepare deletion: {remove_err}");
    let (delete_code, delete_patch, delete_err) = run(
        root,
        &[
            "git",
            "diff",
            "--cached",
            "--full-index",
            "--binary",
            "--",
            "nested",
        ],
    );
    assert_eq!(delete_code, 0, "create deletion patch: {delete_err}");
    let (restore_code, _, restore_err) = run(
        root,
        &[
            "git",
            "update-index",
            "--add",
            "--cacheinfo",
            "160000",
            &gitlink_sha,
            "nested",
        ],
    );
    assert_eq!(restore_code, 0, "restore after deletion: {restore_err}");

    let _ = run(root, &["git", "update-index", "--force-remove", "nested"]);
    let (rename_add_code, _, rename_add_err) = run(
        root,
        &[
            "git",
            "update-index",
            "--add",
            "--cacheinfo",
            "160000",
            &gitlink_sha,
            "renamed-module",
        ],
    );
    assert_eq!(rename_add_code, 0, "prepare rename: {rename_add_err}");
    let (rename_code, rename_patch, rename_err) = run(
        root,
        &["git", "diff", "--cached", "-M", "--full-index", "--binary"],
    );
    assert_eq!(rename_code, 0, "create rename patch: {rename_err}");
    let _ = run(
        root,
        &["git", "update-index", "--force-remove", "renamed-module"],
    );
    let _ = run(
        root,
        &[
            "git",
            "update-index",
            "--add",
            "--cacheinfo",
            "160000",
            &gitlink_sha,
            "nested",
        ],
    );

    let (add_code, _, add_err) = run(
        root,
        &[
            "git",
            "update-index",
            "--add",
            "--cacheinfo",
            "160000",
            &gitlink_sha,
            "second-module",
        ],
    );
    assert_eq!(add_code, 0, "prepare addition: {add_err}");
    let (new_code, new_patch, new_err) = run(
        root,
        &[
            "git",
            "diff",
            "--cached",
            "--full-index",
            "--binary",
            "--",
            "second-module",
        ],
    );
    assert_eq!(new_code, 0, "create addition patch: {new_err}");
    let _ = run(
        root,
        &["git", "update-index", "--force-remove", "second-module"],
    );

    let (mode_code, _, mode_err) = run(
        root,
        &[
            "git",
            "update-index",
            "--cacheinfo",
            "160000",
            &gitlink_sha,
            "ordinary.txt",
        ],
    );
    assert_eq!(mode_code, 0, "prepare mode transition: {mode_err}");
    let (mode_diff_code, mode_patch, mode_diff_err) = run(
        root,
        &[
            "git",
            "diff",
            "--cached",
            "--full-index",
            "--binary",
            "--",
            "ordinary.txt",
        ],
    );
    assert_eq!(mode_diff_code, 0, "create mode patch: {mode_diff_err}");
    let (mode_restore_code, _, mode_restore_err) = run(
        root,
        &[
            "git",
            "update-index",
            "--cacheinfo",
            "100644",
            &ordinary_blob,
            "ordinary.txt",
        ],
    );
    assert_eq!(
        mode_restore_code, 0,
        "restore ordinary mode: {mode_restore_err}"
    );
    let _ = run(&nested, &["git", "config", "--unset", "codex.filterran"]);

    for (name, diff) in [
        ("delete", delete_patch),
        ("rename", rename_patch),
        ("add", new_patch),
        ("mode transition", mode_patch),
    ] {
        assert!(!diff.is_empty(), "{name} patch must not be empty");
        let error = apply_git_patch(&ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff,
            revert: false,
            preflight: false,
        })
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Unsupported, "{name}");
        let (after_code, after_index, after_err) = run(root, &["git", "ls-files", "--stage"]);
        assert_eq!(after_code, 0, "read index after {name}: {after_err}");
        assert_eq!(after_index, before_index, "{name} changed parent index");
        assert!(!configured_filter_ran(&nested), "{name} entered child");
    }
}

#[test]
fn index_stage_parser_preserves_paths_and_rejects_ambiguous_records() {
    let parsed = parse_index_stage_records(
        b"160000 0123456789abcdef0123456789abcdef01234567 0\tnested module\0\
100644 abcdefabcdefabcdefabcdefabcdefabcdefabcd 2\tname\twith-tab\0",
    )
    .expect("parse records");
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].mode, "160000");
    assert_eq!(parsed[0].oid, "0123456789abcdef0123456789abcdef01234567");
    assert_eq!(parsed[0].stage, 0);
    assert_eq!(parsed[0].path, "nested module");
    assert_eq!(parsed[1].path, "name\twith-tab");

    for malformed in [
        b"160000 abc 0\tnested".as_slice(),
        b"160000 not-an-oid 0\tnested\0".as_slice(),
        b"160000 abc 4\tnested\0".as_slice(),
        b"160000 abc 0 nested\0".as_slice(),
    ] {
        assert!(
            parse_index_stage_records(malformed).is_err(),
            "{malformed:?}"
        );
    }
}

#[test]
fn case_sensitive_sibling_of_gitlink_is_not_treated_as_descendant() {
    let _g = env_lock().lock().unwrap();
    let repo = init_repo();
    let root = repo.path();
    init_submodule_with_clean_filter(root);
    let sibling = root.join("NESTED");
    std::fs::create_dir_all(&sibling).expect("create case-distinct sibling");
    if std::fs::canonicalize(&sibling).expect("canonical sibling")
        == std::fs::canonicalize(root.join("nested")).expect("canonical gitlink")
    {
        return;
    }
    std::fs::write(sibling.join("file.txt"), "old\n").expect("write sibling file");
    let (add_code, _, add_err) = run(root, &["git", "add", "NESTED/file.txt"]);
    assert_eq!(add_code, 0, "add sibling: {add_err}");
    let (commit_code, _, commit_err) = run(root, &["git", "commit", "-m", "case sibling"]);
    assert_eq!(commit_code, 0, "commit sibling: {commit_err}");
    std::fs::write(sibling.join("file.txt"), "new\n").expect("modify sibling");
    let (diff_code, diff, diff_err) = run(
        root,
        &["git", "diff", "--full-index", "--", "NESTED/file.txt"],
    );
    assert_eq!(diff_code, 0, "create sibling patch: {diff_err}");
    let (restore_code, _, restore_err) = run(root, &["git", "checkout", "--", "NESTED/file.txt"]);
    assert_eq!(restore_code, 0, "restore sibling: {restore_err}");

    let result = apply_git_patch(&ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff,
        revert: false,
        preflight: false,
    })
    .expect("allow case-distinct sibling");
    assert_eq!(result.exit_code, 0);
    assert_eq!(read_file_normalized(&sibling.join("file.txt")), "new\n");
    assert!(!configured_filter_ran(&root.join("nested")));
}

#[test]
fn absent_case_distinct_sibling_is_allowed_on_case_sensitive_filesystem() {
    let _g = env_lock().lock().unwrap();
    let repo = init_repo();
    let root = repo.path();
    init_submodule_with_clean_filter(root);
    let probe = root.join("NESTED");
    match std::fs::create_dir(&probe) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => return,
        Err(error) => panic!("probe case-distinct name: {error}"),
    }
    if std::fs::canonicalize(&probe).expect("canonical probe")
        == std::fs::canonicalize(root.join("nested")).expect("canonical gitlink")
    {
        return;
    }
    std::fs::remove_dir(&probe).expect("remove probe");

    let diff = "diff --git a/NESTED/new.txt b/NESTED/new.txt\nnew file mode 100644\n--- /dev/null\n+++ b/NESTED/new.txt\n@@ -0,0 +1 @@\n+case-sensitive sibling\n";
    let result = apply_git_patch(&ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff: diff.to_string(),
        revert: false,
        preflight: false,
    })
    .expect("allow absent case-distinct sibling");
    assert_eq!(result.exit_code, 0, "apply sibling: {}", result.stderr);
    assert_eq!(
        read_file_normalized(&root.join("NESTED/new.txt")),
        "case-sensitive sibling\n"
    );
    assert!(!configured_filter_ran(&root.join("nested")));
}

#[test]
fn gitlink_guard_ignores_inherited_git_selection_environment() {
    let _g = env_lock().lock().unwrap();
    if std::env::var_os("CODEX_GIT_UTILS_PATH_ENV_CHILD").is_some() {
        let root = PathBuf::from(
            std::env::var_os("CODEX_GIT_UTILS_TARGET_REPO").expect("target repository"),
        );
        // Use the exact indexed spelling so this test isolates inherited Git
        // selectors on both case-sensitive and case-insensitive filesystems.
        // Case-distinct sibling behavior is covered by dedicated tests.
        let error = ensure_paths_do_not_enter_submodules(&root, &["nested/file.txt".to_string()])
            .expect_err("reject target-repository gitlink");
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
        assert!(!configured_filter_ran(&root.join("nested")));
        return;
    }

    let target = init_repo();
    init_submodule_with_clean_filter(target.path());
    let alternate = init_repo();
    std::fs::write(alternate.path().join("sentinel.txt"), "alternate\n")
        .expect("write alternate file");
    let (add_code, _, add_err) = run(alternate.path(), &["git", "add", "sentinel.txt"]);
    assert_eq!(add_code, 0, "add alternate file: {add_err}");
    let (commit_code, _, commit_err) = run(alternate.path(), &["git", "commit", "-m", "alternate"]);
    assert_eq!(commit_code, 0, "commit alternate file: {commit_err}");

    let alternate_git_dir = alternate.path().join(".git");
    let nonexistent_index = alternate.path().join("nonexistent-index");
    let target_env = ("CODEX_GIT_UTILS_TARGET_REPO", target.path().as_os_str());
    let cases = [
        ("GIT_LITERAL_PATHSPECS", OsStr::new("1")),
        ("GIT_GLOB_PATHSPECS", OsStr::new("1")),
        ("GIT_NOGLOB_PATHSPECS", OsStr::new("1")),
        ("GIT_ICASE_PATHSPECS", OsStr::new("1")),
        ("GIT_DIR", alternate_git_dir.as_os_str()),
        ("GIT_WORK_TREE", alternate.path().as_os_str()),
        ("GIT_COMMON_DIR", alternate_git_dir.as_os_str()),
        ("GIT_INDEX_FILE", nonexistent_index.as_os_str()),
        ("GIT_PREFIX", OsStr::new("elsewhere/")),
    ];
    for (name, value) in cases {
        run_isolated_test(
            "patch_paths::tests::gitlink_guard_ignores_inherited_git_selection_environment",
            &[target_env, (name, value)],
        );
    }
}

#[cfg(unix)]
#[test]
fn public_apply_skips_repository_controlled_primary_git() {
    let _g = env_lock().lock().unwrap();
    if std::env::var_os("CODEX_GIT_UTILS_PATH_ENV_CHILD").is_some() {
        let root = PathBuf::from(
            std::env::var_os("CODEX_GIT_UTILS_TARGET_REPO").expect("target repository"),
        );
        let result = apply_git_patch(&ApplyGitRequest {
            cwd: root.clone(),
            diff: "diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n@@ -1 +1 @@\n-old\n+new\n"
                .to_string(),
            revert: false,
            preflight: false,
        })
        .expect("apply with trusted external Git");
        assert_eq!(result.exit_code, 0, "apply patch: {}", result.stderr);
        assert_eq!(read_file_normalized(&root.join("file.txt")), "new\n");
        return;
    }

    use std::os::unix::fs::PermissionsExt;

    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join("file.txt"), "old\n").expect("fixture file");
    let (add_code, _, add_err) = run(root, &["git", "add", "file.txt"]);
    assert_eq!(add_code, 0, "add fixture: {add_err}");
    let (commit_code, _, commit_err) = run(root, &["git", "commit", "-m", "fixture"]);
    assert_eq!(commit_code, 0, "commit fixture: {commit_err}");
    let (config_code, _, config_err) = run(root, &["git", "config", "filter.unused.clean", "cat"]);
    assert_eq!(config_code, 0, "configure unused filter: {config_err}");

    let repo_bin = root.join("bin");
    std::fs::create_dir(&repo_bin).expect("repository bin");
    let marker = root.join("repository-primary-git-ran");
    let fake_git = repo_bin.join("git");
    std::fs::write(
        &fake_git,
        "#!/bin/sh\nprintf ran > \"$CODEX_GIT_UTILS_PRIMARY_GIT_MARKER\"\nexec \"$CODEX_GIT_UTILS_REAL_GIT\" \"$@\"\n",
    )
    .expect("repository Git");
    let mut permissions = std::fs::metadata(&fake_git)
        .expect("repository Git metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&fake_git, permissions).expect("repository Git executable");

    let output = std::process::Command::new("/bin/sh")
        .args(["-c", "command -v git"])
        .output()
        .expect("resolve fixture Git");
    assert!(output.status.success(), "resolve fixture Git");
    let real_git = PathBuf::from(
        String::from_utf8(output.stdout)
            .expect("Git path UTF-8")
            .trim(),
    );
    let search_path = std::env::join_paths([
        repo_bin.as_path(),
        real_git.parent().expect("Git executable directory"),
    ])
    .expect("controlled PATH");
    run_isolated_test(
        "patch_paths::tests::public_apply_skips_repository_controlled_primary_git",
        &[
            ("CODEX_GIT_UTILS_TARGET_REPO", root.as_os_str()),
            ("CODEX_GIT_UTILS_PRIMARY_GIT_MARKER", marker.as_os_str()),
            ("CODEX_GIT_UTILS_REAL_GIT", real_git.as_os_str()),
            ("PATH", search_path.as_os_str()),
        ],
    );
    assert!(!marker.exists(), "repository-controlled primary Git ran");
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

#[cfg(unix)]
#[test]
fn gitlink_probe_resolves_filesystem_aliases_and_rejects_escapes() {
    let _g = env_lock().lock().unwrap();
    let repo = init_repo();
    let root = repo.path();
    init_submodule_with_clean_filter(root);

    std::os::unix::fs::symlink("nested", root.join("alias")).expect("create gitlink alias");
    let error = ensure_paths_do_not_enter_submodules(root, &["alias/file.txt".to_string()])
        .expect_err("reject alias to gitlink");
    assert_eq!(error.kind(), io::ErrorKind::Unsupported);

    std::fs::create_dir_all(root.join("nested/subdir")).expect("nested subdirectory");
    std::os::unix::fs::symlink("nested/subdir", root.join("deep-alias"))
        .expect("create descendant gitlink alias");
    let error = ensure_paths_do_not_enter_submodules(root, &["deep-alias/file.txt".to_string()])
        .expect_err("reject alias below gitlink");
    assert_eq!(error.kind(), io::ErrorKind::Unsupported);

    let outside = tempfile::tempdir().expect("outside directory");
    std::os::unix::fs::symlink(outside.path(), root.join("outside")).expect("create outside alias");
    let error = ensure_paths_do_not_enter_submodules(root, &["outside/file.txt".to_string()])
        .expect_err("reject alias outside worktree");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);

    let outside_leaf = outside.path().join("leaf-target");
    std::fs::write(&outside_leaf, "outside\n").expect("outside leaf target");
    std::os::unix::fs::symlink(&outside_leaf, root.join("leaf"))
        .expect("create outside leaf alias");
    ensure_paths_do_not_enter_submodules(root, &["leaf".to_string()])
        .expect("allow replacing a leaf symlink");
    assert_eq!(
        std::fs::read_to_string(outside_leaf).expect("outside target"),
        "outside\n"
    );
}

#[cfg(unix)]
#[test]
fn patch_rejects_strict_ancestor_alias_into_git_metadata() {
    let _g = env_lock().lock().unwrap();
    let repo = init_repo();
    let root = repo.path();
    std::os::unix::fs::symlink(".git", root.join("metadata-alias")).expect("create metadata alias");
    let hook = root.join(".git/hooks/codex-test");
    let diff = "diff --git a/metadata-alias/hooks/codex-test b/metadata-alias/hooks/codex-test\nnew file mode 100755\n--- /dev/null\n+++ b/metadata-alias/hooks/codex-test\n@@ -0,0 +1,2 @@\n+#!/bin/sh\n+exit 0\n";

    for preflight in [true, false] {
        let error = apply_git_patch(&ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: diff.to_string(),
            revert: false,
            preflight,
        })
        .expect_err("reject Git metadata alias");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert!(!hook.exists(), "Git metadata was modified");
    }
}

#[cfg(unix)]
#[test]
fn patch_replaces_outside_pointing_leaf_symlink_without_touching_targets() {
    let _g = env_lock().lock().unwrap();
    let repo = init_repo();
    let root = repo.path();
    let outside = tempfile::tempdir().expect("outside directory");
    let old_target = outside.path().join("old-target");
    let new_target = outside.path().join("new-target");
    std::fs::write(&old_target, "old sentinel\n").expect("old target");
    std::fs::write(&new_target, "new sentinel\n").expect("new target");

    let leaf = root.join("leaf");
    std::os::unix::fs::symlink(&old_target, &leaf).expect("old leaf symlink");
    let (add_code, _, add_err) = run(root, &["git", "add", "leaf"]);
    assert_eq!(add_code, 0, "add leaf: {add_err}");
    let (commit_code, _, commit_err) = run(root, &["git", "commit", "-m", "leaf"]);
    assert_eq!(commit_code, 0, "commit leaf: {commit_err}");

    std::fs::remove_file(&leaf).expect("remove old leaf");
    std::os::unix::fs::symlink(&new_target, &leaf).expect("new leaf symlink");
    let (diff_code, diff, diff_err) = run(root, &["git", "diff", "--full-index", "--", "leaf"]);
    assert_eq!(diff_code, 0, "create leaf patch: {diff_err}");
    let (restore_code, _, restore_err) = run(root, &["git", "checkout", "--", "leaf"]);
    assert_eq!(restore_code, 0, "restore old leaf: {restore_err}");

    let preflight = apply_git_patch(&ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff: diff.clone(),
        revert: false,
        preflight: true,
    })
    .expect("preflight leaf patch");
    assert_eq!(preflight.exit_code, 0);
    assert_eq!(std::fs::read_link(&leaf).expect("old leaf"), old_target);

    let applied = apply_git_patch(&ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff: diff.clone(),
        revert: false,
        preflight: false,
    })
    .expect("apply leaf patch");
    assert_eq!(applied.exit_code, 0);
    assert_eq!(std::fs::read_link(&leaf).expect("new leaf"), new_target);

    let reverse_preflight = apply_git_patch(&ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff: diff.clone(),
        revert: true,
        preflight: true,
    })
    .expect("preflight reverse leaf patch");
    assert_eq!(reverse_preflight.exit_code, 0);
    assert_eq!(std::fs::read_link(&leaf).expect("new leaf"), new_target);

    let reverted = apply_git_patch(&ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff,
        revert: true,
        preflight: false,
    })
    .expect("reverse leaf patch");
    assert_eq!(reverted.exit_code, 0);
    assert_eq!(
        std::fs::read_link(&leaf).expect("restored leaf"),
        old_target
    );
    assert_eq!(
        std::fs::read_to_string(outside.path().join("old-target")).expect("old sentinel"),
        "old sentinel\n"
    );
    assert_eq!(
        std::fs::read_to_string(outside.path().join("new-target")).expect("new sentinel"),
        "new sentinel\n"
    );
}

#[test]
fn patch_variants_reject_paths_inside_submodules() {
    let _g = env_lock().lock().unwrap();
    let repo = init_repo();
    let root = repo.path();
    init_submodule_with_clean_filter(root);
    let cases = [
        (
            "git-format",
            "diff --git a/nested/file.txt b/nested/file.txt\n--- a/nested/file.txt\n+++ b/nested/file.txt\n@@ -1 +1 @@\n-original\n+modified\n",
            io::ErrorKind::Unsupported,
        ),
        (
            "headerless",
            "--- old/nested/file.txt\n+++ new/nested/file.txt\n@@ -1 +1 @@\n-original\n+modified\n",
            io::ErrorKind::Unsupported,
        ),
        (
            "case-folded gitlink ancestor",
            "--- old/NESTED/file.txt\n+++ new/NESTED/file.txt\n@@ -1 +1 @@\n-original\n+modified\n",
            io::ErrorKind::Unsupported,
        ),
        (
            "mismatched headers",
            "diff --git a/safe.txt b/safe.txt\n--- old/nested/file.txt\n+++ new/nested/file.txt\n@@ -1 +1 @@\n-original\n+modified\n",
            io::ErrorKind::Unsupported,
        ),
        (
            "mismatched rename metadata",
            "diff --git a/safe-old.txt b/safe-new.txt\nsimilarity index 100%\nrename from nested/file.txt\nrename to nested/renamed.txt\n",
            io::ErrorKind::Unsupported,
        ),
    ];

    for (name, diff, expected_kind) in cases {
        for (revert, preflight) in [(false, true), (false, false), (true, true), (true, false)] {
            let request = ApplyGitRequest {
                cwd: root.to_path_buf(),
                diff: diff.to_string(),
                revert,
                preflight,
            };
            let error = apply_git_patch(&request).unwrap_err();
            assert_eq!(error.kind(), expected_kind, "{name}");
            assert!(!configured_filter_ran(&root.join("nested")));
        }
    }
}

#[test]
fn headerless_patch_ignores_unrelated_submodule_across_variants() {
    let _g = env_lock().lock().unwrap();
    let repo = init_repo();
    let root = repo.path();
    init_submodule_with_clean_filter(root);
    ensure_paths_do_not_enter_submodules(root, &["[n]ested/file.txt".to_string()])
        .expect("treat patch paths as literal pathspecs");
    std::fs::write(root.join("root.txt"), "old\n").expect("write root file");
    let (add_code, _, add_err) = run(root, &["git", "add", "root.txt"]);
    assert_eq!(add_code, 0, "add root file: {add_err}");
    let (commit_code, _, commit_err) = run(root, &["git", "commit", "-m", "root file"]);
    assert_eq!(commit_code, 0, "commit root file: {commit_err}");
    let diff = "--- old/root.txt\n+++ new/root.txt\n@@ -1 +1 @@\n-old\n+new\n";

    for (revert, preflight, expected) in [
        (false, true, "old\n"),
        (false, false, "new\n"),
        (true, true, "new\n"),
        (true, false, "old\n"),
    ] {
        let result = apply_git_patch(&ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: diff.to_string(),
            revert,
            preflight,
        })
        .unwrap_or_else(|error| panic!("revert={revert}, preflight={preflight}: {error}"));
        assert_eq!(
            result.exit_code, 0,
            "revert={revert}, preflight={preflight}"
        );
        assert_eq!(read_file_normalized(&root.join("root.txt")), expected);
        assert!(!configured_filter_ran(&root.join("nested")));
    }
}
