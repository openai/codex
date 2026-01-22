use std::cmp::Reverse;
use std::fs;
use std::fs::OpenOptions;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use clap::Parser;
use codex_core::SESSIONS_SUBDIR;
use codex_protocol::ThreadId;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncSeekExt;

#[derive(Debug, Parser)]
pub struct EventsCli {
    #[command(subcommand)]
    sub: EventsSubcommand,
}

#[derive(Debug, clap::Subcommand)]
enum EventsSubcommand {
    /// Append a single external event to a thread inbox.
    Send(EventsSendArgs),

    /// Print events from a thread inbox (best-effort parsing).
    Show(EventsShowArgs),

    /// Follow a thread inbox and print newly appended events.
    Tail(EventsTailArgs),

    /// List known thread inboxes under `CODEX_HOME/sessions`.
    List(EventsListArgs),

    /// Print the inbox path for a thread.
    InboxPath(EventsInboxPathArgs),
}

#[derive(Debug, Parser)]
struct EventsSendArgs {
    /// Target thread id (UUID).
    #[arg(long = "thread", value_name = "THREAD_ID")]
    thread_id: String,

    /// Event type (e.g. build.status, agent.message).
    #[arg(long = "type", value_name = "TYPE")]
    ty: String,

    /// Severity.
    #[arg(long, value_enum, default_value_t = ExternalEventSeverity::Info)]
    severity: ExternalEventSeverity,

    /// Title.
    #[arg(long, value_name = "TITLE")]
    title: String,

    /// Summary.
    #[arg(long, value_name = "SUMMARY")]
    summary: String,

    /// Optional event id. If omitted, one is generated.
    #[arg(long, value_name = "EVENT_ID")]
    event_id: Option<String>,

    /// Optional event time (milliseconds since epoch). If omitted, uses now.
    #[arg(long, value_name = "UNIX_MS")]
    time_unix_ms: Option<i64>,

    /// Optional JSON payload (passed through verbatim).
    #[arg(long, value_name = "JSON")]
    payload_json: Option<String>,

    /// Override CODEX_HOME (defaults to `$CODEX_HOME` or `~/.codex`).
    #[arg(long, value_name = "PATH")]
    codex_home: Option<PathBuf>,
}

#[derive(Debug, Parser)]
struct EventsShowArgs {
    /// Thread id (UUID).
    #[arg(long = "thread", value_name = "THREAD_ID")]
    thread_id: String,

    /// Show only the last N events.
    #[arg(long, value_name = "N")]
    last: Option<usize>,

    /// Print raw JSON lines instead of a formatted summary.
    #[arg(long, default_value_t = false)]
    raw: bool,

    /// Override CODEX_HOME (defaults to `$CODEX_HOME` or `~/.codex`).
    #[arg(long, value_name = "PATH")]
    codex_home: Option<PathBuf>,
}

#[derive(Debug, Parser)]
struct EventsTailArgs {
    /// Thread id (UUID).
    #[arg(long = "thread", value_name = "THREAD_ID")]
    thread_id: String,

    /// Start at the beginning instead of following from the end.
    #[arg(long, default_value_t = false)]
    from_start: bool,

    /// Override CODEX_HOME (defaults to `$CODEX_HOME` or `~/.codex`).
    #[arg(long, value_name = "PATH")]
    codex_home: Option<PathBuf>,
}

#[derive(Debug, Parser)]
struct EventsListArgs {
    /// Override CODEX_HOME (defaults to `$CODEX_HOME` or `~/.codex`).
    #[arg(long, value_name = "PATH")]
    codex_home: Option<PathBuf>,
}

#[derive(Debug, Parser)]
struct EventsInboxPathArgs {
    /// Thread id (UUID).
    #[arg(long = "thread", value_name = "THREAD_ID")]
    thread_id: String,

    /// Override CODEX_HOME (defaults to `$CODEX_HOME` or `~/.codex`).
    #[arg(long, value_name = "PATH")]
    codex_home: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
enum ExternalEventSeverity {
    Debug,
    Info,
    Warning,
    Error,
    Critical,
}

impl ExternalEventSeverity {
    fn as_label(self) -> &'static str {
        match self {
            ExternalEventSeverity::Debug => "debug",
            ExternalEventSeverity::Info => "info",
            ExternalEventSeverity::Warning => "warning",
            ExternalEventSeverity::Error => "error",
            ExternalEventSeverity::Critical => "critical",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ExternalEvent {
    schema_version: u32,
    event_id: String,
    time_unix_ms: i64,
    #[serde(rename = "type")]
    ty: String,
    severity: ExternalEventSeverity,
    title: String,
    summary: String,
    #[serde(default)]
    payload: Option<Value>,
}

pub async fn run_events(cli: EventsCli) -> anyhow::Result<()> {
    match cli.sub {
        EventsSubcommand::Send(args) => run_send(args),
        EventsSubcommand::Show(args) => run_show(args),
        EventsSubcommand::Tail(args) => run_tail(args).await,
        EventsSubcommand::List(args) => run_list(args),
        EventsSubcommand::InboxPath(args) => run_inbox_path(args),
    }
}

fn run_send(args: EventsSendArgs) -> anyhow::Result<()> {
    let thread_id = ThreadId::from_string(&args.thread_id)?;
    let codex_home = resolve_codex_home(args.codex_home)?;
    let inbox = inbox_path(&codex_home, &thread_id);

    let event_id = args
        .event_id
        .unwrap_or_else(|| format!("evt_{}", default_id_suffix()));
    let time_unix_ms = args.time_unix_ms.unwrap_or_else(now_unix_ms);
    let payload = match args.payload_json {
        Some(s) => Some(serde_json::from_str::<Value>(&s)?),
        None => None,
    };

    let event = ExternalEvent {
        schema_version: 1,
        event_id,
        time_unix_ms,
        ty: args.ty,
        severity: args.severity,
        title: args.title,
        summary: args.summary,
        payload,
    };

    if let Some(parent) = inbox.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = OpenOptions::new().create(true).append(true).open(&inbox)?;
    let line = serde_json::to_string(&event)?;
    writeln!(file, "{line}")?;
    Ok(())
}

fn run_show(args: EventsShowArgs) -> anyhow::Result<()> {
    let thread_id = ThreadId::from_string(&args.thread_id)?;
    let codex_home = resolve_codex_home(args.codex_home)?;
    let inbox = inbox_path(&codex_home, &thread_id);

    let file = fs::File::open(&inbox)?;
    let reader = BufReader::new(file);
    let mut lines: Vec<String> = reader.lines().collect::<Result<_, _>>()?;
    if let Some(last) = args.last {
        if lines.len() > last {
            lines.drain(0..(lines.len() - last));
        }
    }

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if args.raw {
            println!("{trimmed}");
            continue;
        }

        match serde_json::from_str::<ExternalEvent>(trimmed) {
            Ok(event) => {
                println!(
                    "{} [{}] {} {}: {} — {}",
                    event.time_unix_ms,
                    event.severity.as_label(),
                    event.ty,
                    event.event_id,
                    event.title,
                    event.summary
                );
            }
            Err(_) => {
                println!("{trimmed}");
            }
        }
    }

    Ok(())
}

async fn run_tail(args: EventsTailArgs) -> anyhow::Result<()> {
    let thread_id = ThreadId::from_string(&args.thread_id)?;
    let codex_home = resolve_codex_home(args.codex_home)?;
    let inbox = inbox_path(&codex_home, &thread_id);

    if let Some(parent) = inbox.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    if tokio::fs::metadata(&inbox).await.is_err() {
        tokio::fs::File::create(&inbox).await?;
    }

    let mut file = tokio::fs::OpenOptions::new()
        .read(true)
        .open(&inbox)
        .await?;
    if !args.from_start {
        file.seek(std::io::SeekFrom::End(0)).await?;
    }

    let mut reader = tokio::io::BufReader::new(file);
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => {
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                match serde_json::from_str::<ExternalEvent>(trimmed) {
                    Ok(event) => println!(
                        "{} [{}] {} {}: {} — {}",
                        event.time_unix_ms,
                        event.severity.as_label(),
                        event.ty,
                        event.event_id,
                        event.title,
                        event.summary
                    ),
                    Err(_) => println!("{trimmed}"),
                }
            }
            Err(err) => return Err(err.into()),
        }
    }
}

fn run_list(args: EventsListArgs) -> anyhow::Result<()> {
    let codex_home = resolve_codex_home(args.codex_home)?;
    let sessions_dir = codex_home.join(SESSIONS_SUBDIR);
    let mut entries = Vec::new();
    if let Ok(dirents) = fs::read_dir(&sessions_dir) {
        for ent in dirents.flatten() {
            let path = ent.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let Ok(meta) = ent.metadata() else {
                continue;
            };
            let modified = meta.modified().ok();
            entries.push((name.to_string(), modified));
        }
    }

    entries.sort_by_key(|(_name, modified)| Reverse(modified.unwrap_or(SystemTime::UNIX_EPOCH)));
    for (name, modified) in entries {
        let ms = modified
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        println!("{name}\t{ms}");
    }
    Ok(())
}

fn run_inbox_path(args: EventsInboxPathArgs) -> anyhow::Result<()> {
    let thread_id = ThreadId::from_string(&args.thread_id)?;
    let codex_home = resolve_codex_home(args.codex_home)?;
    let inbox = inbox_path(&codex_home, &thread_id);
    println!("{}", inbox.display());
    Ok(())
}

fn resolve_codex_home(override_path: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    Ok(match override_path {
        Some(path) => path,
        None => codex_core::config::find_codex_home()?,
    })
}

fn inbox_path(codex_home: &PathBuf, thread_id: &ThreadId) -> PathBuf {
    codex_home
        .join(SESSIONS_SUBDIR)
        .join(thread_id.to_string())
        .join("external_events.inbox.jsonl")
}

fn now_unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn default_id_suffix() -> String {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let count = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("{}_{}_{}", now_unix_ms(), std::process::id(), count)
}
