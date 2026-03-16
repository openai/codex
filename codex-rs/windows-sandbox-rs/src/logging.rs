use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::OnceLock;

use codex_utils_string::take_bytes_at_char_boundary;

const LOG_COMMAND_PREVIEW_LIMIT: usize = 200;
pub const LOG_FILE_NAME: &str = "sandbox.log";

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

fn log_file_path(base_dir: &Path) -> Option<PathBuf> {
    if base_dir.is_dir() {
        Some(base_dir.join(LOG_FILE_NAME))
    } else {
        None
    }
}

fn append_line(line: &str, base_dir: Option<&Path>) {
    if let Some(dir) = base_dir {
        if let Some(path) = log_file_path(dir) {
            if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
                let _ = writeln!(f, "{}", line);
            }
        }
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

// Unconditional note logging to sandbox.log
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
}
