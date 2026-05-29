use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::OnceLock;

use codex_utils_string::take_bytes_at_char_boundary;
use tracing_appender::rolling::RollingFileAppender;
use tracing_appender::rolling::Rotation;

const LOG_COMMAND_PREVIEW_LIMIT: usize = 200;
pub const LOG_FILE_PREFIX: &str = "sandbox";
pub const LOG_FILE_SUFFIX: &str = "log";
pub const MAX_LOG_FILES: usize = 90;

fn exe_label() -> &'static str {
    static LABEL: OnceLock<String> = OnceLock::new();
    LABEL.get_or_init(|| {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .unwrap_or_else(|| "proc".to_string())
    })
}

fn preview(command: &[String]) -> String {
    let joined = command.join(" ");
    if joined.len() <= LOG_COMMAND_PREVIEW_LIMIT {
        joined
    } else {
        take_bytes_at_char_boundary(&joined, LOG_COMMAND_PREVIEW_LIMIT).to_string()
    }
}

pub fn log_file_path_for_utc_date(base_dir: &Path, date: chrono::NaiveDate) -> PathBuf {
    base_dir.join(format!(
        "{LOG_FILE_PREFIX}.{}.{}",
        date.format("%Y-%m-%d"),
        LOG_FILE_SUFFIX
    ))
}

pub fn current_log_file_path(base_dir: &Path) -> PathBuf {
    log_file_path_for_utc_date(base_dir, chrono::Utc::now().date_naive())
}

pub fn current_log_file_path_for_codex_home(codex_home: &Path) -> PathBuf {
    current_log_file_path(&crate::sandbox_dir(codex_home))
}

pub fn log_writer(base_dir: &Path) -> Option<RollingFileAppender> {
    if !base_dir.is_dir() {
        return None;
    }

    RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix(LOG_FILE_PREFIX)
        .filename_suffix(LOG_FILE_SUFFIX)
        .max_log_files(MAX_LOG_FILES)
        .build(base_dir)
        .ok()
}

fn append_line(line: &str, base_dir: Option<&Path>) {
    if let Some(dir) = base_dir
        && let Some(mut f) = log_writer(dir)
    {
        let _ = writeln!(f, "{line}");
    }
}

pub fn log_start(command: &[String], base_dir: Option<&Path>) {
    let p = preview(command);
    log_note(&format!("START: {p}"), base_dir);
}

pub fn log_success(command: &[String], base_dir: Option<&Path>) {
    let p = preview(command);
    log_note(&format!("SUCCESS: {p}"), base_dir);
}

pub fn log_failure(command: &[String], detail: &str, base_dir: Option<&Path>) {
    let p = preview(command);
    log_note(&format!("FAILURE: {p} ({detail})"), base_dir);
}

// Debug logging helper. Emits only when SBX_DEBUG=1 to avoid noisy logs.
pub fn debug_log(msg: &str, base_dir: Option<&Path>) {
    if std::env::var("SBX_DEBUG").ok().as_deref() == Some("1") {
        append_line(&format!("DEBUG: {msg}"), base_dir);
        eprintln!("{msg}");
    }
}

// Unconditional note logging to the daily sandbox log.
pub fn log_note(msg: &str, base_dir: Option<&Path>) {
    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    append_line(&format!("[{ts} {}] {}", exe_label(), msg), base_dir);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_does_not_panic_on_utf8_boundary() {
        // Place a 4-byte emoji such that naive (byte-based) truncation would split it.
        let prefix = "x".repeat(LOG_COMMAND_PREVIEW_LIMIT - 1);
        let command = vec![format!("{prefix}😀")];
        let result = std::panic::catch_unwind(|| preview(&command));
        assert!(result.is_ok());
        let previewed = result.unwrap();
        assert!(previewed.len() <= LOG_COMMAND_PREVIEW_LIMIT);
    }

    #[test]
    fn log_note_writes_to_daily_rolling_log() {
        let tempdir = tempfile::tempdir().expect("tempdir");

        log_note("hello daily log", Some(tempdir.path()));

        let entries = std::fs::read_dir(tempdir.path())
            .expect("read log dir")
            .collect::<Result<Vec<_>, _>>()
            .expect("read entries");
        assert_eq!(entries.len(), 1);

        let log_path = entries[0].path();
        let filename = log_path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("utf-8 filename");
        assert!(filename.starts_with("sandbox."));
        assert!(filename.ends_with(".log"));

        let log = std::fs::read_to_string(log_path).expect("read log");
        assert!(log.contains("hello daily log"));
    }

    #[test]
    fn log_file_path_for_utc_date_matches_rolling_appender_name() {
        let date = chrono::NaiveDate::from_ymd_opt(2026, 5, 21).expect("valid date");

        assert_eq!(
            log_file_path_for_utc_date(Path::new("logs"), date),
            PathBuf::from("logs").join("sandbox.2026-05-21.log")
        );
    }

    #[test]
    fn current_log_file_path_for_codex_home_uses_sandbox_dir() {
        let codex_home = Path::new("codex-home");

        assert_eq!(
            current_log_file_path_for_codex_home(codex_home),
            current_log_file_path(&codex_home.join(".sandbox"))
        );
    }
}
