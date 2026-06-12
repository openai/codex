use std::ffi::OsString;
use std::future::Future;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;

use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout;

use codex_config::HooksFile;
#[cfg(target_os = "macos")]
use codex_desktop_distribution::VerifiedDesktopDistribution;
use codex_desktop_distribution::locate_current_or_installed_distribution;
use codex_protocol::protocol::HookEventName;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;

use super::CommandShell;
use super::ConfiguredHandler;

#[cfg(not(windows))]
const COMPUTER_USE_EXECUTABLE: &str = "Codex Computer Use.app/Contents/SharedSupport/SkyComputerUseClient.app/Contents/MacOS/SkyComputerUseClient";
const COMPUTER_USE_STOP_ARGUMENT: &str = "codex-stop-hook";
const COMPUTER_USE_STOP_SUFFIX: &str = " codex-stop-hook";
const COMPUTER_USE_PLUGIN_ID: &str = "computer-use@openai-bundled";
const COMPUTER_USE_PLUGIN_RELATIVE: &str = "plugins/openai-bundled/plugins/computer-use";
const INTERNAL_HOOKS_REGISTRY_PATH: &str = "plugins/app-bundled-internal-hooks.json";
const OPENAI_BUNDLED_MARKETPLACE_RELATIVE: &str = "plugins/openai-bundled";

#[cfg(target_os = "macos")]
const INTERNAL_AMBIENT_ENV_ALLOWLIST: &[&str] = &[
    "HOME", "LANG", "LC_ALL", "LC_CTYPE", "LOGNAME", "TMPDIR", "USER",
];
#[cfg(not(any(target_os = "macos", windows)))]
const INTERNAL_AMBIENT_ENV_ALLOWLIST: &[&str] = &[];
#[cfg(windows)]
const INTERNAL_AMBIENT_ENV_ALLOWLIST: &[&str] =
    &["APPDATA", "LOCALAPPDATA", "TEMP", "TMP", "USERPROFILE"];

#[derive(Debug)]
pub(crate) struct CommandRunResult {
    pub started_at: i64,
    pub completed_at: i64,
    pub duration_ms: i64,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub error: Option<String>,
}

pub(crate) async fn run_command(
    shell: &CommandShell,
    handler: &ConfiguredHandler,
    input_json: &str,
    cwd: &Path,
) -> CommandRunResult {
    let started_at = chrono::Utc::now().timestamp();
    let started = Instant::now();
    let run = run_command_inner(shell, handler, input_json, cwd, started_at, started);
    run_with_internal_timeout(
        handler.app_bundled_internal_plugin_root.is_some(),
        Duration::from_secs(handler.timeout_sec),
        handler.timeout_sec,
        started_at,
        started,
        run,
    )
    .await
}

async fn run_with_internal_timeout<F>(
    app_bundled_internal: bool,
    timeout_duration: Duration,
    timeout_sec: u64,
    started_at: i64,
    started: Instant,
    run: F,
) -> CommandRunResult
where
    F: Future<Output = CommandRunResult>,
{
    if !app_bundled_internal {
        return run.await;
    }
    match timeout(timeout_duration, run).await {
        Ok(result) => result,
        Err(_) => failed_run(
            started_at,
            started,
            format!("hook timed out after {timeout_sec}s"),
        ),
    }
}

async fn run_command_inner(
    shell: &CommandShell,
    handler: &ConfiguredHandler,
    input_json: &str,
    cwd: &Path,
    started_at: i64,
    started: Instant,
) -> CommandRunResult {
    let verified_internal = if handler.app_bundled_internal_plugin_root.is_some() {
        let handler = handler.clone();
        match tokio::task::spawn_blocking(move || verify_app_bundled_internal_handler(&handler))
            .await
        {
            Ok(Ok(invocation)) => Some(invocation),
            Ok(Err(error)) => return failed_run(started_at, started, error),
            Err(error) => {
                return failed_run(
                    started_at,
                    started,
                    format!("app-bundled internal hook verifier failed to join: {error}"),
                );
            }
        }
    } else {
        None
    };

    #[cfg(target_os = "macos")]
    if let Some(invocation) = verified_internal.as_ref() {
        return match super::app_bundled_internal_macos::run_authenticated(
            super::app_bundled_internal_macos::AuthenticatedInvocation {
                distribution: invocation.distribution.clone(),
                executable: invocation.executable.clone(),
                plugin_root: invocation.plugin_root.clone(),
                source_path: invocation.source_path.clone(),
                executable_relative: invocation.executable_relative.clone(),
                args: invocation.args.clone(),
                cwd: invocation.working_directory.as_path().to_path_buf(),
            },
            input_json.to_string(),
        )
        .await
        {
            Ok(output) => CommandRunResult {
                started_at,
                completed_at: chrono::Utc::now().timestamp(),
                duration_ms: started.elapsed().as_millis().try_into().unwrap_or(i64::MAX),
                exit_code: output.exit_code,
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                error: None,
            },
            Err(error) => failed_run(started_at, started, error),
        };
    }

    let mut command = match build_command(shell, handler, verified_internal.as_ref()) {
        Ok(command) => command,
        Err(error) => return failed_run(started_at, started, error),
    };
    if verified_internal.is_none() {
        command.current_dir(cwd);
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(err) => {
            return failed_run(started_at, started, err.to_string());
        }
    };

    if let Some(mut stdin) = child.stdin.take()
        && let Err(err) = stdin.write_all(input_json.as_bytes()).await
    {
        let _ = child.kill().await;
        return CommandRunResult {
            started_at,
            completed_at: chrono::Utc::now().timestamp(),
            duration_ms: started.elapsed().as_millis().try_into().unwrap_or(i64::MAX),
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            error: Some(format!("failed to write hook stdin: {err}")),
        };
    }

    let timeout_duration = Duration::from_secs(handler.timeout_sec);
    match timeout(timeout_duration, child.wait_with_output()).await {
        Ok(Ok(output)) => CommandRunResult {
            started_at,
            completed_at: chrono::Utc::now().timestamp(),
            duration_ms: started.elapsed().as_millis().try_into().unwrap_or(i64::MAX),
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            error: None,
        },
        Ok(Err(err)) => CommandRunResult {
            started_at,
            completed_at: chrono::Utc::now().timestamp(),
            duration_ms: started.elapsed().as_millis().try_into().unwrap_or(i64::MAX),
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            error: Some(err.to_string()),
        },
        Err(_) => CommandRunResult {
            started_at,
            completed_at: chrono::Utc::now().timestamp(),
            duration_ms: started.elapsed().as_millis().try_into().unwrap_or(i64::MAX),
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            error: Some(format!("hook timed out after {}s", handler.timeout_sec)),
        },
    }
}

#[derive(Debug)]
struct VerifiedInternalInvocation {
    #[cfg(target_os = "macos")]
    distribution: VerifiedDesktopDistribution,
    executable: AbsolutePathBuf,
    working_directory: AbsolutePathBuf,
    #[cfg(target_os = "macos")]
    plugin_root: AbsolutePathBuf,
    #[cfg(target_os = "macos")]
    source_path: AbsolutePathBuf,
    #[cfg(target_os = "macos")]
    executable_relative: String,
    args: Vec<String>,
}

fn verify_app_bundled_internal_handler(
    handler: &ConfiguredHandler,
) -> Result<VerifiedInternalInvocation, String> {
    let Some(plugin_root) = handler.app_bundled_internal_plugin_root.as_ref() else {
        return Err("app-bundled internal hook is missing its plugin root".to_string());
    };
    let distribution = locate_current_or_installed_distribution()
        .map_err(|error| format!("app-bundled internal hook verification failed: {error}"))?;
    let plugin_relative = plugin_root
        .as_path()
        .strip_prefix(distribution.resources_root().as_path())
        .map_err(|_| {
            "app-bundled internal plugin root moved outside the authenticated distribution"
                .to_string()
        })?;
    if plugin_relative != Path::new(COMPUTER_USE_PLUGIN_RELATIVE) {
        return Err(
            "app-bundled internal hook plugin identity changed after discovery".to_string(),
        );
    }
    let source_relative = handler
        .source_path
        .as_path()
        .strip_prefix(distribution.resources_root().as_path())
        .map_err(|_| {
            "app-bundled internal hook declaration moved outside the authenticated distribution"
                .to_string()
        })?;
    let current_plugin_root = distribution
        .contained_directory(plugin_relative)
        .map_err(|error| format!("app-bundled internal plugin containment failed: {error}"))?;
    let current_source = distribution
        .contained_file(source_relative)
        .map_err(|error| format!("app-bundled internal hook containment failed: {error}"))?;
    if &current_plugin_root != plugin_root || current_source != handler.source_path {
        return Err("app-bundled internal hook provenance changed after discovery".to_string());
    }
    let (executable_relative, args) = parse_internal_invocation(handler)?;
    let executable = distribution
        .contained_file(plugin_relative.join(&executable_relative))
        .map_err(|error| format!("app-bundled internal executable containment failed: {error}"))?;
    distribution
        .reverify()
        .map_err(|error| format!("app-bundled internal hook reverification failed: {error}"))?;
    verify_current_internal_opt_in(
        &distribution,
        &current_plugin_root,
        &current_source,
        &executable_relative,
    )?;
    let expected_hooks = handler
        .app_bundled_internal_source_hooks
        .as_ref()
        .ok_or_else(|| {
            "app-bundled internal hook is missing authenticated source state".to_string()
        })?;
    let current_hooks = std::fs::read_to_string(current_source.as_path())
        .map_err(|error| format!("failed to reread app-bundled internal hook declaration: {error}"))
        .and_then(|contents| {
            serde_json::from_str::<HooksFile>(&contents).map_err(|error| {
                format!("failed to reparse app-bundled internal hook declaration: {error}")
            })
        })?;
    if &current_hooks.hooks != expected_hooks {
        return Err("app-bundled internal hook declaration changed after discovery".to_string());
    }
    distribution
        .reverify()
        .map_err(|error| format!("app-bundled internal hook changed before execution: {error}"))?;
    Ok(VerifiedInternalInvocation {
        #[cfg(target_os = "macos")]
        distribution,
        executable,
        #[cfg(target_os = "macos")]
        working_directory: current_plugin_root.clone(),
        #[cfg(not(target_os = "macos"))]
        working_directory: current_plugin_root,
        #[cfg(target_os = "macos")]
        plugin_root: current_plugin_root,
        #[cfg(target_os = "macos")]
        source_path: current_source,
        #[cfg(target_os = "macos")]
        executable_relative,
        args,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InternalHooksRegistry {
    schema_version: u32,
    plugins: Vec<InternalHooksPlugin>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InternalHooksPlugin {
    plugin_id: String,
    hook_declarations: Vec<String>,
    referenced_files: Vec<String>,
}

#[derive(Deserialize)]
struct BundledMarketplace {
    name: String,
    plugins: Vec<BundledMarketplacePlugin>,
}

#[derive(Deserialize)]
struct BundledMarketplacePlugin {
    name: String,
    source: BundledMarketplacePluginSource,
}

#[derive(Deserialize)]
struct BundledMarketplacePluginSource {
    source: String,
    path: String,
}

#[derive(Deserialize)]
struct BundledPluginManifest {
    name: String,
}

pub(super) fn verify_current_internal_opt_in(
    distribution: &codex_desktop_distribution::VerifiedDesktopDistribution,
    plugin_root: &AbsolutePathBuf,
    source_path: &AbsolutePathBuf,
    executable_relative: &str,
) -> Result<(), String> {
    let registry_path = distribution
        .contained_file(INTERNAL_HOOKS_REGISTRY_PATH)
        .map_err(|error| format!("app-bundled internal registry containment failed: {error}"))?;
    let registry: InternalHooksRegistry = read_bundled_json(&registry_path, "registry")?;
    if registry.schema_version != 1 {
        return Err("app-bundled internal registry schema changed before execution".to_string());
    }
    let entries = registry
        .plugins
        .iter()
        .filter(|entry| entry.plugin_id == COMPUTER_USE_PLUGIN_ID)
        .collect::<Vec<_>>();
    let [entry] = entries.as_slice() else {
        return Err(
            "app-bundled internal registry no longer has one explicit Computer Use opt-in"
                .to_string(),
        );
    };
    let declaration_relative = source_path
        .as_path()
        .strip_prefix(plugin_root.as_path())
        .map_err(|_| "app-bundled internal hook declaration escaped its plugin".to_string())?;
    let declaration_relative = slash_relative_path(declaration_relative)?;
    if !has_unique_entry(&entry.hook_declarations, &declaration_relative)
        || !has_unique_entry(&entry.referenced_files, executable_relative)
    {
        return Err(
            "app-bundled internal registry no longer authenticates the hook declaration and executable"
                .to_string(),
        );
    }

    let marketplace_path = distribution
        .contained_file(
            Path::new(OPENAI_BUNDLED_MARKETPLACE_RELATIVE).join(".agents/plugins/marketplace.json"),
        )
        .map_err(|error| format!("app-bundled internal marketplace containment failed: {error}"))?;
    let marketplace: BundledMarketplace = read_bundled_json(&marketplace_path, "marketplace")?;
    if marketplace.name != "openai-bundled" {
        return Err("app-bundled internal marketplace identity changed".to_string());
    }
    let matching_plugins = marketplace
        .plugins
        .iter()
        .filter(|plugin| plugin.name == "computer-use")
        .collect::<Vec<_>>();
    let [plugin] = matching_plugins.as_slice() else {
        return Err(
            "app-bundled internal marketplace no longer has one Computer Use plugin".to_string(),
        );
    };
    if plugin.source.source != "local" || plugin.source.path != "./plugins/computer-use" {
        return Err("app-bundled internal marketplace source identity changed".to_string());
    }

    let manifest_path = distribution
        .contained_file(Path::new(COMPUTER_USE_PLUGIN_RELATIVE).join(".codex-plugin/plugin.json"))
        .map_err(|error| format!("app-bundled internal manifest containment failed: {error}"))?;
    let manifest: BundledPluginManifest = read_bundled_json(&manifest_path, "manifest")?;
    if manifest.name != "computer-use" {
        return Err("app-bundled internal plugin manifest identity changed".to_string());
    }
    Ok(())
}

fn read_bundled_json<T: for<'de> Deserialize<'de>>(
    path: &AbsolutePathBuf,
    label: &str,
) -> Result<T, String> {
    let contents = std::fs::read_to_string(path.as_path())
        .map_err(|error| format!("failed to reread app-bundled internal {label}: {error}"))?;
    serde_json::from_str(&contents)
        .map_err(|error| format!("failed to reparse app-bundled internal {label}: {error}"))
}

fn slash_relative_path(path: &Path) -> Result<String, String> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        let std::path::Component::Normal(component) = component else {
            return Err("app-bundled internal registry path is not relative".to_string());
        };
        normalized.push(component);
    }
    normalized
        .to_str()
        .map(|path| path.replace('\\', "/"))
        .ok_or_else(|| "app-bundled internal registry path is not UTF-8".to_string())
}

fn has_unique_entry(entries: &[String], expected: &str) -> bool {
    entries
        .iter()
        .filter(|entry| entry.as_str() == expected)
        .count()
        == 1
        && entries
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
            == entries.len()
}

fn parse_internal_invocation(handler: &ConfiguredHandler) -> Result<(String, Vec<String>), String> {
    if handler.event_name != HookEventName::Stop
        || handler.matcher.is_some()
        || handler.timeout_sec != 10
    {
        return Err(
            "app-bundled internal Computer Use hook must use the exact Stop/10s contract"
                .to_string(),
        );
    }
    #[cfg(windows)]
    let prefix = "\"%PLUGIN_ROOT%\\";
    #[cfg(not(windows))]
    let prefix = "\"${PLUGIN_ROOT}/";
    let remainder = handler.command.strip_prefix(prefix).ok_or_else(|| {
        "app-bundled internal hook command lost its authenticated PLUGIN_ROOT prefix".to_string()
    })?;
    let closing_quote = remainder.find('"').ok_or_else(|| {
        "app-bundled internal hook command lost its executable boundary".to_string()
    })?;
    let executable = remainder[..closing_quote].replace('\\', "/");
    let suffix = &remainder[closing_quote + 1..];
    if suffix != COMPUTER_USE_STOP_SUFFIX {
        return Err(
            "app-bundled internal Computer Use hook must invoke only codex-stop-hook".to_string(),
        );
    }
    if executable
        .split('/')
        .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return Err("app-bundled internal hook executable path is invalid".to_string());
    }
    #[cfg(not(windows))]
    if executable != COMPUTER_USE_EXECUTABLE {
        return Err(
            "app-bundled internal Computer Use hook executable identity changed".to_string(),
        );
    }
    #[cfg(windows)]
    if !executable.to_ascii_lowercase().ends_with(".exe") {
        return Err(
            "app-bundled internal Windows hook must directly execute a bundled .exe".to_string(),
        );
    }
    Ok((executable, vec![COMPUTER_USE_STOP_ARGUMENT.to_string()]))
}

fn failed_run(started_at: i64, started: Instant, error: String) -> CommandRunResult {
    CommandRunResult {
        started_at,
        completed_at: chrono::Utc::now().timestamp(),
        duration_ms: started.elapsed().as_millis().try_into().unwrap_or(i64::MAX),
        exit_code: None,
        stdout: String::new(),
        stderr: String::new(),
        error: Some(error),
    }
}

fn build_command(
    shell: &CommandShell,
    handler: &ConfiguredHandler,
    verified_internal: Option<&VerifiedInternalInvocation>,
) -> Result<Command, String> {
    let mut command = if let Some(invocation) = verified_internal {
        let mut command = Command::new(invocation.executable.as_path());
        command.args(&invocation.args);
        command.current_dir(invocation.working_directory.as_path());
        configure_internal_environment(&mut command);
        command
    } else if shell.program.is_empty() {
        default_shell_command()
    } else {
        Command::new(&shell.program)
    };
    if verified_internal.is_none() && shell.program.is_empty() {
        command.arg(&handler.command);
    } else if verified_internal.is_none() {
        command.args(&shell.args);
        command.arg(&handler.command);
    }
    if verified_internal.is_none() {
        command.envs(&handler.env);
    }
    Ok(command)
}

fn configure_internal_environment(command: &mut Command) {
    command.env_clear();
    command.envs(internal_ambient_environment());
}

pub(super) fn internal_ambient_environment() -> Vec<(OsString, OsString)> {
    let environment = INTERNAL_AMBIENT_ENV_ALLOWLIST
        .iter()
        .filter_map(|key| std::env::var_os(key).map(|value| (OsString::from(key), value)))
        .collect::<Vec<_>>();
    #[cfg(windows)]
    {
        let mut environment = environment;
        if let Some(windows_directory) = trusted_windows_directory() {
            environment.push((OsString::from("SystemRoot"), windows_directory.clone()));
            environment.push((OsString::from("WINDIR"), windows_directory));
        }
        environment
    }
    #[cfg(not(windows))]
    {
        environment
    }
}

#[cfg(windows)]
fn trusted_windows_directory() -> Option<OsString> {
    use std::os::windows::ffi::OsStringExt;
    use windows::Win32::System::SystemInformation::GetWindowsDirectoryW;

    let mut buffer = vec![0_u16; 32_768];
    let length = unsafe { GetWindowsDirectoryW(Some(&mut buffer)) } as usize;
    (length > 0 && length < buffer.len()).then(|| OsString::from_wide(&buffer[..length]))
}

fn default_shell_command() -> Command {
    #[cfg(windows)]
    {
        let comspec = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string());
        let mut command = Command::new(comspec);
        command.arg("/C");
        command
    }

    #[cfg(not(windows))]
    {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let mut command = Command::new(shell);
        command.arg("-lc");
        command
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;
    use std::sync::mpsc;
    use std::time::Duration;
    use std::time::Instant;
    use tokio::sync::Notify;

    use codex_config::HookEventsToml;
    use codex_protocol::protocol::HookEventName;
    use codex_protocol::protocol::HookSource;
    use codex_utils_absolute_path::test_support::PathBufExt;
    use codex_utils_absolute_path::test_support::test_path_buf;

    #[cfg(not(windows))]
    use super::COMPUTER_USE_EXECUTABLE;
    use super::CommandShell;
    use super::ConfiguredHandler;
    use super::build_command;
    use super::failed_run;
    use super::run_with_internal_timeout;

    #[tokio::test]
    async fn app_bundled_internal_timeout_covers_pre_spawn_verification() {
        let verification_started = Arc::new(Notify::new());
        let verification_finished = Arc::new(Notify::new());
        let spawn_reached = Arc::new(AtomicBool::new(false));
        let verification_started_for_run = Arc::clone(&verification_started);
        let verification_finished_for_run = Arc::clone(&verification_finished);
        let spawn_reached_for_run = Arc::clone(&spawn_reached);
        let (release_verification, wait_for_release) = mpsc::sync_channel(1);
        let started_at = chrono::Utc::now().timestamp();
        let started = Instant::now();
        let run = async move {
            tokio::task::spawn_blocking(move || {
                verification_started_for_run.notify_one();
                wait_for_release.recv().expect("verification release");
                verification_finished_for_run.notify_one();
            })
            .await
            .expect("verification task");
            spawn_reached_for_run.store(true, Ordering::SeqCst);
            failed_run(started_at, started, "unexpected continuation".to_string())
        };

        let timeout_run = tokio::spawn(run_with_internal_timeout(
            /*app_bundled_internal*/ true,
            Duration::from_millis(10),
            /*timeout_sec*/ 1,
            started_at,
            started,
            run,
        ));
        tokio::time::timeout(Duration::from_secs(1), verification_started.notified())
            .await
            .expect("verification started");
        let result = tokio::time::timeout(Duration::from_secs(1), timeout_run)
            .await
            .expect("internal timeout fired")
            .expect("timeout task");
        release_verification.send(()).expect("release verification");
        tokio::time::timeout(Duration::from_secs(1), verification_finished.notified())
            .await
            .expect("verification finished");

        assert_eq!(result.error.as_deref(), Some("hook timed out after 1s"));
        assert!(!spawn_reached.load(Ordering::SeqCst));
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn app_bundled_internal_hooks_use_direct_argv_without_a_shell() {
        let mut handler = internal_handler();
        handler.env.insert(
            "NODE_OPTIONS".to_string(),
            "--require=/tmp/untrusted.js".to_string(),
        );
        let (executable, args) = super::parse_internal_invocation(&handler).expect("parse command");
        let invocation = super::VerifiedInternalInvocation {
            executable: test_path_buf(&format!("/app/resources/plugins/computer-use/{executable}"))
                .abs(),
            working_directory: test_path_buf("/app/resources/plugins/computer-use").abs(),
            args,
        };
        let command = build_command(
            &CommandShell {
                program: "ignored-user-shell".to_string(),
                args: vec!["--ignored".to_string()],
            },
            &handler,
            Some(&invocation),
        )
        .expect("build command");

        assert_eq!(
            command.as_std().get_program(),
            invocation.executable.as_path()
        );
        let args = command
            .as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(args, vec!["codex-stop-hook".to_string()]);
        assert_eq!(
            command.as_std().get_current_dir(),
            Some(invocation.working_directory.as_path())
        );
        assert!(
            command
                .as_std()
                .get_envs()
                .all(|(key, _)| key != "NODE_OPTIONS")
        );
    }

    #[test]
    fn app_bundled_internal_hook_rejects_shell_suffix() {
        let mut handler = internal_handler();
        handler.command.push_str(" ; payload");

        let error = super::parse_internal_invocation(&handler).expect_err("reject shell suffix");

        assert_eq!(
            error,
            "app-bundled internal Computer Use hook must invoke only codex-stop-hook"
        );
    }

    fn internal_handler() -> ConfiguredHandler {
        ConfiguredHandler {
            event_name: HookEventName::Stop,
            matcher: None,
            command: internal_command(),
            timeout_sec: 10,
            status_message: None,
            source_path: test_path_buf("/app/resources/hooks/hooks.json").abs(),
            source: HookSource::AppBundledInternal,
            app_bundled_internal_plugin_root: Some(
                test_path_buf("/app/resources/plugins/computer-use").abs(),
            ),
            app_bundled_internal_source_hooks: Some(HookEventsToml::default()),
            display_order: 0,
            env: std::collections::HashMap::new(),
        }
    }

    fn internal_command() -> String {
        #[cfg(windows)]
        {
            "\"%PLUGIN_ROOT%\\bin\\SkyComputerUseClient.exe\" codex-stop-hook".to_string()
        }
        #[cfg(not(windows))]
        {
            format!("\"${{PLUGIN_ROOT}}/{COMPUTER_USE_EXECUTABLE}\" codex-stop-hook")
        }
    }
}
