use super::*;
use crate::apply::ApplyGitRequest;
use crate::apply::apply_git_patch;
use crate::get_has_changes;
use crate::git_config::GitConfigScope;
use crate::git_diff_to_remote;
use crate::patch_paths::stage_paths;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::Path;
use std::path::PathBuf;
use tokio::process::Command as TokioCommand;

#[test]
fn rejects_nonempty_filters_at_every_scope_including_lfs() {
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
            assert!(
                config_entries_have_untrusted_filters(&entries),
                "{scope:?} {key}"
            );
        }
    }
}

#[test]
fn allows_only_effective_empty_filter_values() {
    let disabled = filter_entries(
        GitConfigScope::Command,
        Path::new("command line:"),
        "filter.demo.clean",
        "",
    );
    assert!(!config_entries_have_untrusted_filters(&disabled));
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

async fn configure_clean_filter(repo_path: &Path, tracked_path: &str) {
    std::fs::write(
        repo_path.join(".gitattributes"),
        format!("{tracked_path} filter=x=y\n"),
    )
    .expect("write attributes");
    run_git(repo_path, &["add", ".gitattributes"]).await;
    run_git(repo_path, &["commit", "-m", "attributes"]).await;
    run_git(
        repo_path,
        &[
            "config",
            "filter.x=y.clean",
            "git config codex.filterran true && git hash-object --stdin",
        ],
    )
    .await;

    let tracked_file = repo_path.join(tracked_path);
    let contents = std::fs::read_to_string(&tracked_file).expect("read tracked file");
    std::thread::sleep(std::time::Duration::from_secs(1));
    std::fs::write(tracked_file, contents).expect("refresh tracked file");
}

async fn configured_filter_ran(repo_path: &Path) -> bool {
    let output = TokioCommand::new("git")
        .args(["config", "--get", "codex.filterran"])
        .current_dir(repo_path)
        .output()
        .await
        .expect("read filter marker");
    output.status.success()
}

async fn add_submodule_with_clean_filter(parent: &Path) {
    let source = tempfile::tempdir().expect("submodule source");
    let source_path = source.path();
    run_git(source_path, &["init"]).await;
    run_git(source_path, &["config", "user.name", "Test User"]).await;
    run_git(source_path, &["config", "user.email", "test@example.com"]).await;
    std::fs::write(source_path.join("nested.txt"), "original\n").expect("nested file");
    std::fs::write(
        source_path.join(".gitattributes"),
        "nested.txt filter=codex-test\n",
    )
    .expect("nested attributes");
    run_git(source_path, &["add", "."]).await;
    run_git(source_path, &["commit", "-m", "seed"]).await;

    run_git(
        parent,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            source_path.to_str().expect("source path"),
            "nested",
        ],
    )
    .await;
    run_git(parent, &["commit", "-m", "add submodule"]).await;
    let nested = parent.join("nested");
    run_git(
        &nested,
        &[
            "config",
            "filter.codex-test.clean",
            "git config codex.filterran true && git hash-object --stdin",
        ],
    )
    .await;
    std::fs::write(nested.join("nested.txt"), "modified\n").expect("dirty nested file");
}

async fn add_origin_and_push(repo_path: &Path, remote_path: &Path) -> String {
    run_git(
        remote_path.parent().expect("remote parent"),
        &["init", "--bare", remote_path.to_str().expect("remote path")],
    )
    .await;
    run_git(
        repo_path,
        &[
            "remote",
            "add",
            "origin",
            remote_path.to_str().expect("remote path"),
        ],
    )
    .await;
    let branch_output = TokioCommand::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(repo_path)
        .output()
        .await
        .expect("read branch");
    assert!(branch_output.status.success(), "read branch");
    let branch = String::from_utf8(branch_output.stdout)
        .expect("branch utf8")
        .trim()
        .to_string();
    run_git(repo_path, &["push", "-u", "origin", &branch]).await;
    branch
}

#[tokio::test]
async fn get_has_changes_rejects_configured_clean_filter_without_running_it() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo_path = create_test_git_repo(&temp_dir).await;
    configure_clean_filter(&repo_path, "test.txt").await;

    assert_eq!(get_has_changes(&repo_path).await, None);
    assert!(!configured_filter_ran(&repo_path).await);
}

#[tokio::test]
async fn get_has_changes_does_not_enter_dirty_submodules() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo_path = create_test_git_repo(&temp_dir).await;
    add_submodule_with_clean_filter(&repo_path).await;

    assert_eq!(get_has_changes(&repo_path).await, Some(false));
    assert!(!configured_filter_ran(&repo_path.join("nested")).await);
}

#[tokio::test]
async fn git_diff_to_remote_does_not_enter_dirty_submodules() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo_path = create_test_git_repo(&temp_dir).await;
    add_submodule_with_clean_filter(&repo_path).await;
    run_git(&repo_path, &["config", "diff.submodule", "diff"]).await;
    let remote_path = temp_dir.path().join("remote.git");
    add_origin_and_push(&repo_path, &remote_path).await;

    assert!(git_diff_to_remote(&repo_path).await.is_some());
    assert!(!configured_filter_ran(&repo_path.join("nested")).await);
}

#[tokio::test]
async fn git_diff_to_remote_rejects_configured_clean_filter_without_running_it() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo_path = create_test_git_repo(&temp_dir).await;
    let remote_path = temp_dir.path().join("remote.git");
    let branch = add_origin_and_push(&repo_path, &remote_path).await;

    configure_clean_filter(&repo_path, "test.txt").await;
    run_git(&repo_path, &["push", "origin", &branch]).await;

    assert!(git_diff_to_remote(&repo_path).await.is_none());
    assert!(!configured_filter_ran(&repo_path).await);
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
    assert_eq!(error.kind(), io::ErrorKind::Unsupported);
    assert!(!marker.exists(), "apply must not run filter");
    assert_eq!(
        std::fs::read_to_string(repo_path.join("test.txt")).expect("read tracked file"),
        "old\n"
    );

    let error = stage_paths(&repo_path, diff).expect_err("reject filter during staging");
    assert_eq!(error.kind(), io::ErrorKind::Unsupported);
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
                ("GIT_CONFIG_GLOBAL", global_config.as_os_str()),
                ("GIT_CONFIG_SYSTEM", system_config.as_os_str()),
                ("PATH", search_path.as_os_str()),
            ],
        );
        assert!(!marker.exists(), "repository git-lfs must not run");
        return;
    }

    let repo_path =
        PathBuf::from(std::env::var_os("CODEX_GIT_UTILS_TARGET_REPO").expect("target repository"));
    assert_eq!(get_has_changes(&repo_path.join("nested")).await, None);
}

#[cfg(unix)]
#[tokio::test]
async fn status_and_diff_reject_global_relative_filter_without_running_it() {
    if std::env::var_os("CODEX_GIT_UTILS_SAFE_GIT_ENV_CHILD").is_none() {
        let config_dir = tempfile::tempdir().expect("config tempdir");
        let global_config = config_dir.path().join("global.gitconfig");
        let system_config = config_dir.path().join("system.gitconfig");
        std::fs::write(&global_config, "[filter \"evil\"]\n\tclean = ./clean.sh\n")
            .expect("write global config");
        std::fs::write(&system_config, "").expect("write system config");
        run_isolated_test(
            "safe_git::tests::status_and_diff_reject_global_relative_filter_without_running_it",
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
    let remote_path = temp_dir.path().join("remote.git");
    add_origin_and_push(&repo_path, &remote_path).await;
    std::fs::write(repo_path.join(".gitattributes"), "test.txt filter=evil\n").expect("attributes");
    let marker = repo_path.join("filter-ran");
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
        &["-c", "filter.evil.clean=", "add", ".gitattributes"],
    )
    .await;
    run_git(
        &repo_path,
        &["-c", "filter.evil.clean=", "commit", "-m", "attributes"],
    )
    .await;

    assert_eq!(get_has_changes(&repo_path).await, None);
    assert!(git_diff_to_remote(&repo_path).await.is_none());
    assert!(!marker.exists(), "relative global filter must not run");
}
