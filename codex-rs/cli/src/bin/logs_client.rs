use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context;
use chrono::DateTime;
use clap::Parser;
use clap::ValueEnum;
use codex_core::config::ConfigBuilder;
use codex_state::LogQuery;
use codex_state::LogRow;
use codex_state::SqliteConfig;
use codex_state::StateRuntime;
use codex_utils_absolute_path::AbsolutePathBuf;
use owo_colors::OwoColorize;

#[derive(Debug, Parser)]
#[command(name = "codex-state-logs")]
#[command(about = "Tail Codex logs from the dedicated logs SQLite DB with simple filters")]
struct Args {
    /// Path to CODEX_HOME. Defaults to $CODEX_HOME or ~/.codex.
    #[arg(long, env = "CODEX_HOME")]
    codex_home: Option<PathBuf>,

    /// Direct path to the logs SQLite database. Overrides --codex-home.
    #[arg(long)]
    db: Option<PathBuf>,

    /// Minimum log level to include.
    #[arg(long, value_enum, ignore_case = true)]
    level: Option<LogLevelThreshold>,

    /// Start timestamp (RFC3339 or unix seconds).
    #[arg(long, value_name = "RFC3339|UNIX")]
    from: Option<String>,

    /// End timestamp (RFC3339 or unix seconds).
    #[arg(long, value_name = "RFC3339|UNIX")]
    to: Option<String>,

    /// Substring match on module_path. Repeat to include multiple substrings.
    #[arg(long = "module")]
    module: Vec<String>,

    /// Substring match on file path. Repeat to include multiple substrings.
    #[arg(long = "file")]
    file: Vec<String>,

    /// Match one or more thread ids. Repeat to include multiple threads.
    #[arg(long = "thread-id")]
    thread_id: Vec<String>,

    /// Substring match against the rendered log body.
    #[arg(long)]
    search: Option<String>,

    /// Include logs that do not have a thread id.
    #[arg(long)]
    threadless: bool,

    /// Number of matching rows to show before tailing.
    #[arg(long, default_value_t = 200)]
    backfill: usize,

    /// Poll interval in milliseconds.
    #[arg(long, default_value_t = 500)]
    poll_ms: u64,

    /// Show compact output with only time, level, and rendered log body.
    #[arg(long)]
    compact: bool,
}

#[derive(Debug, Clone)]
struct LogFilter {
    levels_upper: Vec<String>,
    from_ts: Option<i64>,
    to_ts: Option<i64>,
    module_like: Vec<String>,
    file_like: Vec<String>,
    thread_ids: Vec<String>,
    search: Option<String>,
    include_threadless: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum LogLevelThreshold {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevelThreshold {
    fn levels_upper(self) -> Vec<String> {
        let levels = match self {
            LogLevelThreshold::Trace => &["TRACE", "DEBUG", "INFO", "WARN", "ERROR"][..],
            LogLevelThreshold::Debug => &["DEBUG", "INFO", "WARN", "ERROR"],
            LogLevelThreshold::Info => &["INFO", "WARN", "ERROR"],
            LogLevelThreshold::Warn => &["WARN", "ERROR"],
            LogLevelThreshold::Error => &["ERROR"],
        };
        levels.iter().map(ToString::to_string).collect()
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let sqlite = resolve_sqlite_config(&args).await?;
    let filter = build_filter(&args)?;
    let runtime = StateRuntime::init(sqlite, "logs-client".to_string()).await?;

    let mut last_id =
        print_backfill(runtime.as_ref(), &filter, args.backfill, args.compact).await?;
    if last_id == 0 {
        last_id = fetch_max_id(runtime.as_ref(), &filter).await?;
    }

    let poll_interval = Duration::from_millis(args.poll_ms);
    loop {
        let rows = fetch_new_rows(runtime.as_ref(), &filter, last_id).await?;
        for row in rows {
            last_id = last_id.max(row.id);
            println!("{}", format_row(&row, args.compact));
        }
        tokio::time::sleep(poll_interval).await;
    }
}

async fn resolve_sqlite_config(args: &Args) -> anyhow::Result<SqliteConfig> {
    if let Some(db_path) = args.db.as_ref() {
        let sqlite_home = db_path
            .parent()
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| PathBuf::from("."));
        return Ok(SqliteConfig::from_sqlite_home(
            AbsolutePathBuf::relative_to_current_dir(sqlite_home)?,
        ));
    }

    let mut config_builder = ConfigBuilder::default();
    if let Some(codex_home) = args.codex_home.as_ref() {
        config_builder = config_builder.codex_home(codex_home.clone());
    }
    let config = config_builder.build().await?;
    Ok(config.sqlite_config().clone())
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

    let levels_upper = args
        .level
        .map_or_else(Vec::new, LogLevelThreshold::levels_upper);
    let module_like = args
        .module
        .iter()
        .filter(|module| !module.is_empty())
        .cloned()
        .collect::<Vec<_>>();
    let file_like = args
        .file
        .iter()
        .filter(|file| !file.is_empty())
        .cloned()
        .collect::<Vec<_>>();
    let thread_ids = args
        .thread_id
        .iter()
        .filter(|thread_id| !thread_id.is_empty())
        .cloned()
        .collect::<Vec<_>>();

    Ok(LogFilter {
        levels_upper,
        from_ts,
        to_ts,
        module_like,
        file_like,
        thread_ids,
        search: args.search.clone(),
        include_threadless: args.threadless,
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
    compact: bool,
) -> anyhow::Result<i64> {
    if backfill == 0 {
        return Ok(0);
    }

    let mut rows = fetch_backfill(runtime, filter, backfill).await?;
    rows.reverse();

    let mut last_id = 0;
    for row in rows {
        last_id = last_id.max(row.id);
        println!("{}", format_row(&row, compact));
    }
    Ok(last_id)
}

async fn fetch_backfill(
    runtime: &StateRuntime,
    filter: &LogFilter,
    backfill: usize,
) -> anyhow::Result<Vec<LogRow>> {
    let query = to_log_query(
        filter,
        Some(backfill),
        /*after_id*/ None,
        /*descending*/ true,
    );
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
    let query = to_log_query(
        filter,
        /*limit*/ None,
        Some(last_id),
        /*descending*/ false,
    );
    runtime
        .query_logs(&query)
        .await
        .context("failed to fetch new logs")
}

async fn fetch_max_id(runtime: &StateRuntime, filter: &LogFilter) -> anyhow::Result<i64> {
    let query = to_log_query(
        filter, /*limit*/ None, /*after_id*/ None, /*descending*/ false,
    );
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
        levels_upper: filter.levels_upper.clone(),
        from_ts: filter.from_ts,
        to_ts: filter.to_ts,
        module_like: filter.module_like.clone(),
        file_like: filter.file_like.clone(),
        thread_ids: filter.thread_ids.clone(),
        search: filter.search.clone(),
        include_threadless: filter.include_threadless,
        after_id,
        limit,
        descending,
    }
}

fn format_row(row: &LogRow, compact: bool) -> String {
    let timestamp = formatter::ts(row.ts, row.ts_nanos, compact);
    let level = row.level.as_str();
    let target = row.target.as_str();
    let message = row.message.as_deref().unwrap_or("");
    let level_colored = formatter::level(level);
    let timestamp_colored = timestamp.dimmed().to_string();
    let thread_id = row.thread_id.as_deref().unwrap_or("-");
    let thread_id_colored = thread_id.blue().dimmed().to_string();
    let target_colored = target.dimmed().to_string();
    let message_colored = heuristic_formatting(message);
    if compact {
        format!("{timestamp_colored} {level_colored} {message_colored}")
    } else {
        format!(
            "{timestamp_colored} {level_colored} [{thread_id_colored}] {target_colored} - {message_colored}"
        )
    }
}

fn heuristic_formatting(message: &str) -> String {
    if matcher::apply_patch(message) {
        formatter::apply_patch(message)
    } else {
        message.bold().to_string()
    }
}

mod matcher {
    pub(super) fn apply_patch(message: &str) -> bool {
        message.contains("ToolCall: apply_patch")
    }
}

mod formatter {
    use chrono::DateTime;
    use chrono::SecondsFormat;
    use chrono::Utc;
    use owo_colors::OwoColorize;

    pub(super) fn apply_patch(message: &str) -> String {
        message
            .lines()
            .map(|line| {
                if line.starts_with('+') {
                    line.green().bold().to_string()
                } else if line.starts_with('-') {
                    line.red().bold().to_string()
                } else {
                    line.bold().to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub(super) fn ts(ts: i64, ts_nanos: i64, compact: bool) -> String {
        let nanos = u32::try_from(ts_nanos).unwrap_or(0);
        match DateTime::<Utc>::from_timestamp(ts, nanos) {
            Some(dt) if compact => dt.format("%H:%M:%S").to_string(),
            Some(dt) => dt.to_rfc3339_opts(SecondsFormat::Millis, true),
            None => format!("{ts}.{ts_nanos:09}Z"),
        }
    }

    pub(super) fn level(level: &str) -> String {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::ffi::OsString;

    #[test]
    fn log_level_threshold_includes_more_severe_levels() {
        assert_eq!(
            LogLevelThreshold::Warn.levels_upper(),
            vec!["WARN".to_string(), "ERROR".to_string()]
        );
        assert_eq!(
            LogLevelThreshold::Trace.levels_upper(),
            vec![
                "TRACE".to_string(),
                "DEBUG".to_string(),
                "INFO".to_string(),
                "WARN".to_string(),
                "ERROR".to_string(),
            ]
        );
    }

    #[test]
    fn log_level_rejects_aliases_and_unknown_values() {
        assert!(Args::try_parse_from(["codex-state-logs", "--level", "warning"]).is_err());
        assert!(Args::try_parse_from(["codex-state-logs", "--level", "err"]).is_err());
        assert!(Args::try_parse_from(["codex-state-logs", "--level", "warn,error"]).is_err());
    }

    #[test]
    fn log_level_accepts_canonical_values_case_insensitively() {
        let args = Args::try_parse_from(["codex-state-logs", "--level", "WARN"])
            .expect("parse uppercase log level");

        assert_eq!(args.level, Some(LogLevelThreshold::Warn));
    }

    /// Explicit database selection must not parse an overridden Codex home.
    #[tokio::test]
    async fn direct_db_skips_codex_home_config() {
        let codex_home = tempfile::tempdir().expect("create Codex home");
        std::fs::write(codex_home.path().join("config.toml"), "model = [")
            .expect("write invalid config");
        let sqlite_home = tempfile::tempdir().expect("create SQLite home");
        let db_path = sqlite_home.path().join("logs_2.sqlite");
        let args = Args::try_parse_from([
            OsString::from("codex-state-logs"),
            OsString::from("--codex-home"),
            codex_home.path().as_os_str().to_owned(),
            OsString::from("--db"),
            db_path.as_os_str().to_owned(),
        ])
        .expect("parse arguments");

        let sqlite = resolve_sqlite_config(&args)
            .await
            .expect("resolve SQLite config");

        assert_eq!(sqlite.logs_db_path(), db_path);
    }

    /// Direct database selection must preserve native path bytes.
    #[cfg(unix)]
    #[tokio::test]
    async fn direct_db_preserves_non_utf8_path() {
        use std::os::unix::ffi::OsStrExt;
        use std::os::unix::ffi::OsStringExt;

        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let mut db_path = temp_dir.path().as_os_str().as_bytes().to_vec();
        db_path.extend_from_slice(b"/non-utf8-\xff/logs_2.sqlite");
        let db_path = PathBuf::from(OsString::from_vec(db_path));
        let args = Args::try_parse_from([
            OsString::from("codex-state-logs"),
            OsString::from("--db"),
            db_path.as_os_str().to_owned(),
        ])
        .expect("parse arguments");

        let sqlite = resolve_sqlite_config(&args)
            .await
            .expect("resolve SQLite config");

        assert_eq!(sqlite.logs_db_path(), db_path);
    }
}
