use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context;
use chrono::DateTime;
use chrono::SecondsFormat;
use chrono::Utc;
use clap::Parser;
use codex_state::LogQuery;
use codex_state::LogRow;
use codex_state::STATE_DB_FILENAME;
use codex_state::StateRuntime;
use dirs::home_dir;
use owo_colors::OwoColorize;

#[derive(Debug, Parser)]
#[command(name = "codex-state-logs")]
#[command(about = "Tail Codex logs from state.sqlite with simple filters")]
struct Args {
    /// Path to CODEX_HOME. Defaults to $CODEX_HOME or ~/.codex.
    #[arg(long, env = "CODEX_HOME")]
    codex_home: Option<PathBuf>,

    /// Direct path to the SQLite database. Overrides --codex-home.
    #[arg(long)]
    db: Option<PathBuf>,

    /// Log level to match exactly (case-insensitive).
    #[arg(long)]
    level: Option<String>,

    /// Start timestamp (RFC3339 or unix seconds).
    #[arg(long, value_name = "RFC3339|UNIX")]
    from: Option<String>,

    /// End timestamp (RFC3339 or unix seconds).
    #[arg(long, value_name = "RFC3339|UNIX")]
    to: Option<String>,

    /// Substring match on module_path.
    #[arg(long)]
    module: Option<String>,

    /// Substring match on file path.
    #[arg(long)]
    file: Option<String>,

    /// Match a specific thread id.
    #[arg(long)]
    thread_id: Option<String>,

    /// Number of matching rows to show before tailing.
    #[arg(long, default_value_t = 200)]
    backfill: usize,

    /// Poll interval in milliseconds.
    #[arg(long, default_value_t = 500)]
    poll_ms: u64,
}

#[derive(Debug, Clone)]
struct LogFilter {
    level_upper: Option<String>,
    from_ts: Option<i64>,
    to_ts: Option<i64>,
    module_like: Option<String>,
    file_like: Option<String>,
    thread_id: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let db_path = resolve_db_path(&args)?;
    let filter = build_filter(&args)?;
    let codex_home = db_path
        .parent()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| PathBuf::from("."));
    let runtime = StateRuntime::init(codex_home, "logs-client".to_string(), None).await?;

    let mut last_id = print_backfill(runtime.as_ref(), &filter, args.backfill).await?;
    if last_id == 0 {
        last_id = fetch_max_id(runtime.as_ref(), &filter).await?;
    }

    let poll_interval = Duration::from_millis(args.poll_ms);
    loop {
        let rows = fetch_new_rows(runtime.as_ref(), &filter, last_id).await?;
        for row in rows {
            last_id = last_id.max(row.id);
            println!("{}", format_row(&row));
        }
        tokio::time::sleep(poll_interval).await;
    }
}

fn resolve_db_path(args: &Args) -> anyhow::Result<PathBuf> {
    if let Some(db) = args.db.as_ref() {
        return Ok(db.clone());
    }

    let codex_home = args.codex_home.clone().unwrap_or_else(default_codex_home);
    Ok(codex_home.join(STATE_DB_FILENAME))
}

fn default_codex_home() -> PathBuf {
    if let Some(home) = home_dir() {
        return home.join(".codex");
    }
    PathBuf::from(".codex")
}

fn build_filter(args: &Args) -> anyhow::Result<LogFilter> {
    let from_ts = args
        .from
        .as_deref()
        .map(parse_timestamp)
        .transpose()
        .context("failed to parse --from")?;
    let to_ts = args
        .to
        .as_deref()
        .map(parse_timestamp)
        .transpose()
        .context("failed to parse --to")?;

    let level_upper = args.level.as_ref().map(|level| level.to_ascii_uppercase());

    Ok(LogFilter {
        level_upper,
        from_ts,
        to_ts,
        module_like: args.module.clone(),
        file_like: args.file.clone(),
        thread_id: args.thread_id.clone(),
    })
}

fn parse_timestamp(value: &str) -> anyhow::Result<i64> {
    if let Ok(secs) = value.parse::<i64>() {
        return Ok(secs);
    }

    let dt = DateTime::parse_from_rfc3339(value)
        .with_context(|| format!("expected RFC3339 or unix seconds, got {value}"))?;
    Ok(dt.timestamp())
}

async fn print_backfill(
    runtime: &StateRuntime,
    filter: &LogFilter,
    backfill: usize,
) -> anyhow::Result<i64> {
    if backfill == 0 {
        return Ok(0);
    }

    let mut rows = fetch_backfill(runtime, filter, backfill).await?;
    rows.reverse();

    let mut last_id = 0;
    for row in rows {
        last_id = last_id.max(row.id);
        println!("{}", format_row(&row));
    }
    Ok(last_id)
}

async fn fetch_backfill(
    runtime: &StateRuntime,
    filter: &LogFilter,
    backfill: usize,
) -> anyhow::Result<Vec<LogRow>> {
    let query = to_log_query(filter, Some(backfill), None, true);
    runtime
        .query_logs(&query)
        .await
        .context("failed to fetch backfill logs")
}

async fn fetch_new_rows(
    runtime: &StateRuntime,
    filter: &LogFilter,
    last_id: i64,
) -> anyhow::Result<Vec<LogRow>> {
    let query = to_log_query(filter, None, Some(last_id), false);
    runtime
        .query_logs(&query)
        .await
        .context("failed to fetch new logs")
}

async fn fetch_max_id(runtime: &StateRuntime, filter: &LogFilter) -> anyhow::Result<i64> {
    let query = to_log_query(filter, None, None, false);
    runtime
        .max_log_id(&query)
        .await
        .context("failed to fetch max log id")
}

fn to_log_query(
    filter: &LogFilter,
    limit: Option<usize>,
    after_id: Option<i64>,
    descending: bool,
) -> LogQuery {
    LogQuery {
        level_upper: filter.level_upper.clone(),
        from_ts: filter.from_ts,
        to_ts: filter.to_ts,
        module_like: filter.module_like.clone(),
        file_like: filter.file_like.clone(),
        thread_id: filter.thread_id.clone(),
        after_id,
        limit,
        descending,
    }
}

fn format_row(row: &LogRow) -> String {
    let timestamp = format_timestamp(row.ts, row.ts_nanos);
    let level = row.level.as_str();
    let target = row.target.as_str();
    let message = row.message.as_deref().unwrap_or("");
    let level_colored = color_level(level);
    let timestamp_colored = timestamp.dimmed().to_string();
    let thread_id = row.thread_id.as_deref().unwrap_or("-");
    let thread_id_colored = thread_id.blue().dimmed().to_string();
    let target_colored = target.dimmed().to_string();
    let message_colored = message.bold().to_string();
    format!(
        "{timestamp_colored} {level_colored} [{thread_id_colored}] {target_colored} - {message_colored}"
    )
}

fn color_level(level: &str) -> String {
    let padded = format!("{level:<5}");
    if level.eq_ignore_ascii_case("error") {
        return padded.red().bold().to_string();
    }
    if level.eq_ignore_ascii_case("warn") {
        return padded.yellow().bold().to_string();
    }
    if level.eq_ignore_ascii_case("info") {
        return padded.green().bold().to_string();
    }
    if level.eq_ignore_ascii_case("debug") {
        return padded.blue().bold().to_string();
    }
    if level.eq_ignore_ascii_case("trace") {
        return padded.magenta().bold().to_string();
    }
    padded.bold().to_string()
}

fn format_timestamp(ts: i64, ts_nanos: i64) -> String {
    let nanos = u32::try_from(ts_nanos).unwrap_or(0);
    match DateTime::<Utc>::from_timestamp(ts, nanos) {
        Some(dt) => dt.to_rfc3339_opts(SecondsFormat::Millis, true),
        None => format!("{ts}.{ts_nanos:09}Z"),
    }
}
