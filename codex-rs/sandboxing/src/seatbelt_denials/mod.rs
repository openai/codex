#[cfg(target_os = "macos")]
use std::collections::HashSet;
#[cfg(target_os = "macos")]
use std::collections::VecDeque;
#[cfg(target_os = "macos")]
use tokio::io::AsyncBufReadExt;
#[cfg(target_os = "macos")]
use tokio::process::Child;
#[cfg(target_os = "macos")]
use tokio::task::JoinHandle;

#[cfg(target_os = "macos")]
use self::pid_tracker::PidTracker;

#[cfg(target_os = "macos")]
const LOG_STREAM_READY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);
#[cfg(target_os = "macos")]
const LOG_STREAM_FLUSH_GRACE_PERIOD: std::time::Duration = std::time::Duration::from_millis(100);
#[cfg(target_os = "macos")]
const LOG_STREAM_READY_PREFIX: &str = "Filtering the log data using ";
#[cfg(target_os = "macos")]
const MAX_COLLECTED_LOG_CHARS: usize = 1_000;

#[cfg(target_os = "macos")]
mod pid_tracker;

#[cfg(target_os = "macos")]
/// A unique macOS Seatbelt sandbox denial emitted by a process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxDenial {
    pub name: String,
    pub capability: String,
}

#[cfg(target_os = "macos")]
#[derive(Clone, Debug, PartialEq, Eq)]
struct LoggedDenial {
    pid: i32,
    denial: SandboxDenial,
    chars: usize,
}

/// Best-effort collector for macOS Seatbelt denials emitted by a process tree.
pub struct DenialLogger {
    #[cfg(target_os = "macos")]
    log_stream: Child,
    #[cfg(target_os = "macos")]
    pid_tracker: Option<PidTracker>,
    #[cfg(target_os = "macos")]
    log_reader: JoinHandle<VecDeque<LoggedDenial>>,
    #[cfg(target_os = "macos")]
    tracked_pid_tx: std::sync::mpsc::Sender<i32>,
}

impl DenialLogger {
    /// Starts collecting Seatbelt denial log messages.
    #[cfg(target_os = "macos")]
    pub async fn new() -> Option<Self> {
        Self::new_with_limit(/*max_chars*/ None).await
    }

    /// Starts collecting Seatbelt denials while retaining at most 1,000 characters.
    #[cfg(target_os = "macos")]
    pub async fn new_bounded() -> Option<Self> {
        Self::new_with_limit(Some(MAX_COLLECTED_LOG_CHARS)).await
    }

    #[cfg(target_os = "macos")]
    async fn new_with_limit(max_chars: Option<usize>) -> Option<Self> {
        let mut log_stream = start_log_stream()?;
        let stdout = log_stream.stdout.take()?;
        let stderr = log_stream.stderr.take()?;
        let (ready_tx, mut ready_rx) = tokio::sync::mpsc::channel(1);
        let (tracked_pid_tx, tracked_pid_rx) = std::sync::mpsc::channel();
        let log_reader = tokio::spawn(async move {
            let mut stdout = tokio::io::BufReader::new(stdout);
            let mut stderr = tokio::io::BufReader::new(stderr);
            let mut logged_denials = VecDeque::new();
            let mut tracked_pids = HashSet::new();
            let mut collected_chars = 0;
            let mut stdout_line = Vec::new();
            let mut stderr_line = String::new();
            let mut stdout_open = true;
            let mut stderr_open = true;
            while stdout_open || stderr_open {
                tokio::select! {
                    result = stdout.read_until(b'\n', &mut stdout_line), if stdout_open => {
                        stdout_open = result.is_ok_and(|read| read > 0);
                        if stdout_line.starts_with(LOG_STREAM_READY_PREFIX.as_bytes()) {
                            let _ = ready_tx.try_send(());
                        } else if let Some(denial) = parse_log_line(&stdout_line) {
                            tracked_pids.extend(tracked_pid_rx.try_iter());
                            push_logged_denial(
                                &mut logged_denials,
                                &mut collected_chars,
                                denial,
                                max_chars,
                                &tracked_pids,
                            );
                        }
                        stdout_line.clear();
                    }
                    result = stderr.read_line(&mut stderr_line), if stderr_open => {
                        stderr_open = result.is_ok_and(|read| read > 0);
                        if stderr_line.starts_with(LOG_STREAM_READY_PREFIX) {
                            let _ = ready_tx.try_send(());
                        }
                        stderr_line.clear();
                    }
                }
            }
            logged_denials
        });

        let ready = tokio::time::timeout(LOG_STREAM_READY_TIMEOUT, ready_rx.recv())
            .await
            .is_ok_and(|result| result.is_some());
        if !ready {
            let _ = log_stream.kill().await;
            let _ = log_stream.wait().await;
            log_reader.abort();
            return None;
        }

        Some(Self {
            log_stream,
            pid_tracker: None,
            log_reader,
            tracked_pid_tx,
        })
    }

    /// Returns no logger on platforms without macOS Seatbelt.
    #[cfg(not(target_os = "macos"))]
    pub async fn new() -> Option<Self> {
        None
    }

    /// Returns no logger on platforms without macOS Seatbelt.
    #[cfg(not(target_os = "macos"))]
    pub async fn new_bounded() -> Option<Self> {
        None
    }

    /// Starts tracking the process tree rooted at `child_pid`.
    #[cfg(target_os = "macos")]
    pub fn on_child_pid(&mut self, child_pid: Option<u32>) {
        if let Some(root_pid) = child_pid {
            self.pid_tracker = PidTracker::new(root_pid as i32, self.tracked_pid_tx.clone());
        }
    }

    /// Does nothing on platforms without macOS Seatbelt.
    #[cfg(not(target_os = "macos"))]
    pub fn on_child_pid(&mut self, child_pid: Option<u32>) {
        let _ = child_pid;
    }

    /// Stops collection and returns unique denials from the tracked process tree.
    #[cfg(target_os = "macos")]
    pub async fn finish(mut self) -> Vec<SandboxDenial> {
        let pid_set = match self.pid_tracker {
            Some(tracker) => tracker.stop().await,
            None => Default::default(),
        };

        if !pid_set.is_empty() {
            tokio::time::sleep(LOG_STREAM_FLUSH_GRACE_PERIOD).await;
        }
        let _ = self.log_stream.kill().await;
        let _ = self.log_stream.wait().await;
        let logged_denials = self.log_reader.await.unwrap_or_default();
        if pid_set.is_empty() {
            return Vec::new();
        }

        let mut seen: HashSet<(String, String)> = HashSet::new();
        let mut denials: Vec<SandboxDenial> = Vec::new();
        for LoggedDenial { pid, denial, .. } in logged_denials {
            if pid_set.contains(&pid)
                && seen.insert((denial.name.clone(), denial.capability.clone()))
            {
                denials.push(denial);
            }
        }
        denials
    }

    /// Returns no denials on platforms without macOS Seatbelt.
    #[cfg(not(target_os = "macos"))]
    pub async fn finish(self) -> Vec<SandboxDenial> {
        Vec::new()
    }
}

#[cfg(target_os = "macos")]
fn push_logged_denial(
    logged_denials: &mut VecDeque<LoggedDenial>,
    collected_chars: &mut usize,
    logged_denial: LoggedDenial,
    max_chars: Option<usize>,
    tracked_pids: &HashSet<i32>,
) {
    let denial_chars = logged_denial.chars;
    if let Some(max_chars) = max_chars {
        if denial_chars > max_chars {
            return;
        }
        logged_denials.push_back(logged_denial);
        *collected_chars += denial_chars;

        while *collected_chars > max_chars {
            let removal_index = logged_denials
                .iter()
                .position(|logged_denial| !tracked_pids.contains(&logged_denial.pid))
                .unwrap_or(0);
            let Some(removed) = logged_denials.remove(removal_index) else {
                break;
            };
            *collected_chars -= removed.chars;
        }
        return;
    }
    logged_denials.push_back(logged_denial);
}

#[cfg(target_os = "macos")]
fn parse_log_line(line: &[u8]) -> Option<LoggedDenial> {
    let json = serde_json::from_slice::<serde_json::Value>(line).ok()?;
    let msg = json.get("eventMessage")?.as_str()?;
    let (pid, name, capability) = parse_message(msg)?;
    let chars = name.chars().count() + capability.chars().count();
    Some(LoggedDenial {
        pid,
        denial: SandboxDenial { name, capability },
        chars,
    })
}

#[cfg(target_os = "macos")]
fn start_log_stream() -> Option<Child> {
    use std::process::Stdio;

    const PREDICATE: &str = r#"(((processID == 0) AND (senderImagePath CONTAINS "/Sandbox")) OR (subsystem == "com.apple.sandbox.reporting"))"#;

    tokio::process::Command::new("log")
        .args(["stream", "--style", "ndjson", "--predicate", PREDICATE])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .ok()
}

#[cfg(target_os = "macos")]
fn parse_message(msg: &str) -> Option<(i32, String, String)> {
    // Example message:
    // Sandbox: processname(1234) deny(1) capability-name args...
    static RE: std::sync::OnceLock<regex_lite::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        #[expect(clippy::unwrap_used)]
        regex_lite::Regex::new(r"^Sandbox:\s*(.+?)\((\d+)\)\s+deny\(.*?\)\s*(.+)$").unwrap()
    });

    let (_, [name, pid_str, capability]) = re.captures(msg)?.extract();
    let pid = pid_str.trim().parse::<i32>().ok()?;
    Some((pid, name.to_string(), capability.to_string()))
}

/// Formats denials for appending to command output.
pub fn format_sandbox_denials(denials: &[SandboxDenial]) -> Option<Vec<u8>> {
    if denials.is_empty() {
        return None;
    }

    let mut formatted = String::from("\n=== Sandbox denials ===\n");
    for SandboxDenial { name, capability } in denials {
        formatted.push_str(&format!("({name}) {capability}\n"));
    }
    Some(formatted.into_bytes())
}

#[cfg(all(test, target_os = "macos"))]
#[path = "seatbelt_denials_tests.rs"]
mod tests;
