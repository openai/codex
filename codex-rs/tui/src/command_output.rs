//! Capture short subprocess output without waiting on a pipe held open by descendants.

use std::io::Read;
use std::io::Seek;
use std::process::Command;
use std::time::Duration;
use std::time::Instant;

const POLL_INTERVAL: Duration = Duration::from_millis(/*millis*/ 10);
const MAX_OUTPUT_BYTES: u64 = 4096;

pub(crate) fn command_stdout_before_deadline(
    command: &mut Command,
    deadline: Instant,
) -> Option<Vec<u8>> {
    if Instant::now() >= deadline {
        return None;
    }
    let mut stdout = tempfile::tempfile().ok()?;
    command.stdout(stdout.try_clone().ok()?);
    let mut child = command.spawn().ok()?;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return None;
                }
                stdout.rewind().ok()?;
                let mut output = Vec::new();
                stdout
                    .take(MAX_OUTPUT_BYTES)
                    .read_to_end(&mut output)
                    .ok()?;
                return Some(output);
            }
            Ok(None) if !remaining.is_zero() => {
                std::thread::sleep(remaining.min(POLL_INTERVAL));
            }
            Ok(None) | Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}
