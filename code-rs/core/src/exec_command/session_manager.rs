#![allow(dead_code)]
use std::collections::{HashMap, VecDeque};
use std::io::ErrorKind;
use std::io::Read;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::AtomicU32;

use portable_pty::CommandBuilder;
use portable_pty::PtySize;
use portable_pty::native_pty_system;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::Duration;
use tokio::time::Instant;
use tokio::time::timeout;

use crate::exec_command::exec_command_params::ExecCommandParams;
use crate::exec_command::exec_command_params::WriteStdinParams;
use crate::exec_command::exec_command_session::ExecCommandSession;
use crate::exec_command::session_id::SessionId;
use code_protocol::models::FunctionCallOutputPayload;

#[derive(Debug, Default)]
pub struct SessionManager {
    next_session_id: AtomicU32,
    sessions: Mutex<HashMap<SessionId, ExecCommandSession>>,
}

#[derive(Debug)]
pub struct ExecCommandOutput {
    wall_time: Duration,
    exit_status: ExitStatus,
    original_token_count: Option<u64>,
    output: String,
}

struct TruncatingCollector {
    cap_bytes: usize,
    total_bytes: u64,
    prefix: Vec<u8>,
    suffix: VecDeque<u8>,
}

impl TruncatingCollector {
    fn new(cap_bytes: usize) -> Self {
        Self {
            cap_bytes,
            total_bytes: 0,
            prefix: Vec::new(),
            suffix: VecDeque::new(),
        }
    }

    fn push(&mut self, chunk: &[u8]) {
        if chunk.is_empty() {
            return;
        }

        self.total_bytes = self
            .total_bytes
            .saturating_add(chunk.len() as u64);

        if self.prefix.len() < self.cap_bytes {
            let remaining = self.cap_bytes - self.prefix.len();
            let take = remaining.min(chunk.len());
            self.prefix.extend_from_slice(&chunk[..take]);
        }

        if self.cap_bytes > 0 {
            for byte in chunk {
                self.suffix.push_back(*byte);
                if self.suffix.len() > self.cap_bytes {
                    self.suffix.pop_front();
                }
            }
        }
    }

    fn finalize(&self) -> (String, Option<u64>) {
        let est_tokens = (self.total_bytes).div_ceil(4);
        if self.cap_bytes == 0 {
            if self.total_bytes == 0 {
                return (String::new(), None);
            }
            return (format!("…{est_tokens} tokens truncated…"), Some(est_tokens));
        }

        if (self.total_bytes as usize) <= self.cap_bytes {
            return (
                String::from_utf8_lossy(&self.prefix).into_owned(),
                None,
            );
        }

        let prefix_str = String::from_utf8_lossy(&self.prefix).into_owned();
        let suffix_bytes: Vec<u8> = self.suffix.iter().copied().collect();
        let suffix_str = String::from_utf8_lossy(&suffix_bytes).into_owned();

        let mut guess_tokens = est_tokens;
        for _ in 0..4 {
            let marker = format!("…{guess_tokens} tokens truncated…");
            let marker_len = marker.len();
            let keep_budget = self.cap_bytes.saturating_sub(marker_len);
            if keep_budget == 0 {
                return (format!("…{est_tokens} tokens truncated…"), Some(est_tokens));
            }
            let left_budget = keep_budget / 2;
            let right_budget = keep_budget - left_budget;
            let prefix_slice = pick_prefix_slice(&prefix_str, left_budget);
            let suffix_slice = pick_suffix_slice(&suffix_str, right_budget);
            let kept_content_bytes = prefix_slice.as_bytes().len() + suffix_slice.as_bytes().len();
            let truncated_content_bytes = self
                .total_bytes
                .saturating_sub(kept_content_bytes as u64);
            let new_tokens = truncated_content_bytes.div_ceil(4);
            if new_tokens == guess_tokens {
                let mut out = String::with_capacity(marker_len + kept_content_bytes + 1);
                out.push_str(prefix_slice);
                out.push_str(&marker);
                out.push('\n');
                out.push_str(suffix_slice);
                return (out, Some(est_tokens));
            }
            guess_tokens = new_tokens;
        }

        let marker = format!("…{guess_tokens} tokens truncated…");
        let marker_len = marker.len();
        let keep_budget = self.cap_bytes.saturating_sub(marker_len);
        if keep_budget == 0 {
            return (format!("…{est_tokens} tokens truncated…"), Some(est_tokens));
        }
        let left_budget = keep_budget / 2;
        let right_budget = keep_budget - left_budget;
        let prefix_slice = pick_prefix_slice(&prefix_str, left_budget);
        let suffix_slice = pick_suffix_slice(&suffix_str, right_budget);
        let mut out = String::with_capacity(
            marker_len + prefix_slice.as_bytes().len() + suffix_slice.as_bytes().len() + 1,
        );
        out.push_str(prefix_slice);
        out.push_str(&marker);
        out.push('\n');
        out.push_str(suffix_slice);
        (out, Some(est_tokens))
    }
}

fn pick_prefix_slice<'a>(input: &'a str, left_budget: usize) -> &'a str {
    if left_budget >= input.len() {
        return input;
    }
    if let Some(head) = input.get(..left_budget) {
        if let Some(idx) = head.rfind('\n') {
            return &input[..idx + 1];
        }
    }
    truncate_on_boundary(input, left_budget)
}

fn pick_suffix_slice<'a>(input: &'a str, right_budget: usize) -> &'a str {
    if right_budget >= input.len() {
        return input;
    }
    let tail_start = input.len().saturating_sub(right_budget);
    if let Some(tail) = input.get(tail_start..) {
        if let Some(idx) = tail.find('\n') {
            return &input[tail_start + idx + 1..];
        }
    }
    let mut idx = tail_start;
    while idx < input.len() && !input.is_char_boundary(idx) {
        idx += 1;
    }
    &input[idx..]
}

fn truncate_on_boundary<'a>(input: &'a str, max_len: usize) -> &'a str {
    if input.len() <= max_len {
        return input;
    }
    let mut end = max_len;
    while end > 0 && !input.is_char_boundary(end) {
        end -= 1;
    }
    &input[..end]
}

impl ExecCommandOutput {
    pub(crate) fn to_text_output(&self) -> String {
        let wall_time_secs = self.wall_time.as_secs_f32();
        let termination_status = match self.exit_status {
            ExitStatus::Exited(code) => format!("Process exited with code {code}"),
            ExitStatus::Ongoing(session_id) => {
                format!("Process running with session ID {}", session_id.0)
            }
        };
        let truncation_status = match self.original_token_count {
            Some(tokens) => {
                format!("\nWarning: truncated output (original token count: {tokens})")
            }
            None => "".to_string(),
        };
        format!(
            r#"Wall time: {wall_time_secs:.3} seconds
{termination_status}{truncation_status}
Output:
{output}"#,
            output = self.output
        )
    }
}

#[derive(Debug)]
pub enum ExitStatus {
    Exited(i32),
    Ongoing(SessionId),
}

pub fn result_into_payload(result: Result<ExecCommandOutput, String>) -> FunctionCallOutputPayload {
    match result {
        Ok(output) => FunctionCallOutputPayload {
            content: output.to_text_output(),
            success: Some(true),
        },
        Err(err) => FunctionCallOutputPayload {
            content: err,
            success: Some(false),
        },
    }
}

impl SessionManager {
    /// Processes the request and is required to send a response via `outgoing`.
    pub async fn handle_exec_command_request(
        &self,
        params: ExecCommandParams,
    ) -> Result<ExecCommandOutput, String> {
        // Allocate a session id.
        let session_id = SessionId(
            self.next_session_id
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst),
        );

        let (session, mut output_rx, mut exit_rx) = create_exec_command_session(params.clone())
            .await
            .map_err(|err| {
                format!(
                    "failed to create exec command session for session id {}: {err}",
                    session_id.0
                )
            })?;

        // Insert into session map.
        self.sessions.lock().await.insert(session_id, session);

        // Collect output until either timeout expires or process exits.
        // Enforce the byte cap incrementally so runaway commands cannot exhaust memory.
        let cap_bytes_u64 = params.max_output_tokens.saturating_mul(4);
        let cap_bytes: usize = cap_bytes_u64.min(usize::MAX as u64) as usize;
        let mut collector = TruncatingCollector::new(cap_bytes);

        let start_time = Instant::now();
        let deadline = start_time + Duration::from_millis(params.yield_time_ms);
        let mut exit_code: Option<i32> = None;

        loop {
            if Instant::now() >= deadline {
                break;
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            tokio::select! {
                biased;
                exit = &mut exit_rx => {
                    exit_code = exit.ok();
                    // Small grace period to pull remaining buffered output
                    let grace_deadline = Instant::now() + Duration::from_millis(25);
                    while Instant::now() < grace_deadline {
                        match timeout(Duration::from_millis(1), output_rx.recv()).await {
                            Ok(Ok(chunk)) => {
                                collector.push(&chunk);
                            }
                            Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => {
                                // Skip missed messages; keep trying within grace period.
                                continue;
                            }
                            Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => break,
                            Err(_) => break,
                        }
                    }
                    break;
                }
                chunk = timeout(remaining, output_rx.recv()) => {
                    match chunk {
                        Ok(Ok(chunk)) => {
                            collector.push(&chunk);
                        }
                        Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => {
                            // Skip missed messages; continue collecting fresh output.
                        }
                        Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => { break; }
                        Err(_) => { break; }
                    }
                }
            }
        }

        let exit_status = if let Some(code) = exit_code {
            ExitStatus::Exited(code)
        } else {
            ExitStatus::Ongoing(session_id)
        };

        let (output, original_token_count) = collector.finalize();
        Ok(ExecCommandOutput {
            wall_time: Instant::now().duration_since(start_time),
            exit_status,
            original_token_count,
            output,
        })
    }

    /// Write characters to a session's stdin and collect combined output for up to `yield_time_ms`.
    pub async fn handle_write_stdin_request(
        &self,
        params: WriteStdinParams,
    ) -> Result<ExecCommandOutput, String> {
        let WriteStdinParams {
            session_id,
            chars,
            yield_time_ms,
            max_output_tokens,
        } = params;

        // Grab handles without holding the sessions lock across await points.
        let (writer_tx, mut output_rx) = {
            let sessions = self.sessions.lock().await;
            match sessions.get(&session_id) {
                Some(session) => {
                    // Touch exit flag to mark the field as used and enable early checks in the future.
                    let _exited = session.has_exited();
                    (session.writer_sender(), session.output_receiver())
                }
                None => {
                    return Err(format!("unknown session id {}", session_id.0));
                }
            }
        };

        // Write stdin if provided.
        if !chars.is_empty() && writer_tx.send(chars.into_bytes()).await.is_err() {
            return Err("failed to write to stdin".to_string());
        }

        let cap_bytes_u64 = max_output_tokens.saturating_mul(4);
        let cap_bytes: usize = cap_bytes_u64.min(usize::MAX as u64) as usize;

        // Collect output up to yield_time_ms, truncating to max_output_tokens bytes.
        let mut collector = TruncatingCollector::new(cap_bytes);
        let start_time = Instant::now();
        let deadline = start_time + Duration::from_millis(yield_time_ms);
        loop {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            let remaining = deadline - now;
            match timeout(remaining, output_rx.recv()).await {
                Ok(Ok(chunk)) => {
                    // Collect all output within the time budget while enforcing the cap.
                    collector.push(&chunk);
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => {
                    // Skip missed messages; continue collecting fresh output.
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => break,
                Err(_) => break, // timeout
            }
        }

        let (output, original_token_count) = collector.finalize();
        Ok(ExecCommandOutput {
            wall_time: Instant::now().duration_since(start_time),
            exit_status: ExitStatus::Ongoing(session_id),
            original_token_count,
            output,
        })
    }

    /// Kill all running exec sessions by dropping their session objects.
    /// This is invoked on user interrupts to ensure no child processes remain.
    pub async fn kill_all(&self) {
        let mut sessions = self.sessions.lock().await;
        sessions.clear(); // dropping ExecCommandSession triggers ChildKiller::kill in Drop
    }
}

#[cfg(test)]
impl SessionManager {
    pub(crate) async fn active_session_count(&self) -> usize {
        self.sessions.lock().await.len()
    }
}

/// Spawn PTY and child process per spawn_exec_command_session logic.
async fn create_exec_command_session(
    params: ExecCommandParams,
) -> anyhow::Result<(
    ExecCommandSession,
    tokio::sync::broadcast::Receiver<Vec<u8>>,
    oneshot::Receiver<i32>,
)> {
    let ExecCommandParams {
        cmd,
        yield_time_ms: _,
        max_output_tokens: _,
        shell,
        login,
    } = params;

    // Use the native pty implementation for the system
    let pty_system = native_pty_system();

    // Create a new pty
    let pair = pty_system.openpty(PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    })?;

    // Spawn a shell into the pty
    let mut command_builder = CommandBuilder::new(shell);
    let shell_mode_opt = if login { "-lc" } else { "-c" };
    command_builder.arg(shell_mode_opt);
    command_builder.arg(cmd);

    let mut child = pair.slave.spawn_command(command_builder)?;
    // Obtain a killer that can signal the process independently of `.wait()`.
    let killer = child.clone_killer();

    // Channel to forward write requests to the PTY writer.
    let (writer_tx, mut writer_rx) = mpsc::channel::<Vec<u8>>(128);
    // Broadcast for streaming PTY output to readers: subscribers receive from subscription time.
    let (output_tx, _) = tokio::sync::broadcast::channel::<Vec<u8>>(256);
    // Reader task: drain PTY and forward chunks to output channel.
    let mut reader = pair.master.try_clone_reader()?;
    let output_tx_clone = output_tx.clone();
    let reader_handle = tokio::task::spawn_blocking(move || {
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break, // EOF
                Ok(n) => {
                    // Forward to broadcast; best-effort if there are subscribers.
                    let _ = output_tx_clone.send(buf[..n].to_vec());
                }
                Err(ref e) if e.kind() == ErrorKind::Interrupted => {
                    // Retry on EINTR
                    continue;
                }
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                    // We're in a blocking thread; back off briefly and retry.
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                }
                Err(_) => break,
            }
        }
    });

    // Writer task: apply stdin writes to the PTY writer.
    let writer = pair.master.take_writer()?;
    let writer = Arc::new(StdMutex::new(writer));
    let writer_handle = tokio::spawn({
        let writer = writer.clone();
        async move {
            while let Some(bytes) = writer_rx.recv().await {
                let writer = writer.clone();
                // Perform blocking write on a blocking thread.
                let _ = tokio::task::spawn_blocking(move || {
                    if let Ok(mut guard) = writer.lock() {
                        use std::io::Write;
                        let _ = guard.write_all(&bytes);
                        let _ = guard.flush();
                    }
                })
                .await;
            }
        }
    });

    // Keep the child alive until it exits, then signal exit code.
    let (exit_tx, exit_rx) = oneshot::channel::<i32>();
    // Track process exit status for concurrent queries.
    let exit_status = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let exit_status_for_wait = exit_status.clone();
    let wait_handle = tokio::task::spawn_blocking(move || {
        let code = match child.wait() {
            Ok(status) => status.exit_code() as i32,
            Err(_) => -1,
        };
        let _ = exit_tx.send(code);
        // Mark as exited so readers can stop without waiting on the channel.
        exit_status_for_wait.store(true, std::sync::atomic::Ordering::SeqCst);
    });

    // Create and store the session with channels.
    let (session, initial_output_rx) = ExecCommandSession::new(
        writer_tx,
        output_tx,
        killer,
        reader_handle,
        writer_handle,
        wait_handle,
        exit_status,
    );
    Ok((session, initial_output_rx, exit_rx))
}

/// Truncate the middle of a UTF-8 string to at most `max_bytes` bytes,
/// preserving the beginning and the end. Returns the possibly truncated
/// string and `Some(original_token_count)` (estimated at 4 bytes/token)
/// if truncation occurred; otherwise returns the original string and `None`.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::exec_command::session_id::SessionId;
    use crate::truncate::truncate_middle;

    /// Test that verifies that [`SessionManager::handle_exec_command_request()`]
    /// and [`SessionManager::handle_write_stdin_request()`] work as expected
    /// in the presence of a process that never terminates (but produces
    /// output continuously).
    #[cfg(unix)]
    #[allow(clippy::print_stderr)]
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn session_manager_streams_and_truncates_from_now() {
        use crate::exec_command::exec_command_params::ExecCommandParams;
        use crate::exec_command::exec_command_params::WriteStdinParams;
        use tokio::time::sleep;

        let session_manager = SessionManager::default();
        // Long-running loop that prints an increasing counter every ~100ms.
        // Use Python for a portable, reliable sleep across shells/PTYs.
        let cmd = r#"python3 - <<'PY'
import sys, time
count = 0
while True:
    print(count)
    sys.stdout.flush()
    count += 100
    time.sleep(0.1)
PY"#
        .to_string();

        // Start the session and collect ~3s of output.
        let params = ExecCommandParams {
            cmd,
            yield_time_ms: 3_000,
            max_output_tokens: 1_000, // large enough to avoid truncation here
            shell: "/bin/bash".to_string(),
            login: false,
        };
        let initial_output = match session_manager
            .handle_exec_command_request(params.clone())
            .await
        {
            Ok(v) => v,
            Err(e) => {
                // PTY may be restricted in some sandboxes; skip in that case.
                if e.contains("openpty") || e.contains("Operation not permitted") {
                    eprintln!("skipping test due to restricted PTY: {e}");
                    return;
                }
                panic!("exec request failed unexpectedly: {e}");
            }
        };
        eprintln!("initial output: {initial_output:?}");

        // Should be ongoing (we launched a never-ending loop).
        let session_id = match initial_output.exit_status {
            ExitStatus::Ongoing(id) => id,
            _ => panic!("expected ongoing session"),
        };

        // Parse the numeric lines and get the max observed value in the first window.
        let first_nums = extract_monotonic_numbers(&initial_output.output);
        assert!(
            !first_nums.is_empty(),
            "expected some output from first window"
        );
        let first_max = *first_nums.iter().max().unwrap();

        // Wait ~4s so counters progress while we're not reading.
        sleep(Duration::from_millis(4_000)).await;

        // Now read ~3s of output "from now" only.
        // Use a small token cap so truncation occurs and we test middle truncation.
        let write_params = WriteStdinParams {
            session_id,
            chars: String::new(),
            yield_time_ms: 3_000,
            max_output_tokens: 16, // 16 tokens ~= 64 bytes -> likely truncation
        };
        let second = session_manager
            .handle_write_stdin_request(write_params)
            .await
            .expect("write stdin should succeed");

        // Verify truncation metadata and size bound (cap is tokens*4 bytes).
        assert!(second.original_token_count.is_some());
        let cap_bytes = (16u64 * 4) as usize;
        assert!(second.output.len() <= cap_bytes);
        // New middle marker should be present.
        assert!(
            second.output.contains("tokens truncated") && second.output.contains('…'),
            "expected truncation marker in output, got: {}",
            second.output
        );

        // Minimal freshness check: the earliest number we see in the second window
        // should be significantly larger than the last from the first window.
        let second_nums = extract_monotonic_numbers(&second.output);
        assert!(
            !second_nums.is_empty(),
            "expected some numeric output from second window"
        );
        let second_min = *second_nums.iter().min().unwrap();

        // We slept 4 seconds (~40 ticks at 100ms/tick, each +100), so expect
        // an increase of roughly 4000 or more. Allow a generous margin.
        assert!(
            second_min >= first_max + 2000,
            "second_min={second_min} first_max={first_max}",
        );
    }

    #[cfg(unix)]
    fn extract_monotonic_numbers(s: &str) -> Vec<i64> {
        s.lines()
            .filter_map(|line| {
                if !line.is_empty() && line.chars().all(|c| c.is_ascii_digit()) {
                    if let Ok(n) = line.parse::<i64>() {
                        // Our generator increments by 100; ignore spurious fragments.
                        if n % 100 == 0 {
                            return Some(n);
                        }
                    }
                }
                None
            })
            .collect()
    }

    #[test]
    fn to_text_output_exited_no_truncation() {
        let out = ExecCommandOutput {
            wall_time: Duration::from_millis(1234),
            exit_status: ExitStatus::Exited(0),
            original_token_count: None,
            output: "hello".to_string(),
        };
        let text = out.to_text_output();
        let expected = r#"Wall time: 1.234 seconds
Process exited with code 0
Output:
hello"#;
        assert_eq!(expected, text);
    }

    #[test]
    fn to_text_output_ongoing_with_truncation() {
        let out = ExecCommandOutput {
            wall_time: Duration::from_millis(500),
            exit_status: ExitStatus::Ongoing(SessionId(42)),
            original_token_count: Some(1000),
            output: "abc".to_string(),
        };
        let text = out.to_text_output();
        let expected = r#"Wall time: 0.500 seconds
Process running with session ID 42
Warning: truncated output (original token count: 1000)
Output:
abc"#;
        assert_eq!(expected, text);
    }

    #[test]
    fn truncating_collector_no_newlines_fallback() {
        let s = "abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
        let cap_bytes = 16; // force truncation
        let mut collector = TruncatingCollector::new(cap_bytes);
        collector.push(s.as_bytes());
        let result = collector.finalize();
        let expected = truncate_middle(s, cap_bytes);
        assert_eq!(result, expected);
    }

    #[test]
    fn truncating_collector_prefers_newline_boundaries() {
        let mut s = String::new();
        for i in 1..=20 {
            s.push_str(&format!("{i:03}\n"));
        }
        assert_eq!(s.len(), 80);

        let cap_bytes = 64;
        let mut collector = TruncatingCollector::new(cap_bytes);
        for chunk in s.as_bytes().chunks(7) {
            collector.push(chunk);
        }
        let result = collector.finalize();
        let expected = truncate_middle(&s, cap_bytes);
        assert_eq!(result, expected);
    }

    #[test]
    fn truncating_collector_handles_zero_cap() {
        let input = "some output that will be completely truncated";
        let mut collector = TruncatingCollector::new(0);
        collector.push(input.as_bytes());
        let (out, original) = collector.finalize();
        let expected_tokens = (input.len() as u64).div_ceil(4);
        assert_eq!(out, format!("…{expected_tokens} tokens truncated…"));
        assert_eq!(original, Some(expected_tokens));
    }
}
