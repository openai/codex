use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context;
use chrono::DateTime;
use chrono::SecondsFormat;
use chrono::Utc;
use clap::Parser;
use codex_state::STATE_DB_FILENAME;
use dirs::home_dir;
use sqlx::QueryBuilder;
use sqlx::Row;
use sqlx::Sqlite;
use sqlx::SqlitePool;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqlitePoolOptions;

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

    /// Number of matching rows to show before tailing.
    #[arg(long, default_value_t = 200)]
    backfill: usize,

    /// Poll interval in milliseconds.
    #[arg(long, default_value_t = 500)]
    poll_ms: u64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct LogRow {
    id: i64,
    ts: i64,
    ts_nanos: i64,
    level: String,
    message: Option<String>,
    fields_json: String,
    module_path: Option<String>,
    file: Option<String>,
    line: Option<i64>,
}

#[derive(Debug, Clone)]
struct LogFilter {
    level_upper: Option<String>,
    from_ts: Option<i64>,
    to_ts: Option<i64>,
    module_like: Option<String>,
    file_like: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let db_path = resolve_db_path(&args)?;
    let filter = build_filter(&args)?;
    let pool = open_read_only_pool(db_path.as_path()).await?;

    let mut last_id = print_backfill(&pool, &filter, args.backfill).await?;
    if last_id == 0 {
        last_id = fetch_max_id(&pool, &filter).await?;
    }

    let poll_interval = Duration::from_millis(args.poll_ms);
    loop {
        let rows = fetch_new_rows(&pool, &filter, last_id).await?;
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

async fn open_read_only_pool(path: &Path) -> anyhow::Result<SqlitePool> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(false)
        .read_only(true)
        .busy_timeout(Duration::from_secs(5));

    let display = path.display();
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .with_context(|| format!("failed to open sqlite db at {display}"))
}

async fn print_backfill(
    pool: &SqlitePool,
    filter: &LogFilter,
    backfill: usize,
) -> anyhow::Result<i64> {
    if backfill == 0 {
        return Ok(0);
    }

    let mut rows = fetch_backfill(pool, filter, backfill).await?;
    rows.reverse();

    let mut last_id = 0;
    for row in rows {
        last_id = last_id.max(row.id);
        println!("{}", format_row(&row));
    }
    Ok(last_id)
}

async fn fetch_backfill(
    pool: &SqlitePool,
    filter: &LogFilter,
    backfill: usize,
) -> anyhow::Result<Vec<LogRow>> {
    let mut builder = base_select_builder();
    push_filters(&mut builder, filter);
    builder.push(" ORDER BY id DESC");
    builder.push(" LIMIT ").push_bind(backfill as i64);

    builder
        .build_query_as::<LogRow>()
        .fetch_all(pool)
        .await
        .context("failed to fetch backfill logs")
}

async fn fetch_new_rows(
    pool: &SqlitePool,
    filter: &LogFilter,
    last_id: i64,
) -> anyhow::Result<Vec<LogRow>> {
    let mut builder = base_select_builder();
    push_filters(&mut builder, filter);
    builder.push(" AND id > ").push_bind(last_id);
    builder.push(" ORDER BY id ASC");

    builder
        .build_query_as::<LogRow>()
        .fetch_all(pool)
        .await
        .context("failed to fetch new logs")
}

async fn fetch_max_id(pool: &SqlitePool, filter: &LogFilter) -> anyhow::Result<i64> {
    let mut builder = QueryBuilder::<Sqlite>::new("SELECT MAX(id) AS max_id FROM logs WHERE 1 = 1");
    push_filters(&mut builder, filter);

    let row = builder
        .build()
        .fetch_one(pool)
        .await
        .context("failed to fetch max log id")?;
    let max_id: Option<i64> = row.try_get("max_id")?;
    Ok(max_id.unwrap_or(0))
}

fn base_select_builder<'a>() -> QueryBuilder<'a, Sqlite> {
    QueryBuilder::<Sqlite>::new(
        "SELECT id, ts, ts_nanos, level, message, fields_json, module_path, file, line FROM logs WHERE 1 = 1",
    )
}

fn push_filters<'a>(builder: &mut QueryBuilder<'a, Sqlite>, filter: &'a LogFilter) {
    if let Some(level_upper) = filter.level_upper.as_ref() {
        builder
            .push(" AND UPPER(level) = ")
            .push_bind(level_upper.as_str());
    }
    if let Some(from_ts) = filter.from_ts {
        builder.push(" AND ts >= ").push_bind(from_ts);
    }
    if let Some(to_ts) = filter.to_ts {
        builder.push(" AND ts <= ").push_bind(to_ts);
    }
    if let Some(module_like) = filter.module_like.as_ref() {
        builder
            .push(" AND module_path LIKE '%' || ")
            .push_bind(module_like.as_str())
            .push(" || '%'");
    }
    if let Some(file_like) = filter.file_like.as_ref() {
        builder
            .push(" AND file LIKE '%' || ")
            .push_bind(file_like.as_str())
            .push(" || '%'");
    }
}

fn format_row(row: &LogRow) -> String {
    let timestamp = format_timestamp(row.ts, row.ts_nanos);
    let location = match (&row.file, row.line) {
        (Some(file), Some(line)) => format!("{file}:{line}"),
        (Some(file), None) => file.clone(),
        _ => "-".to_string(),
    };
    let module = row.module_path.as_deref().unwrap_or("-");
    let message = row.message.as_deref().unwrap_or("");
    let fields = row.fields_json.as_str();
    let level = row.level.as_str();
    if fields == "{}" || fields.is_empty() {
        return format!("{timestamp} {level:<5} [{module}] {location} - {message}");
    }
    format!("{timestamp} {level:<5} [{module}] {location} - {message} {fields}")
}

fn format_timestamp(ts: i64, ts_nanos: i64) -> String {
    let nanos = u32::try_from(ts_nanos).unwrap_or(0);
    match DateTime::<Utc>::from_timestamp(ts, nanos) {
        Some(dt) => dt.to_rfc3339_opts(SecondsFormat::Millis, true),
        None => format!("{ts}.{ts_nanos:09}Z"),
    }
}
