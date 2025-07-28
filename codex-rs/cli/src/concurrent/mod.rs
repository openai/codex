use std::fs::File;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::io::Write; // added for write_all / flush

use anyhow::Context;
use codex_common::ApprovalModeCliArg;
use codex_tui::Cli as TuiCli;

/// Attempt to handle a concurrent background run. Returns Ok(true) if a background exec
/// process was spawned (in which case the caller should NOT start the TUI), or Ok(false)
/// to proceed with normal interactive execution.
pub fn maybe_spawn_concurrent(
    tui_cli: &mut TuiCli,
    root_raw_overrides: &[String],
    concurrent: bool,
    concurrent_automerge: Option<bool>,
    concurrent_branch_name: &Option<String>,
) -> anyhow::Result<bool> {
    if !concurrent { return Ok(false); }

    // Enforce autonomous execution conditions when running interactive mode.
    // Validate git repository presence (required for --concurrent) only if we're in interactive path.
    {
        let dir_to_check = tui_cli
            .cwd
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let status = Command::new("git")
            .arg("-C")
            .arg(&dir_to_check)
            .arg("rev-parse")
            .arg("--git-dir")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if status.as_ref().map(|s| !s.success()).unwrap_or(true) {
            eprintln!(
                "Error: --concurrent requires a git repository (directory {:?} is not managed by git).",
                dir_to_check
            );
            std::process::exit(2);
        }
    }

    let ap = tui_cli.approval_policy;
    let approval_on_failure = matches!(ap, Some(ApprovalModeCliArg::OnFailure));
    // (removed unused `autonomous` variable – full_auto logic applied directly below where needed)

    // Build exec args from interactive CLI for autonomous run without TUI (background).
    // todo: pap dynamically get those
    let mut worker_args: Vec<String> = Vec::new();
    // Map model/profile directly.
    if let Some(model) = &tui_cli.model { worker_args.push("--model".into()); worker_args.push(model.clone()); }
    if let Some(profile) = &tui_cli.config_profile { worker_args.push("--profile".into()); worker_args.push(profile.clone()); }
    // Derive approval-policy & sandbox (respect explicit flags first, then full-auto / dangerous shortcuts).
    let mut approval_policy: Option<String> = tui_cli.approval_policy.map(|a| format!("{a:?}").to_lowercase().replace('_', "-"));
    let mut sandbox: Option<String> = tui_cli.sandbox_mode.map(|s| format!("{s:?}").to_lowercase().replace('_', "-"));
    if approval_policy.is_none() && tui_cli.full_auto { approval_policy = Some("on-failure".into()); }
    if sandbox.is_none() && tui_cli.full_auto { sandbox = Some("workspace-write".into()); }
    if tui_cli.dangerously_bypass_approvals_and_sandbox { approval_policy = Some("never".into()); sandbox = Some("danger-full-access".into()); }
    if let Some(ap) = approval_policy { worker_args.push("--approval-policy".into()); worker_args.push(ap); }
    if let Some(sb) = sandbox { worker_args.push("--sandbox".into()); worker_args.push(sb); }
    // Config overrides (-c) from root and interactive CLI.
    for raw in root_raw_overrides { worker_args.push("--worker-config".into()); worker_args.push(raw.clone()); }
    for raw in &tui_cli.config_overrides.raw_overrides { worker_args.push("--worker-config".into()); worker_args.push(raw.clone()); }

    // Derive a single slug (shared by worktree branch & log filename) from the prompt.
    let raw_prompt = tui_cli.prompt.as_deref().unwrap_or("");
    let snippet = raw_prompt.chars().take(32).collect::<String>();
    let mut slug: String = snippet
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect();
    while slug.contains("--") { slug = slug.replace("--", "-"); }
    slug = slug.trim_matches('-').to_string();
    if slug.is_empty() { slug = "prompt".into(); }

    // Determine concurrent defaults from env (no config file), then apply CLI precedence.
    let env_automerge = parse_env_bool("CONCURRENT_AUTOMERGE");
    let env_branch_name = std::env::var("CONCURRENT_BRANCH_NAME").ok();
    let effective_automerge = concurrent_automerge.or(env_automerge).unwrap_or(true);
    let user_branch_name_opt = concurrent_branch_name.clone().or(env_branch_name);
    let branch_name_effective = if let Some(bn_raw) = user_branch_name_opt.as_ref() {
        let bn_trim = bn_raw.trim();
        if bn_trim.is_empty() { format!("codex/{slug}") } else { bn_trim.to_string() }
    } else {
        format!("codex/{slug}")
    };

    // Unique job id for this concurrent run (used for log file naming instead of slug).
    let task_id = uuid::Uuid::new_v4().to_string();

    // Prepare log file path early so we can write pre-spawn logs (e.g. worktree creation output) into it.
    let log_dir = match codex_base_dir() {
        Ok(base) => {
            let d = base.join("log");
            let _ = std::fs::create_dir_all(&d);
            d
        }
        Err(_) => PathBuf::from("/tmp"),
    };
    let log_path = log_dir.join(format!("codex-logs-{}.log", task_id));

    // If user did NOT specify an explicit cwd, create an isolated git worktree.
    let mut created_worktree: Option<(PathBuf, String)> = None; // (path, branch)
    let mut original_branch: Option<String> = None;
    let mut original_commit: Option<String> = None;
    let mut pre_spawn_logs = String::new();
    if tui_cli.cwd.is_none() {
        original_branch = git_capture(["rev-parse", "--abbrev-ref", "HEAD"]).ok();
        original_commit = git_capture(["rev-parse", "HEAD"]).ok();
        match create_concurrent_worktree(&branch_name_effective) {
            Ok(Some(info)) => {
                // Record worktree path to pass as --cwd to worker
                worker_args.push("--cwd".into());
                worker_args.push(info.worktree_path.display().to_string());
                created_worktree = Some((info.worktree_path, info.branch_name.clone()));
                // Keep the original git output plus a concise created line (for log file only).
                pre_spawn_logs.push_str(&info.logs);
                pre_spawn_logs.push_str(&format!(
                    "Created git worktree at {} (branch {}) for concurrent run\n",
                    created_worktree.as_ref().unwrap().0.display(), info.branch_name
                ));
            }
            Ok(None) => {
                // Silence console noise: do not warn here to keep stdout clean; we still proceed.
            }
            Err(e) => {
                eprintln!("Error: failed to create git worktree for --concurrent: {e}");
                eprintln!("Hint: remove or rename existing branch '{branch_name_effective}', or pass --concurrent-branch-name to choose a unique name.");
                std::process::exit(3);
            }
        }
    } else if let Some(explicit) = &tui_cli.cwd {
        worker_args.push("--cwd".into());
        worker_args.push(explicit.display().to_string());
    }

    // Prompt (safe to unwrap due to earlier validation in autonomous case). For non-autonomous
    // (interactive later) runs we intentionally do NOT pass the prompt to the subprocess so it
    // will wait for a Submission over stdin.
    if let Some(prompt) = tui_cli.prompt.clone() { worker_args.push("--prompt".into()); worker_args.push(prompt); } else { eprintln!("Error: --concurrent requires a prompt."); return Ok(false); }

    // Create (or truncate) the log file and write any pre-spawn logs we captured.
    let file = match File::create(&log_path) {
        Ok(mut f) => {
            if !pre_spawn_logs.is_empty() {
                let _ = f.write_all(pre_spawn_logs.as_bytes());
                let _ = f.flush();
            }
            f
        }
        Err(e) => {
            eprintln!("Failed to create log file {}: {e}. Falling back to interactive mode.", log_path.display());
            return Ok(false);
        }
    };
    let file_err = file.try_clone().ok();
    let mut cmd = Command::new(
        std::env::current_exe().unwrap_or_else(|_| PathBuf::from("codex"))
    );
    cmd.arg("worker");
    for a in &worker_args { cmd.arg(a); }
    if let Some((wt_path, branch)) = &created_worktree {
        if effective_automerge { cmd.env("CODEX_CONCURRENT_AUTOMERGE", "1"); }
        cmd.env("CODEX_CONCURRENT_BRANCH", branch);
        cmd.env("CODEX_CONCURRENT_WORKTREE", wt_path);
        if let Some(ob) = &original_branch { cmd.env("CODEX_ORIGINAL_BRANCH", ob); }
        if let Some(oc) = &original_commit { cmd.env("CODEX_ORIGINAL_COMMIT", oc); }
        if let Ok(orig_root) = std::env::current_dir() { cmd.env("CODEX_ORIGINAL_ROOT", orig_root); }
    }
    cmd.env("CODEX_TASK_ID", &task_id);
    cmd.stdout(Stdio::from(file));
    if let Some(f2) = file_err { cmd.stderr(Stdio::from(f2)); }
    match cmd.spawn() {
        Ok(mut child) => {
            let branch_val = created_worktree.as_ref().map(|(_, b)| b.as_str()).unwrap_or("(none)");
            let worktree_val = created_worktree
                .as_ref()
                .map(|(p, _)| p.display().to_string())
                .unwrap_or_else(|| "(original cwd)".to_string());
            println!("\x1b[1mTask ID:\x1b[0m {}", task_id);
            println!("\x1b[1mPID:\x1b[0m {}", child.id());
            println!("\x1b[1mBranch:\x1b[0m {}", branch_val);
            println!("\x1b[1mWorktree:\x1b[0m {}", worktree_val);
            let initial_state = "started";
            println!("\x1b[1mState:\x1b[0m {}", initial_state);
            println!("\nStreaming logs (press Ctrl+C to abort view; task will continue)...\n");
            let record_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if let Ok(base) = codex_base_dir() {
                let tasks_path = base.join("tasks.jsonl");
                let record = serde_json::json!({
                    "task_id": task_id,
                    "pid": child.id(),
                    "worktree": created_worktree.as_ref().map(|(p, _)| p.display().to_string()),
                    "branch": created_worktree.as_ref().map(|(_, b)| b.clone()),
                    "original_branch": original_branch,
                    "original_commit": original_commit,
                    "log_path": log_path.display().to_string(),
                    "prompt": raw_prompt,
                    "model": tui_cli.model.clone(),
                    "start_time": record_time,
                    "automerge": effective_automerge,
                    "explicit_branch_name": user_branch_name_opt,
                    "token_count": serde_json::Value::Null,
                    "state": initial_state,
                });
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&tasks_path) {
                    use std::io::Write;
                    let _ = writeln!(f, "{}", record.to_string());
                }
            }
            if let Err(e) = stream_log_until_exit(&log_path, &mut child) {
                eprintln!("Error streaming logs: {e}");
            }
            return Ok(true);
        }
        Err(e) => {
            eprintln!("Failed to start background exec: {e}. Falling back to interactive mode.");
        }
    }

    Ok(false)
}

/// Return the base Codex directory under the user's home (~/.codex), creating it if necessary.
fn codex_base_dir() -> anyhow::Result<PathBuf> {
    if let Ok(val) = std::env::var("CODEX_HOME") {
        if !val.is_empty() {
            return Ok(PathBuf::from(val).canonicalize()?);
        }
    }
    let home = std::env::var_os("HOME").ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
    let base = PathBuf::from(home).join(".codex");
    std::fs::create_dir_all(&base)?;
    Ok(base)
}

/// Attempt to create a git worktree for an isolated concurrent run capturing git output.
struct WorktreeInfo { worktree_path: PathBuf, branch_name: String, logs: String }
fn create_concurrent_worktree(branch_name: &str) -> anyhow::Result<Option<WorktreeInfo>> {
    // Determine repository root.
    let output = Command::new("git").arg("rev-parse").arg("--show-toplevel").output();
    let repo_root = match output {
        Ok(out) if out.status.success() => {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if s.is_empty() { return Ok(None); }
            PathBuf::from(s)
        }
        _ => return Ok(None),
    };

    // Derive repo name from root directory.
    let repo_name = repo_root
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("repo");

    // Fast-fail if branch already exists.
    if Command::new("git")
        .current_dir(&repo_root)
        .arg("rev-parse")
        .arg("--verify")
        .arg(branch_name)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false) {
        anyhow::bail!("branch '{branch_name}' already exists");
    }

    // Construct worktree directory under ~/.codex/worktrees/<repo_name>/.
    let base_dir = codex_base_dir()?.join("worktrees").join(repo_name);
    std::fs::create_dir_all(&base_dir)?;
    let mut worktree_path = base_dir.join(branch_name.replace('/', "-"));

    if worktree_path.exists() {
        for i in 1..1000 {
            let candidate = base_dir.join(format!("{}-{}", branch_name.replace('/', "-"), i));
            if !candidate.exists() { worktree_path = candidate; break; }
        }
    }

    // Run git worktree add capturing output (stdout+stderr).
    let add_out = Command::new("git")
        .current_dir(&repo_root)
        .arg("worktree")
        .arg("add")
        .arg("-b")
        .arg(&branch_name)
        .arg(&worktree_path)
        .arg("HEAD")
        .output()?;
    if !add_out.status.success() {
        anyhow::bail!("git worktree add failed with status {}", add_out.status);
    }
    let mut logs = String::new();
    if !add_out.stdout.is_empty() { logs.push_str(&String::from_utf8_lossy(&add_out.stdout)); }
    if !add_out.stderr.is_empty() { logs.push_str(&String::from_utf8_lossy(&add_out.stderr)); }

    Ok(Some(WorktreeInfo { worktree_path, branch_name: branch_name.to_string(), logs }))
}

/// Helper: capture trimmed stdout of a git command.
fn git_capture<I, S>(args: I) -> anyhow::Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut cmd = Command::new("git");
    for a in args { cmd.arg(a.as_ref()); }
    let out = cmd.output().context("running git command")?;
    if !out.status.success() { anyhow::bail!("git command failed"); }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Parse common boolean environment variable representations.
fn parse_env_bool(name: &str) -> Option<bool> {
    let raw = std::env::var(name).ok()?;
    let lower = raw.to_ascii_lowercase();
    match lower.as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

// Attach helper: follow the log file while the child runs.
// todo: remove this once we have a tui
fn stream_log_until_exit(log_path: &std::path::Path, child: &mut std::process::Child) -> anyhow::Result<()> {
    use std::io::{Read, Seek, SeekFrom};
    use std::time::Duration;
    let mut f = std::fs::OpenOptions::new().read(true).open(log_path)?;
    // Print any existing content first.
    let mut existing = String::new();
    f.read_to_string(&mut existing)?;
    print!("{}", existing);
    let mut pos: u64 = existing.len() as u64;
    loop {
        // Check if process has exited.
        if let Some(status) = child.try_wait()? {
            // Drain any remaining bytes.
            let mut tail = String::new();
            f.seek(SeekFrom::Start(pos))?;
            f.read_to_string(&mut tail)?;
            if !tail.is_empty() { print!("{}", tail); }
            println!("\n\x1b[1mTask exited with status: {}\x1b[0m", status);
            break;
        }
        // Read new bytes if any.
        let meta = f.metadata()?;
        let len = meta.len();
        if len > pos {
            f.seek(SeekFrom::Start(pos))?;
            let mut buf = String::new();
            f.read_to_string(&mut buf)?;
            if !buf.is_empty() { print!("{}", buf); }
            pos = len;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    Ok(())
} 
