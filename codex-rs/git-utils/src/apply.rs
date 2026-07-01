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

use crate::FsmonitorOverride;
use crate::apply_output::parse_git_apply_output;
use crate::git_command::GitRunner;
use crate::patch_paths::extract_effective_paths_from_patch;
use crate::patch_paths::stage_effective_paths;
use crate::safe_git::DISABLED_HOOKS_PATH;
use crate::safe_git::ensure_no_selected_executable_git_filters;
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

/// Apply a unified diff to the target repository by shelling out to `git apply`.
///
/// When [`ApplyGitRequest::preflight`] is `true`, this behaves like `git apply --check` and
/// leaves the working tree untouched while still parsing the command output for diagnostics.
pub fn apply_git_patch(req: &ApplyGitRequest) -> io::Result<ApplyGitResult> {
    let git = GitRunner::for_cwd_io(&req.cwd)?;
    let mut cfg_parts = configured_git_config_parts();
    let git_root = resolve_git_root(&git, &req.cwd, &cfg_parts)?;

    // Write unified diff into a temporary file
    let (tmpdir, patch_path) = write_temp_patch(&req.diff)?;
    // Keep tmpdir alive until function end to ensure the file exists
    let _guard = tmpdir;
    let patch_paths = extract_effective_paths_from_patch(&git, &patch_path, req.revert)?;
    ensure_no_selected_executable_git_filters(&git, &git_root, &patch_paths, &cfg_parts)?;

    if req.revert && !req.preflight {
        // Stage WT paths first to avoid index mismatch on revert.
        stage_effective_paths(&git, &git_root, &patch_paths, &cfg_parts)?;
    }

    // Build git args
    let mut args: Vec<String> = vec!["apply".into(), "--3way".into()];
    if req.revert {
        args.push("-R".into());
    }

    cfg_parts.extend(safe_git_config_parts());

    args.push(patch_path.to_string_lossy().to_string());

    // Optional preflight: dry-run only; do not modify working tree
    if req.preflight {
        let mut check_args = vec!["apply".to_string(), "--check".to_string()];
        if req.revert {
            check_args.push("-R".to_string());
        }
        check_args.push(patch_path.to_string_lossy().to_string());
        let rendered = render_command_for_log(&git_root, &cfg_parts, &check_args);
        let (c_code, c_out, c_err) = run_git(&git, &git_root, &cfg_parts, &check_args)?;
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

    let cmd_for_log = render_command_for_log(&git_root, &cfg_parts, &args);
    let (code, stdout, stderr) = run_git(&git, &git_root, &cfg_parts, &args)?;

    let (mut applied_paths, mut skipped_paths, mut conflicted_paths) =
        parse_git_apply_output(&stdout, &stderr);
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

fn resolve_git_root(
    git: &GitRunner,
    cwd: &Path,
    git_config_args: &[String],
) -> io::Result<PathBuf> {
    let requested_cwd = std::fs::canonicalize(cwd)?;
    let mut command = git.command();
    command
        .args(git_config_args)
        .arg("rev-parse")
        .arg("--show-toplevel")
        .current_dir(&requested_cwd);
    let out = git.output(command)?;
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
    let expected_root = crate::get_git_repo_root(&requested_cwd)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "refusing to apply a patch because Git resolved worktree {} without a .git marker above requested cwd {}",
                    root.display(),
                    requested_cwd.display()
                ),
            )
        })
        .and_then(std::fs::canonicalize)?;
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

fn configured_git_config_parts() -> Vec<String> {
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

pub(crate) fn run_git(
    git: &GitRunner,
    cwd: &Path,
    git_cfg: &[String],
    args: &[String],
) -> io::Result<(i32, String, String)> {
    let mut cmd = git.command();
    for p in git_cfg {
        cmd.arg(p);
    }
    for a in args {
        cmd.arg(a);
    }
    cmd.current_dir(cwd);
    let out = git.output(cmd)?;
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    Ok((code, stdout, stderr))
}

pub(crate) fn safe_git_config_parts() -> Vec<String> {
    vec![
        "-c".to_string(),
        format!("core.hooksPath={DISABLED_HOOKS_PATH}"),
        "-c".to_string(),
        FsmonitorOverride::Disabled.git_config_arg().to_string(),
    ]
}

fn quote_shell(s: &str) -> String {
    let simple = s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "-_.:/@%+".contains(c));
    if simple {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

fn render_command_for_log(cwd: &Path, git_cfg: &[String], args: &[String]) -> String {
    let mut parts: Vec<String> = Vec::new();
    parts.push("git".to_string());
    for a in git_cfg {
        parts.push(quote_shell(a));
    }
    for a in args {
        parts.push(quote_shell(a));
    }
    format!(
        "(cd {} && {})",
        quote_shell(&cwd.display().to_string()),
        parts.join(" ")
    )
}

#[cfg(all(test, unix))]
#[path = "apply_transport_tests.rs"]
mod transport_tests;

#[cfg(test)]
#[path = "apply_filter_tests.rs"]
mod filter_tests;

#[cfg(test)]
mod tests {
    use super::*;
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

    fn commit_filter_attributes(root: &Path, tracked_path: &str) {
        std::fs::write(
            root.join(".gitattributes"),
            format!("{tracked_path} filter=x=y\n"),
        )
        .expect("write attributes");
        let (add_code, _, add_err) = run(root, &["git", "add", ".gitattributes"]);
        assert_eq!(add_code, 0, "add attributes: {add_err}");
        let (commit_code, _, commit_err) = run(root, &["git", "commit", "-m", "attributes"]);
        assert_eq!(commit_code, 0, "commit attributes: {commit_err}");
    }

    fn configure_clean_filter(root: &Path, tracked_path: &str) {
        commit_filter_attributes(root, tracked_path);
        let (config_code, _, config_err) = run(
            root,
            &[
                "git",
                "config",
                "filter.x=y.clean",
                "git config codex.filterran true && git hash-object --stdin",
            ],
        );
        assert_eq!(config_code, 0, "configure filter: {config_err}");
    }

    fn configure_worktree_clean_filter(root: &Path, tracked_path: &str) {
        commit_filter_attributes(root, tracked_path);
        let (extension_code, _, extension_err) = run(
            root,
            &["git", "config", "extensions.worktreeConfig", "true"],
        );
        assert_eq!(extension_code, 0, "enable worktree config: {extension_err}");
        let (config_code, _, config_err) = run(
            root,
            &[
                "git",
                "config",
                "--worktree",
                "filter.x=y.clean",
                "git config codex.filterran true && git hash-object --stdin",
            ],
        );
        assert_eq!(config_code, 0, "configure worktree filter: {config_err}");
    }

    fn configured_filter_ran(root: &Path) -> bool {
        let (code, _, _) = run(root, &["git", "config", "--get", "codex.filterran"]);
        code == 0
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
        let r = apply_git_patch(&req).expect("run apply");
        assert_eq!(r.exit_code, 0, "exit code 0");
        // File exists now
        assert!(root.join("hello.txt").exists());
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
        let res_apply = apply_git_patch(&apply_req).expect("apply ok");
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
        let res_revert = apply_git_patch(&revert_req).expect("revert ok");
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
        let res_preflight = apply_git_patch(&preflight_req).expect("preflight ok");
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
        let r1 = apply_git_patch(&req1).expect("preflight apply");
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
        let r2 = apply_git_patch(&req2).expect("direct apply");
        assert_ne!(r2.exit_code, 0, "apply is expected to fail overall");
        assert!(
            !r2.cmd_for_log.contains("--check"),
            "non-preflight path should not use --check"
        );
    }

    #[test]
    fn apply_rejects_configured_clean_filter_without_running_it() {
        let _g = env_lock().lock().unwrap();
        let repo = init_repo();
        let root = repo.path();
        std::fs::write(root.join("file.txt"), "orig\n").expect("write file");
        let (add_code, _, add_err) = run(root, &["git", "add", "file.txt"]);
        assert_eq!(add_code, 0, "add file: {add_err}");
        let (commit_code, _, commit_err) = run(root, &["git", "commit", "-m", "seed"]);
        assert_eq!(commit_code, 0, "commit file: {commit_err}");
        configure_clean_filter(root, "file.txt");

        let diff = "diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n@@ -1,1 +1,1 @@\n-orig\n+next\n";
        for (revert, preflight) in [(false, false), (false, true), (true, false), (true, true)] {
            let request = ApplyGitRequest {
                cwd: root.to_path_buf(),
                diff: diff.to_string(),
                revert,
                preflight,
            };
            let error = apply_git_patch(&request).expect_err("reject configured filter");
            assert_eq!(error.kind(), io::ErrorKind::Unsupported);
            assert!(!configured_filter_ran(root));
            assert_eq!(read_file_normalized(&root.join("file.txt")), "orig\n");
        }
    }

    #[test]
    fn apply_rejects_worktree_scoped_clean_filter_without_running_it() {
        let _g = env_lock().lock().unwrap();
        let repo = init_repo();
        let root = repo.path();
        std::fs::write(root.join("file.txt"), "orig\n").expect("write file");
        let (add_code, _, add_err) = run(root, &["git", "add", "file.txt"]);
        assert_eq!(add_code, 0, "add file: {add_err}");
        let (commit_code, _, commit_err) = run(root, &["git", "commit", "-m", "seed"]);
        assert_eq!(commit_code, 0, "commit file: {commit_err}");
        configure_worktree_clean_filter(root, "file.txt");

        let request = ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: "diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n@@ -1,1 +1,1 @@\n-orig\n+next\n".to_string(),
            revert: false,
            preflight: true,
        };
        let error = apply_git_patch(&request).expect_err("reject worktree filter");
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
        assert!(!configured_filter_ran(root));
        assert_eq!(read_file_normalized(&root.join("file.txt")), "orig\n");
    }

    #[test]
    fn apply_probe_rejects_command_scoped_clean_filter() {
        let _g = env_lock().lock().unwrap();
        if std::env::var_os("CODEX_GIT_UTILS_APPLY_ENV_CHILD").is_none() {
            run_isolated_test(
                "apply::tests::apply_probe_rejects_command_scoped_clean_filter",
                &[(
                    "CODEX_APPLY_GIT_CFG",
                    OsStr::new(
                        "filter.codex-test.clean=git config codex.filterran true && git hash-object --stdin",
                    ),
                )],
            );
            return;
        }

        let repo = init_repo();
        let root = repo.path();
        std::fs::write(root.join("test.txt"), "orig\n").expect("write file");
        let (add_code, _, add_err) = run(root, &["git", "add", "test.txt"]);
        assert_eq!(add_code, 0, "add file: {add_err}");
        let (commit_code, _, commit_err) = run(root, &["git", "commit", "-m", "seed"]);
        assert_eq!(commit_code, 0, "commit file: {commit_err}");
        std::fs::write(root.join(".gitattributes"), "test.txt filter=codex-test\n")
            .expect("attributes");

        let request = ApplyGitRequest {
            cwd: root.to_path_buf(),
            diff: "diff --git a/test.txt b/test.txt\n--- a/test.txt\n+++ b/test.txt\n@@ -1 +1 @@\n-orig\n+next\n".to_string(),
            revert: false,
            preflight: true,
        };
        let error = apply_git_patch(&request).expect_err("reject command-scoped filter");
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
        assert!(!configured_filter_ran(root));
        assert_eq!(read_file_normalized(&root.join("test.txt")), "orig\n");
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
            let error =
                resolve_git_root(&git, &attacker, &[]).expect_err("reject redirected worktree");
            assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
            assert!(error.to_string().contains("instead of expected worktree"));
        }
    }
}
