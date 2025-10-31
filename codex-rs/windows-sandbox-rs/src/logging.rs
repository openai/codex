use std::fs::OpenOptions;
use std::io::Write;

const LOG_COMMAND_PREVIEW_LIMIT: usize = 200;
pub const LOG_FILE_NAME: &str = "sandbox_commands.rust.log";

fn preview(command: &[String]) -> String {
    let joined = command.join(" ");
    if joined.len() <= LOG_COMMAND_PREVIEW_LIMIT {
        joined
    } else {
        joined[..LOG_COMMAND_PREVIEW_LIMIT].to_string()
    }
}

fn append_line(line: &str) {
    if let Ok(mut f) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG_FILE_NAME)
    {
        let _ = writeln!(f, "{}", line);
    }
}

pub fn log_start(command: &[String]) {
    let p = preview(command);
    append_line(&format!("START: {}", p));
}

pub fn log_success(command: &[String]) {
    let p = preview(command);
    append_line(&format!("SUCCESS: {}", p));
}

pub fn log_failure(command: &[String], detail: &str) {
    let p = preview(command);
    append_line(&format!("FAILURE: {} ({})", p, detail));
}

// Debug logging helper. Emits only when SBX_DEBUG=1 to avoid noisy logs.
pub fn debug_log(msg: &str) {
    if std::env::var("SBX_DEBUG").ok().as_deref() == Some("1") {
        append_line(&format!("DEBUG: {}", msg));
        eprintln!("{}", msg);
    }
}
