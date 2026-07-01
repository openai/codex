use super::*;
use crate::apply::ApplyGitRequest;
use crate::apply::apply_git_patch;
use crate::git_config::GitConfigScope;
#[cfg(unix)]
use crate::patch_paths::stage_paths;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
#[cfg(unix)]
use std::ffi::OsStr;
use std::path::Path;
#[cfg(unix)]
use std::path::PathBuf;
use tokio::process::Command as TokioCommand;

#[test]
fn selected_filter_policy_allows_unused_and_rejects_selected_at_every_scope() {
    let config_dir = tempfile::tempdir().expect("config");
    let config = config_dir.path().join("global.gitconfig");
    std::fs::write(&config, "").expect("config file");

    for scope in [
        GitConfigScope::System,
        GitConfigScope::Global,
        GitConfigScope::Local,
        GitConfigScope::Worktree,
        GitConfigScope::Command,
    ] {
        for (key, value) in [
            ("filter.demo.clean", "./clean.sh"),
            ("filter.lfs.clean", "git-lfs clean -- %f"),
            ("filter.lfs.smudge", "git-lfs smudge -- %f"),
            ("filter.lfs.process", "git-lfs filter-process"),
        ] {
            let entries = filter_entries(scope, &config, key, value);
            let driver = filter_driver_name(key).expect("driver name");
            let selected = BTreeMap::from([(b"file.txt".to_vec(), driver.clone())]);
            assert!(
                selected_executable_filter(&entries, &selected)
                    .expect("selected filter policy")
                    .is_some(),
                "{scope:?} {key}"
            );
            let unused = BTreeMap::from([(b"file.txt".to_vec(), "other".to_string())]);
            assert_eq!(
                selected_executable_filter(&entries, &unused).expect("unused filter policy"),
                None,
                "{scope:?} {key}"
            );
        }
    }
}

#[test]
fn selected_filter_policy_allows_effective_empty_value() {
    let disabled = filter_entries(
        GitConfigScope::Command,
        Path::new("command line:"),
        "filter.demo.clean",
        "",
    );
    let selected = BTreeMap::from([(b"file.txt".to_vec(), "demo".to_string())]);
    assert_eq!(
        selected_executable_filter(&disabled, &selected).expect("empty filter policy"),
        None
    );
}

#[test]
fn git_add_filter_policy_rejects_clean_and_process_but_allows_smudge_only() {
    let selected = BTreeMap::from([(b"file.txt".to_vec(), "demo".to_string())]);
    for (key, rejected) in [
        ("filter.demo.clean", true),
        ("filter.demo.smudge", false),
        ("filter.demo.process", true),
    ] {
        let entries = filter_entries(
            GitConfigScope::Local,
            Path::new(".git/config"),
            key,
            "codex-definitely-missing-filter-command",
        );
        assert_eq!(
            selected_executable_filter_for(&entries, &selected, FilterExecution::GitAdd)
                .expect("Git add filter policy")
                .is_some(),
            rejected,
            "{key}"
        );
    }
}

#[test]
fn filter_attribute_parser_rejects_malformed_or_unexpected_records() {
    let paths = vec![b"a.txt".to_vec(), b"b.txt".to_vec()];
    let parsed =
        parse_filter_attributes(b"a.txt\0filter\0unspecified\0b.txt\0filter\0lfs\0", &paths)
            .expect("parse attributes");
    assert_eq!(
        parsed.get(b"a.txt".as_slice()).map(String::as_str),
        Some("unspecified")
    );
    assert_eq!(
        parsed.get(b"b.txt".as_slice()).map(String::as_str),
        Some("lfs")
    );

    for output in [
        b"a.txt\0filter\0unspecified".as_slice(),
        b"a.txt\0merge\0unspecified\0b.txt\0filter\0lfs\0".as_slice(),
        b"a.txt\0filter\0unspecified\0".as_slice(),
        b"a.txt\0filter\0unspecified\0a.txt\0filter\0lfs\0".as_slice(),
    ] {
        assert!(
            parse_filter_attributes(output, &paths).is_err(),
            "{output:?}"
        );
    }
}

fn filter_entries(
    scope: GitConfigScope,
    origin: &Path,
    key: &str,
    value: &str,
) -> BTreeMap<String, GitConfigEntry> {
    let origin = if origin == Path::new("command line:") {
        "command line:".to_string()
    } else {
        format!("file:{}", origin.display())
    };
    BTreeMap::from([(
        key.to_string(),
        GitConfigEntry {
            scope,
            origin,
            key: key.to_string(),
            value: value.to_string(),
        },
    )])
}

#[cfg(unix)]
fn run_isolated_test(test_name: &str, env: &[(&str, &OsStr)]) {
    let mut command = std::process::Command::new(std::env::current_exe().expect("test binary"));
    isolate_git_command_environment(&mut command);
    command
        .arg(test_name)
        .arg("--exact")
        .arg("--nocapture")
        .env("CODEX_GIT_UTILS_SAFE_GIT_ENV_CHILD", "1")
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

async fn run_git(repo_path: &Path, args: &[&str]) {
    let output = TokioCommand::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .await
        .expect("run git command");
    assert!(
        output.status.success(),
        "git command failed: {args:?}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

async fn create_test_git_repo(temp_dir: &tempfile::TempDir) -> std::path::PathBuf {
    let repo_path = temp_dir.path().join("repo");
    std::fs::create_dir(&repo_path).expect("create repo dir");
    run_git(&repo_path, &["init"]).await;
    run_git(&repo_path, &["config", "user.name", "Test User"]).await;
    run_git(&repo_path, &["config", "user.email", "test@example.com"]).await;
    std::fs::write(repo_path.join("test.txt"), "test content").expect("write test file");
    run_git(&repo_path, &["add", "."]).await;
    run_git(&repo_path, &["commit", "-m", "initial"]).await;
    repo_path
}

#[tokio::test]
async fn ordinary_apply_allows_an_unselected_executable_filter() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo_path = create_test_git_repo(&temp_dir).await;
    std::fs::write(repo_path.join("test.txt"), "old\n").expect("write fixture");
    run_git(&repo_path, &["add", "test.txt"]).await;
    run_git(&repo_path, &["commit", "-m", "normalize fixture"]).await;
    run_git(
        &repo_path,
        &[
            "config",
            "filter.unused.clean",
            "codex-definitely-missing-filter-command",
        ],
    )
    .await;

    let result = apply_git_patch(&ApplyGitRequest {
        cwd: repo_path.clone(),
        diff: "diff --git a/test.txt b/test.txt\n--- a/test.txt\n+++ b/test.txt\n@@ -1 +1 @@\n-old\n+new\n"
            .to_string(),
        revert: false,
        preflight: false,
    })
    .expect("unused filter must not block apply");
    assert_eq!(result.exit_code, 0);
    let contents = std::fs::read_to_string(repo_path.join("test.txt")).expect("read result");
    assert!(
        matches!(contents.as_str(), "new\n" | "new\r\n"),
        "expected the patched contents with a platform line ending, got {contents:?}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn apply_and_stage_reject_global_relative_filter_without_running_it() {
    if std::env::var_os("CODEX_GIT_UTILS_SAFE_GIT_ENV_CHILD").is_none() {
        let config_dir = tempfile::tempdir().expect("config tempdir");
        let global_config = config_dir.path().join("global.gitconfig");
        let system_config = config_dir.path().join("system.gitconfig");
        std::fs::write(&global_config, "[filter \"evil\"]\n\tclean = ./clean.sh\n")
            .expect("write global config");
        std::fs::write(&system_config, "").expect("write system config");
        run_isolated_test(
            "safe_git::tests::apply_and_stage_reject_global_relative_filter_without_running_it",
            &[
                ("GIT_CONFIG_GLOBAL", global_config.as_os_str()),
                ("GIT_CONFIG_SYSTEM", system_config.as_os_str()),
            ],
        );
        return;
    }

    use std::os::unix::fs::PermissionsExt;

    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo_path = create_test_git_repo(&temp_dir).await;
    let marker = repo_path.join("filter-ran");
    std::fs::write(repo_path.join("test.txt"), "old\n").expect("tracked file");
    std::fs::write(repo_path.join(".gitattributes"), "test.txt filter=evil\n").expect("attributes");
    std::fs::write(
        repo_path.join("clean.sh"),
        format!("#!/bin/sh\ntouch '{}'\ncat\n", marker.display()),
    )
    .expect("relative filter");
    let mut permissions = std::fs::metadata(repo_path.join("clean.sh"))
        .expect("filter metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(repo_path.join("clean.sh"), permissions).expect("filter executable");
    run_git(
        &repo_path,
        &[
            "-c",
            "filter.evil.clean=",
            "add",
            "test.txt",
            ".gitattributes",
        ],
    )
    .await;
    run_git(
        &repo_path,
        &["-c", "filter.evil.clean=", "commit", "-m", "fixture"],
    )
    .await;
    assert!(!marker.exists(), "setup must not run filter");

    let diff = "diff --git a/test.txt b/test.txt\n--- a/test.txt\n+++ b/test.txt\n@@ -1 +1 @@\n-old\n+new\n";
    let error = apply_git_patch(&ApplyGitRequest {
        cwd: repo_path.clone(),
        diff: diff.to_string(),
        revert: false,
        preflight: false,
    })
    .expect_err("reject relative global filter");
    assert_eq!(error.kind(), std::io::ErrorKind::Unsupported);
    assert!(!marker.exists(), "apply must not run filter");
    assert_eq!(
        std::fs::read_to_string(repo_path.join("test.txt")).expect("read tracked file"),
        "old\n"
    );

    let error = stage_paths(&repo_path, diff).expect_err("reject filter during staging");
    assert_eq!(error.kind(), std::io::ErrorKind::Unsupported);
    assert!(!marker.exists(), "staging must not run filter");
}

#[cfg(unix)]
#[tokio::test]
async fn nested_cwd_rejects_global_lfs_filter_without_running_it() {
    if std::env::var_os("CODEX_GIT_UTILS_SAFE_GIT_ENV_CHILD").is_none() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;
        let nested = repo_path.join("nested");
        let repo_bin = repo_path.join("bin");
        std::fs::create_dir(&nested).expect("nested cwd");
        std::fs::create_dir(&repo_bin).expect("repository bin");
        std::fs::write(repo_path.join(".gitattributes"), "test.txt filter=lfs\n")
            .expect("attributes");
        run_git(&repo_path, &["add", ".gitattributes"]).await;
        run_git(&repo_path, &["commit", "-m", "attributes"]).await;
        std::fs::write(repo_path.join("test.txt"), "changed\n").expect("modify tracked file");

        let config_dir = tempfile::tempdir().expect("config tempdir");
        let global_config = config_dir.path().join("global.gitconfig");
        let system_config = config_dir.path().join("system.gitconfig");
        std::fs::write(
            &global_config,
            "[filter \"lfs\"]\n\tclean = git-lfs clean -- %f\n",
        )
        .expect("write global config");
        std::fs::write(&system_config, "").expect("write system config");
        let marker = config_dir.path().join("repo-lfs-ran");
        let primary_git_marker = config_dir.path().join("repo-primary-git-ran");
        let repo_git_lfs = repo_bin.join("git-lfs");
        std::fs::write(
            &repo_git_lfs,
            "#!/bin/sh\n: > \"$CODEX_GIT_UTILS_UNSAFE_LFS_MARKER\"\nwhile IFS= read -r line\ndo\n  printf '%s\\n' \"$line\"\ndone\n",
        )
        .expect("repository git-lfs");
        let mut permissions = std::fs::metadata(&repo_git_lfs)
            .expect("repository git-lfs metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&repo_git_lfs, permissions)
            .expect("make repository git-lfs executable");

        let output = std::process::Command::new("/bin/sh")
            .args(["-c", "command -v git"])
            .output()
            .expect("resolve git executable");
        assert!(output.status.success(), "resolve git executable");
        let git_path = PathBuf::from(
            String::from_utf8(output.stdout)
                .expect("Git path UTF-8")
                .trim(),
        );
        let repo_git = repo_bin.join("git");
        std::fs::write(
            &repo_git,
            "#!/bin/sh\nprintf ran > \"$CODEX_GIT_UTILS_PRIMARY_GIT_MARKER\"\nexec \"$CODEX_GIT_UTILS_REAL_GIT\" \"$@\"\n",
        )
        .expect("repository Git");
        let mut permissions = std::fs::metadata(&repo_git)
            .expect("repository Git metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&repo_git, permissions).expect("make repository Git executable");
        let search_path = std::env::join_paths([
            repo_bin.as_path(),
            git_path.parent().expect("Git executable directory"),
        ])
        .expect("construct controlled PATH");
        run_isolated_test(
            "safe_git::tests::nested_cwd_rejects_global_lfs_filter_without_running_it",
            &[
                ("CODEX_GIT_UTILS_TARGET_REPO", repo_path.as_os_str()),
                ("CODEX_GIT_UTILS_UNSAFE_LFS_MARKER", marker.as_os_str()),
                (
                    "CODEX_GIT_UTILS_PRIMARY_GIT_MARKER",
                    primary_git_marker.as_os_str(),
                ),
                ("CODEX_GIT_UTILS_REAL_GIT", git_path.as_os_str()),
                ("GIT_CONFIG_GLOBAL", global_config.as_os_str()),
                ("GIT_CONFIG_SYSTEM", system_config.as_os_str()),
                ("PATH", search_path.as_os_str()),
            ],
        );
        assert!(!marker.exists(), "repository git-lfs must not run");
        assert!(
            !primary_git_marker.exists(),
            "repository-controlled primary Git must not run"
        );
        return;
    }

    let repo_path =
        PathBuf::from(std::env::var_os("CODEX_GIT_UTILS_TARGET_REPO").expect("target repository"));
    let diff = "diff --git a/test.txt b/test.txt\n--- a/test.txt\n+++ b/test.txt\n@@ -1 +1 @@\n-old\n+new\n";
    let error = apply_git_patch(&ApplyGitRequest {
        cwd: repo_path.join("nested"),
        diff: diff.to_string(),
        revert: false,
        preflight: false,
    })
    .expect_err("reject global Git LFS filter from nested cwd");
    assert_eq!(error.kind(), std::io::ErrorKind::Unsupported);

    let error = stage_paths(&repo_path, diff).expect_err("reject global Git LFS during staging");
    assert_eq!(error.kind(), std::io::ErrorKind::Unsupported);
}
