//! Collects bounded current and previous updater log tails after feedback log consent.
//! Read failures omit individual files; only fixed regular-file names are considered.

use std::fs;
use std::io;
use std::io::Read;
use std::io::Seek;
use std::path::Path;

use crate::FeedbackAttachment;

const MAX_LOG_BYTES: u64 = 256 * 1024;
const LOG_NAMES: [&str; 2] = ["daemon-updater.stderr.log", "app-server-updater.stderr.log"];

/// Collect at most two 256 KiB updater tails (512 KiB total) from the app-server host.
/// Call only after feedback consent includes logs, on a blocking worker.
pub fn daemon_log_attachments(codex_home: &Path) -> Vec<FeedbackAttachment> {
    let directory = codex_home.join("app-server-daemon");
    let mut attachments = Vec::new();
    for suffix in ["", ".previous"] {
        for name in LOG_NAMES {
            let filename = format!("{name}{suffix}");
            let path = directory.join(&filename);
            let result = (|| -> io::Result<Vec<u8>> {
                // Reject symlinks and nonregular files before opening (e.g. FIFOs).
                if !fs::symlink_metadata(&path)?.is_file() {
                    return Ok(Vec::new());
                }
                let mut file = fs::File::open(&path)?;
                let metadata = file.metadata()?;
                if !metadata.is_file() {
                    return Ok(Vec::new());
                }
                file.seek(io::SeekFrom::Start(
                    metadata.len().saturating_sub(MAX_LOG_BYTES),
                ))?;
                let mut bytes = Vec::new();
                file.take(MAX_LOG_BYTES).read_to_end(&mut bytes)?;
                Ok(bytes)
            })();
            match result {
                Ok(buffer) if !buffer.is_empty() => {
                    attachments.push(FeedbackAttachment {
                        filename,
                        content_type: Some("text/plain".to_string()),
                        buffer,
                    });
                    // Legacy filenames are a fallback, not additional attachments.
                    break;
                }
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => {
                    tracing::warn!(filename, error_kind = ?error.kind(), "failed to read daemon feedback log; skipping attachment")
                }
            }
        }
    }
    attachments
}

#[cfg(test)]
#[path = "daemon_logs_tests.rs"]
mod tests;
