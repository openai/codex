use clap::Parser;
use serde::Deserialize;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::fs;

#[derive(Debug, Parser)]
pub struct InspectCli {
    /// Task identifier (full/short task id or exact branch name)
    pub id: String,
    /// Output JSON instead of human table
    #[arg(long)]
    pub json: bool,
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
    automerge: Option<bool>,
    explicit_branch_name: Option<String>,
}

#[derive(Debug, serde::Serialize, Default, Clone)]
struct TaskFull {
    task_id: String,
    pid: Option<u64>,
    branch: Option<String>,
    worktree: Option<String>,
    original_branch: Option<String>,
    original_commit: Option<String>,
    log_path: Option<String>,
    prompt: Option<String>,
    model: Option<String>,
    start_time: Option<u64>,
    end_time: Option<u64>,
    state: Option<String>,
    total_tokens: Option<u64>,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    reasoning_output_tokens: Option<u64>,
    automerge: Option<bool>,
    explicit_branch_name: Option<String>,
    last_update_time: Option<u64>,
    duration_secs: Option<u64>,
}

pub fn run_inspect(cli: InspectCli) -> anyhow::Result<()> {
    let id = cli.id.to_lowercase();
    let tasks = load_task_records()?;
    let matches: Vec<TaskFull> = tasks
        .into_iter()
        .filter(|t| t.task_id.starts_with(&id) || t.branch.as_deref().map(|b| b == id).unwrap_or(false))
        .collect();
    if matches.is_empty() {
        eprintln!("No task matches identifier '{}'.", id);
        return Ok(());
    }
    if matches.len() > 1 {
        eprintln!("Identifier '{}' is ambiguous; matches: {}", id, matches.iter().map(|m| &m.task_id[..8]).collect::<Vec<_>>().join(", "));
        return Ok(());
    }
    let task = &matches[0];
    if cli.json {
        println!("{}", serde_json::to_string_pretty(task)?);
        return Ok(());
    }
    print_human(task);
    Ok(())
}

fn base_dir() -> Option<PathBuf> {
    if let Ok(val) = std::env::var("CODEX_HOME") { if !val.is_empty() { return std::fs::canonicalize(val).ok(); } }
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".codex"))
}

fn load_task_records() -> anyhow::Result<Vec<TaskFull>> {
    let mut map: std::collections::HashMap<String, TaskFull> = std::collections::HashMap::new();
    let Some(base) = base_dir() else { return Ok(vec![]); };
    let tasks = base.join("tasks.jsonl");
    if !tasks.exists() { return Ok(vec![]); }
    let f = File::open(tasks)?;
    let reader = BufReader::new(f);
    for line in reader.lines() {
        let Ok(line) = line else { continue };
        if line.trim().is_empty() { continue; }
        let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) else { continue };
        let Ok(rec) = serde_json::from_value::<RawRecord>(val) else { continue };
        let Some(task_id) = rec.task_id.clone() else { continue };
        let entry = map.entry(task_id.clone()).or_insert_with(|| TaskFull { task_id: task_id.clone(), ..Default::default() });
        // Initial metadata fields
        if rec.start_time.is_some() {
            entry.pid = rec.pid.or(entry.pid);
            entry.branch = rec.branch.or(entry.branch.clone());
            entry.worktree = rec.worktree.or(entry.worktree.clone());
            entry.original_branch = rec.original_branch.or(entry.original_branch.clone());
            entry.original_commit = rec.original_commit.or(entry.original_commit.clone());
            entry.log_path = rec.log_path.or(entry.log_path.clone());
            entry.prompt = rec.prompt.or(entry.prompt.clone());
            entry.model = rec.model.or(entry.model.clone());
            entry.start_time = rec.start_time.or(entry.start_time);
            entry.automerge = rec.automerge.or(entry.automerge);
            entry.explicit_branch_name = rec.explicit_branch_name.or(entry.explicit_branch_name.clone());
        }
        if let Some(state) = rec.state { entry.state = Some(state); }
        if rec.update_time.is_some() { entry.last_update_time = rec.update_time; }
        if rec.end_time.is_some() || rec.completion_time.is_some() {
            entry.end_time = rec.end_time.or(rec.completion_time).or(entry.end_time);
        }
        if let Some(tc) = rec.token_count.as_ref() {
            if let Some(total) = tc.get("total_tokens").and_then(|v| v.as_u64()) { entry.total_tokens = Some(total); }
            if let Some(inp) = tc.get("input_tokens").and_then(|v| v.as_u64()) { entry.input_tokens = Some(inp); }
            if let Some(out) = tc.get("output_tokens").and_then(|v| v.as_u64()) { entry.output_tokens = Some(out); }
            if let Some(rout) = tc.get("reasoning_output_tokens").and_then(|v| v.as_u64()) { entry.reasoning_output_tokens = Some(rout); }
        }
    }
    // Compute duration
    for t in map.values_mut() {
        if let (Some(s), Some(e)) = (t.start_time, t.end_time) { t.duration_secs = Some(e.saturating_sub(s)); }
    }
    Ok(map.into_values().collect())
}

fn print_human(task: &TaskFull) {
    println!("Task {}", task.task_id);
    println!("State: {}", task.state.as_deref().unwrap_or("?"));
    if let Some(model) = &task.model { println!("Model: {}", model); } else { println!("Model: {}", resolve_default_model()); }
    if let Some(branch) = &task.branch { println!("Branch: {}", branch); }
    if let Some(wt) = &task.worktree { println!("Worktree: {}", wt); }
    if let Some(ob) = &task.original_branch { println!("Original branch: {}", ob); }
    if let Some(oc) = &task.original_commit { println!("Original commit: {}", oc); }
    if let Some(start) = task.start_time { println!("Start: {}", format_epoch(start)); }
    if let Some(end) = task.end_time { println!("End: {}", format_epoch(end)); }
    if let Some(d) = task.duration_secs { println!("Duration: {}s", d); }
    if let Some(pid) = task.pid { println!("PID: {}", pid); }
    if let Some(log) = &task.log_path { println!("Log: {}", log); }
    if let Some(am) = task.automerge { println!("Automerge: {}", am); }
    if let Some(exp) = &task.explicit_branch_name { println!("Explicit branch name: {}", exp); }
    if let Some(total) = task.total_tokens { println!("Total tokens: {}", total); }
    if task.input_tokens.is_some() || task.output_tokens.is_some() {
        println!("  Input: {:?} Output: {:?} Reasoning: {:?}", task.input_tokens, task.output_tokens, task.reasoning_output_tokens);
    }
    if let Some(p) = &task.prompt { println!("Prompt:\n{}", p); }
}

fn format_epoch(secs: u64) -> String {
    use chrono::{TimeZone, Utc};
    if let Some(dt) = Utc.timestamp_opt(secs as i64, 0).single() { dt.to_rfc3339() } else { secs.to_string() }
}

fn resolve_default_model() -> String {
    if let Some(base) = base_dir() {
        let candidates = ["config.json", "config.yaml", "config.yml"];
        for name in candidates {
            let p = base.join(name);
            if p.exists() {
                if let Ok(raw) = fs::read_to_string(&p) {
                    if name.ends_with(".json") {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                            if let Some(m) = v.get("model").and_then(|x| x.as_str()) { if !m.trim().is_empty() { return m.to_string(); } }
                        }
                    } else {
                        for line in raw.lines() { if let Some(rest) = line.trim().strip_prefix("model:") { let val = rest.trim().trim_matches('"'); if !val.is_empty() { return val.to_string(); } } }
                    }
                }
            }
        }
    }
    "codex-mini-latest".to_string()
} 