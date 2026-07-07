#![allow(clippy::expect_used)]

use anyhow::Context as _;
use anyhow::ensure;
use codex_arg0::Arg0PathEntryGuard;
use codex_utils_cargo_bin::CargoBinError;
use ctor::ctor;
use std::sync::OnceLock;
use tempfile::TempDir;

use codex_config::CloudConfigBundleLoader;
use codex_config::LoaderOverrides;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_core::CodexThread;
use codex_core::config::Config;
use codex_core::config::ConfigBuilder;
use codex_core::config::ConfigOverrides;
pub use codex_core::test_support::TestCodexResponsesRequestKind;
pub use codex_core::test_support::responses_metadata;
use codex_utils_absolute_path::AbsolutePathBuf;
pub use codex_utils_absolute_path::test_support::PathBufExt;
pub use codex_utils_absolute_path::test_support::PathExt;
use regex_lite::Regex;
use std::path::Path;
use std::path::PathBuf;

pub mod apps_test_server;
pub mod context_snapshot;
pub mod hooks;
pub mod process;
pub mod responses;
pub mod streaming_sse;
pub mod test_codex;
pub mod test_codex_exec;
mod test_environment;
pub mod tracing;
pub mod zsh_fork;

pub(crate) use test_environment::TestEnvironment;
pub use test_environment::TestTargetOs;
pub use test_environment::is_remote_test_environment;
#[doc(hidden)]
pub use test_environment::is_wine_exec_test_environment;
#[doc(hidden)]
pub use test_environment::test_docker_container_name;
pub(crate) use test_environment::test_environment;
pub use test_environment::test_target_os;

static TEST_ARG0_PATH_ENTRY: OnceLock<Option<Arg0PathEntryGuard>> = OnceLock::new();

#[ctor]
fn enable_deterministic_unified_exec_process_ids_for_tests() {
    codex_core::test_support::set_thread_manager_test_mode(/*enabled*/ true);
    codex_core::test_support::set_deterministic_process_ids(/*enabled*/ true);
}

#[ctor]
fn configure_arg0_dispatch_for_test_binaries() {
    let _ = TEST_ARG0_PATH_ENTRY.get_or_init(codex_arg0::arg0_dispatch);
}

#[ctor]
fn configure_insta_workspace_root_for_snapshot_tests() {
    if std::env::var_os("INSTA_WORKSPACE_ROOT").is_some() {
        return;
    }

    let workspace_root = codex_utils_cargo_bin::repo_root()
        .ok()
        .map(|root| root.join("codex-rs"));

    if let Some(workspace_root) = workspace_root
        && let Ok(workspace_root) = workspace_root.canonicalize()
    {
        // Safety: this ctor runs at process startup before test threads begin.
        unsafe {
            std::env::set_var("INSTA_WORKSPACE_ROOT", workspace_root);
        }
    }
}

#[track_caller]
pub fn assert_regex_match<'s>(pattern: &str, actual: &'s str) -> regex_lite::Captures<'s> {
    let regex = Regex::new(pattern).expect("failed to compile regex");
    regex
        .captures(actual)
        .expect("regex did not match actual value")
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
    test_path_buf_with_windows(unix_path, /*windows_path*/ None)
}

pub fn test_absolute_path_with_windows(
    unix_path: &str,
    windows_path: Option<&str>,
) -> AbsolutePathBuf {
    AbsolutePathBuf::from_absolute_path(test_path_buf_with_windows(unix_path, windows_path))
        .expect("test path should be absolute")
}

pub fn test_absolute_path(unix_path: &str) -> AbsolutePathBuf {
    test_absolute_path_with_windows(unix_path, /*windows_path*/ None)
}

#[cfg(unix)]
#[allow(clippy::expect_used)]
pub fn create_directory_symlink(source: &Path, link: &Path) {
    std::os::unix::fs::symlink(source, link).expect("create directory symlink");
}

#[cfg(windows)]
#[allow(clippy::expect_used)]
pub fn create_directory_symlink(source: &Path, link: &Path) {
    // Running this test locally may require Windows Developer Mode or an elevated process.
    std::os::windows::fs::symlink_dir(source, link)
        .expect("create directory symlink; enable Developer Mode or run the test elevated");
}

pub trait TempDirExt {
    fn abs(&self) -> AbsolutePathBuf;
}

impl TempDirExt for TempDir {
    fn abs(&self) -> AbsolutePathBuf {
        self.path().abs()
    }
}

pub fn test_tmp_path() -> AbsolutePathBuf {
    test_absolute_path_with_windows("/tmp", Some(r"C:\Users\codex\AppData\Local\Temp"))
}

pub fn test_tmp_path_buf() -> PathBuf {
    test_tmp_path().into_path_buf()
}

/// Fetch a DotSlash resource and return the resolved executable/file path.
pub fn fetch_dotslash_file(
    dotslash_file: &std::path::Path,
    dotslash_cache: Option<&std::path::Path>,
) -> anyhow::Result<PathBuf> {
    let mut command = std::process::Command::new("dotslash");
    command.arg("--").arg("fetch").arg(dotslash_file);
    if let Some(dotslash_cache) = dotslash_cache {
        command.env("DOTSLASH_CACHE", dotslash_cache);
    }
    let output = command.output().with_context(|| {
        format!(
            "failed to run dotslash to fetch resource {}",
            dotslash_file.display()
        )
    })?;
    ensure!(
        output.status.success(),
        "dotslash fetch failed for {}: {}",
        dotslash_file.display(),
        String::from_utf8_lossy(&output.stderr).trim()
    );
    let fetched_path = String::from_utf8(output.stdout)
        .context("dotslash fetch output was not utf8")?
        .trim()
        .to_string();
    ensure!(!fetched_path.is_empty(), "dotslash fetch output was empty");
    let fetched_path = PathBuf::from(fetched_path);
    ensure!(
        fetched_path.is_file(),
        "dotslash returned non-file path: {}",
        fetched_path.display()
    );
    Ok(fetched_path)
}

/// Returns a default `Config` whose on-disk state is confined to the provided
/// temporary directory. Using a per-test directory keeps tests hermetic and
/// avoids clobbering a developer’s real `~/.codex`.
pub async fn load_default_config_for_test(codex_home: &TempDir) -> Config {
    load_default_config_for_test_with_cloud_config_bundle(
        codex_home,
        CloudConfigBundleLoader::default(),
    )
    .await
}

/// Returns a default `Config` with test-provided cloud bundle requirements applied.
/// during config construction.
pub async fn load_default_config_for_test_with_cloud_config_bundle(
    codex_home: &TempDir,
    cloud_config_bundle: CloudConfigBundleLoader,
) -> Config {
    ConfigBuilder::default()
        .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
        .codex_home(codex_home.path().to_path_buf())
        .harness_overrides(default_test_overrides())
        .cloud_config_bundle(cloud_config_bundle)
        .build()
        .await
        .expect("defaults for test should always succeed")
}

pub fn managed_network_requirements_loader() -> CloudConfigBundleLoader {
    CloudConfigBundleFixture::loader_with_enterprise_requirement(
        r#"
[experimental_network]
enabled = true
allow_local_binding = true

[windows]
allowed_sandbox_implementations = ["elevated"]
"#,
    )
}

#[cfg(target_os = "linux")]
fn default_test_overrides() -> ConfigOverrides {
    ConfigOverrides {
        codex_linux_sandbox_exe: Some(
            find_codex_linux_sandbox_exe().expect("should find binary for codex-linux-sandbox"),
        ),
        ..ConfigOverrides::default()
    }
}

#[cfg(not(target_os = "linux"))]
fn default_test_overrides() -> ConfigOverrides {
    ConfigOverrides::default()
}

#[cfg(target_os = "linux")]
pub fn find_codex_linux_sandbox_exe() -> Result<PathBuf, CargoBinError> {
    if let Some(path) = TEST_ARG0_PATH_ENTRY
        .get()
        .and_then(Option::as_ref)
        .and_then(|path_entry| path_entry.paths().codex_linux_sandbox_exe.clone())
    {
        return Ok(path);
    }

    if let Ok(path) = std::env::current_exe() {
        return Ok(path);
    }

    codex_utils_cargo_bin::cargo_bin("codex-linux-sandbox")
}

pub async fn wait_for_event<F>(
    codex: &CodexThread,
    predicate: F,
) -> codex_protocol::protocol::EventMsg
where
    F: FnMut(&codex_protocol::protocol::EventMsg) -> bool,
{
    use tokio::time::Duration;
    wait_for_event_with_timeout(codex, predicate, Duration::from_secs(1)).await
}

/// Waits for a configured MCP server to finish startup and requires it to be ready.
pub async fn wait_for_mcp_server(codex: &CodexThread, server_name: &str) -> anyhow::Result<()> {
    use codex_protocol::protocol::EventMsg;

    // Wait for the startup summary regardless of outcome, then interpret the
    // requested server's ready, failed, or cancelled entry below.
    let summary = loop {
        let event = codex
            .next_event()
            .await
            .expect("stream ended unexpectedly while waiting for MCP startup");
        if let EventMsg::McpStartupComplete(summary) = event.msg {
            break summary;
        }
    };
    if let Some(failure) = summary
        .failed
        .iter()
        .find(|failure| failure.server == server_name)
    {
        let error = &failure.error;
        anyhow::bail!("MCP server {server_name} failed to start: {error}");
    }
    if summary.cancelled.iter().any(|server| server == server_name) {
        anyhow::bail!("MCP server {server_name} startup was cancelled");
    }
    assert!(
        summary.ready.iter().any(|server| server == server_name),
        "expected MCP server {server_name} to be ready; startup summary: {summary:?}"
    );
    Ok(())
}

pub async fn submit_thread_settings(
    codex: &CodexThread,
    thread_settings: codex_protocol::protocol::ThreadSettingsOverrides,
) -> anyhow::Result<()> {
    use codex_protocol::protocol::EventMsg;
    use codex_protocol::protocol::Op;
    use tokio::time::Duration;
    use tokio::time::timeout;

    let submission_id = codex.submit(Op::ThreadSettings { thread_settings }).await?;
    loop {
        let ev = timeout(Duration::from_secs(10), codex.next_event())
            .await
            .expect("timeout waiting for thread settings update")
            .expect("stream ended unexpectedly");
        if ev.id == submission_id {
            match ev.msg {
                EventMsg::ThreadSettingsApplied(_) => return Ok(()),
                EventMsg::Error(err) => panic!("thread settings update failed: {}", err.message),
                other => panic!("unexpected thread settings update event: {other:?}"),
            }
        }
    }
}

pub async fn wait_for_event_match<T, F>(codex: &CodexThread, matcher: F) -> T
where
    F: Fn(&codex_protocol::protocol::EventMsg) -> Option<T>,
{
    let ev = wait_for_event(codex, |ev| matcher(ev).is_some()).await;
    matcher(&ev).expect("EventMsg should match matcher predicate")
}

pub async fn wait_for_event_with_timeout<F>(
    codex: &CodexThread,
    mut predicate: F,
    wait_time: tokio::time::Duration,
) -> codex_protocol::protocol::EventMsg
where
    F: FnMut(&codex_protocol::protocol::EventMsg) -> bool,
{
    use tokio::time::Duration;
    use tokio::time::timeout;
    loop {
        // Allow a bit more time to accommodate async startup work (e.g. config IO, tool discovery)
        let ev = timeout(wait_time.max(Duration::from_secs(10)), codex.next_event())
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
    codex_core::shell::default_user_shell().derive_exec_args(command, /*use_login_shell*/ true)
}

pub fn format_with_current_shell_display(command: &str) -> String {
    let args = format_with_current_shell(command);
    shlex::try_join(args.iter().map(String::as_str)).expect("serialize current shell command")
}

pub fn format_with_current_shell_non_login(command: &str) -> Vec<String> {
    codex_core::shell::default_user_shell()
        .derive_exec_args(command, /*use_login_shell*/ false)
}

pub fn format_with_current_shell_display_non_login(command: &str) -> String {
    let args = format_with_current_shell_non_login(command);
    shlex::try_join(args.iter().map(String::as_str))
        .expect("serialize current shell command without login")
}

pub fn stdio_server_bin() -> Result<String, CargoBinError> {
    codex_utils_cargo_bin::cargo_bin("test_stdio_server").map(|p| p.to_string_lossy().to_string())
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
                return Ok(path);
            }
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            let remaining = deadline.saturating_duration_since(now);
            match rx.recv_timeout(remaining) {
                Ok(Ok(_event)) => {
                    if path.exists() {
                        return Ok(path);
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

// Exported so the public skip macros can expand in downstream test crates.
#[macro_export]
#[doc(hidden)]
macro_rules! skip_if_test_condition {
    ($condition:expr, $environment:expr, $reason:expr $(,)?) => {{
        if $condition {
            eprintln!("Skipping test in {}: {}", $environment, $reason);
            return;
        }
    }};
    ($return_value:expr, $condition:expr, $environment:expr, $reason:expr $(,)?) => {{
        if $condition {
            eprintln!("Skipping test in {}: {}", $environment, $reason);
            return $return_value;
        }
    }};
}

#[macro_export]
macro_rules! skip_if_remote {
    ($reason:expr $(,)?) => {{
        $crate::skip_if_test_condition!(
            $crate::is_remote_test_environment(),
            "a remote test environment",
            $reason,
        );
    }};
    ($return_value:expr, $reason:expr $(,)?) => {{
        $crate::skip_if_test_condition!(
            $return_value,
            $crate::is_remote_test_environment(),
            "a remote test environment",
            $reason,
        );
    }};
}

#[macro_export]
macro_rules! skip_if_no_remote_env {
    () => {{
        if !$crate::is_remote_test_environment() {
            eprintln!("Skipping test because it requires a remote test environment.");
            return;
        }
    }};
    ($return_value:expr $(,)?) => {{
        if !$crate::is_remote_test_environment() {
            eprintln!("Skipping test because it requires a remote test environment.");
            return $return_value;
        }
    }};
}

#[macro_export]
macro_rules! skip_if_wine_exec {
    ($reason:expr $(,)?) => {{
        $crate::skip_if_test_condition!(
            $crate::is_wine_exec_test_environment(),
            "the Wine-exec test environment",
            $reason,
        );
    }};
    ($return_value:expr, $reason:expr $(,)?) => {{
        $crate::skip_if_test_condition!(
            $return_value,
            $crate::is_wine_exec_test_environment(),
            "the Wine-exec test environment",
            $reason,
        );
    }};
}

#[macro_export]
macro_rules! skip_if_target_windows {
    ($reason:expr $(,)?) => {{
        $crate::skip_if_test_condition!(
            $crate::test_target_os() == $crate::TestTargetOs::Windows,
            "a Windows target environment",
            $reason,
        );
    }};
    ($return_value:expr, $reason:expr $(,)?) => {{
        $crate::skip_if_test_condition!(
            $return_value,
            $crate::test_target_os() == $crate::TestTargetOs::Windows,
            "a Windows target environment",
            $reason,
        );
    }};
}

#[macro_export]
macro_rules! codex_linux_sandbox_exe_or_skip {
    () => {{
        #[cfg(target_os = "linux")]
        {
            match $crate::find_codex_linux_sandbox_exe() {
                Ok(path) => Some(path),
                Err(err) => {
                    eprintln!("codex-linux-sandbox binary not available, skipping test: {err}");
                    return;
                }
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            None
        }
    }};
    ($return_value:expr $(,)?) => {{
        #[cfg(target_os = "linux")]
        {
            match $crate::find_codex_linux_sandbox_exe() {
                Ok(path) => Some(path),
                Err(err) => {
                    eprintln!("codex-linux-sandbox binary not available, skipping test: {err}");
                    return $return_value;
                }
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            None
        }
    }};
}

#[macro_export]
macro_rules! skip_if_host_windows {
    ($return_value:expr $(,)?) => {{
        if cfg!(target_os = "windows") {
            println!("Skipping test because it cannot execute on Windows.");
            return $return_value;
        }
    }};
}
