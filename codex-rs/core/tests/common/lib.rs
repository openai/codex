#![expect(clippy::expect_used)]

use tempfile::TempDir;

use codex_core::CodexConversation;
use codex_core::config::Config;
use codex_core::config::ConfigBuilder;
use codex_core::config::ConfigOverrides;
use codex_utils_absolute_path::AbsolutePathBuf;
use regex_lite::Regex;
use std::path::PathBuf;

pub mod process;
pub mod responses;
pub mod streaming_sse;
pub mod test_codex;
pub mod test_codex_exec;

#[track_caller]
pub fn assert_regex_match<'s>(pattern: &str, actual: &'s str) -> regex_lite::Captures<'s> {
    let regex = Regex::new(pattern).unwrap_or_else(|err| {
        panic!("failed to compile regex {pattern:?}: {err}");
    });
    regex
        .captures(actual)
        .unwrap_or_else(|| panic!("regex {pattern:?} did not match {actual:?}"))
}

pub fn test_path_buf_with_windows(unix_path: &str, windows_path: Option<&str>) -> PathBuf {
    if cfg!(windows) {
        if let Some(windows) = windows_path {
            PathBuf::from(windows)
        } else {
            let mut path = PathBuf::from(r"C:\");
            path.extend(
                unix_path
                    .trim_start_matches('/')
                    .split('/')
                    .filter(|segment| !segment.is_empty()),
            );
            path
        }
    } else {
        PathBuf::from(unix_path)
    }
}

pub fn test_path_buf(unix_path: &str) -> PathBuf {
    test_path_buf_with_windows(unix_path, None)
}

pub fn test_absolute_path_with_windows(
    unix_path: &str,
    windows_path: Option<&str>,
) -> AbsolutePathBuf {
    AbsolutePathBuf::from_absolute_path(test_path_buf_with_windows(unix_path, windows_path))
        .expect("test path should be absolute")
}

pub fn test_absolute_path(unix_path: &str) -> AbsolutePathBuf {
    test_absolute_path_with_windows(unix_path, None)
}

pub fn test_tmp_path() -> AbsolutePathBuf {
    test_absolute_path_with_windows("/tmp", Some(r"C:\Users\codex\AppData\Local\Temp"))
}

pub fn test_tmp_path_buf() -> PathBuf {
    test_tmp_path().into_path_buf()
}

/// Returns a default `Config` whose on-disk state is confined to the provided
/// temporary directory. Using a per-test directory keeps tests hermetic and
/// avoids clobbering a developer’s real `~/.codex`.
pub async fn load_default_config_for_test(codex_home: &TempDir) -> Config {
    ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .harness_overrides(default_test_overrides())
        .build()
        .await
        .expect("defaults for test should always succeed")
}

#[cfg(target_os = "linux")]
fn default_test_overrides() -> ConfigOverrides {
    ConfigOverrides {
        codex_linux_sandbox_exe: Some(
            codex_utils_cargo_bin::cargo_bin("codex-linux-sandbox")
                .expect("should find binary for codex-linux-sandbox"),
        ),
        ..ConfigOverrides::default()
    }
}

#[cfg(not(target_os = "linux"))]
fn default_test_overrides() -> ConfigOverrides {
    ConfigOverrides::default()
}

/// Builds an SSE stream body from a JSON fixture.
///
/// The fixture must contain an array of objects where each object represents a
/// single SSE event with at least a `type` field matching the `event:` value.
/// Additional fields become the JSON payload for the `data:` line. An object
/// with only a `type` field results in an event with no `data:` section. This
/// makes it trivial to extend the fixtures as OpenAI adds new event kinds or
/// fields.
pub fn load_sse_fixture(path: impl AsRef<std::path::Path>) -> String {
    let events: Vec<serde_json::Value> =
        serde_json::from_reader(std::fs::File::open(path).expect("read fixture"))
            .expect("parse JSON fixture");
    events
        .into_iter()
        .map(|e| {
            let kind = e
                .get("type")
                .and_then(|v| v.as_str())
                .expect("fixture event missing type");
            if e.as_object().map(|o| o.len() == 1).unwrap_or(false) {
                format!("event: {kind}\n\n")
            } else {
                format!("event: {kind}\ndata: {e}\n\n")
            }
        })
        .collect()
}

pub fn load_sse_fixture_with_id_from_str(raw: &str, id: &str) -> String {
    let replaced = raw.replace("__ID__", id);
    let events: Vec<serde_json::Value> =
        serde_json::from_str(&replaced).expect("parse JSON fixture");
    events
        .into_iter()
        .map(|e| {
            let kind = e
                .get("type")
                .and_then(|v| v.as_str())
                .expect("fixture event missing type");
            if e.as_object().map(|o| o.len() == 1).unwrap_or(false) {
                format!("event: {kind}\n\n")
            } else {
                format!("event: {kind}\ndata: {e}\n\n")
            }
        })
        .collect()
}

/// Same as [`load_sse_fixture`], but replaces the placeholder `__ID__` in the
/// fixture template with the supplied identifier before parsing. This lets a
/// single JSON template be reused by multiple tests that each need a unique
/// `response_id`.
pub fn load_sse_fixture_with_id(path: impl AsRef<std::path::Path>, id: &str) -> String {
    let raw = std::fs::read_to_string(path).expect("read fixture template");
    let replaced = raw.replace("__ID__", id);
    let events: Vec<serde_json::Value> =
        serde_json::from_str(&replaced).expect("parse JSON fixture");
    events
        .into_iter()
        .map(|e| {
            let kind = e
                .get("type")
                .and_then(|v| v.as_str())
                .expect("fixture event missing type");
            if e.as_object().map(|o| o.len() == 1).unwrap_or(false) {
                format!("event: {kind}\n\n")
            } else {
                format!("event: {kind}\ndata: {e}\n\n")
            }
        })
        .collect()
}

pub async fn wait_for_event<F>(
    codex: &CodexConversation,
    predicate: F,
) -> codex_core::protocol::EventMsg
where
    F: FnMut(&codex_core::protocol::EventMsg) -> bool,
{
    use tokio::time::Duration;
    wait_for_event_with_timeout(codex, predicate, Duration::from_secs(1)).await
}

pub async fn wait_for_event_match<T, F>(codex: &CodexConversation, matcher: F) -> T
where
    F: Fn(&codex_core::protocol::EventMsg) -> Option<T>,
{
    let ev = wait_for_event(codex, |ev| matcher(ev).is_some()).await;
    matcher(&ev).unwrap()
}

pub async fn wait_for_event_with_timeout<F>(
    codex: &CodexConversation,
    mut predicate: F,
    wait_time: tokio::time::Duration,
) -> codex_core::protocol::EventMsg
where
    F: FnMut(&codex_core::protocol::EventMsg) -> bool,
{
    use tokio::time::Duration;
    use tokio::time::timeout;
    loop {
        // Allow a bit more time to accommodate async startup work (e.g. config IO, tool discovery)
        let ev = timeout(wait_time.max(Duration::from_secs(5)), codex.next_event())
            .await
            .expect("timeout waiting for event")
            .expect("stream ended unexpectedly");
        if predicate(&ev.msg) {
            return ev.msg;
        }
    }
}

pub fn sandbox_env_var() -> &'static str {
    codex_core::spawn::CODEX_SANDBOX_ENV_VAR
}

pub fn sandbox_network_env_var() -> &'static str {
    codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR
}

pub fn format_with_current_shell(command: &str) -> Vec<String> {
    codex_core::shell::default_user_shell().derive_exec_args(command, true)
}

pub fn format_with_current_shell_display(command: &str) -> String {
    let args = format_with_current_shell(command);
    shlex::try_join(args.iter().map(String::as_str)).expect("serialize current shell command")
}

pub fn format_with_current_shell_non_login(command: &str) -> Vec<String> {
    codex_core::shell::default_user_shell().derive_exec_args(command, false)
}

pub fn format_with_current_shell_display_non_login(command: &str) -> String {
    let args = format_with_current_shell_non_login(command);
    shlex::try_join(args.iter().map(String::as_str))
        .expect("serialize current shell command without login")
}

pub mod fs_wait {
    use anyhow::Result;
    use anyhow::anyhow;
    use notify::RecursiveMode;
    use notify::Watcher;
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::mpsc;
    use std::sync::mpsc::RecvTimeoutError;
    use std::time::Duration;
    use std::time::Instant;
    use tokio::task;
    use walkdir::WalkDir;

    pub async fn wait_for_path_exists(
        path: impl Into<PathBuf>,
        timeout: Duration,
    ) -> Result<PathBuf> {
        let path = path.into();
        task::spawn_blocking(move || wait_for_path_exists_blocking(path, timeout)).await?
    }

    pub async fn wait_for_matching_file(
        root: impl Into<PathBuf>,
        timeout: Duration,
        predicate: impl FnMut(&Path) -> bool + Send + 'static,
    ) -> Result<PathBuf> {
        let root = root.into();
        task::spawn_blocking(move || {
            let mut predicate = predicate;
            blocking_find_matching_file(root, timeout, &mut predicate)
        })
        .await?
    }

    fn wait_for_path_exists_blocking(path: PathBuf, timeout: Duration) -> Result<PathBuf> {
        if path.exists() {
            return Ok(path);
        }

        let watch_root = nearest_existing_ancestor(&path);
        let (tx, rx) = mpsc::channel();
        let mut watcher = notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        })?;
        watcher.watch(&watch_root, RecursiveMode::Recursive)?;

        let deadline = Instant::now() + timeout;
        loop {
            if path.exists() {
                return Ok(path.clone());
            }
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            let remaining = deadline.saturating_duration_since(now);
            match rx.recv_timeout(remaining) {
                Ok(Ok(_event)) => {
                    if path.exists() {
                        return Ok(path.clone());
                    }
                }
                Ok(Err(err)) => return Err(err.into()),
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }

        if path.exists() {
            Ok(path)
        } else {
            Err(anyhow!("timed out waiting for {path:?}"))
        }
    }

    fn blocking_find_matching_file(
        root: PathBuf,
        timeout: Duration,
        predicate: &mut impl FnMut(&Path) -> bool,
    ) -> Result<PathBuf> {
        let root = wait_for_path_exists_blocking(root, timeout)?;

        if let Some(found) = scan_for_match(&root, predicate) {
            return Ok(found);
        }

        let (tx, rx) = mpsc::channel();
        let mut watcher = notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        })?;
        watcher.watch(&root, RecursiveMode::Recursive)?;

        let deadline = Instant::now() + timeout;

        while Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match rx.recv_timeout(remaining) {
                Ok(Ok(_event)) => {
                    if let Some(found) = scan_for_match(&root, predicate) {
                        return Ok(found);
                    }
                }
                Ok(Err(err)) => return Err(err.into()),
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }

        if let Some(found) = scan_for_match(&root, predicate) {
            Ok(found)
        } else {
            Err(anyhow!("timed out waiting for matching file in {root:?}"))
        }
    }

    fn scan_for_match(root: &Path, predicate: &mut impl FnMut(&Path) -> bool) -> Option<PathBuf> {
        for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
            let path = entry.path();
            if !entry.file_type().is_file() {
                continue;
            }
            if predicate(path) {
                return Some(path.to_path_buf());
            }
        }
        None
    }

    fn nearest_existing_ancestor(path: &Path) -> PathBuf {
        let mut current = path;
        loop {
            if current.exists() {
                return current.to_path_buf();
            }
            match current.parent() {
                Some(parent) => current = parent,
                None => return PathBuf::from("."),
            }
        }
    }
}

#[macro_export]
macro_rules! skip_if_sandbox {
    () => {{
        if ::std::env::var($crate::sandbox_env_var())
            == ::core::result::Result::Ok("seatbelt".to_string())
        {
            eprintln!(
                "{} is set to 'seatbelt', skipping test.",
                $crate::sandbox_env_var()
            );
            return;
        }
    }};
    ($return_value:expr $(,)?) => {{
        if ::std::env::var($crate::sandbox_env_var())
            == ::core::result::Result::Ok("seatbelt".to_string())
        {
            eprintln!(
                "{} is set to 'seatbelt', skipping test.",
                $crate::sandbox_env_var()
            );
            return $return_value;
        }
    }};
}

#[macro_export]
macro_rules! skip_if_no_network {
    () => {{
        if ::std::env::var($crate::sandbox_network_env_var()).is_ok() {
            println!(
                "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
            );
            return;
        }
    }};
    ($return_value:expr $(,)?) => {{
        if ::std::env::var($crate::sandbox_network_env_var()).is_ok() {
            println!(
                "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
            );
            return $return_value;
        }
    }};
}

#[macro_export]
macro_rules! skip_if_windows {
    ($return_value:expr $(,)?) => {{
        if cfg!(target_os = "windows") {
            println!("Skipping test because it cannot execute on Windows.");
            return $return_value;
        }
    }};
}
