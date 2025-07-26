use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::fs;
use chrono::Local;
use codex_common::elapsed::format_duration;

#[derive(Debug, Parser)]
pub struct TasksCli {
    #[command(subcommand)]
    pub cmd: TasksCommand,
}

#[derive(Debug, Subcommand)]
pub enum TasksCommand {
    /// List background concurrent tasks (from ~/.codex/tasks.jsonl)
    Ls(TasksListArgs),
}

#[derive(Debug, Parser)]
pub struct TasksListArgs {
    /// Output raw JSON instead of table
    #[arg(long)]
    pub json: bool,
    /// Limit number of tasks displayed (most recent first)
    #[arg(long)]
    pub limit: Option<usize>,
    /// Show completed tasks as well (by default only running tasks)
    #[arg(short = 'a', long = "all")]
    pub all: bool,
    /// Show all columns including prompt text
    #[arg(long = "all-columns")]
    pub all_columns: bool,
}

#[derive(Debug, Deserialize)]
struct RawRecord {
    task_id: Option<String>,
    pid: Option<u64>,
    worktree: Option<String>,
    branch: Option<String>,
    original_branch: Option<String>,
    original_commit: Option<String>,
    log_path: Option<String>,
    prompt: Option<String>,
    model: Option<String>,
    start_time: Option<u64>,
    update_time: Option<u64>,
    token_count: Option<serde_json::Value>,
    state: Option<String>,
    completion_time: Option<u64>,
    end_time: Option<u64>,
}

#[derive(Debug, Serialize, Default, Clone)]
struct TaskAggregate {
    task_id: String,
    pid: Option<u64>,
    branch: Option<String>,
    worktree: Option<String>,
    prompt: Option<String>,
    model: Option<String>,
    start_time: Option<u64>,
    last_update_time: Option<u64>,
    total_tokens: Option<u64>,
    state: Option<String>,
    end_time: Option<u64>,
}

pub fn run_tasks(cmd: TasksCli) -> anyhow::Result<()> {
    match cmd.cmd {
        TasksCommand::Ls(args) => list_tasks(args),
    }
}

fn base_dir() -> Option<std::path::PathBuf> {
    if let Ok(val) = std::env::var("CODEX_HOME") { if !val.is_empty() { return std::fs::canonicalize(val).ok(); } }
    let home = std::env::var_os("HOME")?;
    let base = std::path::PathBuf::from(home).join(".codex");
    Some(base)
}

fn list_tasks(args: TasksListArgs) -> anyhow::Result<()> {
    let Some(base) = base_dir() else {
        println!("No home directory found; cannot locate tasks.jsonl");
        return Ok(());
    };
    let path = base.join("tasks.jsonl");
    if !path.exists() {
        println!("No tasks.jsonl found (no concurrent tasks recorded yet)");
        return Ok(());
    }

    let f = File::open(&path)?;
    let reader = BufReader::new(f);

    let mut agg: HashMap<String, TaskAggregate> = HashMap::new();
    for line_res in reader.lines() {
        let line = match line_res { Ok(l) => l, Err(_) => continue };
        if line.trim().is_empty() { continue; }
        let raw: serde_json::Value = match serde_json::from_str(&line) { Ok(v) => v, Err(_) => continue };
        let rec: RawRecord = match serde_json::from_value(raw) { Ok(r) => r, Err(_) => continue };
        let Some(task_id) = rec.task_id.clone() else { continue }; // must have task_id
        let entry = agg.entry(task_id.clone()).or_insert_with(|| TaskAggregate { task_id: task_id.clone(), ..Default::default() });
        if rec.start_time.is_some() { // initial metadata line
            entry.pid = rec.pid.or(entry.pid);
            entry.branch = rec.branch.or(entry.branch.clone());
            entry.worktree = rec.worktree.or(entry.worktree.clone());
            entry.prompt = rec.prompt.or(entry.prompt.clone());
            entry.model = rec.model.or(entry.model.clone());
            entry.start_time = rec.start_time.or(entry.start_time);
        }
        if let Some(tc_val) = rec.token_count.as_ref() { if tc_val.is_object() { if let Some(total) = tc_val.get("total_tokens").and_then(|v| v.as_u64()) { entry.total_tokens = Some(total); } } }
        if rec.update_time.is_some() { entry.last_update_time = rec.update_time; }
        if let Some(state) = rec.state { entry.state = Some(state); }
        if rec.completion_time.is_some() || rec.end_time.is_some() {
            entry.end_time = rec.end_time.or(rec.completion_time).or(entry.end_time);
        }
    }

    // Collect and sort by start_time desc
    let mut tasks: Vec<TaskAggregate> = agg.into_values().collect();
    tasks.sort_by_key(|j| std::cmp::Reverse(j.start_time.unwrap_or(0)));

    if !args.all { tasks.retain(|j| j.state.as_deref() != Some("done")); }
    if let Some(limit) = args.limit { tasks.truncate(limit); }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&tasks)?);
        return Ok(());
    }

    if tasks.is_empty() {
        println!("No tasks found");
        return Ok(());
    }

    // Table header
    if args.all_columns {
        println!("\x1b[1m{:<8} {:>6} {:<22} {:<12} {:<8} {:>8} {:<12} {}\x1b[0m", "TASK_ID", "PID", "BRANCH", "START", "STATE", "TOKENS", "MODEL", "PROMPT");
    } else {
        // Widened branch column to 22 chars for better readability.
        println!("\x1b[1m{:<8} {:>6} {:<22} {:<12} {:<8} {:>8} {:<12}\x1b[0m", "TASK_ID", "PID", "BRANCH", "START", "STATE", "TOKENS", "MODEL");
    }
    for t in tasks {
        let task_short = if t.task_id.len() > 8 { &t.task_id[..8] } else { &t.task_id };
        let pid_str = t.pid.map(|p| p.to_string()).unwrap_or_default();
        let mut branch = t.branch.clone().unwrap_or_default();
        let branch_limit = if args.all_columns { 22 } else { 22 }; // unified width
        if branch.len() > branch_limit { branch.truncate(branch_limit); }
        let start = t.start_time.map(|start_secs| {
            let now = Local::now().timestamp() as u64;
            if now > start_secs {
                let elapsed = std::time::Duration::from_secs(now - start_secs);
                format!("{} ago", format_duration(elapsed))
            } else {
                "just now".to_string()
            }
        }).unwrap_or_default();
        let tokens = t.total_tokens.map(|t| t.to_string()).unwrap_or_default();
        let state = t.state.clone().unwrap_or_else(|| "?".into());
        let mut model = t.model.clone().unwrap_or_default();
        if model.trim().is_empty() { model = resolve_default_model(); }
        if model.is_empty() { model.push('-'); }
        if model.len() > 12 { model.truncate(12); }
        if args.all_columns {
            let mut prompt = t.prompt.clone().unwrap_or_default().replace('\n', " ");
            if prompt.len() > 60 { prompt.truncate(60); }
            println!("{:<8} {:>6} {:<22} {:<12} {:<8} {:>8} {:<12} {}", task_short, pid_str, branch, start, state, tokens, model, prompt);
        } else {
            println!("{:<8} {:>6} {:<22} {:<12} {:<8} {:>8} {:<12}", task_short, pid_str, branch, start, state, tokens, model);
        }
    }

    Ok(())
}

fn resolve_default_model() -> String {
    // Attempt to read config json/yaml for model, otherwise fallback to hardcoded default.
    if let Some(base) = base_dir() {
        let candidates = ["config.json", "config.yaml", "config.yml"];
        for name in candidates {
            let p = base.join(name);
            if p.exists() {
                if let Ok(raw) = fs::read_to_string(&p) {
                    // Try JSON first.
                    if name.ends_with(".json") {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                            if let Some(m) = v.get("model").and_then(|x| x.as_str()) {
                                if !m.trim().is_empty() { return m.to_string(); }
                            }
                        }
                    } else {
                        // Very lightweight YAML parse: look for line starting with model:
                        for line in raw.lines() {
                            if let Some(rest) = line.trim().strip_prefix("model:") {
                                let val = rest.trim().trim_matches('"');
                                if !val.is_empty() {
                                    return val.to_string();
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    // Fallback default agentic model used elsewhere.
    "codex-mini-latest".to_string()
} 