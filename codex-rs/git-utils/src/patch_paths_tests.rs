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
    let paths = extract_effective_paths_from_patch(&patch_path, revert)?;
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
    let error = validate_patch_path("..\\nested\\file.txt".to_string())
        .expect_err("reject Windows path separators");
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    let error = validate_patch_path("C:/outside.txt".to_string()).expect_err("reject drive prefix");
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
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
fn gitlink_guard_ignores_inherited_git_selection_environment() {
    let _g = env_lock().lock().unwrap();
    if std::env::var_os("CODEX_GIT_UTILS_PATH_ENV_CHILD").is_some() {
        let root = PathBuf::from(
            std::env::var_os("CODEX_GIT_UTILS_TARGET_REPO").expect("target repository"),
        );
        let error = ensure_paths_do_not_enter_submodules(&root, &["NESTED/file.txt".to_string()])
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

#[test]
fn staging_preserves_trusted_config_while_clearing_pathspec_modes() {
    let _g = env_lock().lock().unwrap();
    if std::env::var_os("CODEX_GIT_UTILS_PATH_ENV_CHILD").is_some() {
        let root = PathBuf::from(
            std::env::var_os("CODEX_GIT_UTILS_TARGET_REPO").expect("target repository"),
        );
        stage_effective_paths(&root, &["global.txt".to_string(), "system.txt".to_string()])
            .expect("stage with trusted filters");
        return;
    }

    let repo = init_repo();
    let root = repo.path();
    std::fs::write(
        root.join(".gitattributes"),
        "global.txt filter=trusted-global\nsystem.txt filter=trusted-system\n",
    )
    .expect("write attributes");
    std::fs::write(root.join("global.txt"), "old global\n").expect("write global file");
    std::fs::write(root.join("system.txt"), "old system\n").expect("write system file");
    let (add_code, _, add_err) = run(root, &["git", "add", "."]);
    assert_eq!(add_code, 0, "add base files: {add_err}");
    let (commit_code, _, commit_err) = run(root, &["git", "commit", "-m", "base"]);
    assert_eq!(commit_code, 0, "commit base files: {commit_err}");
    std::fs::write(root.join("global.txt"), "new global\n").expect("modify global file");
    std::fs::write(root.join("system.txt"), "new system\n").expect("modify system file");

    let config_dir = tempfile::tempdir().expect("config tempdir");
    let global_config = config_dir.path().join("global.gitconfig");
    let system_config = config_dir.path().join("system.gitconfig");
    std::fs::write(
        &global_config,
        "[filter \"trusted-global\"]\n\tclean = git config codex.globalfilterran true && git hash-object --stdin\n\trequired = true\n",
    )
    .expect("write global config");
    std::fs::write(
        &system_config,
        "[filter \"trusted-system\"]\n\tclean = git config codex.systemfilterran true && git hash-object --stdin\n\trequired = true\n",
    )
    .expect("write system config");
    run_isolated_test(
        "patch_paths::tests::staging_preserves_trusted_config_while_clearing_pathspec_modes",
        &[
            ("CODEX_GIT_UTILS_TARGET_REPO", root.as_os_str()),
            ("GIT_CONFIG_GLOBAL", global_config.as_os_str()),
            ("GIT_CONFIG_SYSTEM", system_config.as_os_str()),
            ("GIT_GLOB_PATHSPECS", OsStr::new("1")),
            ("GIT_ICASE_PATHSPECS", OsStr::new("1")),
        ],
    );

    for marker in ["codex.globalfilterran", "codex.systemfilterran"] {
        let (code, value, error) = run(root, &["git", "config", "--get", marker]);
        assert_eq!(code, 0, "read {marker}: {error}");
        assert_eq!(value.trim(), "true", "{marker}");
    }
    let (diff_code, staged, diff_err) = run(root, &["git", "diff", "--cached", "--name-only"]);
    assert_eq!(diff_code, 0, "read staged paths: {diff_err}");
    assert_eq!(
        staged.lines().collect::<Vec<_>>(),
        ["global.txt", "system.txt"]
    );
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

    let outside = tempfile::tempdir().expect("outside directory");
    std::os::unix::fs::symlink(outside.path(), root.join("outside")).expect("create outside alias");
    let error = ensure_paths_do_not_enter_submodules(root, &["outside/file.txt".to_string()])
        .expect_err("reject alias outside worktree");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
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
