#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;

use pretty_assertions::assert_eq;
use tokio::process::Command;

use super::*;
use crate::safe_git::SentinelFilterProbeBudget;

async fn run_git(cwd: &Path, args: &[&str]) -> std::process::Output {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .await
        .expect("run Git");
    assert!(
        output.status.success(),
        "git {args:?} failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

async fn run_git_without_attr_source(cwd: &Path, args: &[&str]) -> std::process::Output {
    let output = Command::new("git")
        .env_remove("GIT_ATTR_SOURCE")
        .args(args)
        .current_dir(cwd)
        .output()
        .await
        .expect("run Git without GIT_ATTR_SOURCE");
    assert!(
        output.status.success(),
        "git {args:?} without GIT_ATTR_SOURCE failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

async fn init_repo(temp_dir: &tempfile::TempDir, file_count: usize) -> PathBuf {
    let repo = temp_dir.path().join("repo");
    std::fs::create_dir(&repo).expect("create repository");
    run_git(&repo, &["init", "-q"]).await;
    run_git(&repo, &["config", "user.name", "Test User"]).await;
    run_git(&repo, &["config", "user.email", "test@example.com"]).await;
    for index in 0..file_count {
        std::fs::write(repo.join(format!("file-{index}.txt")), "contents\n")
            .expect("write tracked file");
    }
    if file_count == 0 {
        std::fs::write(repo.join("test.txt"), "contents\n").expect("write tracked file");
    }
    run_git(&repo, &["add", "."]).await;
    run_git(&repo, &["commit", "-m", "seed"]).await;
    repo
}

fn real_git() -> PathBuf {
    let output = std::process::Command::new("/bin/sh")
        .args(["-c", "command -v git"])
        .output()
        .expect("resolve Git");
    assert!(output.status.success(), "resolve Git");
    PathBuf::from(
        String::from_utf8(output.stdout)
            .expect("Git path UTF-8")
            .trim(),
    )
}

fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}

fn write_wrapper(path: &Path, body: &str) {
    std::fs::write(path, body).expect("write Git wrapper");
    let mut permissions = std::fs::metadata(path)
        .expect("read wrapper metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).expect("mark wrapper executable");
}

fn run_isolated_config_test(test_name: &str) {
    let environment = tempfile::tempdir().expect("isolated Git environment");
    let global_config = environment.path().join("global.gitconfig");
    let system_config = environment.path().join("system.gitconfig");
    std::fs::write(&global_config, "").expect("empty global config");
    std::fs::write(&system_config, "").expect("empty system config");

    let mut command = std::process::Command::new(std::env::current_exe().expect("test binary"));
    crate::safe_git::isolate_git_command_environment(&mut command);
    let output = command
        .arg(test_name)
        .arg("--exact")
        .arg("--nocapture")
        .env("CODEX_GIT_UTILS_STATUS_GUARD_ENV_CHILD", "1")
        .env("GIT_CONFIG_GLOBAL", &global_config)
        .env("GIT_CONFIG_SYSTEM", &system_config)
        .env("GIT_CONFIG_NOSYSTEM", "0")
        .env("GIT_CONFIG_COUNT", "0")
        .env_remove("GIT_CONFIG_PARAMETERS")
        .env("RUST_TEST_THREADS", "1")
        .output()
        .expect("run isolated test process");
    assert!(
        output.status.success(),
        "isolated test {test_name} failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn run_isolated_environment_test(
    test_name: &str,
    marker: &str,
    variables: &[(&str, &str)],
    global_attributes: Option<&str>,
) {
    let environment = tempfile::tempdir().expect("isolated Git environment");
    let global_config = environment.path().join("global.gitconfig");
    let system_config = environment.path().join("system.gitconfig");
    let home = environment.path().join("home");
    let xdg = environment.path().join("xdg");
    std::fs::create_dir_all(xdg.join("git")).expect("create isolated XDG Git directory");
    std::fs::create_dir_all(&home).expect("create isolated HOME");
    std::fs::write(&global_config, "").expect("empty global config");
    std::fs::write(&system_config, "").expect("empty system config");
    if let Some(contents) = global_attributes {
        std::fs::write(xdg.join("git/attributes"), contents)
            .expect("write isolated global attributes");
    }

    let mut command = std::process::Command::new(std::env::current_exe().expect("test binary"));
    crate::safe_git::isolate_git_command_environment(&mut command);
    command
        .arg(test_name)
        .arg("--exact")
        .arg("--nocapture")
        .env(marker, "1")
        .env("GIT_CONFIG_GLOBAL", &global_config)
        .env("GIT_CONFIG_SYSTEM", &system_config)
        .env("GIT_CONFIG_NOSYSTEM", "0")
        .env("GIT_CONFIG_COUNT", "0")
        .env("GIT_ATTR_NOSYSTEM", "0")
        .env("HOME", &home)
        .env("XDG_CONFIG_HOME", &xdg)
        .env_remove("GIT_CONFIG_PARAMETERS")
        .env("RUST_TEST_THREADS", "1");
    for (name, value) in variables {
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

async fn guarded_status(repo: &Path, git: &GitRunner) -> Result<bool, GitReadError> {
    let mut guard = prepare_status_config(git, repo).await?;
    detect_status_fsmonitor(&mut guard).await;
    read_status(&guard).await
}

async fn replacement_fixture(repo: &Path) -> (String, String) {
    let original = String::from_utf8(run_git(repo, &["rev-parse", "HEAD"]).await.stdout)
        .expect("original commit UTF-8")
        .trim()
        .to_string();
    std::fs::write(repo.join("test.txt"), "replacement\n").expect("write replacement contents");
    run_git(repo, &["add", "test.txt"]).await;
    run_git(repo, &["commit", "-m", "replacement commit"]).await;
    let replacement = String::from_utf8(run_git(repo, &["rev-parse", "HEAD"]).await.stdout)
        .expect("replacement commit UTF-8")
        .trim()
        .to_string();
    run_git(repo, &["checkout", "--detach", &original]).await;
    (original, replacement)
}

#[test]
fn io_error_mapping_preserves_authority_provenance() {
    for error in [
        std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "ordinary filesystem denial",
        ),
        std::io::Error::new(std::io::ErrorKind::NotFound, "ordinary missing file"),
    ] {
        assert_eq!(
            map_io_error("filterAttributes", error),
            GitReadError::CommandFailed {
                operation: "filterAttributes".to_string(),
                exit_code: None,
            }
        );
    }

    assert_eq!(
        map_io_error(
            "resolveGitRoot",
            crate::repository_authority::authority_refusal("policy refusal"),
        ),
        GitReadError::AuthorityRefused {
            operation: "resolveGitRoot".to_string(),
        }
    );
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn process_relative_config_policy_maps_to_authority_refusal() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo = init_repo(&temp_dir, /*file_count*/ 0).await;
    let git = GitRunner::for_cwd(&repo).expect("trusted Git");
    let error = git
        .ensure_config_source_is_not_worktree_controlled(
            Path::new("/proc/self/cwd/config"),
            "Git config include",
        )
        .expect_err("process-relative config must be refused");

    assert_eq!(
        map_io_error("statusFilterPreparation", error),
        GitReadError::AuthorityRefused {
            operation: "statusFilterPreparation".to_string(),
        }
    );
}

#[tokio::test]
async fn dropping_status_preparation_kills_the_active_git_child() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo = init_repo(&temp_dir, /*file_count*/ 0).await;
    let wrapper = temp_dir.path().join("git-wrapper");
    let pid_file = wrapper.with_extension("pid");
    let blocker = wrapper.with_extension("block");
    let completed = wrapper.with_extension("completed");
    let mkfifo = std::process::Command::new("mkfifo")
        .arg(&blocker)
        .status()
        .expect("create blocking FIFO");
    assert!(mkfifo.success(), "create blocking FIFO");
    write_wrapper(
        &wrapper,
        "#!/bin/sh\n\
         printf '%s\\n' \"$$\" >\"$0.pid\"\n\
         IFS= read -r line <\"$0.block\"\n\
         : >\"$0.completed\"\n\
         exit 0\n",
    );
    let git = std::sync::Arc::new(
        GitRunner::from_executable_for_test(&repo, wrapper).expect("test Git runner"),
    );
    let task_git = std::sync::Arc::clone(&git);
    let task_repo = repo.clone();
    let task = tokio::spawn(async move {
        prepare_status_config(&task_git, &task_repo)
            .await
            .map(|_| ())
    });

    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while !pid_file.exists() {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("status preparation should launch its first Git child");
    let pid = std::fs::read_to_string(&pid_file)
        .expect("read direct Git child PID")
        .trim()
        .to_string();
    task.abort();
    let error = match task.await {
        Err(error) => error,
        Ok(_) => panic!("caller cancellation should abort status preparation"),
    };
    assert!(error.is_cancelled(), "{error}");

    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let alive = std::process::Command::new("/bin/kill")
                .args(["-0", &pid])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .expect("probe direct Git child PID")
                .success();
            if !alive {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("direct Git child should disappear after caller cancellation");
    assert!(
        !completed.exists(),
        "direct Git child completed instead of being cancelled"
    );
}

async fn filter_marker_is_set(repo: &Path, key: &str) -> bool {
    Command::new("git")
        .args(["config", "--get", key])
        .current_dir(repo)
        .status()
        .await
        .expect("read filter marker")
        .success()
}

#[tokio::test]
async fn prepared_guard_neutralizes_filter_selected_after_the_probe() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo = init_repo(&temp_dir, /*file_count*/ 0).await;
    run_git(
        &repo,
        &[
            "config",
            "filter.race.clean",
            "git config codex.race-ran true && git hash-object --stdin",
        ],
    )
    .await;

    let git = GitRunner::for_cwd(&repo).expect("trusted Git");
    let mut guard = prepare_status_config(&git, &repo)
        .await
        .expect("prepare status guard");
    std::fs::write(repo.join(".gitattributes"), "test.txt filter=race\n")
        .expect("select filter after probe");

    detect_status_fsmonitor(&mut guard).await;
    assert!(read_status(&guard).await.is_ok(), "guarded status succeeds");
    assert!(!filter_marker_is_set(&repo, "codex.race-ran").await);
}

#[tokio::test]
async fn frozen_status_blocks_a_new_filter_namespace_and_attribute_after_preparation() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo = init_repo(&temp_dir, /*file_count*/ 0).await;
    let git = GitRunner::for_cwd(&repo).expect("trusted Git");
    let mut guard = prepare_status_config(&git, &repo)
        .await
        .expect("prepare zero-driver status guard");

    run_git(
        &repo,
        &[
            "config",
            "filter.fresh.clean",
            "git config codex.fresh-ran true && git hash-object --stdin",
        ],
    )
    .await;
    std::fs::write(repo.join(".gitattributes"), "test.txt filter=fresh\n")
        .expect("select new filter namespace after preparation");
    std::fs::write(repo.join("test.txt"), "changed\n").expect("modify tracked path");

    detect_status_fsmonitor(&mut guard).await;
    assert_eq!(read_status(&guard).await, Ok(true));
    assert!(!filter_marker_is_set(&repo, "codex.fresh-ran").await);
}

#[tokio::test]
async fn frozen_status_preserves_info_global_core_and_system_attribute_selection() {
    const MARKER: &str = "CODEX_GIT_UTILS_STATUS_ATTRIBUTE_ENV_CHILD";
    if std::env::var_os(MARKER).is_none() {
        run_isolated_environment_test(
            "status_guard::tests::frozen_status_preserves_info_global_core_and_system_attribute_selection",
            MARKER,
            &[],
            Some("global.txt ident\n"),
        );
        return;
    }

    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo = init_repo(&temp_dir, /*file_count*/ 0).await;
    let core_attributes = temp_dir.path().join("core.attributes");
    std::fs::write(&core_attributes, "core.txt ident\n").expect("write core attributes file");
    std::fs::write(repo.join(".git/info/attributes"), "info.txt ident\n")
        .expect("write info attributes");
    for name in ["global.txt", "core.txt", "info.txt", "literal.txt"] {
        std::fs::write(repo.join(name), "$Id$\n").expect("write source-parity fixture");
    }
    run_git(&repo, &["add", "."]).await;
    run_git(&repo, &["commit", "-m", "attribute source fixtures"]).await;
    for name in ["global.txt", "info.txt"] {
        std::fs::remove_file(repo.join(name)).expect("remove source-parity fixture");
        run_git(&repo, &["checkout", "--", name]).await;
        assert!(
            std::fs::read_to_string(repo.join(name))
                .expect("read expanded ident fixture")
                .starts_with("$Id: "),
            "{name} did not select its attribute source"
        );
    }
    let stock = run_git(&repo, &["status", "--porcelain"]).await;
    assert!(
        stock.stdout.is_empty(),
        "global/info source-parity fixture must be clean under stock Git: {}",
        String::from_utf8_lossy(&stock.stdout),
    );

    let wrapper = temp_dir.path().join("git-wrapper");
    let log = temp_dir.path().join("git-wrapper.log");
    let real_git = shell_quote(&real_git());
    write_wrapper(
        &wrapper,
        &format!(
            "#!/bin/sh\nprintf '%s|%s|%s|%s\\n' \"${{GIT_ATTR_NOSYSTEM-unset}}\" \"${{HOME-unset}}\" \"${{XDG_CONFIG_HOME-unset}}\" \"$*\" >>{}\nexec {real_git} \"$@\"\n",
            shell_quote(&log),
        ),
    );
    let git = GitRunner::from_executable_for_test(&repo, wrapper).expect("test Git runner");
    let mut global_info_guard = prepare_status_config(&git, &repo)
        .await
        .expect("prepare global/info source-parity guard");
    detect_status_fsmonitor(&mut global_info_guard).await;
    let global_info_output = global_info_guard
        .status_output_async()
        .await
        .expect("run global/info source-parity status");
    assert!(global_info_output.status.success());
    assert!(
        global_info_output.stdout.is_empty(),
        "isolated global/info source parity diverged: {}",
        String::from_utf8_lossy(&global_info_output.stdout)
    );
    assert!(
        !global_info_guard
            .status_has_untracked_snapshot()
            .expect("untracked snapshot")
    );

    let home = std::env::var("HOME").expect("isolated HOME");
    let xdg = std::env::var("XDG_CONFIG_HOME").expect("isolated XDG config home");
    let log_contents = std::fs::read_to_string(&log).expect("read attribute environment log");
    let final_status = log_contents
        .lines()
        .find(|line| line.contains(" status --porcelain"))
        .expect("final status environment");
    assert!(
        final_status.starts_with(&format!("0|{home}|{xdg}|")),
        "captured system/global attribute selectors were not restored: {final_status}"
    );

    run_git(
        &repo,
        &["config", "core.attributesFile", "../core.attributes"],
    )
    .await;
    std::fs::remove_file(repo.join("global.txt")).expect("remove global attribute fixture");
    run_git(&repo, &["checkout", "--", "global.txt"]).await;
    std::fs::remove_file(repo.join("core.txt")).expect("remove core attribute fixture");
    run_git(&repo, &["checkout", "--", "core.txt"]).await;
    assert!(
        std::fs::read_to_string(repo.join("core.txt"))
            .expect("read expanded core ident fixture")
            .starts_with("$Id: "),
        "relative core.attributesFile did not select ident"
    );
    let stock = run_git(&repo, &["status", "--porcelain"]).await;
    assert!(
        stock.stdout.is_empty(),
        "core.attributesFile fixture must be clean under stock Git: {}",
        String::from_utf8_lossy(&stock.stdout)
    );
    let mut core_guard = prepare_status_config(&git, &repo)
        .await
        .expect("prepare core.attributesFile source-parity guard");
    detect_status_fsmonitor(&mut core_guard).await;
    let core_output = core_guard
        .status_output_async()
        .await
        .expect("run core.attributesFile source-parity status");
    assert!(core_output.status.success());
    assert!(
        core_output.stdout.is_empty(),
        "isolated core.attributesFile source parity diverged: {}",
        String::from_utf8_lossy(&core_output.stdout)
    );
    assert!(
        !core_guard
            .status_has_untracked_snapshot()
            .expect("untracked snapshot")
    );

    std::fs::write(
        repo.join(".git/info/attributes"),
        "info.txt ident\nliteral.txt ident=set\n",
    )
    .expect("write literal sentinel info attribute");
    std::fs::write(repo.join("literal.txt"), "$Id: fake $\n")
        .expect("write literal-sentinel ident fixture");
    assert_eq!(
        guarded_status(&repo, &git).await,
        Ok(true),
        "literal ident=set must not be reserialized as special Set"
    );
}

#[tokio::test]
async fn frozen_status_refuses_git_attr_source_and_attr_tree() {
    const MARKER: &str = "CODEX_GIT_UTILS_STATUS_ATTR_SOURCE_CHILD";
    if std::env::var_os(MARKER).is_some() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let repo = temp_dir.path().join("repo");
        std::fs::create_dir(&repo).expect("create repository");
        run_git_without_attr_source(&repo, &["init", "-q"]).await;
        run_git_without_attr_source(&repo, &["config", "user.name", "Test User"]).await;
        run_git_without_attr_source(&repo, &["config", "user.email", "test@example.com"]).await;
        std::fs::write(repo.join("test.txt"), "contents\n").expect("write tracked file");
        run_git_without_attr_source(&repo, &["add", "test.txt"]).await;
        run_git_without_attr_source(&repo, &["commit", "-m", "seed"]).await;
        let git = GitRunner::for_cwd(&repo).expect("trusted Git");
        assert!(
            prepare_status_config(&git, &repo).await.is_err(),
            "GIT_ATTR_SOURCE must make frozen Status unavailable"
        );
        return;
    }

    run_isolated_environment_test(
        "status_guard::tests::frozen_status_refuses_git_attr_source_and_attr_tree",
        MARKER,
        &[("GIT_ATTR_SOURCE", "HEAD")],
        /*global_attributes*/ None,
    );

    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo = init_repo(&temp_dir, /*file_count*/ 0).await;
    run_git(&repo, &["config", "attr.tree", "HEAD"]).await;
    let git = GitRunner::for_cwd(&repo).expect("trusted Git");
    assert!(
        prepare_status_config(&git, &repo).await.is_err(),
        "attr.tree must make frozen Status unavailable"
    );
}

#[tokio::test]
async fn frozen_untracked_presence_honors_all_standard_ignore_sources() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo = init_repo(&temp_dir, /*file_count*/ 0).await;
    std::fs::write(repo.join(".gitignore"), "root-ignored.txt\n").expect("write root ignore");
    run_git(&repo, &["add", ".gitignore"]).await;
    run_git(&repo, &["commit", "-m", "ignore rules"]).await;
    std::fs::write(repo.join(".git/info/exclude"), "info-ignored.txt\n")
        .expect("write info exclude");
    let global_excludes = temp_dir.path().join("global-excludes");
    std::fs::write(&global_excludes, "global-ignored.txt\n").expect("write global excludes");
    run_git(
        &repo,
        &[
            "config",
            "core.excludesFile",
            global_excludes.to_str().expect("UTF-8 excludes path"),
        ],
    )
    .await;
    for name in ["root-ignored.txt", "info-ignored.txt", "global-ignored.txt"] {
        std::fs::write(repo.join(name), "ignored\n").expect("write ignored file");
    }

    let git = GitRunner::for_cwd(&repo).expect("trusted Git");
    let mut guard = prepare_status_config(&git, &repo)
        .await
        .expect("prepare ignored-only status");
    detect_status_fsmonitor(&mut guard).await;
    assert_eq!(read_status(&guard).await, Ok(false));

    std::fs::write(repo.join("visible.txt"), "visible\n").expect("write visible untracked file");
    run_git(&repo, &["config", "status.showUntrackedFiles", "no"]).await;
    let mut guard = prepare_status_config(&git, &repo)
        .await
        .expect("prepare visible-untracked status");
    detect_status_fsmonitor(&mut guard).await;
    assert_eq!(read_status(&guard).await, Ok(true));
}

#[tokio::test]
async fn frozen_status_reports_an_ordinary_unmerged_index_as_dirty() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo = init_repo(&temp_dir, /*file_count*/ 0).await;
    std::fs::write(repo.join(".gitattributes"), "* filter\n")
        .expect("write special filter attribute");
    run_git(&repo, &["add", ".gitattributes"]).await;
    run_git(&repo, &["commit", "-m", "special filter attribute"]).await;
    let base_branch = String::from_utf8(
        run_git(&repo, &["rev-parse", "--abbrev-ref", "HEAD"])
            .await
            .stdout,
    )
    .expect("base branch UTF-8")
    .trim()
    .to_string();
    run_git(&repo, &["checkout", "-b", "side"]).await;
    std::fs::write(repo.join("test.txt"), "side\n").expect("write side contents");
    run_git(&repo, &["commit", "-am", "side change"]).await;
    run_git(&repo, &["checkout", &base_branch]).await;
    std::fs::write(repo.join("test.txt"), "main\n").expect("write main contents");
    run_git(&repo, &["commit", "-am", "main change"]).await;
    let merge = Command::new("git")
        .args(["merge", "side"])
        .current_dir(&repo)
        .output()
        .await
        .expect("run conflicting merge");
    assert!(!merge.status.success(), "fixture must produce a conflict");
    let stages = run_git(&repo, &["ls-files", "--unmerged"]).await;
    assert!(
        stages.stdout.split(|byte| *byte == b'\n').count() >= 3,
        "fixture must retain multiple index stages"
    );
    run_git(
        &repo,
        &[
            "config",
            "filter.set.clean",
            "git config codex.unmerged-filter-ran true && git hash-object --stdin",
        ],
    )
    .await;

    let git = GitRunner::for_cwd(&repo).expect("trusted Git");
    assert_eq!(guarded_status(&repo, &git).await, Ok(true));
    assert!(!filter_marker_is_set(&repo, "codex.unmerged-filter-ran").await);
}

#[tokio::test]
async fn sentinel_pathspec_handles_opaque_tracked_path_bytes_without_helpers() {
    use std::ffi::OsString;
    #[cfg(not(target_os = "macos"))]
    use std::os::unix::ffi::OsStringExt;

    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo = init_repo(&temp_dir, /*file_count*/ 0).await;
    #[cfg(target_os = "macos")]
    let paths = vec![
        OsString::from("-leading.txt"),
        OsString::from("line\nbreak.txt"),
    ];
    // macOS filesystem APIs reject non-UTF-8 names before Git can observe
    // them. Other Unix targets exercise the raw-byte path end to end; the
    // constructor unit test covers byte preservation on every Unix target.
    #[cfg(not(target_os = "macos"))]
    let paths = vec![
        OsString::from("-leading.txt"),
        OsString::from("line\nbreak.txt"),
        OsString::from_vec(b"non-utf8-\xff.txt".to_vec()),
    ];
    for path in paths {
        std::fs::write(repo.join(&path), "opaque path\n")
            .unwrap_or_else(|error| panic!("write opaque tracked path {path:?}: {error}"));
    }
    std::fs::write(repo.join(".gitattributes"), "* filter\n")
        .expect("write special filter attribute");
    run_git(&repo, &["add", "."]).await;
    run_git(&repo, &["commit", "-m", "opaque tracked paths"]).await;
    run_git(
        &repo,
        &[
            "config",
            "filter.set.clean",
            "git config codex.opaque-filter-ran true && git hash-object --stdin",
        ],
    )
    .await;

    let git = GitRunner::for_cwd(&repo).expect("trusted Git");
    assert_eq!(guarded_status(&repo, &git).await, Ok(false));
    assert!(!filter_marker_is_set(&repo, "codex.opaque-filter-ran").await);
}

#[tokio::test]
async fn status_head_snapshot_distinguishes_valid_detached_unborn_and_corrupt_states() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo = init_repo(&temp_dir, /*file_count*/ 0).await;
    let commit = String::from_utf8(run_git(&repo, &["rev-parse", "HEAD"]).await.stdout)
        .expect("commit UTF-8")
        .trim()
        .to_string();
    let git = GitRunner::for_cwd(&repo).expect("trusted Git");
    prepare_status_config(&git, &repo)
        .await
        .expect("normal symbolic HEAD");

    run_git(&repo, &["checkout", "--detach", &commit]).await;
    prepare_status_config(&git, &repo)
        .await
        .expect("valid detached HEAD");
    run_git(&repo, &["checkout", "-B", "restored", &commit]).await;

    let ref_path = repo.join(".git/refs/heads/restored");
    std::fs::write(&ref_path, format!("{}\n", "a".repeat(40))).expect("write missing-object ref");
    assert!(
        prepare_status_config(&git, &repo).await.is_err(),
        "missing-object symbolic HEAD must not become unborn"
    );

    let blob = String::from_utf8(
        run_git(&repo, &["hash-object", "-w", "test.txt"])
            .await
            .stdout,
    )
    .expect("blob UTF-8")
    .trim()
    .to_string();
    std::fs::write(&ref_path, format!("{blob}\n")).expect("write blob ref");
    assert_eq!(
        prepare_status_config(&git, &repo).await.map(|_| ()),
        Err(GitReadError::CommandFailed {
            operation: "statusFilterPreparation".to_string(),
            exit_code: Some(1),
        })
    );

    let unborn = temp_dir.path().join("unborn");
    std::fs::create_dir(&unborn).expect("create unborn repo");
    run_git(&unborn, &["init", "-q"]).await;
    let unborn_git = GitRunner::for_cwd(&unborn).expect("unborn Git runner");
    let mut guard = prepare_status_config(&unborn_git, &unborn)
        .await
        .expect("genuine unborn HEAD");
    detect_status_fsmonitor(&mut guard).await;
    assert_eq!(read_status(&guard).await, Ok(false));
}

#[tokio::test]
async fn frozen_status_refuses_active_or_custom_replacements_and_ignores_late_replacements() {
    const MARKER: &str = "CODEX_GIT_UTILS_STATUS_REPLACE_BASE_CHILD";
    if std::env::var_os(MARKER).is_some() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let repo = init_repo(&temp_dir, /*file_count*/ 0).await;
        let git = GitRunner::for_cwd(&repo).expect("trusted Git");
        assert!(
            prepare_status_config(&git, &repo).await.is_err(),
            "custom replacement-ref namespace must make frozen Status unavailable"
        );
        return;
    }
    run_isolated_environment_test(
        "status_guard::tests::frozen_status_refuses_active_or_custom_replacements_and_ignores_late_replacements",
        MARKER,
        &[("GIT_REPLACE_REF_BASE", "refs/codex-replacements/")],
        /*global_attributes*/ None,
    );

    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo = init_repo(&temp_dir, /*file_count*/ 0).await;
    let (original, replacement) = replacement_fixture(&repo).await;
    let git = GitRunner::for_cwd(&repo).expect("trusted Git");
    let mut guard = prepare_status_config(&git, &repo)
        .await
        .expect("prepare before replacement ref exists");
    run_git(&repo, &["replace", &original, &replacement]).await;
    let stock = run_git(&repo, &["status", "--porcelain"]).await;
    assert!(
        !stock.stdout.is_empty(),
        "replacement fixture must make stock Status dirty"
    );
    detect_status_fsmonitor(&mut guard).await;
    assert_eq!(
        read_status(&guard).await,
        Ok(false),
        "late replacement must not alter the frozen no-replacement view"
    );
    assert!(
        prepare_status_config(&git, &repo).await.is_err(),
        "an active default replacement must make frozen Status unavailable"
    );
}

#[tokio::test]
async fn optional_smudge_only_filter_is_allowed_but_required_is_refused() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo = init_repo(&temp_dir, /*file_count*/ 0).await;
    std::fs::write(repo.join(".gitattributes"), "test.txt filter=smudge-only\n")
        .expect("select smudge filter");
    run_git(&repo, &["add", ".gitattributes"]).await;
    run_git(&repo, &["commit", "-m", "attributes"]).await;
    run_git(
        &repo,
        &[
            "config",
            "filter.smudge-only.smudge",
            "git config codex.smudge-ran true && cat",
        ],
    )
    .await;

    let git = GitRunner::for_cwd(&repo).expect("trusted Git");
    prepare_status_config(&git, &repo)
        .await
        .expect("allow optional smudge-only filter");
    assert!(!filter_marker_is_set(&repo, "codex.smudge-ran").await);

    run_git(&repo, &["config", "filter.smudge-only.required", "true"]).await;
    assert_eq!(
        prepare_status_config(&git, &repo).await.map(|_| ()),
        Err(GitReadError::SelectedExecutableFilter {
            driver: "smudge-only".to_string(),
            path: "test.txt".to_string(),
        })
    );
    assert!(!filter_marker_is_set(&repo, "codex.smudge-ran").await);
}

#[tokio::test]
async fn selected_required_only_filter_is_typed_and_refused_when_truthy() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo = init_repo(&temp_dir, /*file_count*/ 0).await;
    std::fs::write(
        repo.join(".gitattributes"),
        "test.txt filter=required-only\n",
    )
    .expect("select required-only filter");
    run_git(&repo, &["add", ".gitattributes"]).await;
    run_git(&repo, &["commit", "-m", "required-only attribute"]).await;

    run_git(&repo, &["config", "filter.required-only.required", "false"]).await;
    let git = GitRunner::for_cwd(&repo).expect("trusted Git");
    prepare_status_config(&git, &repo)
        .await
        .expect("selected required=false filter is inert");

    run_git(&repo, &["config", "filter.required-only.required", "true"]).await;
    assert_eq!(
        prepare_status_config(&git, &repo).await.map(|_| ()),
        Err(GitReadError::SelectedExecutableFilter {
            driver: "required-only".to_string(),
            path: "test.txt".to_string(),
        })
    );

    run_git(
        &repo,
        &["config", "filter.required-only.required", "not-a-bool"],
    )
    .await;
    assert!(
        prepare_status_config(&git, &repo).await.is_err(),
        "selected malformed required value must fail closed"
    );

    std::fs::write(
        repo.join(".gitattributes"),
        "other.txt filter=required-only\n",
    )
    .expect("move required-only filter off tracked paths");
    run_git(&repo, &["config", "filter.required-only.required", "true"]).await;
    prepare_status_config(&git, &repo)
        .await
        .expect("unselected required-only namespace remains available");
}

#[tokio::test]
async fn optional_smudge_required_value_is_queried_once_per_driver() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo = init_repo(&temp_dir, /*file_count*/ 2).await;
    std::fs::write(
        repo.join(".gitattributes"),
        "file-0.txt filter=smudge-only\nfile-1.txt filter=smudge-only\n",
    )
    .expect("select smudge filter for two paths");
    run_git(&repo, &["add", ".gitattributes"]).await;
    run_git(&repo, &["commit", "-m", "attributes"]).await;
    run_git(
        &repo,
        &[
            "config",
            "filter.smudge-only.smudge",
            "git config codex.smudge-ran true && cat",
        ],
    )
    .await;

    let wrapper = temp_dir.path().join("git-wrapper");
    let log = temp_dir.path().join("git-wrapper.log");
    let real_git = shell_quote(&real_git());
    write_wrapper(
        &wrapper,
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >>{}\nexec {real_git} \"$@\"\n",
            shell_quote(&log)
        ),
    );
    let git = GitRunner::from_executable_for_test(&repo, wrapper).expect("test Git runner");

    prepare_status_config(&git, &repo)
        .await
        .expect("allow optional smudge-only filter");
    let required_queries = std::fs::read_to_string(log)
        .expect("read wrapper log")
        .lines()
        .filter(|line| line.contains(" config --type=bool --get filter.smudge-only.required"))
        .count();
    assert_eq!(required_queries, 1);
    assert!(!filter_marker_is_set(&repo, "codex.smudge-ran").await);
}

#[tokio::test]
async fn sentinel_pathspec_refuses_a_raced_new_filter_namespace_without_helper() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo = init_repo(&temp_dir, /*file_count*/ 0).await;
    std::fs::write(repo.join(".gitattributes"), "test.txt filter\n")
        .expect("write sentinel attribute");
    run_git(&repo, &["add", ".gitattributes"]).await;
    run_git(&repo, &["commit", "-m", "sentinel attribute"]).await;
    run_git(
        &repo,
        &[
            "config",
            "filter.set.clean",
            "git config codex.probe-filter-ran true && git hash-object --stdin",
        ],
    )
    .await;

    let wrapper = temp_dir.path().join("git-wrapper");
    let real_git = shell_quote(&real_git());
    let attributes = shell_quote(&repo.join(".gitattributes"));
    write_wrapper(
        &wrapper,
        &format!(
            "#!/bin/sh\nfor arg in \"$@\"; do\n  if [ \"$arg\" = check-attr ]; then\n    {real_git} \"$@\" >\"$0.output\"\n    status=$?\n    {real_git} config filter.evil.clean 'git config codex.probe-filter-ran true && git hash-object --stdin'\n    printf 'test.txt filter=evil\\n' >{attributes}\n    /bin/cat \"$0.output\"\n    /bin/rm -f \"$0.output\"\n    exit $status\n  fi\ndone\nexec {real_git} \"$@\"\n"
        ),
    );
    let git = GitRunner::from_executable_for_test(&repo, wrapper).expect("test Git runner");

    assert_eq!(
        prepare_status_config(&git, &repo).await.map(|_| ()),
        Err(GitReadError::SelectedExecutableFilter {
            driver: "set".to_string(),
            path: "test.txt".to_string(),
        })
    );
    assert!(!filter_marker_is_set(&repo, "codex.probe-filter-ran").await);
}

#[tokio::test]
async fn sentinel_probe_budget_fails_closed_at_the_exact_process_limit() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let max_probes = SentinelFilterProbeBudget::max_probes();
    let repo = init_repo(&temp_dir, max_probes + 4).await;
    run_git(
        &repo,
        &[
            "config",
            "filter.unspecified.clean",
            "git config codex.budget-filter-ran true && git hash-object --stdin",
        ],
    )
    .await;

    let wrapper = temp_dir.path().join("git-wrapper");
    let log = temp_dir.path().join("git-wrapper.log");
    let real_git = shell_quote(&real_git());
    write_wrapper(
        &wrapper,
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >>{}\nexec {real_git} \"$@\"\n",
            shell_quote(&log)
        ),
    );
    let git = GitRunner::from_executable_for_test(&repo, wrapper).expect("test Git runner");

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        Box::pin(prepare_status_config(&git, &repo)),
    )
    .await
    .expect("sentinel budget test should complete");
    assert_eq!(
        result.map(|_| ()),
        Err(GitReadError::FilterSelectionProbeLimitExceeded { max_probes })
    );
    let probe_count = std::fs::read_to_string(log)
        .expect("read wrapper log")
        .lines()
        .filter(|line| {
            line.contains(" ls-files --cached --full-name -z -- :(top,literal,attr:!filter)")
        })
        .count();
    assert_eq!(probe_count, max_probes);
    assert!(!filter_marker_is_set(&repo, "codex.budget-filter-ran").await);
}

#[tokio::test]
async fn ordinary_driver_uses_one_bulk_path_and_attribute_probe() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo = init_repo(&temp_dir, /*file_count*/ 24).await;
    run_git(
        &repo,
        &[
            "config",
            "filter.ordinary.clean",
            "git config codex.ordinary-filter-ran true && git hash-object --stdin",
        ],
    )
    .await;

    let wrapper = temp_dir.path().join("git-wrapper");
    let log = temp_dir.path().join("git-wrapper.log");
    let real_git = shell_quote(&real_git());
    write_wrapper(
        &wrapper,
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >>{}\nexec {real_git} \"$@\"\n",
            shell_quote(&log)
        ),
    );
    let git = GitRunner::from_executable_for_test(&repo, wrapper).expect("test Git runner");

    let mut config = prepare_status_config(&git, &repo)
        .await
        .expect("prepare ordinary-driver guard");
    detect_status_fsmonitor(&mut config).await;
    read_status(&config).await.expect("final guarded status");
    let lines = std::fs::read_to_string(log).expect("read wrapper log");
    assert_eq!(
        lines
            .lines()
            .filter(|line| line.contains(" ls-files -z --cached"))
            .count(),
        1
    );
    assert_eq!(
        lines
            .lines()
            .filter(|line| line.contains(" check-attr --stdin -z filter"))
            .count(),
        1
    );
    assert_eq!(
        lines
            .lines()
            .filter(|line| line.contains(" hash-object --stdin --path "))
            .count(),
        0
    );
    let status = lines
        .lines()
        .find(|line| line.contains(" status --porcelain --ignore-submodules=dirty"))
        .expect("final status command");
    let hooks = status
        .find("core.hooksPath=")
        .expect("hooks override before status");
    let fsmonitor = status
        .find("core.fsmonitor=")
        .expect("fsmonitor override before status");
    let status_command = status
        .find(" status --porcelain")
        .expect("status subcommand");
    assert!(hooks < fsmonitor && fsmonitor < status_command);
    assert!(!status.contains("include.path="));
    assert!(status.ends_with("--untracked-files=no"));
    assert!(!filter_marker_is_set(&repo, "codex.ordinary-filter-ran").await);
}

#[tokio::test]
async fn fsmonitor_probe_is_base_only_and_the_sealed_decision_controls_final_order() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo = init_repo(&temp_dir, /*file_count*/ 0).await;
    run_git(&repo, &["config", "core.fsmonitor", "true"]).await;

    let wrapper = temp_dir.path().join("git-wrapper");
    let log = temp_dir.path().join("git-wrapper.log");
    let real_git = shell_quote(&real_git());
    write_wrapper(
        &wrapper,
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >>{}\ncase \" $* \" in *\" version --build-options \"*) printf 'feature: fsmonitor--daemon\\n'; exit 0 ;; esac\nexec {real_git} \"$@\"\n",
            shell_quote(&log)
        ),
    );
    let git = GitRunner::from_executable_for_test(&repo, wrapper).expect("test Git runner");
    let mut config = prepare_status_config(&git, &repo)
        .await
        .expect("prepare status capability");
    std::fs::write(&log, "").expect("clear preparation log");

    assert_eq!(
        detect_status_fsmonitor(&mut config).await,
        FsmonitorOverride::Disabled
    );
    assert_eq!(
        detect_status_fsmonitor(&mut config).await,
        FsmonitorOverride::Disabled,
        "the retained decision must avoid a second probe"
    );
    read_status(&config).await.expect("final status");

    let lines = std::fs::read_to_string(&log)
        .expect("read wrapper log")
        .lines()
        .map(str::to_string)
        .collect::<Vec<_>>();
    assert_eq!(
        lines.len(),
        1,
        "synthetic context must not probe or start fsmonitor"
    );
    let hooks = format!("core.hooksPath={}", crate::safe_git::DISABLED_HOOKS_PATH);
    let hooks_offset = lines[0].find(&hooks).expect("hooks override");
    let fsmonitor_offset = lines[0]
        .find("core.fsmonitor=false")
        .expect("disabled fsmonitor override");
    let status_offset = lines[0].find("status --porcelain").expect("status command");
    assert!(hooks_offset < fsmonitor_offset && fsmonitor_offset < status_offset);
    assert!(
        lines[0].ends_with("status --porcelain --ignore-submodules=dirty --untracked-files=no")
    );
}

#[tokio::test]
async fn zero_driver_status_builds_the_same_frozen_context_without_an_overlay() {
    if std::env::var_os("CODEX_GIT_UTILS_STATUS_GUARD_ENV_CHILD").is_none() {
        run_isolated_config_test(
            "status_guard::tests::zero_driver_status_builds_the_same_frozen_context_without_an_overlay",
        );
        return;
    }
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo = init_repo(&temp_dir, /*file_count*/ 0).await;
    let wrapper = temp_dir.path().join("git-wrapper");
    let log = temp_dir.path().join("git-wrapper.log");
    let real_git = shell_quote(&real_git());
    write_wrapper(
        &wrapper,
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >>{}\nexec {real_git} \"$@\"\n",
            shell_quote(&log)
        ),
    );
    let git = GitRunner::from_executable_for_test(&repo, wrapper).expect("test Git runner");

    prepare_status_config(&git, &repo)
        .await
        .expect("zero-driver status capability");
    let lines = std::fs::read_to_string(log).expect("read wrapper log");
    assert!(lines.contains(" ls-files -z --cached"), "{lines}");
    assert!(!lines.contains(" check-attr --stdin -z filter"), "{lines}");
    assert!(!lines.contains(" check-attr --stdin -z"), "{lines}");
    assert!(lines.contains("config --file "), "{lines}");
    assert!(!lines.contains("include.path="), "{lines}");
}

#[tokio::test]
async fn status_driver_limit_fails_before_paths_or_overlay_writes() {
    use std::io::Write;

    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo = init_repo(&temp_dir, /*file_count*/ 0).await;
    let mut local_config = std::fs::OpenOptions::new()
        .append(true)
        .open(repo.join(".git/config"))
        .expect("open local config");
    for index in 0..=crate::safe_git::MAX_EXECUTABLE_FILTER_DRIVERS {
        writeln!(local_config, "[filter \"driver-{index}\"]\n\tclean = cat")
            .expect("append filter driver");
    }
    drop(local_config);

    let wrapper = temp_dir.path().join("git-wrapper");
    let log = temp_dir.path().join("git-wrapper.log");
    let real_git = shell_quote(&real_git());
    write_wrapper(
        &wrapper,
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >>{}\nexec {real_git} \"$@\"\n",
            shell_quote(&log)
        ),
    );
    let git = GitRunner::from_executable_for_test(&repo, wrapper).expect("test Git runner");
    let mut config = GuardedGitConfig::authorize_status_async(&git)
        .await
        .expect("authorized status capability");
    config
        .verify_status_root_async(&repo)
        .await
        .expect("matching root");
    std::fs::write(&log, "").expect("clear authorization log");

    let error = config
        .install_status_policy_async()
        .await
        .expect_err("257th driver must fail closed");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    let lines = std::fs::read_to_string(log).expect("read wrapper log");
    assert!(!lines.contains(" ls-files -z --cached"), "{lines}");
    assert!(!lines.contains(" check-attr --stdin -z filter"), "{lines}");
    assert!(!lines.contains(" config --file "), "{lines}");
}
