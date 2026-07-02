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
async fn sentinel_probe_neutralizes_a_race_selected_known_driver() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo = init_repo(&temp_dir, /*file_count*/ 0).await;
    std::fs::write(repo.join(".gitattributes"), "test.txt filter\n")
        .expect("write sentinel attribute");
    run_git(&repo, &["add", ".gitattributes"]).await;
    run_git(&repo, &["commit", "-m", "sentinel attribute"]).await;
    for driver in ["set", "race"] {
        run_git(
            &repo,
            &[
                "config",
                &format!("filter.{driver}.clean"),
                "git config codex.probe-filter-ran true && git hash-object --stdin",
            ],
        )
        .await;
    }

    let wrapper = temp_dir.path().join("git-wrapper");
    let real_git = shell_quote(&real_git());
    let attributes = shell_quote(&repo.join(".gitattributes"));
    write_wrapper(
        &wrapper,
        &format!(
            "#!/bin/sh\nfor arg in \"$@\"; do\n  if [ \"$arg\" = check-attr ]; then\n    {real_git} \"$@\" >\"$0.output\"\n    status=$?\n    printf 'test.txt filter=race\\n' >{attributes}\n    /bin/cat \"$0.output\"\n    /bin/rm -f \"$0.output\"\n    exit $status\n  fi\ndone\nexec {real_git} \"$@\"\n"
        ),
    );
    let git = GitRunner::from_executable_for_test(&repo, wrapper).expect("test Git runner");

    prepare_status_config(&git, &repo)
        .await
        .expect("probe remains non-executing across attribute race");
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
        .filter(|line| line.contains(" hash-object --stdin --path "))
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
    let filter = status
        .find("include.path=")
        .expect("sealed filter include before status");
    let status_command = status
        .find(" status --porcelain")
        .expect("status subcommand");
    assert!(hooks < fsmonitor && fsmonitor < filter && filter < status_command);
    assert_eq!(status.matches("include.path=").count(), 1);
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
        FsmonitorOverride::BuiltIn
    );
    assert_eq!(
        detect_status_fsmonitor(&mut config).await,
        FsmonitorOverride::BuiltIn,
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
        3,
        "one config probe, one feature probe, one status"
    );
    assert!(lines[0].ends_with("config --null --get core.fsmonitor"));
    assert!(!lines[0].contains("core.fsmonitor=false"));
    assert!(lines[1].ends_with("version --build-options"));
    assert!(!lines[1].contains("core.fsmonitor=false"));
    let hooks = format!("core.hooksPath={}", crate::safe_git::DISABLED_HOOKS_PATH);
    let hooks_offset = lines[2].find(&hooks).expect("hooks override");
    let fsmonitor_offset = lines[2]
        .find("core.fsmonitor=true")
        .expect("typed fsmonitor override");
    let status_offset = lines[2].find("status --porcelain").expect("status command");
    assert!(hooks_offset < fsmonitor_offset && fsmonitor_offset < status_offset);
    assert!(lines[2].ends_with("status --porcelain --ignore-submodules=dirty"));
}

#[tokio::test]
async fn zero_driver_status_skips_tracked_paths_attributes_and_overlay_writes() {
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
    assert!(!lines.contains(" ls-files -z --cached"), "{lines}");
    assert!(!lines.contains(" check-attr --stdin -z filter"), "{lines}");
    assert!(!lines.contains(" config --file "), "{lines}");
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
