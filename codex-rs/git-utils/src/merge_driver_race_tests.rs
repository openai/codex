use super::*;
use crate::apply::ApplyGitRequest;
use crate::apply::apply_git_patch;
use pretty_assertions::assert_eq;
use std::fs::OpenOptions;
use std::path::Path;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use std::time::Instant;

const OLD_DRIVER_COMMAND: &str = "git config codex.oldmergeran true && false";
const NEW_DRIVER_COMMAND: &str = "git config codex.newmergeran true && false";
const TRACE_MUTATION_MARKER: &str = "CODEX_MERGE_DRIVER_TEST_MUTATION";

fn run(cwd: &Path, args: &[&str]) -> (i32, String, String) {
    let mut command = std::process::Command::new("git");
    isolate_git_command_environment(&mut command);
    let output = command
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("run Git");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn run_success(cwd: &Path, args: &[&str]) -> String {
    let (code, stdout, stderr) = run(cwd, args);
    assert_eq!(code, 0, "git {args:?}: {stderr}");
    stdout
}

fn init_repo() -> tempfile::TempDir {
    let repo = tempfile::tempdir().expect("repo tempdir");
    let root = repo.path();
    run_success(root, &["init"]);
    run_success(root, &["config", "user.email", "codex@example.com"]);
    run_success(root, &["config", "user.name", "Codex"]);
    run_success(root, &["config", "core.autocrlf", "false"]);
    repo
}

fn configure_driver(root: &Path, driver: &str, command: &str) {
    run_success(
        root,
        &["config", &format!("merge.{driver}.driver"), command],
    );
}

fn configured_marker_exists(root: &Path, marker: &str) -> bool {
    run(root, &["config", "--get", marker]).0 == 0
}

fn build_conflicting_patch(root: &Path, attributes: &str) -> String {
    std::fs::write(root.join(".gitattributes"), attributes).expect("write base attributes");
    std::fs::write(root.join("target.txt"), "top\nbase\nbottom\n").expect("write base");
    run_success(root, &["add", ".gitattributes", "target.txt"]);
    run_success(root, &["commit", "-m", "base"]);
    let base = run_success(root, &["rev-parse", "HEAD"]);
    let base = base.trim();

    std::fs::write(root.join("target.txt"), "top\ntheirs\nbottom\n").expect("write theirs");
    run_success(root, &["add", "target.txt"]);
    run_success(root, &["commit", "-m", "theirs"]);
    let patch = run_success(
        root,
        &["diff", "--full-index", base, "HEAD", "--", "target.txt"],
    );

    run_success(root, &["checkout", "-b", "ours", base]);
    std::fs::write(root.join("target.txt"), "top\nours\nbottom\n").expect("write ours");
    run_success(root, &["add", "target.txt"]);
    run_success(root, &["commit", "-m", "ours"]);
    patch
}

fn request(root: &Path, diff: String, revert: bool) -> ApplyGitRequest {
    ApplyGitRequest {
        cwd: root.to_path_buf(),
        diff,
        revert,
        preflight: false,
    }
}

fn index_tree(root: &Path) -> String {
    run_success(root, &["write-tree"])
}

fn run_isolated_test(test_name: &str) {
    let environment = tempfile::tempdir().expect("isolated Git environment");
    let global_config = environment.path().join("global.gitconfig");
    let system_config = environment.path().join("system.gitconfig");
    let trace = environment.path().join("trace.jsonl");
    std::fs::write(&global_config, "").expect("empty global config");
    std::fs::write(&system_config, "").expect("empty system config");
    std::fs::write(&trace, "").expect("empty trace");

    let mut command = std::process::Command::new(std::env::current_exe().expect("test binary"));
    isolate_git_command_environment(&mut command);
    let output = command
        .arg(test_name)
        .arg("--exact")
        .arg("--nocapture")
        .env("CODEX_GIT_UTILS_MERGE_RACE_CHILD", "1")
        .env("GIT_CONFIG_GLOBAL", &global_config)
        .env("GIT_CONFIG_SYSTEM", &system_config)
        .env("GIT_TRACE2_EVENT", &trace)
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

fn trace_path() -> PathBuf {
    PathBuf::from(std::env::var_os("GIT_TRACE2_EVENT").expect("trace path"))
}

fn wait_for_merge_attribute_probe(trace: &Path) -> bool {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        let contents = std::fs::read_to_string(trace).unwrap_or_default();
        if contents
            .find("check-attr")
            .is_some_and(|offset| contents[offset..].contains("\"event\":\"exit\""))
        {
            return true;
        }
        thread::yield_now();
    }
    false
}

fn record_trace_mutation(trace: &Path) {
    let mut trace = OpenOptions::new()
        .append(true)
        .open(trace)
        .expect("open trace for mutation marker");
    writeln!(trace, "{TRACE_MUTATION_MARKER}").expect("record mutation marker");
}

fn assert_mutation_happened_between_probe_and_three_way(trace: &Path) {
    let contents = std::fs::read_to_string(trace).expect("read completed trace");
    let probe_start = contents.find("check-attr").expect("merge attribute probe");
    let probe_exit = probe_start
        + contents[probe_start..]
            .find("\"event\":\"exit\"")
            .expect("merge attribute probe exit");
    let mutation = contents
        .find(TRACE_MUTATION_MARKER)
        .expect("mutation trace marker");
    let three_way = contents.find("--3way").expect("final three-way apply");
    assert!(probe_exit < mutation, "mutation preceded probe exit");
    assert!(mutation < three_way, "final Git started before mutation");
}

fn append_driver_config(root: &Path, driver: &str, command: &str) {
    let mut config = OpenOptions::new()
        .append(true)
        .open(root.join(".git/config"))
        .expect("open local config");
    writeln!(config, "\n[merge \"{driver}\"]\n\tdriver = {command}")
        .expect("append replacement merge driver");
}

#[test]
fn clean_reverse_checks_worktree_before_staging_and_skips_merge_driver() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join(".gitattributes"), "file.txt merge=demo\n").expect("write attributes");
    std::fs::write(root.join("file.txt"), "old\n").expect("write old file");
    run_success(root, &["add", ".gitattributes", "file.txt"]);
    run_success(root, &["commit", "-m", "base"]);
    configure_driver(root, "demo", OLD_DRIVER_COMMAND);

    std::fs::write(root.join("file.txt"), "new\n").expect("write unstaged forward state");
    let patch = "diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n@@ -1 +1 @@\n-old\n+new\n";
    let result = apply_git_patch(&request(root, patch.to_string(), /*revert*/ true))
        .expect("clean reverse apply");

    assert_eq!(result.exit_code, 0, "{}", result.stderr);
    assert_eq!(result.applied_paths, vec!["file.txt"]);
    assert!(result.cmd_for_log.contains("--index"));
    assert!(!result.cmd_for_log.contains("--3way"));
    assert_eq!(
        std::fs::read_to_string(root.join("file.txt")).expect("read restored file"),
        "old\n"
    );
    assert!(!configured_marker_exists(root, "codex.oldmergeran"));
    assert!(run_success(root, &["status", "--porcelain"]).is_empty());
}

#[derive(Clone, Copy, Debug)]
enum ReverseTopology {
    Delete,
    Rename,
}

#[test]
fn clean_reverse_stages_missing_delete_and_rename_endpoints() {
    for topology in [ReverseTopology::Delete, ReverseTopology::Rename] {
        let repo = init_repo();
        let root = repo.path();
        std::fs::write(root.join(".gitattributes"), "*.txt merge=demo\n")
            .expect("write attributes");
        std::fs::write(root.join("old.txt"), "old\n").expect("write old file");
        run_success(root, &["add", ".gitattributes", "old.txt"]);
        run_success(root, &["commit", "-m", "base"]);
        configure_driver(root, "demo", OLD_DRIVER_COMMAND);

        match topology {
            ReverseTopology::Delete => {
                run_success(root, &["rm", "old.txt"]);
            }
            ReverseTopology::Rename => {
                run_success(root, &["mv", "old.txt", "new.txt"]);
            }
        }
        let patch = run_success(root, &["diff", "--cached", "--full-index", "--binary"]);
        run_success(root, &["reset", "--hard", "HEAD"]);
        match topology {
            ReverseTopology::Delete => {
                std::fs::remove_file(root.join("old.txt")).expect("delete worktree file");
            }
            ReverseTopology::Rename => {
                std::fs::rename(root.join("old.txt"), root.join("new.txt"))
                    .expect("rename worktree file");
            }
        }

        let result = apply_git_patch(&request(root, patch, /*revert*/ true))
            .expect("clean topology reverse");

        assert_eq!(result.exit_code, 0, "{topology:?}: {}", result.stderr);
        assert_eq!(result.applied_paths, vec!["old.txt"], "{topology:?}");
        assert!(result.cmd_for_log.contains("--index"), "{topology:?}");
        assert!(!result.cmd_for_log.contains("--3way"), "{topology:?}");
        assert_eq!(
            std::fs::read_to_string(root.join("old.txt")).expect("restored old file"),
            "old\n",
            "{topology:?}"
        );
        assert!(!root.join("new.txt").exists(), "{topology:?}");
        assert!(!configured_marker_exists(root, "codex.oldmergeran"));
        assert!(
            run_success(root, &["status", "--porcelain"]).is_empty(),
            "{topology:?}"
        );
    }
}

#[test]
fn selected_empty_and_equals_named_drivers_reject_without_marker_or_index_mutation() {
    for (driver, attribute) in [
        ("", "target.txt merge=\n"),
        ("x=y", "target.txt merge=x=y\n"),
    ] {
        let repo = init_repo();
        let root = repo.path();
        let patch = build_conflicting_patch(root, attribute);
        configure_driver(root, driver, OLD_DRIVER_COMMAND);
        let before_tree = index_tree(root);
        let before_contents =
            std::fs::read_to_string(root.join("target.txt")).expect("read before contents");

        let error = apply_git_patch(&request(root, patch, /*revert*/ false))
            .expect_err("reject selected merge driver");

        assert_eq!(error.kind(), io::ErrorKind::Unsupported, "{driver:?}");
        assert!(!configured_marker_exists(root, "codex.oldmergeran"));
        assert_eq!(index_tree(root), before_tree, "{driver:?}");
        assert_eq!(
            std::fs::read_to_string(root.join("target.txt")).expect("read after contents"),
            before_contents,
            "{driver:?}"
        );
        assert!(run_success(root, &["status", "--porcelain"]).is_empty());
    }
}

#[test]
fn post_probe_attribute_change_cannot_run_empty_named_driver() {
    if std::env::var_os("CODEX_GIT_UTILS_MERGE_RACE_CHILD").is_none() {
        run_isolated_test(
            "merge_driver::race_tests::post_probe_attribute_change_cannot_run_empty_named_driver",
        );
        return;
    }

    let repo = init_repo();
    let root = repo.path();
    let patch = build_conflicting_patch(root, "# initially safe\n");
    configure_driver(root, "", OLD_DRIVER_COMMAND);
    let trace = trace_path();
    std::fs::write(&trace, "").expect("clear fixture trace");
    let watcher_trace = trace.clone();
    let attributes = root.join(".gitattributes");
    let watcher = thread::spawn(move || {
        let observed = wait_for_merge_attribute_probe(&watcher_trace);
        if observed {
            std::fs::write(attributes, "target.txt merge=\n")
                .expect("select empty-name merge driver");
            record_trace_mutation(&watcher_trace);
        }
        observed
    });

    let result = apply_git_patch(&request(root, patch, /*revert*/ false))
        .expect("neutralized three-way apply");
    assert!(watcher.join().expect("attribute watcher"));

    assert_mutation_happened_between_probe_and_three_way(&trace);
    assert_ne!(result.exit_code, 0);
    assert!(!configured_marker_exists(root, "codex.oldmergeran"));
}

#[test]
fn post_probe_attribute_and_same_driver_command_change_stays_neutralized() {
    if std::env::var_os("CODEX_GIT_UTILS_MERGE_RACE_CHILD").is_none() {
        run_isolated_test(
            "merge_driver::race_tests::post_probe_attribute_and_same_driver_command_change_stays_neutralized",
        );
        return;
    }

    let repo = init_repo();
    let root = repo.path();
    let patch = build_conflicting_patch(root, "# initially safe\n");
    configure_driver(root, "x=y", OLD_DRIVER_COMMAND);
    let trace = trace_path();
    std::fs::write(&trace, "").expect("clear fixture trace");
    let watcher_trace = trace.clone();
    let watcher_root = root.to_path_buf();
    let watcher = thread::spawn(move || {
        let observed = wait_for_merge_attribute_probe(&watcher_trace);
        if observed {
            append_driver_config(&watcher_root, "x=y", NEW_DRIVER_COMMAND);
            std::fs::write(
                watcher_root.join(".gitattributes"),
                "target.txt merge=x=y\n",
            )
            .expect("select replacement merge driver");
            record_trace_mutation(&watcher_trace);
        }
        observed
    });

    let result = apply_git_patch(&request(root, patch, /*revert*/ false))
        .expect("neutralized three-way apply");
    assert!(watcher.join().expect("config watcher"));

    assert_mutation_happened_between_probe_and_three_way(&trace);
    assert_ne!(result.exit_code, 0);
    assert!(!configured_marker_exists(root, "codex.oldmergeran"));
    assert!(!configured_marker_exists(root, "codex.newmergeran"));
}

#[test]
fn same_patch_attribute_activation_does_not_run_merge_driver() {
    let repo = init_repo();
    let root = repo.path();
    std::fs::write(root.join(".gitattributes"), "# initially safe\n")
        .expect("write base attributes");
    std::fs::write(root.join("target.txt"), "top\nbase\nbottom\n").expect("write base");
    run_success(root, &["add", ".gitattributes", "target.txt"]);
    run_success(root, &["commit", "-m", "base"]);
    let base = run_success(root, &["rev-parse", "HEAD"]);
    let base = base.trim();

    std::fs::write(root.join(".gitattributes"), "target.txt merge=x=y\n")
        .expect("write patch attributes");
    std::fs::write(root.join("target.txt"), "top\ntheirs\nbottom\n").expect("write theirs");
    run_success(root, &["add", ".gitattributes", "target.txt"]);
    run_success(root, &["commit", "-m", "theirs"]);
    let patch = run_success(root, &["diff", "--full-index", base, "HEAD"]);

    run_success(root, &["checkout", "-b", "ours", base]);
    std::fs::write(root.join("target.txt"), "top\nours\nbottom\n").expect("write ours");
    run_success(root, &["add", "target.txt"]);
    run_success(root, &["commit", "-m", "ours"]);
    configure_driver(root, "x=y", OLD_DRIVER_COMMAND);

    let result = apply_git_patch(&request(root, patch, /*revert*/ false))
        .expect("same-patch attribute activation remains neutralized");

    assert_ne!(result.exit_code, 0);
    assert!(!configured_marker_exists(root, "codex.oldmergeran"));
}
