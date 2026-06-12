use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;

use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout;

use codex_config::HooksFile;
use codex_desktop_distribution::locate_current_or_installed_distribution;

use super::CommandShell;
use super::ConfiguredHandler;

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

    if handler.app_bundled_internal_plugin_root.is_some() {
        let handler = handler.clone();
        match tokio::task::spawn_blocking(move || verify_app_bundled_internal_handler(&handler))
            .await
        {
            Ok(Ok(())) => {}
            Ok(Err(error)) => return failed_run(started_at, started, error),
            Err(error) => {
                return failed_run(
                    started_at,
                    started,
                    format!("app-bundled internal hook verifier failed to join: {error}"),
                );
            }
        }
    }

    let mut command = match build_command(shell, handler) {
        Ok(command) => command,
        Err(error) => return failed_run(started_at, started, error),
    };
    command
        .current_dir(cwd)
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

fn verify_app_bundled_internal_handler(handler: &ConfiguredHandler) -> Result<(), String> {
    let Some(plugin_root) = handler.app_bundled_internal_plugin_root.as_ref() else {
        return Ok(());
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
    distribution
        .reverify()
        .map_err(|error| format!("app-bundled internal hook reverification failed: {error}"))?;
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
        .map_err(|error| format!("app-bundled internal hook changed before execution: {error}"))
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

fn build_command(shell: &CommandShell, handler: &ConfiguredHandler) -> Result<Command, String> {
    let app_bundled_internal = handler.app_bundled_internal_plugin_root.is_some();
    let mut command = if app_bundled_internal {
        trusted_system_shell_command()?
    } else if shell.program.is_empty() {
        default_shell_command()
    } else {
        Command::new(&shell.program)
    };
    if app_bundled_internal || shell.program.is_empty() {
        command.arg(&handler.command);
    } else {
        command.args(&shell.args);
        command.arg(&handler.command);
    }
    command.envs(&handler.env);
    Ok(command)
}

fn trusted_system_shell_command() -> Result<Command, String> {
    #[cfg(windows)]
    {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;

        use windows::Win32::System::SystemInformation::GetSystemDirectoryW;

        let mut system_directory = vec![0_u16; 32_768];
        let length = unsafe { GetSystemDirectoryW(Some(&mut system_directory)) } as usize;
        if length == 0 || length >= system_directory.len() {
            return Err("failed to resolve the authenticated Windows system shell".to_string());
        }
        let mut command_path =
            std::path::PathBuf::from(OsString::from_wide(&system_directory[..length]));
        command_path.push("cmd.exe");
        let mut command = Command::new(command_path);
        command.args(["/D", "/S", "/C"]);
        Ok(command)
    }

    #[cfg(not(windows))]
    {
        let mut command = Command::new("/bin/sh");
        command.arg("-c");
        Ok(command)
    }
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
    use codex_config::HookEventsToml;
    use codex_protocol::protocol::HookEventName;
    use codex_protocol::protocol::HookSource;
    use codex_utils_absolute_path::test_support::PathBufExt;
    use codex_utils_absolute_path::test_support::test_path_buf;

    use super::CommandShell;
    use super::ConfiguredHandler;
    use super::build_command;

    #[cfg(not(windows))]
    #[test]
    fn app_bundled_internal_hooks_use_non_login_system_shell() {
        let command = build_command(
            &CommandShell {
                program: "ignored-user-shell".to_string(),
                args: vec!["--ignored".to_string()],
            },
            &internal_handler(),
        )
        .expect("build command");

        assert_eq!(command.as_std().get_program(), "/bin/sh");
        let args = command
            .as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(args, vec!["-c".to_string(), "echo trusted".to_string()]);
    }

    #[cfg(windows)]
    #[test]
    fn app_bundled_internal_hooks_disable_cmd_autorun() {
        let command = build_command(
            &CommandShell {
                program: "ignored-user-shell".to_string(),
                args: vec!["--ignored".to_string()],
            },
            &internal_handler(),
        )
        .expect("build command");
        assert!(
            command
                .as_std()
                .get_program()
                .to_string_lossy()
                .to_ascii_lowercase()
                .ends_with("\\system32\\cmd.exe")
        );
        let args = command
            .as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            args,
            vec![
                "/D".to_string(),
                "/S".to_string(),
                "/C".to_string(),
                "echo trusted".to_string(),
            ]
        );
    }

    fn internal_handler() -> ConfiguredHandler {
        ConfiguredHandler {
            event_name: HookEventName::Stop,
            matcher: None,
            command: "echo trusted".to_string(),
            timeout_sec: 5,
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
}
