//! Helpers for applying unified diffs using the system `git` binary.
//!
//! The entry point is [`apply_git_patch`], which writes a diff to a temporary
//! file, shells out to `git apply` with the right flags, and then parses the
//! command’s output into structured details. Callers can opt into dry-run
//! mode via [`ApplyGitRequest::preflight`] and inspect the resulting paths to
//! learn what would change before applying for real.

use std::io;
use std::path::Path;
use std::path::PathBuf;

use crate::apply_output::parse_git_apply_output;
use crate::git_command::GitRunner;
use crate::guarded_config::GuardedGitConfig;
use crate::patch_paths::extract_patch_path_inventory_guarded;
use crate::reverse_staging::ReverseApplyMode;
use crate::reverse_staging::execute_reverse_staging_plan;
use crate::reverse_staging::execute_reverse_staging_plan_in_scratch;
use crate::reverse_staging::prepare_reverse_staging_plan;
use crate::reverse_staging::seal_reverse_staging_plan;
use crate::reverse_staging::validate_effective_paths_for_reverse;
#[cfg(test)]
use crate::safe_git::isolate_git_command_environment;

/// Parameters for invoking [`apply_git_patch`].
#[derive(Debug, Clone)]
pub struct ApplyGitRequest {
    pub cwd: PathBuf,
    pub diff: String,
    pub revert: bool,
    pub preflight: bool,
}

/// Result of running [`apply_git_patch`], including paths gleaned from stdout/stderr.
#[derive(Debug, Clone)]
pub struct ApplyGitResult {
    pub exit_code: i32,
    pub applied_paths: Vec<String>,
    pub skipped_paths: Vec<String>,
    pub conflicted_paths: Vec<String>,
    pub stdout: String,
    pub stderr: String,
    pub cmd_for_log: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ApplyStrategy {
    Direct,
    ThreeWay,
}

#[cfg(test)]
thread_local! {
    static AFTER_REVERSE_STAGING_HOOK: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
        std::cell::RefCell::new(None);
}

#[cfg(test)]
fn install_after_reverse_staging_hook(hook: impl FnOnce() + 'static) {
    AFTER_REVERSE_STAGING_HOOK.with(|slot| {
        let previous = slot.borrow_mut().replace(Box::new(hook));
        assert!(
            previous.is_none(),
            "reverse staging test hook already installed"
        );
    });
}

#[cfg(test)]
fn run_after_reverse_staging_hook() {
    AFTER_REVERSE_STAGING_HOOK.with(|slot| {
        if let Some(hook) = slot.borrow_mut().take() {
            hook();
        }
    });
}

impl ApplyStrategy {
    fn reverse_mode(self) -> ReverseApplyMode {
        match self {
            Self::Direct => ReverseApplyMode::Direct,
            Self::ThreeWay => ReverseApplyMode::ThreeWay,
        }
    }
}

/// Apply a unified diff to the target repository by shelling out to `git apply`.
///
/// When [`ApplyGitRequest::preflight`] is `true`, this behaves like `git apply --check` and
/// leaves the working tree untouched while still parsing the command output for diagnostics.
pub fn apply_git_patch(req: &ApplyGitRequest) -> io::Result<ApplyGitResult> {
    let cfg_parts = configured_git_config_parts();
    // Construct from the caller's exact route before canonicalization so
    // repository authority retains lexical enclosing worktrees.
    let git = GitRunner::for_cwd_io(&req.cwd)?;
    let requested_cwd = std::fs::canonicalize(&req.cwd)?;
    let expected_root = crate::get_git_repo_root(&requested_cwd)
        .ok_or_else(|| io::Error::other("not a Git repository"))
        .and_then(std::fs::canonicalize)?;
    let mut config = GuardedGitConfig::authorize(&git, &expected_root, cfg_parts)?;
    resolve_git_root(&config, &requested_cwd)?;
    config.freeze_apply_policy()?;

    // Write unified diff into a temporary file
    let (tmpdir, patch_path) = write_temp_patch(&req.diff)?;
    // Keep tmpdir alive until function end to ensure the file exists
    let _guard = tmpdir;
    let patch_path_inventory =
        extract_patch_path_inventory_guarded(&config, &patch_path, req.revert)?;
    let patch_paths = &patch_path_inventory.effective_paths;
    config.authorize_filter_paths(patch_paths)?;
    let patch_arg = patch_path.to_string_lossy().to_string();

    // Applicability checks conflate content conflicts with fatal whitespace
    // policy. Parse the requested orientation independently first: numstat
    // enforces the frozen whitespace policy without consulting or changing
    // the index/worktree. This ordering makes a mixed conflict + policy error
    // fail before merge-policy reads or reverse staging.
    let (policy_rendered, policy_output) = config.run_apply_policy_gate(req.revert, &patch_arg)?;
    let policy_code = policy_output.status.code().unwrap_or(-1);
    let policy_stdout = String::from_utf8_lossy(&policy_output.stdout).into_owned();
    let policy_stderr = String::from_utf8_lossy(&policy_output.stderr).into_owned();
    if policy_code != 0 {
        return Ok(structured_apply_result(
            policy_code,
            policy_stdout,
            policy_stderr,
            policy_rendered,
        ));
    }

    // Optional preflight: dry-run only; do not modify working tree
    if req.preflight {
        let (rendered, output) = config.run_apply_preflight_check(req.revert, &patch_arg)?;
        let c_code = output.status.code().unwrap_or(-1);
        let c_out = String::from_utf8_lossy(&output.stdout).into_owned();
        let c_err = String::from_utf8_lossy(&output.stderr).into_owned();
        if req.revert {
            let mode = if c_code == 0 {
                ReverseApplyMode::Direct
            } else {
                ReverseApplyMode::ThreeWay
            };
            validate_effective_paths_for_reverse(&config, patch_paths, mode)?;
        }
        let (mut applied_paths, mut skipped_paths, mut conflicted_paths) =
            parse_git_apply_output(&c_out, &c_err);
        applied_paths.sort();
        applied_paths.dedup();
        skipped_paths.sort();
        skipped_paths.dedup();
        conflicted_paths.sort();
        conflicted_paths.dedup();
        return Ok(ApplyGitResult {
            exit_code: c_code,
            applied_paths,
            skipped_paths,
            conflicted_paths,
            stdout: c_out,
            stderr: c_err,
            cmd_for_log: rendered,
        });
    }

    // Avoid three-way machinery entirely when the patch applies cleanly. A
    // reverse check must inspect the working tree before staging: requiring
    // the old index to match would misclassify a clean undo as a three-way
    // fallback. Forward direct application still requires index agreement.
    let plain_check = config.run_apply_strategy_check(req.revert, &patch_arg)?;
    let plain_check_code = plain_check.status.code().unwrap_or(-1);

    let strategy = if plain_check_code != 0 {
        ApplyStrategy::ThreeWay
    } else {
        ApplyStrategy::Direct
    };
    let reverse_plan = req
        .revert
        .then(|| prepare_reverse_staging_plan(&config, patch_paths, strategy.reverse_mode()))
        .transpose()?;

    if strategy == ApplyStrategy::ThreeWay {
        config.install_three_way_merge_policy(&patch_path_inventory.primary_records)?;
    }
    let mut reverse_plan = reverse_plan
        .map(|plan| seal_reverse_staging_plan(&mut config, plan))
        .transpose()?;

    if strategy == ApplyStrategy::ThreeWay && config.three_way_requires_merge_policy_proof()? {
        let scratch = config.create_three_way_scratch_storage()?;
        if let Some(plan) = &mut reverse_plan {
            execute_reverse_staging_plan_in_scratch(&config, plan, &scratch)?;
        }
        config.prove_three_way_merge_policy_safety(&scratch, req.revert, &patch_arg)?;
    }

    if let Some(plan) = reverse_plan {
        // The same sealed plan was modeled in scratch before any merge-policy
        // refusal could mutate the real index.
        execute_reverse_staging_plan(&config, plan)?;
        #[cfg(test)]
        run_after_reverse_staging_hook();
    }

    let (cmd_for_log, output) = match strategy {
        ApplyStrategy::Direct => {
            // The fixed helper requires proof that the successful frozen-
            // policy gate covered this exact patch and orientation.
            let rendered = config.render_direct_apply_for_log(req.revert, &patch_arg)?;
            let output = config.run_direct_apply(req.revert, &patch_arg)?;
            (
                rendered,
                (
                    output.status.code().unwrap_or(-1),
                    String::from_utf8_lossy(&output.stdout).into_owned(),
                    String::from_utf8_lossy(&output.stderr).into_owned(),
                ),
            )
        }
        ApplyStrategy::ThreeWay => {
            let rendered = config.render_three_way_apply_for_log(req.revert, &patch_arg)?;
            let output = config.run_three_way_apply(req.revert, &patch_arg)?;
            (
                rendered,
                (
                    output.status.code().unwrap_or(-1),
                    String::from_utf8_lossy(&output.stdout).into_owned(),
                    String::from_utf8_lossy(&output.stderr).into_owned(),
                ),
            )
        }
    };
    let (code, stdout, stderr) = output;

    let (mut applied_paths, mut skipped_paths, mut conflicted_paths) = if code == 0 {
        (patch_path_inventory.primary_paths, Vec::new(), Vec::new())
    } else {
        parse_git_apply_output(&stdout, &stderr)
    };
    applied_paths.sort();
    applied_paths.dedup();
    skipped_paths.sort();
    skipped_paths.dedup();
    conflicted_paths.sort();
    conflicted_paths.dedup();

    Ok(ApplyGitResult {
        exit_code: code,
        applied_paths,
        skipped_paths,
        conflicted_paths,
        stdout,
        stderr,
        cmd_for_log,
    })
}

fn structured_apply_result(
    exit_code: i32,
    stdout: String,
    stderr: String,
    cmd_for_log: String,
) -> ApplyGitResult {
    let (mut applied_paths, mut skipped_paths, mut conflicted_paths) =
        parse_git_apply_output(&stdout, &stderr);
    applied_paths.sort();
    applied_paths.dedup();
    skipped_paths.sort();
    skipped_paths.dedup();
    conflicted_paths.sort();
    conflicted_paths.dedup();
    ApplyGitResult {
        exit_code,
        applied_paths,
        skipped_paths,
        conflicted_paths,
        stdout,
        stderr,
        cmd_for_log,
    }
}

fn resolve_git_root(config: &GuardedGitConfig<'_>, requested_cwd: &Path) -> io::Result<PathBuf> {
    let mut command = config.rev_parse_command()?;
    command.arg("--show-toplevel");
    let out = command.output()?;
    let code = out.status.code().unwrap_or(-1);
    if code != 0 {
        return Err(io::Error::other(format!(
            "not a git repository (exit {}): {}",
            code,
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    let reported_root = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
    let root = std::fs::canonicalize(&reported_root)?;
    let expected_root = config.canonical_root();
    if root != expected_root {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "refusing to apply a patch because Git resolved worktree {} instead of expected worktree {} for requested cwd {}",
                root.display(),
                expected_root.display(),
                requested_cwd.display()
            ),
        ));
    }
    Ok(root)
}

pub(crate) fn configured_git_config_parts() -> Vec<String> {
    let mut cfg_parts = Vec::new();
    if let Ok(cfg) = std::env::var("CODEX_APPLY_GIT_CFG") {
        for pair in cfg.split(',') {
            let pair = pair.trim();
            if pair.is_empty() || !pair.contains('=') {
                continue;
            }
            cfg_parts.push("-c".to_string());
            cfg_parts.push(pair.to_string());
        }
    }
    cfg_parts
}

pub(crate) fn write_temp_patch(diff: &str) -> io::Result<(tempfile::TempDir, PathBuf)> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("patch.diff");
    std::fs::write(&path, diff)?;
    Ok((dir, path))
}

#[cfg(all(test, unix))]
#[path = "apply_transport_tests.rs"]
mod transport_tests;

#[cfg(test)]
#[path = "apply_filter_tests.rs"]
mod filter_tests;

#[cfg(test)]
#[path = "reverse_apply_tests.rs"]
mod reverse_apply_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guarded_config::config_source_authorization_count;
    use crate::guarded_config::merge_attribute_read_count;
    use crate::guarded_config::merge_config_read_count;
    use crate::guarded_config::merge_overlay_count;
    use crate::guarded_config::reset_config_source_authorization_count;
    use crate::guarded_config::reset_merge_policy_counts;
    use crate::safe_git::filter_policy_overlay_count;
    use crate::safe_git::filter_policy_read_count;
    use crate::safe_git::reset_filter_policy_counts;
    use std::ffi::OsStr;
    use std::path::Path;
    use std::sync::Mutex;
    use std::sync::OnceLock;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn run(cwd: &Path, args: &[&str]) -> (i32, String, String) {
        let mut command = std::process::Command::new(args[0]);
        isolate_git_command_environment(&mut command);
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
        isolate_git_command_environment(&mut command);
        command
            .arg(test_name)
            .arg("--exact")
            .arg("--nocapture")
            .env("CODEX_GIT_UTILS_APPLY_ENV_CHILD", "1")
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

    #[cfg(unix)]
    fn trusted_git_directory() -> PathBuf {
        std::env::split_paths(&std::env::var_os("PATH").expect("PATH"))
            .find(|directory| directory.is_absolute() && directory.join("git").is_file())
            .expect("trusted Git directory")
    }

    #[test]
    fn parse_output_unescapes_quoted_paths() {
        let stderr = "error: patch failed: \"hello\\tworld.txt\":1\n";
        let (applied, skipped, conflicted) = parse_git_apply_output("", stderr);
        assert_eq!(applied, Vec::<String>::new());
        assert_eq!(conflicted, Vec::<String>::new());
        assert_eq!(skipped, vec!["hello\tworld.txt".to_string()]);
    }

    #[test]
    fn apply_add_success() {
        let _g = env_lock().lock().unwrap();
        let repo = init_repo();
        let root = repo.path();
        let nested_cwd = root.join("nested");
        std::fs::create_dir(&nested_cwd).expect("nested cwd");

        let diff = "diff --git a/hello.txt b/hello.txt\nnew file mode 100644\n--- /dev/null\n+++ b/hello.txt\n@@ -0,0 +1,2 @@\n+hello\n+world\n";
        let req = ApplyGitRequest {
            cwd: nested_cwd,
            diff: diff.to_string(),
            revert: false,
            preflight: false,
        };
        reset_config_source_authorization_count();
        let r = apply_git_patch(&req).expect("run apply");
        assert_eq!(config_source_authorization_count(), 1);
        assert_eq!(r.exit_code, 0, "exit code 0");
        assert_eq!(r.applied_paths, vec!["hello.txt"]);
        // File exists now
        assert!(root.join("hello.txt").exists());
    }

    #[test]
    fn preflight_uses_one_authorization_and_skips_merge_policy() {
        let _g = env_lock().lock().unwrap();
        let repo = init_repo();
        let root = repo.path();
        let request = ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: "diff --git a/preflight.txt b/preflight.txt\nnew file mode 100644\n--- /dev/null\n+++ b/preflight.txt\n@@ -0,0 +1 @@\n+preflight\n".to_string(),
            revert: false,
            preflight: true,
        };

        reset_config_source_authorization_count();
        reset_merge_policy_counts();
        let result = apply_git_patch(&request).expect("preflight");

        assert_eq!(result.exit_code, 0, "{}", result.stderr);
        assert_eq!(config_source_authorization_count(), 1);
        assert_eq!(merge_config_read_count(), 0);
        assert_eq!(merge_attribute_read_count(), 0);
        assert!(!root.join("preflight.txt").exists());
    }

    #[test]
    fn apply_resolves_relative_primary_config_from_repository_root() {
        let _g = env_lock().lock().unwrap();
        if std::env::var_os("CODEX_GIT_UTILS_APPLY_ENV_CHILD").is_none() {
            run_isolated_test(
                "apply::tests::apply_resolves_relative_primary_config_from_repository_root",
                &[("GIT_CONFIG_GLOBAL", OsStr::new("../external/config"))],
            );
            return;
        }

        let fixture = tempfile::tempdir().expect("fixture");
        let root = fixture.path().join("repo");
        let nested_cwd = root.join("nested");
        let external_config = fixture.path().join("external/config");
        let mismatched_nested_config = root.join("external/config");
        std::fs::create_dir_all(&nested_cwd).expect("nested cwd");
        std::fs::create_dir_all(external_config.parent().expect("config parent"))
            .expect("external config directory");
        std::fs::create_dir_all(
            mismatched_nested_config
                .parent()
                .expect("mismatched config parent"),
        )
        .expect("mismatched config directory");
        std::fs::write(&external_config, "[codex]\n\tprobe = loaded\n").expect("external config");
        std::fs::write(&mismatched_nested_config, "[invalid\n")
            .expect("mismatched nested-cwd config");
        let (init_code, _, init_err) = run(&root, &["git", "init"]);
        assert_eq!(init_code, 0, "init repository: {init_err}");

        let result = apply_git_patch(&ApplyGitRequest {
            cwd: nested_cwd,
            diff: "diff --git a/hello.txt b/hello.txt\nnew file mode 100644\n--- /dev/null\n+++ b/hello.txt\n@@ -0,0 +1 @@\n+hello\n".to_string(),
            revert: false,
            preflight: false,
        })
        .expect("apply with root-relative primary config");
        assert_eq!(result.exit_code, 0, "apply result: {result:?}");
        assert_eq!(read_file_normalized(&root.join("hello.txt")), "hello\n");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn apply_refuses_process_relative_primary_config_before_mutating_the_repository() {
        const TEST_NAME: &str = "apply::tests::apply_refuses_process_relative_primary_config_before_mutating_the_repository";
        if std::env::var_os("CODEX_GIT_UTILS_APPLY_ENV_CHILD").is_none() {
            run_isolated_test(
                TEST_NAME,
                &[
                    (
                        "GIT_CONFIG_GLOBAL",
                        OsStr::new("/proc/self/cwd/codex-process-relative.gitconfig"),
                    ),
                    ("GIT_CONFIG_NOSYSTEM", OsStr::new("1")),
                ],
            );
            return;
        }

        let repo = init_repo();
        let root = repo.path();
        std::fs::write(
            root.join("codex-process-relative.gitconfig"),
            "[filter \"unsafe\"]\nclean = false\n",
        )
        .expect("worktree config");
        let before_index = run(root, &["git", "ls-files", "--stage", "-z"]).1;
        let request = ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: "diff --git a/hello.txt b/hello.txt\nnew file mode 100644\n--- /dev/null\n+++ b/hello.txt\n@@ -0,0 +1 @@\n+hello\n".to_string(),
            revert: false,
            preflight: false,
        };

        let error = apply_git_patch(&request).expect_err("process-relative primary config");

        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied, "{error}");
        assert!(error.to_string().contains("process-relative"), "{error}");
        assert!(!root.join("hello.txt").exists());
        assert_eq!(
            run(root, &["git", "ls-files", "--stage", "-z"]).1,
            before_index
        );
    }

    #[test]
    fn numstat_path_discovery_does_not_preempt_apply_whitespace_result() {
        let _g = env_lock().lock().unwrap();
        let repo = init_repo();
        let root = repo.path();
        let (config_code, _, config_err) =
            run(root, &["git", "config", "apply.whitespace", "error"]);
        assert_eq!(config_code, 0, "configure whitespace policy: {config_err}");

        let result = apply_git_patch(&ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: "diff --git a/trailing.txt b/trailing.txt\nnew file mode 100644\n--- /dev/null\n+++ b/trailing.txt\n@@ -0,0 +1 @@\n+trailing \n".to_string(),
            revert: false,
            preflight: false,
        })
        .expect("path discovery must leave apply failure in the structured result");
        assert_ne!(result.exit_code, 0);
        assert!(
            result.stderr.contains("trailing whitespace"),
            "apply result: {result:?}"
        );
        assert!(!root.join("trailing.txt").exists());
    }

    #[test]
    fn fatal_whitespace_policy_precedes_selected_merge_driver_without_mutation() {
        let _g = env_lock().lock().unwrap();
        for policy in ["error", "error-all"] {
            let repo = init_repo();
            let root = repo.path();
            std::fs::write(root.join(".gitattributes"), "file.txt merge=selected\n")
                .expect("write attributes");
            std::fs::write(root.join("file.txt"), "old\n").expect("write file");
            assert_eq!(run(root, &["git", "add", "."]).0, 0);
            assert_eq!(run(root, &["git", "commit", "-m", "base"]).0, 0);
            assert_eq!(
                run(
                    root,
                    &[
                        "git",
                        "config",
                        "merge.selected.driver",
                        "git config --file .git/config codex.mergeran true && false",
                    ],
                )
                .0,
                0
            );
            assert_eq!(
                run(root, &["git", "config", "apply.whitespace", policy]).0,
                0
            );
            let index_before = std::fs::read(root.join(".git/index")).expect("read index");

            reset_merge_policy_counts();
            let result = apply_git_patch(&ApplyGitRequest {
                cwd: root.to_path_buf(),
                diff: "diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n@@ -1 +1 @@\n-old\n+new \n".to_string(),
                revert: false,
                preflight: false,
            })
            .expect("return structured whitespace failure");

            assert_ne!(result.exit_code, 0, "{policy}: {result:?}");
            assert!(
                result.stderr.contains("trailing whitespace"),
                "{policy}: {result:?}"
            );
            assert!(result.cmd_for_log.contains("--numstat"), "{result:?}");
            assert_eq!(merge_config_read_count(), 0, "{policy}");
            assert_eq!(merge_attribute_read_count(), 0, "{policy}");
            assert_eq!(merge_overlay_count(), 0, "{policy}");
            assert_eq!(read_file_normalized(&root.join("file.txt")), "old\n");
            assert_eq!(
                std::fs::read(root.join(".git/index")).expect("reread index"),
                index_before,
                "{policy}"
            );
            assert_ne!(
                run(root, &["git", "config", "--get", "codex.mergeran"]).0,
                0,
                "{policy}: merge driver must not run"
            );
        }
    }

    #[test]
    fn nonfatal_whitespace_policies_reach_the_final_apply() {
        let _g = env_lock().lock().unwrap();
        for (policy, expected) in [("warn", "new \n"), ("fix", "new\n")] {
            let repo = init_repo();
            let root = repo.path();
            std::fs::write(root.join("file.txt"), "old\n").expect("write file");
            assert_eq!(run(root, &["git", "add", "file.txt"]).0, 0);
            assert_eq!(run(root, &["git", "commit", "-m", "base"]).0, 0);
            assert_eq!(
                run(root, &["git", "config", "apply.whitespace", policy]).0,
                0
            );

            let result = apply_git_patch(&ApplyGitRequest {
                cwd: root.to_path_buf(),
                diff: "diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n@@ -1 +1 @@\n-old\n+new \n".to_string(),
                revert: false,
                preflight: false,
            })
            .expect("apply with nonfatal whitespace policy");

            assert_eq!(result.exit_code, 0, "{policy}: {result:?}");
            assert_eq!(read_file_normalized(&root.join("file.txt")), expected);
        }
    }

    #[test]
    fn fatal_whitespace_gate_remains_authoritative_after_reverse_staging() {
        let _g = env_lock().lock().unwrap();
        for policy in ["error", "error-all"] {
            let repo = init_repo();
            let root = repo.path();
            std::fs::write(root.join(".gitattributes"), "file.txt -whitespace\n")
                .expect("write permissive attributes");
            std::fs::write(root.join("file.txt"), "bad \n").expect("write base file");
            assert_eq!(
                run(root, &["git", "add", ".gitattributes", "file.txt"]).0,
                0
            );
            assert_eq!(run(root, &["git", "commit", "-m", "base"]).0, 0);
            std::fs::write(root.join("file.txt"), "good\n").expect("write patched file");
            let patch = run(root, &["git", "diff", "--full-index", "--", "file.txt"]);
            assert_eq!(patch.0, 0, "create patch: {}", patch.2);
            assert_eq!(
                run(root, &["git", "config", "apply.whitespace", policy]).0,
                0
            );

            let attributes = root.join(".gitattributes");
            install_after_reverse_staging_hook(move || {
                std::fs::write(attributes, "file.txt whitespace\n")
                    .expect("tighten whitespace attributes after staging");
            });
            let result = apply_git_patch(&ApplyGitRequest {
                cwd: root.to_path_buf(),
                diff: patch.1,
                revert: true,
                preflight: false,
            })
            .expect("reverse apply after attribute mutation");

            assert_eq!(result.exit_code, 0, "{policy}: {result:?}");
            assert!(
                result.cmd_for_log.contains("--whitespace=nowarn"),
                "{policy}: {}",
                result.cmd_for_log
            );
            assert_eq!(
                read_file_normalized(&root.join(".gitattributes")),
                "file.txt whitespace\n",
                "{policy}: race hook did not tighten the attribute"
            );
            assert_eq!(read_file_normalized(&root.join("file.txt")), "bad \n");
            let cached = run(
                root,
                &["git", "diff", "--cached", "--quiet", "--", "file.txt"],
            );
            assert_eq!(cached.0, 0, "{policy}: index retained staged data");
        }
    }

    #[test]
    fn fatal_whitespace_gate_and_projection_cover_reverse_three_way_staging() {
        let _g = env_lock().lock().unwrap();
        let repo = init_repo();
        let root = repo.path();
        std::fs::write(root.join(".gitattributes"), "file.txt -whitespace\n")
            .expect("write permissive attributes");
        let base_contents = "01\n02\n03\n04\n05\n06\n07\n08\n09\nbad \n11\n12\n13\n14\n15\n";
        let patched_contents = "01\n02\n03\n04\n05\n06\n07\n08\n09\ngood\n11\n12\n13\n14\n15\n";
        let independent_contents =
            "01\n02\n03\n04\n05\n06\nSEVEN\n08\n09\ngood\n11\n12\n13\n14\n15\n";
        let expected_contents = "01\n02\n03\n04\n05\n06\nSEVEN\n08\n09\nbad \n11\n12\n13\n14\n15\n";
        std::fs::write(root.join("file.txt"), base_contents).expect("write base");
        assert_eq!(
            run(root, &["git", "add", ".gitattributes", "file.txt"]).0,
            0
        );
        assert_eq!(run(root, &["git", "commit", "-m", "base"]).0, 0);
        let base = run(root, &["git", "rev-parse", "HEAD"]).1;
        std::fs::write(root.join("file.txt"), patched_contents).expect("write patch side");
        assert_eq!(run(root, &["git", "add", "file.txt"]).0, 0);
        assert_eq!(run(root, &["git", "commit", "-m", "patched"]).0, 0);
        let patch = run(
            root,
            &[
                "git",
                "diff",
                "--full-index",
                base.trim(),
                "HEAD",
                "--",
                "file.txt",
            ],
        );
        assert_eq!(patch.0, 0, "create patch: {}", patch.2);
        std::fs::write(root.join("file.txt"), independent_contents)
            .expect("write independent worktree edit");
        assert_eq!(
            run(root, &["git", "config", "apply.whitespace", "error"]).0,
            0
        );

        let attributes = root.join(".gitattributes");
        install_after_reverse_staging_hook(move || {
            std::fs::write(attributes, "file.txt whitespace\n")
                .expect("tighten whitespace attributes after staging");
        });
        let result = apply_git_patch(&ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: patch.1,
            revert: true,
            preflight: false,
        })
        .expect("reverse three-way after attribute mutation");

        assert_eq!(result.exit_code, 0, "{result:?}");
        assert!(result.cmd_for_log.contains("GIT_COMMON_DIR=<isolated>"));
        assert!(result.cmd_for_log.contains("--3way"));
        assert!(result.cmd_for_log.contains("--whitespace=nowarn"));
        assert_eq!(
            read_file_normalized(&root.join(".gitattributes")),
            "file.txt whitespace\n",
            "race hook did not tighten the attribute"
        );
        assert_eq!(
            read_file_normalized(&root.join("file.txt")),
            expected_contents
        );
        assert_eq!(
            run(root, &["git", "diff", "--quiet", "--", "file.txt"]).0,
            0,
            "final index and worktree differ"
        );
    }

    #[test]
    fn mixed_reverse_conflict_and_whitespace_error_does_not_mutate() {
        let _g = env_lock().lock().unwrap();
        let repo = init_repo();
        let root = repo.path();
        let base = "01\n02\n03\n04\n05\n06\n07\n08\n09\n10\nbase \n12\n13\n14\n15\n";
        let theirs = "01\n02\n03\n04\n05\n06\n07\n08\n09\n10\ntheirs\n12\n13\n14\n15\n";
        let independently_edited =
            "01\n02\n03\n04\n05\n06\n07\nEIGHT\n09\n10\ntheirs\n12\n13\n14\n15\n";
        std::fs::write(root.join("file.txt"), base).expect("write base");
        assert_eq!(run(root, &["git", "add", "file.txt"]).0, 0);
        assert_eq!(run(root, &["git", "commit", "-m", "base"]).0, 0);
        let base_oid = run(root, &["git", "rev-parse", "HEAD"]).1;
        std::fs::write(root.join("file.txt"), theirs).expect("write theirs");
        assert_eq!(run(root, &["git", "add", "file.txt"]).0, 0);
        assert_eq!(run(root, &["git", "commit", "-m", "theirs"]).0, 0);
        let patch = run(
            root,
            &[
                "git",
                "diff",
                "--full-index",
                base_oid.trim(),
                "HEAD",
                "--",
                "file.txt",
            ],
        );
        assert_eq!(patch.0, 0, "create patch: {}", patch.2);
        std::fs::write(root.join("file.txt"), independently_edited)
            .expect("write independent edit");
        let patch_dir = tempfile::tempdir().expect("patch directory");
        let patch_path = patch_dir.path().join("mixed.diff");
        std::fs::write(&patch_path, &patch.1).expect("write patch");
        let patch_path = patch_path.to_str().expect("UTF-8 patch path");
        let structural = run(
            root,
            &[
                "git",
                "apply",
                "--check",
                "--whitespace=nowarn",
                "-R",
                patch_path,
            ],
        );
        assert_ne!(
            structural.0, 0,
            "fixture must retain a structural reverse conflict"
        );
        assert_eq!(
            run(root, &["git", "config", "apply.whitespace", "error"]).0,
            0
        );
        let index_before = std::fs::read(root.join(".git/index")).expect("read index");
        let worktree_before = std::fs::read(root.join("file.txt")).expect("read worktree");

        reset_merge_policy_counts();
        let result = apply_git_patch(&ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: patch.1,
            revert: true,
            preflight: false,
        })
        .expect("return mixed policy failure");

        assert_ne!(result.exit_code, 0, "{result:?}");
        assert!(result.stderr.contains("trailing whitespace"), "{result:?}");
        assert_eq!(merge_config_read_count(), 0);
        assert_eq!(merge_attribute_read_count(), 0);
        assert_eq!(merge_overlay_count(), 0);
        assert_eq!(
            std::fs::read(root.join(".git/index")).expect("reread index"),
            index_before
        );
        assert_eq!(
            std::fs::read(root.join("file.txt")).expect("reread worktree"),
            worktree_before
        );
    }

    #[test]
    fn apply_uses_cwd_repo_despite_inherited_repository_selectors() {
        let _g = env_lock().lock().unwrap();
        if std::env::var_os("CODEX_GIT_UTILS_APPLY_ENV_CHILD").is_none() {
            let alternate = init_repo();
            let alternate_root = alternate.path();
            std::fs::write(alternate_root.join("sentinel.txt"), "alternate\n")
                .expect("write alternate sentinel");
            let (add_code, _, add_err) = run(alternate_root, &["git", "add", "sentinel.txt"]);
            assert_eq!(add_code, 0, "add alternate sentinel: {add_err}");
            let (commit_code, _, commit_err) =
                run(alternate_root, &["git", "commit", "-m", "alternate"]);
            assert_eq!(commit_code, 0, "commit alternate sentinel: {commit_err}");

            let alternate_git_dir = alternate_root.join(".git");
            let alternate_index = alternate_git_dir.join("index");
            run_isolated_test(
                "apply::tests::apply_uses_cwd_repo_despite_inherited_repository_selectors",
                &[
                    ("GIT_DIR", alternate_git_dir.as_os_str()),
                    ("GIT_WORK_TREE", alternate_root.as_os_str()),
                    ("GIT_COMMON_DIR", alternate_git_dir.as_os_str()),
                    ("GIT_INDEX_FILE", alternate_index.as_os_str()),
                    ("GIT_PREFIX", OsStr::new("elsewhere/")),
                ],
            );
            assert_eq!(
                read_file_normalized(&alternate_root.join("sentinel.txt")),
                "alternate\n"
            );
            return;
        }

        let repo = init_repo();
        let root = repo.path();
        std::fs::write(root.join("file.txt"), "old\n").expect("write target file");
        let (add_code, _, add_err) = run(root, &["git", "add", "file.txt"]);
        assert_eq!(add_code, 0, "add target file: {add_err}");
        let (commit_code, _, commit_err) = run(root, &["git", "commit", "-m", "target"]);
        assert_eq!(commit_code, 0, "commit target file: {commit_err}");

        let result = apply_git_patch(&ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: "diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n@@ -1 +1 @@\n-old\n+new\n".to_string(),
            revert: false,
            preflight: false,
        })
        .expect("apply in cwd-selected repository");
        assert_eq!(result.exit_code, 0);
        assert_eq!(read_file_normalized(&root.join("file.txt")), "new\n");
    }

    #[cfg(unix)]
    #[test]
    fn apply_uses_logical_process_cwd_to_reject_enclosing_git() {
        use std::os::unix::fs::PermissionsExt;

        let _g = env_lock().lock().unwrap();
        if std::env::var_os("CODEX_GIT_UTILS_APPLY_LOGICAL_CWD_CHILD").is_none() {
            let fixture = tempfile::tempdir().expect("fixture");
            let outer = fixture.path().join("outer");
            let physical_nested = fixture.path().join("physical-nested");
            let lexical_nested = outer.join("nested");
            let outer_bin = outer.join("bin");
            let outer_git = outer_bin.join("git");
            let marker = outer_bin.join("git.ran");
            std::fs::create_dir_all(&outer_bin).expect("outer Git directory");
            std::fs::create_dir_all(&physical_nested).expect("physical nested repository");
            let (outer_init, _, outer_err) = run(&outer, &["git", "init", "-q"]);
            assert_eq!(outer_init, 0, "init outer repository: {outer_err}");
            let (nested_init, _, nested_err) = run(&physical_nested, &["git", "init", "-q"]);
            assert_eq!(nested_init, 0, "init nested repository: {nested_err}");
            std::os::unix::fs::symlink(&physical_nested, &lexical_nested)
                .expect("symlink nested repository");
            std::fs::write(&outer_git, "#!/bin/sh\nprintf ran >\"$0.ran\"\nexit 1\n")
                .expect("outer Git shim");
            let mut permissions = std::fs::metadata(&outer_git)
                .expect("outer Git metadata")
                .permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&outer_git, permissions).expect("executable outer Git");
            let path = std::env::join_paths([outer_bin, trusted_git_directory()]).expect("PATH");

            let mut command =
                std::process::Command::new(std::env::current_exe().expect("test binary"));
            isolate_git_command_environment(&mut command);
            let output = command
                .arg("apply::tests::apply_uses_logical_process_cwd_to_reject_enclosing_git")
                .arg("--exact")
                .arg("--nocapture")
                .current_dir(&lexical_nested)
                .env("CODEX_GIT_UTILS_APPLY_LOGICAL_CWD_CHILD", "1")
                .env("CODEX_GIT_UTILS_APPLY_LOGICAL_CWD_MARKER", &marker)
                .env("PWD", &lexical_nested)
                .env("PATH", path)
                .env("RUST_TEST_THREADS", "1")
                .output()
                .expect("run isolated logical-cwd test");
            assert!(
                output.status.success(),
                "isolated logical-cwd test failed:\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(!marker.exists(), "enclosing Git shim must not run");
            return;
        }

        let cwd = std::env::current_dir().expect("physical process cwd");
        let marker = PathBuf::from(
            std::env::var_os("CODEX_GIT_UTILS_APPLY_LOGICAL_CWD_MARKER").expect("marker path"),
        );
        let result = apply_git_patch(&ApplyGitRequest {
            cwd,
            diff: "diff --git a/hello.txt b/hello.txt\nnew file mode 100644\n--- /dev/null\n+++ b/hello.txt\n@@ -0,0 +1 @@\n+hello\n".to_string(),
            revert: false,
            preflight: true,
        })
        .expect("preflight through trusted Git");
        assert_eq!(result.exit_code, 0, "preflight should succeed");
        assert!(!marker.exists(), "enclosing Git shim must not run");
    }

    #[cfg(unix)]
    #[test]
    fn apply_uses_physical_repository_for_symlinked_nested_cwd() {
        let _g = env_lock().lock().unwrap();
        let fixture = tempfile::tempdir().expect("fixture");
        let outer = fixture.path().join("outer");
        let target = fixture.path().join("target");
        let nested = target.join("nested");
        std::fs::create_dir_all(&outer).expect("outer");
        std::fs::create_dir_all(&nested).expect("nested");
        let (outer_init, _, outer_error) = run(&outer, &["git", "init", "-q"]);
        assert_eq!(outer_init, 0, "outer init: {outer_error}");
        let (target_init, _, target_error) = run(&target, &["git", "init", "-q"]);
        assert_eq!(target_init, 0, "target init: {target_error}");
        let lexical_cwd = outer.join("linked-nested");
        std::os::unix::fs::symlink(&nested, &lexical_cwd).expect("nested cwd symlink");

        reset_config_source_authorization_count();
        let result = apply_git_patch(&ApplyGitRequest {
            cwd: lexical_cwd,
            diff: "diff --git a/physical.txt b/physical.txt\nnew file mode 100644\n--- /dev/null\n+++ b/physical.txt\n@@ -0,0 +1 @@\n+physical\n".to_string(),
            revert: false,
            preflight: false,
        })
        .expect("apply through symlinked nested cwd");

        assert_eq!(result.exit_code, 0);
        assert_eq!(config_source_authorization_count(), 1);
        assert_eq!(
            read_file_normalized(&target.join("physical.txt")),
            "physical\n"
        );
        assert!(!outer.join("physical.txt").exists());
    }

    #[test]
    fn apply_modify_conflict() {
        let _g = env_lock().lock().unwrap();
        let repo = init_repo();
        let root = repo.path();
        // seed file and commit
        std::fs::write(root.join("file.txt"), "line1\nline2\nline3\n").unwrap();
        let _ = run(root, &["git", "add", "file.txt"]);
        let _ = run(root, &["git", "commit", "-m", "seed"]);
        // local edit (unstaged)
        std::fs::write(root.join("file.txt"), "line1\nlocal2\nline3\n").unwrap();
        // patch wants to change the same line differently
        let diff = "diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n@@ -1,3 +1,3 @@\n line1\n-line2\n+remote2\n line3\n";
        let req = ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: diff.to_string(),
            revert: false,
            preflight: false,
        };
        let r = apply_git_patch(&req).expect("run apply");
        assert_ne!(r.exit_code, 0, "non-zero exit on conflict");
    }

    #[test]
    fn apply_modify_skipped_missing_index() {
        let _g = env_lock().lock().unwrap();
        let repo = init_repo();
        let root = repo.path();
        // Try to modify a file that is not in the index
        let diff = "diff --git a/ghost.txt b/ghost.txt\n--- a/ghost.txt\n+++ b/ghost.txt\n@@ -1,1 +1,1 @@\n-old\n+new\n";
        let req = ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: diff.to_string(),
            revert: false,
            preflight: false,
        };
        let r = apply_git_patch(&req).expect("run apply");
        assert_ne!(r.exit_code, 0, "non-zero exit on missing index");
    }

    #[test]
    fn apply_then_revert_success() {
        let _g = env_lock().lock().unwrap();
        if std::env::var_os("CODEX_GIT_UTILS_APPLY_ENV_CHILD").is_none() {
            let config_dir = tempfile::tempdir().expect("config tempdir");
            let global_config = config_dir.path().join("global.gitconfig");
            let system_config = config_dir.path().join("system.gitconfig");
            std::fs::write(&global_config, "").expect("empty global config");
            std::fs::write(&system_config, "").expect("empty system config");
            run_isolated_test(
                "apply::tests::apply_then_revert_success",
                &[
                    ("GIT_CONFIG_GLOBAL", global_config.as_os_str()),
                    ("GIT_CONFIG_SYSTEM", system_config.as_os_str()),
                ],
            );
            return;
        }

        let repo = init_repo();
        let root = repo.path();
        // Seed file and commit original content
        std::fs::write(root.join("file.txt"), "orig\n").unwrap();
        let _ = run(root, &["git", "add", "file.txt"]);
        let _ = run(root, &["git", "commit", "-m", "seed"]);

        // Forward patch: orig -> ORIG
        let diff = "diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n@@ -1,1 +1,1 @@\n-orig\n+ORIG\n";
        let apply_req = ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: diff.to_string(),
            revert: false,
            preflight: false,
        };
        reset_config_source_authorization_count();
        let res_apply = apply_git_patch(&apply_req).expect("apply ok");
        assert_eq!(config_source_authorization_count(), 1);
        assert_eq!(res_apply.exit_code, 0, "forward apply succeeded");
        let after_apply = read_file_normalized(&root.join("file.txt"));
        assert_eq!(after_apply, "ORIG\n");

        // Revert patch: ORIG -> orig (stage paths first; engine handles it)
        let revert_req = ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: diff.to_string(),
            revert: true,
            preflight: false,
        };
        reset_config_source_authorization_count();
        reset_filter_policy_counts();
        let res_revert = apply_git_patch(&revert_req).expect("revert ok");
        assert_eq!(config_source_authorization_count(), 1);
        assert_eq!(
            filter_policy_read_count(),
            1,
            "reverse staging must skip a fresh Git-add policy when the index already matches"
        );
        assert_eq!(filter_policy_overlay_count(), 0);
        assert_eq!(res_revert.exit_code, 0, "revert apply succeeded");
        let after_revert = read_file_normalized(&root.join("file.txt"));
        assert_eq!(after_revert, "orig\n");
    }

    #[test]
    fn revert_preflight_does_not_stage_index() {
        let _g = env_lock().lock().unwrap();
        let repo = init_repo();
        let root = repo.path();
        // Seed repo and apply forward patch so the working tree reflects the change.
        std::fs::write(root.join("file.txt"), "orig\n").unwrap();
        let _ = run(root, &["git", "add", "file.txt"]);
        let _ = run(root, &["git", "commit", "-m", "seed"]);

        let diff = "diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n@@ -1,1 +1,1 @@\n-orig\n+ORIG\n";
        let apply_req = ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: diff.to_string(),
            revert: false,
            preflight: false,
        };
        let res_apply = apply_git_patch(&apply_req).expect("apply ok");
        assert_eq!(res_apply.exit_code, 0, "forward apply succeeded");
        let (commit_code, _, commit_err) = run(root, &["git", "commit", "-am", "apply change"]);
        assert_eq!(commit_code, 0, "commit applied change: {commit_err}");

        let (_code_before, staged_before, _stderr_before) =
            run(root, &["git", "diff", "--cached", "--name-only"]);

        let preflight_req = ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: diff.to_string(),
            revert: true,
            preflight: true,
        };
        reset_config_source_authorization_count();
        let res_preflight = apply_git_patch(&preflight_req).expect("preflight ok");
        assert_eq!(config_source_authorization_count(), 1);
        assert_eq!(res_preflight.exit_code, 0, "revert preflight succeeded");
        let (_code_after, staged_after, _stderr_after) =
            run(root, &["git", "diff", "--cached", "--name-only"]);
        assert_eq!(
            staged_after.trim(),
            staged_before.trim(),
            "preflight should not stage new paths",
        );

        let after_preflight = read_file_normalized(&root.join("file.txt"));
        assert_eq!(after_preflight, "ORIG\n");
    }

    #[test]
    fn preflight_blocks_partial_changes() {
        let _g = env_lock().lock().unwrap();
        let repo = init_repo();
        let root = repo.path();
        // Build a multi-file diff: one valid add (ok.txt) and one invalid modify (ghost.txt)
        let diff = "diff --git a/ok.txt b/ok.txt\nnew file mode 100644\n--- /dev/null\n+++ b/ok.txt\n@@ -0,0 +1,2 @@\n+alpha\n+beta\n\n\
diff --git a/ghost.txt b/ghost.txt\n--- a/ghost.txt\n+++ b/ghost.txt\n@@ -1,1 +1,1 @@\n-old\n+new\n";

        // 1) With preflight enabled, nothing should be changed (even though ok.txt could be added)
        let req1 = ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: diff.to_string(),
            revert: false,
            preflight: true,
        };
        reset_config_source_authorization_count();
        let r1 = apply_git_patch(&req1).expect("preflight apply");
        assert_eq!(config_source_authorization_count(), 1);
        assert_ne!(r1.exit_code, 0, "preflight reports failure");
        assert!(
            !root.join("ok.txt").exists(),
            "preflight must prevent adding ok.txt"
        );
        assert!(
            r1.cmd_for_log.contains("--check"),
            "preflight path recorded --check"
        );

        // 2) Without preflight, we should see no --check in the executed command
        let req2 = ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: diff.to_string(),
            revert: false,
            preflight: false,
        };
        reset_config_source_authorization_count();
        let r2 = apply_git_patch(&req2).expect("direct apply");
        assert_eq!(config_source_authorization_count(), 1);
        assert_ne!(r2.exit_code, 0, "apply is expected to fail overall");
        assert!(
            !r2.cmd_for_log.contains("--check"),
            "non-preflight path should not use --check"
        );
    }

    #[test]
    fn resolve_git_root_rejects_core_worktree_redirection() {
        let temp = tempfile::tempdir().expect("tempdir");
        let attacker = temp.path().join("attacker");
        let victim = temp.path().join("victim");
        std::fs::create_dir_all(&attacker).expect("attacker");
        std::fs::create_dir_all(&victim).expect("victim");
        let (init_code, _, init_err) = run(&attacker, &["git", "init"]);
        assert_eq!(init_code, 0, "init attacker repo: {init_err}");

        for redirected_worktree in [&victim, temp.path()] {
            let redirected_worktree = redirected_worktree.to_string_lossy();
            let (config_code, _, config_err) = run(
                &attacker,
                &["git", "config", "core.worktree", &redirected_worktree],
            );
            assert_eq!(config_code, 0, "configure core.worktree: {config_err}");

            let git = GitRunner::for_cwd_io(&attacker).expect("trusted Git");
            let guarded = GuardedGitConfig::authorize(&git, &attacker, Vec::new())
                .expect("authorize attacker repository config");
            let error =
                resolve_git_root(&guarded, &attacker).expect_err("reject redirected worktree");
            assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
            assert!(error.to_string().contains("instead of expected worktree"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn apply_propagates_unsafe_repository_metadata_before_git_launch() {
        use std::os::unix::fs::symlink;

        let repo = init_repo();
        let root = repo.path();
        let external = tempfile::tempdir().expect("external metadata parent");
        let admin = external.path().join("admin");
        std::fs::rename(root.join(".git"), &admin).expect("move metadata external");
        symlink(&admin, root.join("switch")).expect("worktree metadata switch");
        std::fs::write(root.join(".git"), "gitdir: switch\n").expect("write gitdir marker");
        let (code, _, stderr) = run(root, &["git", "rev-parse", "--absolute-git-dir"]);
        assert_eq!(code, 0, "native Git fixture: {stderr}");

        let request = ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: String::new(),
            revert: false,
            preflight: true,
        };
        let error = apply_git_patch(&request).expect_err("unsafe metadata route");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert!(error.to_string().contains("Git metadata route crosses"));
        assert!(!error.to_string().contains("PATH"));
    }
}
