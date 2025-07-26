use clap::Parser;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

#[derive(Debug, Parser)]
pub struct LogsCli {
    /// Task identifier: full/short task UUID or branch name
    pub id: String,
    /// Follow log output (stream new lines)
    #[arg(short = 'f', long = "follow")]
    pub follow: bool,
    /// Show only the last N lines (like tail -n). If omitted, show full file.
    #[arg(short = 'n', long = "lines")]
    pub lines: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct RawRecord {
    task_id: Option<String>,
    branch: Option<String>,
    log_path: Option<String>,
    start_time: Option<u64>,
}

#[derive(Debug, Clone)]
struct TaskMeta {
    task_id: String,
    branch: Option<String>,
    log_path: String,
    start_time: Option<u64>,
}

pub fn run_logs(cli: LogsCli) -> anyhow::Result<()> {
    let id = cli.id.to_lowercase();
    let tasks = load_tasks_index()?;
    if tasks.is_empty() {
        eprintln!("No tasks found in tasks.jsonl");
        return Ok(());
    }
    let matches: Vec<&TaskMeta> = tasks
        .values()
        .filter(|meta| {
            meta.task_id.starts_with(&id) || meta.branch.as_deref().map(|b| b == id).unwrap_or(false)
        })
        .collect();
    if matches.is_empty() {
        eprintln!("No task matches identifier '{}'.", id);
        return Ok(());
    }
    if matches.len() > 1 {
        eprintln!("Identifier '{}' is ambiguous; matches: {}", id, matches.iter().map(|m| &m.task_id[..8]).collect::<Vec<_>>().join(", "));
        return Ok(());
    }
    let task = matches[0];
    let path = PathBuf::from(&task.log_path);
    if !path.exists() {
        eprintln!("Log file not found at {}", path.display());
        return Ok(());
    }

    if cli.follow {
        tail_file(&path, cli.lines)?;
    } else {
        print_file(&path, cli.lines)?;
    }
    Ok(())
}

fn base_dir() -> Option<PathBuf> {
    if let Ok(val) = std::env::var("CODEX_HOME") { if !val.is_empty() { return std::fs::canonicalize(val).ok(); } }
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".codex"))
}

fn load_tasks_index() -> anyhow::Result<HashMap<String, TaskMeta>> {
    let mut map: HashMap<String, TaskMeta> = HashMap::new();
    let Some(base) = base_dir() else { return Ok(map); };
    let tasks = base.join("tasks.jsonl");
    if !tasks.exists() { return Ok(map); }
    let f = File::open(tasks)?;
    let reader = BufReader::new(f);
    for line in reader.lines() {
        let Ok(line) = line else { continue };
        if line.trim().is_empty() { continue; }
        let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) else { continue };
        let Ok(rec) = serde_json::from_value::<RawRecord>(val) else { continue };
        let (Some(task_id), Some(log_path)) = (rec.task_id.clone(), rec.log_path.clone()) else { continue };
        // Insert or update only if not already present (we just need initial metadata)
        map.entry(task_id.clone()).or_insert(TaskMeta {
            task_id,
            branch: rec.branch,
            log_path,
            start_time: rec.start_time,
        });
    }
    Ok(map)
}

fn print_file(path: &PathBuf, last_lines: Option<usize>) -> anyhow::Result<()> {
    if let Some(n) = last_lines {
        let f = File::open(path)?;
        let reader = BufReader::new(f);
        let mut buf: std::collections::VecDeque<String> = std::collections::VecDeque::with_capacity(n);
        for line in reader.lines() {
            if let Ok(l) = line { if buf.len() == n { buf.pop_front(); } buf.push_back(l); }
        }
        for l in buf { println!("{}", l); }
        return Ok(());
    }
    // Full file
    let mut f = File::open(path)?;
    let mut contents = String::new();
    f.read_to_string(&mut contents)?;
    print!("{}", contents);
    Ok(())
}

fn tail_file(path: &PathBuf, last_lines: Option<usize>) -> anyhow::Result<()> {
    use std::io::{self};
    // Initial output
    if let Some(n) = last_lines { print_file(path, Some(n))?; } else { print_file(path, None)?; }
    let mut f = File::open(path)?;
    let mut pos = f.metadata()?.len();
    loop {
        thread::sleep(Duration::from_millis(500));
        let meta = match f.metadata() { Ok(m) => m, Err(_) => break };
        let len = meta.len();
        if len < pos { // truncated
            pos = 0;
        }
        if len > pos {
            f.seek(SeekFrom::Start(pos))?;
            let mut buf = String::new();
            f.read_to_string(&mut buf)?;
            if !buf.is_empty() { print!("{}", buf); io::Write::flush(&mut std::io::stdout())?; }
            pos = len;
        }
    }
    Ok(())
} 