#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;

use pretty_assertions::assert_eq;
use tokio::process::Command;

use super::*;

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
    let repo = init_repo(&temp_dir, 0).await;
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
    let task =
        tokio::spawn(async move { prepare_status_filter_guard(&task_git, &task_repo).await });

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
    let repo = init_repo(&temp_dir, 0).await;
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
    let (root, guard) = prepare_status_filter_guard(&git, &repo)
        .await
        .expect("prepare status guard");
    std::fs::write(repo.join(".gitattributes"), "test.txt filter=race\n")
        .expect("select filter after probe");

    let mut command = git
        .async_command_for_cwd(&root)
        .expect("authorized status cwd");
    command
        .args(["-c", &format!("core.hooksPath={DISABLED_HOOKS_PATH}")])
        .args(["-c", "core.fsmonitor=false"])
        .args(guard.git_config_args())
        .args(["status", "--porcelain"]);
    let output = git.output_async(command).await.expect("run guarded status");

    assert!(output.status.success(), "guarded status succeeds");
    assert!(!filter_marker_is_set(&repo, "codex.race-ran").await);
}

#[tokio::test]
async fn optional_smudge_only_filter_is_allowed_but_required_is_refused() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo = init_repo(&temp_dir, 0).await;
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
    prepare_status_filter_guard(&git, &repo)
        .await
        .expect("allow optional smudge-only filter");
    assert!(!filter_marker_is_set(&repo, "codex.smudge-ran").await);

    run_git(&repo, &["config", "filter.smudge-only.required", "true"]).await;
    assert_eq!(
        prepare_status_filter_guard(&git, &repo).await.map(|_| ()),
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
    let repo = init_repo(&temp_dir, 2).await;
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

    prepare_status_filter_guard(&git, &repo)
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
    let repo = init_repo(&temp_dir, 0).await;
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

    prepare_status_filter_guard(&git, &repo)
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
        Box::pin(prepare_status_filter_guard_within_deadline(&git, &repo)),
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
    let repo = init_repo(&temp_dir, 24).await;
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

    prepare_status_filter_guard(&git, &repo)
        .await
        .expect("prepare ordinary-driver guard");
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
    assert!(!filter_marker_is_set(&repo, "codex.ordinary-filter-ran").await);
}
