//! Shell command executor with timeout, background support, and shell snapshotting.
//!
//! ## Sandbox Mode
//!
//! This executor currently runs in **non-sandbox mode** by default, which means
//! commands execute directly without any sandbox restrictions. This matches
//! Claude Code's architecture where sandbox is optional and disabled by default.
//!
//! To check if a command should be sandboxed, use [`cocode_sandbox::SandboxSettings::is_sandboxed()`].
//! When sandbox mode is enabled in the future, the executor will wrap commands with
//! platform-specific sandbox enforcement (Landlock on Linux, Seatbelt on macOS).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::Instant;

use tokio::io::AsyncReadExt;
use tokio::sync::Mutex;
use tokio::sync::Notify;

use crate::background::BackgroundProcess;
use crate::background::BackgroundTaskRegistry;
use crate::command::CommandResult;
use crate::command::ExtractedPaths;
use crate::path_extractor::PathExtractor;
use crate::path_extractor::filter_existing_files;
use crate::path_extractor::truncate_for_extraction;
use crate::shell_types::Shell;
use crate::shell_types::default_user_shell;
use crate::snapshot::ShellSnapshot;
use crate::snapshot::SnapshotConfig;

/// Default command timeout in seconds.
const DEFAULT_TIMEOUT_SECS: i64 = 120;

/// Maximum output size in bytes before truncation (30KB).
const MAX_OUTPUT_BYTES: i64 = 30_000;

/// Environment variable to disable shell snapshotting.
const DISABLE_SNAPSHOT_ENV: &str = "COCODE_DISABLE_SHELL_SNAPSHOT";

/// Marker for CWD extraction from command output (start).
const CWD_MARKER_START: &str = "__COCODE_CWD_START__";

/// Marker for CWD extraction from command output (end).
const CWD_MARKER_END: &str = "__COCODE_CWD_END__";

/// Shell command executor.
///
/// Provides async execution of shell commands with timeout support,
/// output capture, background task management, and optional shell
/// environment snapshotting.
///
/// ## Shell Snapshotting
///
/// When enabled (default), the executor captures the user's shell environment
/// (aliases, functions, exports, options) and sources it before each command.
/// This ensures commands run with the same environment as the user's interactive shell.
///
/// To disable snapshotting, set the environment variable:
/// ```sh
/// export COCODE_DISABLE_SHELL_SNAPSHOT=1
/// ```
///
/// ## Path Extraction
///
/// When a path extractor is configured (via `with_path_extractor`), the executor
/// can extract file paths from command output for fast model pre-reading.
/// Use `execute_with_extraction` to enable this feature.
#[derive(Clone)]
pub struct ShellExecutor {
    /// Default timeout for command execution in seconds.
    pub default_timeout_secs: i64,
    /// Working directory for command execution (shared across clones).
    cwd: Arc<StdMutex<PathBuf>>,
    /// Registry for background tasks.
    pub background_registry: BackgroundTaskRegistry,
    /// Shell configuration with optional snapshot.
    shell: Option<Shell>,
    /// Whether snapshot was initialized.
    snapshot_initialized: bool,
    /// Optional path extractor for extracting file paths from command output.
    path_extractor: Option<Arc<dyn PathExtractor>>,
}

impl std::fmt::Debug for ShellExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShellExecutor")
            .field("default_timeout_secs", &self.default_timeout_secs)
            .field("cwd", &*self.cwd.lock().unwrap())
            .field("background_registry", &self.background_registry)
            .field("shell", &self.shell)
            .field("snapshot_initialized", &self.snapshot_initialized)
            .field("path_extractor", &self.path_extractor.is_some())
            .finish()
    }
}

impl ShellExecutor {
    /// Creates a new executor with the given working directory.
    ///
    /// Shell snapshotting is **not** automatically started. Call `start_snapshotting()`
    /// or `with_shell()` to enable snapshot support.
    pub fn new(cwd: PathBuf) -> Self {
        Self {
            default_timeout_secs: DEFAULT_TIMEOUT_SECS,
            cwd: Arc::new(StdMutex::new(cwd)),
            background_registry: BackgroundTaskRegistry::new(),
            shell: None,
            snapshot_initialized: false,
            path_extractor: None,
        }
    }

    /// Creates a new executor with the given shell configuration.
    ///
    /// The shell's snapshot receiver will be used if available.
    pub fn with_shell(cwd: PathBuf, shell: Shell) -> Self {
        Self {
            default_timeout_secs: DEFAULT_TIMEOUT_SECS,
            cwd: Arc::new(StdMutex::new(cwd)),
            background_registry: BackgroundTaskRegistry::new(),
            shell: Some(shell),
            snapshot_initialized: false,
            path_extractor: None,
        }
    }

    /// Creates a new executor with the user's default shell.
    pub fn with_default_shell(cwd: PathBuf) -> Self {
        Self::with_shell(cwd, default_user_shell())
    }

    /// Sets the path extractor for extracting file paths from command output.
    ///
    /// When a path extractor is configured, `execute_with_extraction()` can
    /// analyze command output to find file paths for fast model pre-reading.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use cocode_shell::{ShellExecutor, NoOpExtractor};
    /// use std::sync::Arc;
    /// use std::path::PathBuf;
    ///
    /// let executor = ShellExecutor::new(PathBuf::from("/project"))
    ///     .with_path_extractor(Arc::new(NoOpExtractor));
    /// ```
    pub fn with_path_extractor(mut self, extractor: Arc<dyn PathExtractor>) -> Self {
        self.path_extractor = Some(extractor);
        self
    }

    /// Returns the configured path extractor, if any.
    pub fn path_extractor(&self) -> Option<&Arc<dyn PathExtractor>> {
        self.path_extractor.as_ref()
    }

    /// Returns true if a path extractor is configured and enabled.
    pub fn has_path_extractor(&self) -> bool {
        self.path_extractor.as_ref().is_some_and(|e| e.is_enabled())
    }

    /// Starts asynchronous shell snapshotting.
    ///
    /// This captures the user's shell environment in the background.
    /// The snapshot will be sourced before each command once available.
    ///
    /// If snapshotting is disabled via environment variable, this is a no-op.
    ///
    /// # Arguments
    ///
    /// * `cocode_home` - Path to cocode home directory (e.g., `~/.cocode`)
    /// * `session_id` - Unique session identifier for the snapshot file
    pub fn start_snapshotting(&mut self, cocode_home: PathBuf, session_id: &str) {
        if is_snapshot_disabled() {
            tracing::debug!("Shell snapshotting disabled via {DISABLE_SNAPSHOT_ENV}");
            self.snapshot_initialized = true;
            return;
        }

        // Initialize shell if not already set
        if self.shell.is_none() {
            self.shell = Some(default_user_shell());
        }

        if let Some(ref mut shell) = self.shell {
            let config = SnapshotConfig::new(&cocode_home);
            ShellSnapshot::start_snapshotting(config, session_id, shell);
            self.snapshot_initialized = true;
            tracing::debug!("Started shell snapshotting for session {session_id}");
        }
    }

    /// Returns the current shell configuration.
    pub fn shell(&self) -> Option<&Shell> {
        self.shell.as_ref()
    }

    /// Returns the current shell snapshot if available.
    pub fn shell_snapshot(&self) -> Option<Arc<ShellSnapshot>> {
        self.shell.as_ref().and_then(|s| s.shell_snapshot())
    }

    /// Returns whether snapshotting has been initialized.
    pub fn is_snapshot_initialized(&self) -> bool {
        self.snapshot_initialized
    }

    /// Returns the current working directory.
    pub fn cwd(&self) -> PathBuf {
        self.cwd.lock().unwrap().clone()
    }

    /// Updates the working directory.
    pub fn set_cwd(&mut self, cwd: PathBuf) {
        *self.cwd.lock().unwrap() = cwd;
    }

    /// Creates a shell executor for subagent use.
    ///
    /// The forked executor:
    /// - Uses the provided `initial_cwd` (not the current tracked CWD)
    /// - Shares the shell configuration and snapshot (Arc, read-only)
    /// - Has its own independent background task registry
    /// - Does NOT track CWD changes (always resets to initial)
    ///
    /// This matches Claude Code's behavior where subagents always
    /// have their CWD reset between bash calls. Subagents should use
    /// absolute paths since CWD resets between calls.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use cocode_shell::ShellExecutor;
    /// use std::path::PathBuf;
    ///
    /// let main_executor = ShellExecutor::with_default_shell(PathBuf::from("/project"));
    /// let subagent_executor = main_executor.fork_for_subagent(PathBuf::from("/project"));
    ///
    /// // Subagent bash calls always start from initial CWD
    /// // cd in one call does NOT affect the next call
    /// ```
    pub fn fork_for_subagent(&self, initial_cwd: PathBuf) -> Self {
        Self {
            default_timeout_secs: self.default_timeout_secs,
            cwd: Arc::new(StdMutex::new(initial_cwd)), // Independent CWD for subagent
            background_registry: BackgroundTaskRegistry::new(), // Independent registry
            shell: self.shell.clone(), // Share shell config (Arc snapshot is shared)
            snapshot_initialized: self.snapshot_initialized,
            path_extractor: self.path_extractor.clone(), // Share path extractor
        }
    }

    /// Executes a command for subagent use (no CWD tracking).
    ///
    /// Unlike `execute_with_cwd_tracking`, this method:
    /// - Always uses the executor's current CWD setting
    /// - Does NOT update internal CWD state after execution
    /// - Suitable for subagent use where CWD should reset between calls
    ///
    /// This is essentially an alias for `execute()` to make the intent clear
    /// when used in subagent contexts.
    pub async fn execute_for_subagent(&self, command: &str, timeout_secs: i64) -> CommandResult {
        self.execute(command, timeout_secs).await
    }

    /// Executes a shell command with the given timeout.
    ///
    /// The command is run via the configured shell with the executor's working directory.
    /// If a shell snapshot is available and the command uses login shell mode (`-lc`),
    /// it is rewritten to source the snapshot via non-login shell (`-c`).
    /// Output is truncated if it exceeds the maximum size limit.
    ///
    /// If the command times out, a `CommandResult` is returned with exit code -1
    /// and a timeout message in stderr.
    pub async fn execute(&self, command: &str, timeout_secs: i64) -> CommandResult {
        let start = Instant::now();

        let timeout = if timeout_secs > 0 {
            timeout_secs
        } else {
            self.default_timeout_secs
        };

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(timeout as u64),
            self.run_command(command),
        )
        .await;

        let duration_ms = start.elapsed().as_millis() as i64;

        match result {
            Ok(cmd_result) => {
                let mut cmd_result = cmd_result;
                cmd_result.duration_ms = duration_ms;
                cmd_result
            }
            Err(_) => CommandResult {
                exit_code: -1,
                stdout: String::new(),
                stderr: format!("Command timed out after {timeout} seconds"),
                duration_ms,
                truncated: false,
                new_cwd: None,
                extracted_paths: None,
            },
        }
    }

    /// Executes a command and updates CWD if changed.
    ///
    /// This is similar to `execute()` but also tracks working directory changes.
    /// If the command succeeds and the CWD changed, the executor's internal CWD
    /// is updated to match.
    pub async fn execute_with_cwd_tracking(
        &mut self,
        command: &str,
        timeout_secs: i64,
    ) -> CommandResult {
        let result = self.execute(command, timeout_secs).await;

        // Update internal CWD if command succeeded and CWD changed
        if result.exit_code == 0 {
            if let Some(ref new_cwd) = result.new_cwd {
                let current_cwd = self.cwd.lock().unwrap().clone();
                if new_cwd.exists() && *new_cwd != current_cwd {
                    tracing::debug!(
                        "CWD changed: {} -> {}",
                        current_cwd.display(),
                        new_cwd.display()
                    );
                    *self.cwd.lock().unwrap() = new_cwd.clone();
                }
            }
        }

        result
    }

    /// Executes a command and extracts file paths from output.
    ///
    /// This combines command execution with path extraction for fast model pre-reading.
    /// If a path extractor is configured and the command succeeds, file paths are
    /// extracted from the output for preloading.
    ///
    /// The output is truncated to 2000 characters for extraction efficiency
    /// (matching Claude Code's behavior).
    ///
    /// # Arguments
    ///
    /// * `command` - The shell command to execute
    /// * `timeout_secs` - Timeout in seconds (0 uses default)
    ///
    /// # Returns
    ///
    /// A `CommandResult` with `extracted_paths` populated if extraction was performed.
    pub async fn execute_with_extraction(&self, command: &str, timeout_secs: i64) -> CommandResult {
        let mut result = self.execute(command, timeout_secs).await;

        // Only extract paths if command succeeded and extractor is available
        if result.exit_code == 0 && self.has_path_extractor() {
            if let Some(ref extractor) = self.path_extractor {
                let extraction_start = Instant::now();
                let cwd = self.cwd.lock().unwrap().clone();

                // Truncate output for extraction efficiency
                let output_for_extraction = truncate_for_extraction(&result.stdout);

                match extractor
                    .extract_paths(command, output_for_extraction, &cwd)
                    .await
                {
                    Ok(extraction_result) => {
                        // Filter to only existing files
                        let existing_paths = filter_existing_files(extraction_result.paths, &cwd);

                        let extraction_ms = extraction_start.elapsed().as_millis() as i64;

                        if !existing_paths.is_empty() {
                            tracing::debug!(
                                "Extracted {} file paths from command output in {}ms",
                                existing_paths.len(),
                                extraction_ms
                            );
                        }

                        result.extracted_paths =
                            Some(ExtractedPaths::new(existing_paths, extraction_ms));
                    }
                    Err(e) => {
                        // Log warning but don't fail the command
                        tracing::warn!("Path extraction failed: {e}");
                        result.extracted_paths = Some(ExtractedPaths::not_attempted());
                    }
                }
            }
        }

        result
    }

    /// Executes a command with both CWD tracking and path extraction.
    ///
    /// Combines the functionality of `execute_with_cwd_tracking` and
    /// `execute_with_extraction` for main agent use cases.
    pub async fn execute_with_cwd_tracking_and_extraction(
        &mut self,
        command: &str,
        timeout_secs: i64,
    ) -> CommandResult {
        let result = self.execute_with_extraction(command, timeout_secs).await;

        // Update internal CWD if command succeeded and CWD changed
        if result.exit_code == 0 {
            if let Some(ref new_cwd) = result.new_cwd {
                let current_cwd = self.cwd.lock().unwrap().clone();
                if new_cwd.exists() && *new_cwd != current_cwd {
                    tracing::debug!(
                        "CWD changed: {} -> {}",
                        current_cwd.display(),
                        new_cwd.display()
                    );
                    *self.cwd.lock().unwrap() = new_cwd.clone();
                }
            }
        }

        result
    }

    /// POSIX-only: rewrite login shell commands to source snapshot.
    ///
    /// For commands of the form `[shell, "-lc", "<script>"]`, when a snapshot
    /// is available, rewrite to `[shell, "-c", ". SNAPSHOT && <script>"]`.
    ///
    /// This preserves the semantic that login shell is used for snapshot capture,
    /// while non-login shell with snapshot sourcing is used for execution.
    fn maybe_wrap_shell_lc_with_snapshot(&self, args: Vec<String>) -> Vec<String> {
        let Some(snapshot) = self.shell_snapshot() else {
            return args;
        };

        // Only rewrite if snapshot file exists
        if !snapshot.path.exists() {
            return args;
        }

        // Require at least [shell, flag, script]
        if args.len() < 3 {
            return args;
        }

        // Only rewrite login shell commands (-lc)
        if args[1] != "-lc" {
            return args;
        }

        let snapshot_path = snapshot.path.to_string_lossy();
        let rewritten_script = format!(". \"{snapshot_path}\" && {}", args[2]);

        vec![args[0].clone(), "-c".to_string(), rewritten_script]
    }

    /// Spawns a command in the background and returns a task ID.
    ///
    /// The command output is captured asynchronously and can be retrieved
    /// via the background registry using the returned task ID.
    pub async fn spawn_background(&self, command: &str) -> Result<String, String> {
        let task_id = format!("bg-{}", uuid_simple());
        let output = Arc::new(Mutex::new(String::new()));
        let completed = Arc::new(Notify::new());

        let process = BackgroundProcess {
            id: task_id.clone(),
            command: command.to_string(),
            output: Arc::clone(&output),
            completed: Arc::clone(&completed),
        };

        self.background_registry
            .register(task_id.clone(), process)
            .await;

        let cwd = self.cwd.lock().unwrap().clone();
        let registry = self.background_registry.clone();
        let bg_task_id = task_id.clone();
        let shell_args = self.get_shell_args(command);
        let shell_args = self.maybe_wrap_shell_lc_with_snapshot(shell_args);

        tokio::spawn(async move {
            let child = tokio::process::Command::new(&shell_args[0])
                .args(&shell_args[1..])
                .current_dir(&cwd)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .kill_on_drop(true)
                .spawn();

            match child {
                Ok(mut child) => {
                    // Read stdout
                    if let Some(mut stdout) = child.stdout.take() {
                        let output = Arc::clone(&output);
                        tokio::spawn(async move {
                            let mut buf = vec![0u8; 4096];
                            loop {
                                match stdout.read(&mut buf).await {
                                    Ok(0) => break,
                                    Ok(n) => {
                                        if let Ok(text) = String::from_utf8(buf[..n].to_vec()) {
                                            let mut out = output.lock().await;
                                            out.push_str(&text);
                                        }
                                    }
                                    Err(_) => break,
                                }
                            }
                        });
                    }

                    // Wait for process to complete
                    let _ = child.wait().await;
                }
                Err(e) => {
                    let mut out = output.lock().await;
                    out.push_str(&format!("Failed to spawn command: {e}"));
                }
            }

            completed.notify_waiters();

            // Remove from registry when done
            registry.stop(&bg_task_id).await;
        });

        Ok(task_id)
    }

    /// Gets shell arguments for executing a command.
    ///
    /// Uses login shell (`-lc`) when a shell is configured, as `maybe_wrap_shell_lc_with_snapshot`
    /// will rewrite to `-c` with snapshot sourcing if needed.
    fn get_shell_args(&self, command: &str) -> Vec<String> {
        if let Some(ref shell) = self.shell {
            // Use login shell (-lc) when snapshot might be available
            // maybe_wrap_shell_lc_with_snapshot will rewrite to -c if needed
            shell.derive_exec_args(command, true)
        } else {
            // Fallback to bash (non-login, no snapshot support)
            vec!["bash".to_string(), "-c".to_string(), command.to_string()]
        }
    }

    /// Internal: runs a command and captures output, tracking CWD changes.
    async fn run_command(&self, command: &str) -> CommandResult {
        let args = self.get_shell_args(command);
        let args = self.maybe_wrap_shell_lc_with_snapshot(args);
        let cwd = self.cwd.lock().unwrap().clone();

        // Wrap the script to capture CWD after execution
        let wrapped_script = format!(
            "{}; __cocode_exit=$?; echo '{}' \"$(pwd)\" '{}'; exit $__cocode_exit",
            &args[2], CWD_MARKER_START, CWD_MARKER_END
        );
        let args = vec![args[0].clone(), args[1].clone(), wrapped_script];

        let child = tokio::process::Command::new(&args[0])
            .args(&args[1..])
            .current_dir(&cwd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn();

        let child = match child {
            Ok(c) => c,
            Err(e) => {
                return CommandResult {
                    exit_code: -1,
                    stdout: String::new(),
                    stderr: format!("Failed to spawn command: {e}"),
                    duration_ms: 0,
                    truncated: false,
                    new_cwd: None,
                    extracted_paths: None,
                };
            }
        };

        let output = match child.wait_with_output().await {
            Ok(o) => o,
            Err(e) => {
                return CommandResult {
                    exit_code: -1,
                    stdout: String::new(),
                    stderr: format!("Failed to wait for command: {e}"),
                    duration_ms: 0,
                    truncated: false,
                    new_cwd: None,
                    extracted_paths: None,
                };
            }
        };

        let exit_code = output.status.code().unwrap_or(-1);
        let (raw_stdout, truncated_stdout) = truncate_output(&output.stdout);
        let (stderr, truncated_stderr) = truncate_output(&output.stderr);

        // Extract CWD from output and clean the stdout
        let (stdout, new_cwd) = extract_cwd_from_output(&raw_stdout);

        CommandResult {
            exit_code,
            stdout,
            stderr,
            duration_ms: 0, // Will be set by caller
            truncated: truncated_stdout || truncated_stderr,
            new_cwd,
            extracted_paths: None,
        }
    }
}

/// Checks if shell snapshotting is disabled via environment variable.
fn is_snapshot_disabled() -> bool {
    std::env::var(DISABLE_SNAPSHOT_ENV)
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(false)
}

/// Truncates output bytes to a string, returning (text, was_truncated).
fn truncate_output(bytes: &[u8]) -> (String, bool) {
    let max = MAX_OUTPUT_BYTES as usize;
    if bytes.len() > max {
        let truncated_bytes = &bytes[..max];
        let text = String::from_utf8_lossy(truncated_bytes).into_owned();
        (text, true)
    } else {
        let text = String::from_utf8_lossy(bytes).into_owned();
        (text, false)
    }
}

/// Extracts CWD from command output that contains CWD markers.
///
/// Returns (cleaned_output, Option<new_cwd>).
/// The markers are removed from the output.
fn extract_cwd_from_output(output: &str) -> (String, Option<PathBuf>) {
    // Look for the CWD marker line at the end of output
    if let Some(start) = output.rfind(CWD_MARKER_START) {
        if let Some(end_offset) = output[start..].find(CWD_MARKER_END) {
            let cwd_start = start + CWD_MARKER_START.len();
            let cwd_end = start + end_offset;
            let cwd_str = output[cwd_start..cwd_end].trim();

            // Clean the output: remove from the marker start to end of marker
            let marker_end = start + end_offset + CWD_MARKER_END.len();
            let cleaned = format!(
                "{}{}",
                output[..start].trim_end_matches('\n'),
                &output[marker_end..]
            )
            .trim_end()
            .to_string();

            // Only return CWD if it's a valid non-empty path
            if !cwd_str.is_empty() {
                return (cleaned, Some(PathBuf::from(cwd_str)));
            }

            return (cleaned, None);
        }
    }

    (output.to_string(), None)
}

/// Generates a simple unique identifier (timestamp-based).
fn uuid_simple() -> String {
    use std::time::SystemTime;
    use std::time::UNIX_EPOCH;
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{nanos:x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[tokio::test]
    async fn test_execute_simple_command() {
        let executor = ShellExecutor::new(std::env::temp_dir());
        let result = executor.execute("echo hello", 10).await;
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout.trim(), "hello");
        assert!(result.stderr.is_empty());
        assert!(!result.truncated);
        assert!(result.duration_ms >= 0);
    }

    #[tokio::test]
    async fn test_execute_failing_command() {
        let executor = ShellExecutor::new(std::env::temp_dir());
        let result = executor.execute("exit 42", 10).await;
        assert_eq!(result.exit_code, 42);
    }

    #[tokio::test]
    async fn test_execute_with_stderr() {
        let executor = ShellExecutor::new(std::env::temp_dir());
        let result = executor.execute("echo err >&2", 10).await;
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stderr.trim(), "err");
    }

    #[tokio::test]
    async fn test_execute_timeout() {
        let executor = ShellExecutor::new(std::env::temp_dir());
        let result = executor.execute("sleep 30", 1).await;
        assert_eq!(result.exit_code, -1);
        assert!(result.stderr.contains("timed out"));
    }

    #[tokio::test]
    async fn test_execute_uses_cwd() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let executor = ShellExecutor::new(tmp.path().to_path_buf());
        let result = executor.execute("pwd", 10).await;
        assert_eq!(result.exit_code, 0);
        // The output should contain the temp dir path
        let output_path = result.stdout.trim();
        // On macOS, /tmp may resolve to /private/tmp
        assert!(
            output_path.contains(tmp.path().to_str().expect("path to str"))
                || tmp
                    .path()
                    .to_str()
                    .expect("path to str")
                    .contains(output_path),
            "Expected cwd to match temp dir: output={output_path}, temp={}",
            tmp.path().display()
        );
    }

    #[tokio::test]
    async fn test_default_timeout() {
        let executor = ShellExecutor::new(std::env::temp_dir());
        assert_eq!(executor.default_timeout_secs, DEFAULT_TIMEOUT_SECS);
    }

    #[tokio::test]
    async fn test_spawn_background() {
        let executor = ShellExecutor::new(std::env::temp_dir());
        let task_id = executor
            .spawn_background("echo background-test")
            .await
            .expect("spawn");
        assert!(task_id.starts_with("bg-"));

        // Wait a bit for the background task to complete
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    #[test]
    fn test_truncate_output_small() {
        let data = b"hello world";
        let (text, truncated) = truncate_output(data);
        assert_eq!(text, "hello world");
        assert!(!truncated);
    }

    #[test]
    fn test_truncate_output_large() {
        let data = vec![b'x'; 50_000];
        let (text, truncated) = truncate_output(&data);
        assert_eq!(text.len(), MAX_OUTPUT_BYTES as usize);
        assert!(truncated);
    }

    #[test]
    fn test_uuid_simple_uniqueness() {
        let a = uuid_simple();
        // Small sleep to ensure different timestamp
        std::thread::sleep(std::time::Duration::from_millis(1));
        let b = uuid_simple();
        assert_ne!(a, b);
    }

    #[tokio::test]
    async fn test_with_default_shell() {
        let executor = ShellExecutor::with_default_shell(std::env::temp_dir());
        assert!(executor.shell.is_some());
        let result = executor.execute("echo test", 10).await;
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout.trim(), "test");
    }

    #[test]
    fn test_is_snapshot_disabled() {
        // SAFETY: This test modifies environment variables. It should not run
        // in parallel with other tests that depend on this variable.
        unsafe {
            // Clear any existing value
            std::env::remove_var(DISABLE_SNAPSHOT_ENV);
            assert!(!is_snapshot_disabled());

            std::env::set_var(DISABLE_SNAPSHOT_ENV, "1");
            assert!(is_snapshot_disabled());

            std::env::set_var(DISABLE_SNAPSHOT_ENV, "true");
            assert!(is_snapshot_disabled());

            std::env::set_var(DISABLE_SNAPSHOT_ENV, "TRUE");
            assert!(is_snapshot_disabled());

            std::env::set_var(DISABLE_SNAPSHOT_ENV, "0");
            assert!(!is_snapshot_disabled());

            std::env::set_var(DISABLE_SNAPSHOT_ENV, "false");
            assert!(!is_snapshot_disabled());

            // Clean up
            std::env::remove_var(DISABLE_SNAPSHOT_ENV);
        }
    }

    /// Tests that maybe_wrap_shell_lc_with_snapshot passes through unchanged
    /// when no snapshot is available.
    #[tokio::test]
    async fn test_maybe_wrap_shell_lc_no_snapshot_passthrough() {
        let executor = ShellExecutor::new(std::env::temp_dir());

        let args = vec![
            "/bin/bash".to_string(),
            "-lc".to_string(),
            "echo test".to_string(),
        ];

        let result = executor.maybe_wrap_shell_lc_with_snapshot(args.clone());

        // Without snapshot, should pass through unchanged
        assert_eq!(result, args);
    }

    /// Tests that maybe_wrap_shell_lc_with_snapshot passes through non-login
    /// shell commands unchanged.
    #[tokio::test]
    async fn test_maybe_wrap_shell_lc_non_login_passthrough() {
        let executor = ShellExecutor::new(std::env::temp_dir());

        // Non-login shell command (-c instead of -lc)
        let args = vec![
            "/bin/bash".to_string(),
            "-c".to_string(),
            "echo test".to_string(),
        ];

        let result = executor.maybe_wrap_shell_lc_with_snapshot(args.clone());

        // Non-login commands should pass through unchanged
        assert_eq!(result, args);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_start_snapshotting() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let mut executor = ShellExecutor::with_default_shell(std::env::temp_dir());

        // Clear disable flag
        // SAFETY: This test modifies environment variables. It should not run
        // in parallel with other tests that depend on this variable.
        unsafe {
            std::env::remove_var(DISABLE_SNAPSHOT_ENV);
        }

        executor.start_snapshotting(tmp.path().to_path_buf(), "test-session");
        assert!(executor.is_snapshot_initialized());

        // Give the background task time to complete
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

        // Snapshot should be available now (on Unix with bash/zsh)
        // Note: This may fail in CI environments without proper shell setup
    }

    /// Tests that maybe_wrap_shell_lc_with_snapshot correctly rewrites
    /// login shell commands to source the snapshot file.
    #[cfg(unix)]
    #[tokio::test]
    async fn test_maybe_wrap_shell_lc_with_snapshot_rewrites_correctly() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let mut executor = ShellExecutor::with_default_shell(std::env::temp_dir());

        // Clear disable flag
        // SAFETY: This test modifies environment variables. It should not run
        // in parallel with other tests that depend on this variable.
        unsafe {
            std::env::remove_var(DISABLE_SNAPSHOT_ENV);
        }

        executor.start_snapshotting(tmp.path().to_path_buf(), "wrap-test");

        // Wait for snapshot to be ready (longer wait for snapshot creation)
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        // Only test if snapshot became available
        if executor.shell_snapshot().is_some() {
            // Input: login shell command
            let args = vec![
                "/bin/bash".to_string(),
                "-lc".to_string(),
                "echo test".to_string(),
            ];

            let rewritten = executor.maybe_wrap_shell_lc_with_snapshot(args);

            // Should rewrite to non-login with snapshot source
            assert_eq!(rewritten[1], "-c", "should change -lc to -c");
            assert!(rewritten[2].contains(". \""), "should source snapshot file");
            assert!(
                rewritten[2].contains("&& echo test"),
                "should chain command"
            );
        }
    }

    #[test]
    fn test_extract_cwd_from_output_with_marker() {
        let output = "hello world\n__COCODE_CWD_START__ /home/user/project __COCODE_CWD_END__\n";
        let (cleaned, cwd) = extract_cwd_from_output(output);

        assert_eq!(cleaned, "hello world");
        assert_eq!(cwd, Some(PathBuf::from("/home/user/project")));
    }

    #[test]
    fn test_extract_cwd_from_output_no_marker() {
        let output = "just normal output\n";
        let (cleaned, cwd) = extract_cwd_from_output(output);

        assert_eq!(cleaned, "just normal output\n");
        assert!(cwd.is_none());
    }

    #[test]
    fn test_extract_cwd_from_output_empty_cwd() {
        let output = "output\n__COCODE_CWD_START__  __COCODE_CWD_END__\n";
        let (cleaned, cwd) = extract_cwd_from_output(output);

        assert_eq!(cleaned, "output");
        assert!(cwd.is_none());
    }

    #[test]
    fn test_extract_cwd_from_output_preserves_other_content() {
        let output = "line1\nline2\n__COCODE_CWD_START__ /tmp __COCODE_CWD_END__";
        let (cleaned, cwd) = extract_cwd_from_output(output);

        assert_eq!(cleaned, "line1\nline2");
        assert_eq!(cwd, Some(PathBuf::from("/tmp")));
    }

    #[tokio::test]
    async fn test_cwd_captured_in_result() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let executor = ShellExecutor::new(tmp.path().to_path_buf());

        let result = executor.execute("pwd", 10).await;

        assert_eq!(result.exit_code, 0);
        // new_cwd should be captured
        assert!(result.new_cwd.is_some());
        // On macOS, /tmp may resolve to /private/tmp
        let cwd = result.new_cwd.expect("cwd should be Some");
        let cwd_str = cwd.to_string_lossy();
        let tmp_str = tmp.path().to_string_lossy();
        assert!(
            cwd_str.contains(&*tmp_str) || tmp_str.contains(&*cwd_str),
            "CWD should match temp dir: cwd={cwd_str}, temp={tmp_str}"
        );
    }

    #[tokio::test]
    async fn test_cwd_marker_not_in_output() {
        let executor = ShellExecutor::new(std::env::temp_dir());
        let result = executor.execute("echo hello", 10).await;

        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout.trim(), "hello");
        // CWD markers should not appear in output
        assert!(!result.stdout.contains("__COCODE_CWD"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_cwd_tracking_with_cd() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let subdir = tmp.path().join("subdir");
        std::fs::create_dir(&subdir).expect("create subdir");

        let mut executor = ShellExecutor::new(tmp.path().to_path_buf());

        // Initial CWD
        // On macOS, temp_dir might be symlinked
        let initial_cwd = executor.cwd().to_path_buf();

        // Execute cd command with CWD tracking
        let result = executor.execute_with_cwd_tracking("cd subdir", 10).await;

        assert_eq!(result.exit_code, 0);

        // CWD should have changed
        let new_cwd = executor.cwd();
        assert_ne!(new_cwd, initial_cwd);
        // New CWD should end with "subdir"
        assert!(
            new_cwd.ends_with("subdir"),
            "Expected CWD to end with 'subdir', got: {}",
            new_cwd.display()
        );
    }

    #[tokio::test]
    async fn test_cwd_not_updated_on_failure() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let mut executor = ShellExecutor::new(tmp.path().to_path_buf());

        let initial_cwd = executor.cwd().to_path_buf();

        // Try to cd to non-existent directory
        let result = executor
            .execute_with_cwd_tracking("cd nonexistent_dir_12345", 10)
            .await;

        assert_ne!(result.exit_code, 0);

        // CWD should remain unchanged
        assert_eq!(executor.cwd(), initial_cwd);
    }

    #[test]
    fn test_cwd_getter_setter() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let mut executor = ShellExecutor::new(tmp.path().to_path_buf());

        assert_eq!(executor.cwd(), tmp.path());

        let new_path = PathBuf::from("/new/path");
        executor.set_cwd(new_path.clone());

        assert_eq!(executor.cwd(), new_path);
    }

    // ==========================================================================
    // Subagent Shell Isolation Tests
    // ==========================================================================

    #[test]
    fn test_fork_for_subagent_uses_initial_cwd() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let initial_cwd = tmp.path().to_path_buf();

        // Main executor with different CWD
        let mut main_executor = ShellExecutor::new(PathBuf::from("/some/other/path"));
        main_executor.set_cwd(PathBuf::from("/changed/path"));

        // Fork for subagent with specific initial CWD
        let subagent_executor = main_executor.fork_for_subagent(initial_cwd.clone());

        // Subagent should use the provided initial_cwd, not main's current cwd
        assert_eq!(subagent_executor.cwd(), initial_cwd);
        assert_ne!(subagent_executor.cwd(), main_executor.cwd());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_fork_for_subagent_cwd_resets_between_calls() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let subdir = tmp.path().join("subdir");
        std::fs::create_dir(&subdir).expect("create subdir");

        let initial_cwd = tmp.path().to_path_buf();
        let main_executor = ShellExecutor::new(initial_cwd.clone());
        let subagent_executor = main_executor.fork_for_subagent(initial_cwd.clone());

        // Subagent executes cd - this should NOT affect subsequent calls
        let result1 = subagent_executor.execute("cd subdir && pwd", 10).await;
        assert_eq!(result1.exit_code, 0);
        // First call cd'd into subdir
        assert!(
            result1.stdout.contains("subdir"),
            "First call should be in subdir, got: {}",
            result1.stdout.trim()
        );

        // Second call - CWD should be back to initial (no tracking)
        let result2 = subagent_executor.execute("pwd", 10).await;
        assert_eq!(result2.exit_code, 0);
        // Should be back at initial directory
        let output = result2.stdout.trim();
        let tmp_str = tmp.path().to_str().expect("path to str");
        assert!(
            output.contains(tmp_str) || tmp_str.contains(output),
            "CWD should reset to initial for subagent, got: {output}, expected to contain: {tmp_str}"
        );
        // Should NOT be in subdir
        assert!(
            !output.ends_with("subdir"),
            "Subagent CWD should reset, not stay in subdir: {output}"
        );
    }

    #[tokio::test]
    async fn test_fork_for_subagent_independent_background_registry() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let main_executor = ShellExecutor::new(tmp.path().to_path_buf());
        let subagent_executor = main_executor.fork_for_subagent(tmp.path().to_path_buf());

        // Main agent starts a background task
        let main_task_id = main_executor
            .spawn_background("sleep 5")
            .await
            .expect("spawn");

        // Subagent should NOT see main agent's background task
        let subagent_output = subagent_executor
            .background_registry
            .get_output(&main_task_id)
            .await;
        assert!(
            subagent_output.is_none(),
            "Subagent should have independent background registry"
        );

        // Main agent should still see its own task
        let main_output = main_executor
            .background_registry
            .get_output(&main_task_id)
            .await;
        assert!(
            main_output.is_some(),
            "Main agent should see its own background task"
        );

        // Cleanup
        main_executor.background_registry.stop(&main_task_id).await;
    }

    #[test]
    fn test_fork_for_subagent_shares_shell_config() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let main_executor = ShellExecutor::with_default_shell(tmp.path().to_path_buf());
        let subagent_executor = main_executor.fork_for_subagent(tmp.path().to_path_buf());

        // Both should have shell config
        assert!(main_executor.shell().is_some());
        assert!(subagent_executor.shell().is_some());

        // Snapshot initialization state should be shared
        assert_eq!(
            main_executor.is_snapshot_initialized(),
            subagent_executor.is_snapshot_initialized()
        );
    }

    #[test]
    fn test_fork_for_subagent_inherits_timeout() {
        let mut main_executor = ShellExecutor::new(std::env::temp_dir());
        main_executor.default_timeout_secs = 300;

        let subagent_executor = main_executor.fork_for_subagent(std::env::temp_dir());

        assert_eq!(subagent_executor.default_timeout_secs, 300);
    }

    #[tokio::test]
    async fn test_execute_for_subagent_no_cwd_tracking() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let subdir = tmp.path().join("subdir");
        std::fs::create_dir(&subdir).expect("create subdir");

        let main_executor = ShellExecutor::new(tmp.path().to_path_buf());
        let subagent_executor = main_executor.fork_for_subagent(tmp.path().to_path_buf());

        // execute_for_subagent should not track CWD
        let result = subagent_executor
            .execute_for_subagent("cd subdir && pwd", 10)
            .await;
        assert_eq!(result.exit_code, 0);

        // CWD should remain unchanged (no tracking)
        let tmp_str = tmp.path().to_str().expect("path to str");
        let cwd_path = subagent_executor.cwd();
        let cwd_str = cwd_path.to_str().expect("cwd to str");
        assert!(
            cwd_str.contains(tmp_str) || tmp_str.contains(cwd_str),
            "execute_for_subagent should not track CWD changes"
        );
    }

    // ==========================================================================
    // Path Extraction Tests
    // ==========================================================================

    #[test]
    fn test_has_path_extractor_default() {
        let executor = ShellExecutor::new(std::env::temp_dir());
        assert!(!executor.has_path_extractor());
        assert!(executor.path_extractor().is_none());
    }

    #[test]
    fn test_with_path_extractor_noop() {
        use crate::path_extractor::NoOpExtractor;

        let executor =
            ShellExecutor::new(std::env::temp_dir()).with_path_extractor(Arc::new(NoOpExtractor));

        // NoOpExtractor is not enabled, so has_path_extractor returns false
        assert!(!executor.has_path_extractor());
        // But path_extractor() returns Some
        assert!(executor.path_extractor().is_some());
    }

    /// Mock extractor that returns predefined paths.
    struct MockExtractor {
        paths: Vec<PathBuf>,
    }

    impl MockExtractor {
        fn new(paths: Vec<PathBuf>) -> Self {
            Self { paths }
        }
    }

    impl crate::path_extractor::PathExtractor for MockExtractor {
        fn extract_paths<'a>(
            &'a self,
            _command: &'a str,
            _output: &'a str,
            _cwd: &'a Path,
        ) -> crate::path_extractor::BoxFuture<
            'a,
            anyhow::Result<crate::path_extractor::PathExtractionResult>,
        > {
            let paths = self.paths.clone();
            Box::pin(async move { Ok(crate::path_extractor::PathExtractionResult::new(paths, 10)) })
        }

        fn is_enabled(&self) -> bool {
            true
        }
    }

    #[tokio::test]
    async fn test_execute_with_extraction_no_extractor() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let executor = ShellExecutor::new(tmp.path().to_path_buf());

        let result = executor.execute_with_extraction("echo hello", 10).await;

        assert_eq!(result.exit_code, 0);
        // No extractor configured, so extracted_paths should be None
        assert!(result.extracted_paths.is_none());
    }

    #[tokio::test]
    async fn test_execute_with_extraction_filters_nonexistent() {
        let tmp = tempfile::tempdir().expect("create temp dir");

        // Create one file that exists
        let existing_file = tmp.path().join("exists.txt");
        std::fs::write(&existing_file, "test").expect("write file");

        // Mock extractor returns both existing and non-existing files
        let mock_extractor = MockExtractor::new(vec![
            existing_file.clone(),
            tmp.path().join("does_not_exist.txt"),
        ]);

        let executor = ShellExecutor::new(tmp.path().to_path_buf())
            .with_path_extractor(Arc::new(mock_extractor));

        let result = executor.execute_with_extraction("echo hello", 10).await;

        assert_eq!(result.exit_code, 0);
        assert!(result.extracted_paths.is_some());

        let extracted = result.extracted_paths.expect("extracted_paths");
        assert!(extracted.extraction_attempted);
        // Only the existing file should be in the result
        assert_eq!(extracted.paths.len(), 1);
        assert_eq!(extracted.paths[0], existing_file);
    }

    #[tokio::test]
    async fn test_execute_with_extraction_failed_command() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let mock_extractor = MockExtractor::new(vec![PathBuf::from("/some/file")]);

        let executor = ShellExecutor::new(tmp.path().to_path_buf())
            .with_path_extractor(Arc::new(mock_extractor));

        // Command that fails
        let result = executor.execute_with_extraction("exit 1", 10).await;

        assert_ne!(result.exit_code, 0);
        // Should not extract paths for failed commands
        assert!(result.extracted_paths.is_none());
    }

    #[tokio::test]
    async fn test_execute_with_cwd_tracking_and_extraction() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let subdir = tmp.path().join("subdir");
        std::fs::create_dir(&subdir).expect("create subdir");

        // Create a file in subdir
        let test_file = subdir.join("test.txt");
        std::fs::write(&test_file, "test").expect("write file");

        let mock_extractor = MockExtractor::new(vec![test_file.clone()]);

        let mut executor = ShellExecutor::new(tmp.path().to_path_buf())
            .with_path_extractor(Arc::new(mock_extractor));

        // Execute with both CWD tracking and extraction
        let result = executor
            .execute_with_cwd_tracking_and_extraction("cd subdir && pwd", 10)
            .await;

        assert_eq!(result.exit_code, 0);

        // CWD should be updated
        assert!(
            executor.cwd().ends_with("subdir"),
            "CWD should be updated to subdir, got: {}",
            executor.cwd().display()
        );

        // Paths should be extracted
        assert!(result.extracted_paths.is_some());
        let extracted = result.extracted_paths.expect("extracted_paths");
        assert_eq!(extracted.paths.len(), 1);
    }

    #[test]
    fn test_fork_for_subagent_shares_path_extractor() {
        use crate::path_extractor::NoOpExtractor;

        let main_executor =
            ShellExecutor::new(std::env::temp_dir()).with_path_extractor(Arc::new(NoOpExtractor));

        let subagent_executor = main_executor.fork_for_subagent(std::env::temp_dir());

        // Subagent should share the path extractor
        assert!(subagent_executor.path_extractor().is_some());
    }
}
