use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;
use std::sync::OnceLock;

use tokio::process::Command as TokioCommand;
use tokio::sync::Semaphore;

use anyhow::Context;
use codex_common::CliConfigOverrides;
use codex_exec::Cli as ExecCli;

// Serialize git worktree add operations across tasks to avoid repository lock contention.
static GIT_WORKTREE_ADD_SEMAPHORE: OnceLock<Semaphore> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct ConcurrentRunResult {
    pub branch: String,
    pub worktree_dir: PathBuf,
    pub log_file: Option<PathBuf>,
    pub exec_exit_code: Option<i32>,
    pub _had_changes: bool,
    pub _applied_changes: Option<usize>,
}

fn compute_codex_home() -> PathBuf {
    if let Ok(val) = std::env::var("CODEX_HOME") {
        if !val.is_empty() {
            return PathBuf::from(val);
        }
    }
    // Fallback to default (~/.codex) without requiring it to already exist.
    codex_core::config::find_codex_home().unwrap_or_else(|_| {
        let mut p = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
        if p.as_os_str().is_empty() {
            return PathBuf::from(".codex");
        }
        p.push(".codex");
        p
    })
}

fn slugify_prompt(prompt: &str, max_len: usize) -> String {
    let mut out = String::with_capacity(prompt.len());
    let mut prev_hyphen = false;
    for ch in prompt.chars() {
        let c = ch.to_ascii_lowercase();
        let keep = matches!(c, 'a'..='z' | '0'..='9');
        if keep {
            out.push(c);
            prev_hyphen = false;
        } else if c.is_ascii_whitespace() || matches!(c, '-' | '_' | '+') {
            if !prev_hyphen && !out.is_empty() {
                out.push('-');
                prev_hyphen = true;
            }
        } else {
            // skip other punctuation/symbols
        }
        if out.len() >= max_len {
            break;
        }
    }
    // Trim trailing hyphens
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "task".to_string()
    } else {
        out
    }
}

fn git_output(repo_dir: &Path, args: &[&str]) -> anyhow::Result<String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(repo_dir)
        .output()
        .with_context(|| format!("running git {args:?}"))?;
    if !out.status.success() {
        anyhow::bail!(
            "git {:?} failed with status {}: {}",
            args,
            out.status,
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn git_capture_stdout(repo_dir: &Path, args: &[&str]) -> anyhow::Result<Vec<u8>> {
    let out = Command::new("git")
        .args(args)
        .current_dir(repo_dir)
        .output()
        .with_context(|| format!("running git {args:?}"))?;
    if !out.status.success() {
        anyhow::bail!(
            "git {:?} failed with status {}: {}",
            args,
            out.status,
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(out.stdout)
}

fn count_files_in_patch(diff: &[u8]) -> usize {
    // Count occurrences of lines starting with "diff --git ", which mark file boundaries.
    // This works for text and binary patches produced by `git diff --binary`.
    let mut count = 0usize;
    for line in diff.split(|&b| b == b'\n') {
        if line.starts_with(b"diff --git ") {
            count += 1;
        }
    }
    count
}

pub async fn run_concurrent_flow(
    prompt: String,
    cli_config_overrides: CliConfigOverrides,
    codex_linux_sandbox_exe: Option<PathBuf>,
    automerge: bool,
    quiet: bool,
) -> anyhow::Result<ConcurrentRunResult> {
    let cwd = std::env::current_dir()?;

    // Ensure we are in a git repo and find repo root.
    let repo_root_str = git_output(&cwd, &["rev-parse", "--show-toplevel"]);
    let repo_root = match repo_root_str {
        Ok(p) => PathBuf::from(p),
        Err(err) => {
            eprintln!("Not inside a Git repo: {err}");
            std::process::exit(1);
        }
    };

    // Determine current branch and original head commit.
    let current_branch = git_output(&repo_root, &["rev-parse", "--abbrev-ref", "HEAD"])
        .unwrap_or_else(|_| "HEAD".to_string());
    let original_head =
        git_output(&repo_root, &["rev-parse", "HEAD"]).context("finding original HEAD commit")?;

    // Build worktree target path under $CODEX_HOME/worktrees/<repo>/<branch>
    let mut codex_home = compute_codex_home();
    codex_home.push("worktrees");
    // repo name = last component of repo_root
    let repo_name = repo_root
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "repo".to_string());
    codex_home.push(repo_name.clone());

    // Prepare branch name: codex/<slug>, retrying with a numeric suffix to avoid races.
    let slug = slugify_prompt(&prompt, 64);
    let mut branch: String;
    let worktree_dir: PathBuf;
    let mut attempt: u32 = 1;
    loop {
        branch = if attempt == 1 {
            format!("codex/{slug}")
        } else {
            format!("codex/{slug}-{attempt}")
        };

        let mut candidate_dir = codex_home.clone();
        candidate_dir.push(&branch);

        // Create parent directories for candidate path.
        if let Some(parent) = candidate_dir.parent() {
            std::fs::create_dir_all(parent)?;
        }

        if !quiet {
            println!(
                "Creating worktree at {} with branch {}",
                candidate_dir.display(),
                branch
            );
        }

        // Try to add worktree with new branch from current HEAD
        let worktree_path_str = candidate_dir.to_string_lossy().to_string();
        let add_status = Command::new("git")
            .arg("worktree")
            .arg("add")
            .arg("-b")
            .arg(&branch)
            .arg(&worktree_path_str)
            .current_dir(&repo_root)
            .status()?;
        if add_status.success() {
            worktree_dir = candidate_dir;
            break;
        }

        attempt += 1;
        if attempt > 50 {
            anyhow::bail!("Failed to create git worktree after multiple attempts");
        }
        // Retry with a new branch name.
    }

    // Either run codex exec inline (verbose) or as a subprocess with logs redirected.
    let mut log_file: Option<PathBuf> = None;
    let mut exec_exit_code: Option<i32> = None;
    if quiet {
        let exe = std::env::current_exe()
            .map_err(|e| anyhow::anyhow!("failed to locate current executable: {e}"))?;

        // Prepare logs directory: $CODEX_HOME/logs/<repo_name>
        let mut logs_dir = compute_codex_home();
        logs_dir.push("logs");
        logs_dir.push(&repo_name);
        std::fs::create_dir_all(&logs_dir)?;

        let sanitized_branch = branch.replace('/', "_");
        let log_path = logs_dir.join(format!("{sanitized_branch}.log"));
        let log_f = File::create(&log_path)?;
        log_file = Some(log_path.clone());

        let mut cmd = Command::new(exe);
        cmd.arg("exec")
            .arg("--full-auto")
            .arg("--cd")
            .arg(worktree_dir.as_os_str())
            .stdout(Stdio::from(log_f.try_clone()?))
            .stderr(Stdio::from(log_f));

        // Forward any root-level config overrides.
        for ov in cli_config_overrides.raw_overrides.iter() {
            cmd.arg("-c").arg(ov);
        }

        // Append the prompt last (positional argument).
        cmd.arg(&prompt);

        let status = cmd.status()?;
        exec_exit_code = status.code();
        if !status.success() && !quiet {
            eprintln!("codex exec failed with exit code {exec_exit_code:?}");
        }
    } else {
        // Build an ExecCli to run in full-auto mode at the worktree directory.
        let mut exec_cli = ExecCli {
            images: vec![],
            model: None,
            sandbox_mode: None,
            config_profile: None,
            full_auto: true,
            dangerously_bypass_approvals_and_sandbox: false,
            cwd: Some(worktree_dir.clone()),
            skip_git_repo_check: false,
            config_overrides: CliConfigOverrides::default(),
            color: Default::default(),
            json: false,
            last_message_file: None,
            prompt: Some(prompt.clone()),
        };

        // Prepend any root-level config overrides.
        super::prepend_config_flags(&mut exec_cli.config_overrides, cli_config_overrides);

        // Run codex exec
        if let Err(e) = codex_exec::run_main(exec_cli, codex_linux_sandbox_exe).await {
            eprintln!("codex exec failed: {e}");
            // Do not attempt to bring changes on failure; leave worktree for inspection.
            return Err(e);
        }
    }

    // Auto-commit changes in the worktree if any
    let status_out = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&worktree_dir)
        .output()?;
    let status_text = String::from_utf8_lossy(&status_out.stdout);
    let had_changes = !status_text.trim().is_empty();
    if had_changes {
        // Stage and commit
        if !Command::new("git")
            .args(["add", "-A"])
            .current_dir(&worktree_dir)
            .status()?
            .success()
        {
            anyhow::bail!("git add failed in worktree");
        }
        let commit_message = format!("Codex concurrent: {prompt}");
        if !Command::new("git")
            .args(["commit", "-m", &commit_message])
            .current_dir(&worktree_dir)
            .status()?
            .success()
        {
            if !quiet {
                eprintln!("No commit created (maybe no changes)");
            }
        } else if !quiet {
            println!("Committed changes in worktree branch {branch}");
        }
    } else if !quiet {
        println!("No changes detected in worktree; skipping commit.");
    }

    if !automerge {
        if !quiet {
            println!(
                "Auto-merge disabled; leaving changes in worktree {} on branch {}.",
                worktree_dir.display(),
                branch
            );
            println!(
                "You can review and manually merge from that branch into {current_branch} when ready."
            );
            println!("Summary: Auto-merge disabled.");
        }
        return Ok(ConcurrentRunResult {
            branch,
            worktree_dir,
            log_file,
            exec_exit_code,
            _had_changes: had_changes,
            _applied_changes: None,
        });
    }

    // Bring the changes into the main working tree as UNSTAGED modifications.
    // We generate a patch from the original HEAD to the worktree branch tip, then apply with 3-way merge.
    if !quiet {
        println!("Applying changes from {branch} onto {current_branch} as unstaged modifications");
    }
    let range = format!("{original_head}..{branch}");
    let mut diff_bytes =
        git_capture_stdout(&repo_root, &["diff", "--binary", "--full-index", &range])?;

    // Fallback: if there is nothing in the commit range (e.g., commit didn't happen),
    // try to capture uncommitted changes from the worktree working tree.
    if diff_bytes.is_empty() && had_changes {
        // If we saw changes earlier but no commit diff was produced, fall back to working tree diff.
        // This captures unstaged changes relative to HEAD in the worktree.
        diff_bytes =
            git_capture_stdout(&worktree_dir, &["diff", "--binary", "--full-index", "HEAD"])?;
    }

    if diff_bytes.is_empty() {
        if !quiet {
            println!("Summary: 0 changes detected.");
        }
        return Ok(ConcurrentRunResult {
            branch,
            worktree_dir,
            log_file,
            exec_exit_code,
            _had_changes: had_changes,
            _applied_changes: Some(0),
        });
    }

    let changed_files = count_files_in_patch(&diff_bytes);

    let mut child = Command::new("git")
        .arg("apply")
        .arg("-3")
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .current_dir(&repo_root)
        .spawn()
        .context("spawning git apply")?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(&diff_bytes)
            .context("writing patch to git apply stdin")?;
    }
    let status = child.wait().context("waiting for git apply")?;
    if !status.success() {
        if !quiet {
            eprintln!(
                "Applying changes failed. You can manually inspect {} and apply diffs.",
                worktree_dir.display()
            );
            println!("Summary: Apply failed.");
        }
    } else {
        if !quiet {
            println!("Changes applied to working tree (unstaged).");
            println!("Summary: Applied {changed_files} files changed.");
        }

        // Cleanup: remove the worktree and delete the temporary branch.
        if !quiet {
            println!(
                "Cleaning up worktree {} and branch {}",
                worktree_dir.display(),
                branch
            );
        }
        let worktree_path_str = worktree_dir.to_string_lossy().to_string();
        let remove_status = Command::new("git")
            .args(["worktree", "remove", &worktree_path_str])
            .current_dir(&repo_root)
            .status();
        match remove_status {
            Ok(s) if s.success() => {
                // removed
            }
            _ => {
                if !quiet {
                    eprintln!("git worktree remove failed; retrying with --force");
                }
                let _ = Command::new("git")
                    .args(["worktree", "remove", "--force", &worktree_path_str])
                    .current_dir(&repo_root)
                    .status();
            }
        }

        let del_status = Command::new("git")
            .args(["branch", "-D", &branch])
            .current_dir(&repo_root)
            .status();
        if let Ok(s) = del_status {
            if !s.success() && !quiet {
                eprintln!("Failed to delete branch {branch}");
            }
        } else if !quiet {
            eprintln!("Error running git branch -D {branch}");
        }
    }

    Ok(ConcurrentRunResult {
        branch,
        worktree_dir,
        log_file,
        exec_exit_code,
        _had_changes: had_changes,
        _applied_changes: Some(changed_files),
    })
}

/// A Send-friendly variant used for best-of-n: run quietly (logs redirected) and do not auto-merge.
/// This intentionally avoids referencing non-Send types from codex-exec.
pub async fn run_concurrent_flow_quiet_no_automerge(
    prompt: String,
    cli_config_overrides: CliConfigOverrides,
    _codex_linux_sandbox_exe: Option<PathBuf>,
) -> anyhow::Result<ConcurrentRunResult> {
    let cwd = std::env::current_dir()?;

    let repo_root_str = git_output(&cwd, &["rev-parse", "--show-toplevel"]);
    let repo_root = match repo_root_str {
        Ok(p) => PathBuf::from(p),
        Err(err) => {
            eprintln!("Not inside a Git repo: {err}");
            std::process::exit(1);
        }
    };

    // Capture basic repo info (not used further in quiet/no-automerge flow).

    let mut codex_home = compute_codex_home();
    codex_home.push("worktrees");
    let repo_name = repo_root
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "repo".to_string());
    codex_home.push(repo_name.clone());

    let slug = slugify_prompt(&prompt, 64);
    let mut branch: String;
    let worktree_dir: PathBuf;
    // Serialize worktree creation to avoid git repo lock contention across tasks.
    {
        let semaphore = GIT_WORKTREE_ADD_SEMAPHORE.get_or_init(|| Semaphore::new(1));
        let _permit = semaphore.acquire().await.expect("semaphore closed");

        let mut attempt: u32 = 1;
        loop {
            branch = if attempt == 1 {
                format!("codex/{slug}")
            } else {
                format!("codex/{slug}-{attempt}")
            };

            let mut candidate_dir = codex_home.clone();
            candidate_dir.push(&branch);

            if let Some(parent) = candidate_dir.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let worktree_path_str = candidate_dir.to_string_lossy().to_string();
            let add_status = TokioCommand::new("git")
                .arg("worktree")
                .arg("add")
                .arg("-b")
                .arg(&branch)
                .arg(&worktree_path_str)
                .current_dir(&repo_root)
                .status()
                .await?;
            if add_status.success() {
                worktree_dir = candidate_dir;
                break;
            }
            attempt += 1;
            if attempt > 50 {
                anyhow::bail!("Failed to create git worktree after multiple attempts");
            }
        }
    }

    // Run the CLI in quiet mode (logs redirected).
    let exe = std::env::current_exe()
        .map_err(|e| anyhow::anyhow!("failed to locate current executable: {e}"))?;

    let mut logs_dir = compute_codex_home();
    logs_dir.push("logs");
    logs_dir.push(&repo_name);
    std::fs::create_dir_all(&logs_dir)?;

    let sanitized_branch = branch.replace('/', "_");
    let log_path = logs_dir.join(format!("{sanitized_branch}.log"));
    let log_f = File::create(&log_path)?;
    let log_file = Some(log_path.clone());

    let mut cmd = TokioCommand::new(exe);
    cmd.arg("exec")
        .arg("--full-auto")
        .arg("--cd")
        .arg(worktree_dir.as_os_str())
        .stdout(Stdio::from(log_f.try_clone()?))
        .stderr(Stdio::from(log_f));
    for ov in cli_config_overrides.raw_overrides.iter() {
        cmd.arg("-c").arg(ov);
    }
    cmd.arg(&prompt);

    let status = cmd.status().await?;
    let exec_exit_code = status.code();

    // Auto-commit changes in the worktree if any
    let status_out = TokioCommand::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&worktree_dir)
        .output()
        .await?;
    let status_text = String::from_utf8_lossy(&status_out.stdout);
    let had_changes = !status_text.trim().is_empty();
    if had_changes {
        if !TokioCommand::new("git")
            .args(["add", "-A"])
            .current_dir(&worktree_dir)
            .status()
            .await?
            .success()
        {
            anyhow::bail!("git add failed in worktree");
        }
        let commit_message = format!("Codex concurrent: {prompt}");
        let _ = TokioCommand::new("git")
            .args(["commit", "-m", &commit_message])
            .current_dir(&worktree_dir)
            .status()
            .await?;
    }

    Ok(ConcurrentRunResult {
        branch,
        worktree_dir,
        log_file,
        exec_exit_code,
        _had_changes: had_changes,
        _applied_changes: None,
    })
}
