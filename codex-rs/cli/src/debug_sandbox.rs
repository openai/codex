#[cfg(target_os = "macos")]
mod pid_tracker;
#[cfg(target_os = "macos")]
mod seatbelt;

use std::path::PathBuf;

use codex_common::CliConfigOverrides;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::exec_env::create_env;
#[cfg(target_os = "macos")]
use codex_core::seatbelt::spawn_command_under_seatbelt;
use codex_core::spawn::StdioPolicy;
use codex_protocol::config_types::SandboxMode;

#[cfg(target_os = "linux")]
use codex_core::landlock::spawn_command_under_linux_sandbox;

use crate::LandlockCommand;
use crate::SeatbeltCommand;
use crate::WindowsCommand;
use crate::exit_status::handle_exit_status;

#[cfg(target_os = "macos")]
use seatbelt::DenialLogger;

#[cfg(target_os = "macos")]
pub async fn run_command_under_seatbelt(
    command: SeatbeltCommand,
    codex_linux_sandbox_exe: Option<PathBuf>,
) -> anyhow::Result<()> {
    let SeatbeltCommand {
        full_auto,
        log_denials,
        config_overrides,
        command,
    } = command;
    run_command_under_sandbox(
        full_auto,
        command,
        config_overrides,
        codex_linux_sandbox_exe,
        log_denials,
    )
    .await
}

#[cfg(not(target_os = "macos"))]
pub async fn run_command_under_seatbelt(
    _command: SeatbeltCommand,
    _codex_linux_sandbox_exe: Option<PathBuf>,
) -> anyhow::Result<()> {
    anyhow::bail!("Seatbelt sandbox is only available on macOS");
}

#[cfg(target_os = "linux")]
pub async fn run_command_under_landlock(
    command: LandlockCommand,
    codex_linux_sandbox_exe: Option<PathBuf>,
) -> anyhow::Result<()> {
    let LandlockCommand {
        full_auto,
        config_overrides,
        command,
    } = command;
    run_command_under_sandbox(
        full_auto,
        command,
        config_overrides,
        codex_linux_sandbox_exe,
        false,
    )
    .await
}

#[cfg(not(target_os = "linux"))]
pub async fn run_command_under_landlock(
    _command: LandlockCommand,
    _codex_linux_sandbox_exe: Option<PathBuf>,
) -> anyhow::Result<()> {
    anyhow::bail!("Landlock sandbox is only available on Linux");
}

pub async fn run_command_under_windows(
    command: WindowsCommand,
    codex_linux_sandbox_exe: Option<PathBuf>,
) -> anyhow::Result<()> {
    let WindowsCommand {
        full_auto,
        config_overrides,
        command,
    } = command;
    run_command_under_sandbox(
        full_auto,
        command,
        config_overrides,
        codex_linux_sandbox_exe,
        false,
    )
    .await
}

async fn run_command_under_sandbox(
    full_auto: bool,
    command: Vec<String>,
    config_overrides: CliConfigOverrides,
    codex_linux_sandbox_exe: Option<PathBuf>,
    log_denials: bool,
) -> anyhow::Result<()> {
    #[cfg(not(target_os = "macos"))]
    let _ = log_denials;

    let sandbox_mode = create_sandbox_mode(full_auto);
    let config = Config::load_with_cli_overrides(
        config_overrides
            .parse_overrides()
            .map_err(anyhow::Error::msg)?,
        ConfigOverrides {
            sandbox_mode: Some(sandbox_mode),
            codex_linux_sandbox_exe,
            ..Default::default()
        },
    )
    .await?;

    // In practice, this should be `std::env::current_dir()` because this CLI
    // does not support `--cwd`, but let's use the config value for consistency.
    let cwd = config.cwd.clone();
    // For now, we always use the same cwd for both the command and the
    // sandbox policy. In the future, we could add a CLI option to set them
    // separately.
    let sandbox_policy_cwd = cwd.clone();

    let stdio_policy = StdioPolicy::Inherit;
    let env = create_env(&config.shell_environment_policy);

    // Special-case Windows sandbox: execute and exit the process to emulate inherited stdio.
    #[cfg(target_os = "windows")]
    {
        use codex_windows_sandbox::run_windows_sandbox_capture;

        let policy_str = serde_json::to_string(&config.sandbox_policy)?;

        let sandbox_cwd = sandbox_policy_cwd.clone();
        let cwd_clone = cwd.clone();
        let env_map = env.clone();
        let command_vec = command.clone();
        let base_dir = config.codex_home.clone();

        // Preflight audit is invoked elsewhere at the appropriate times.
        let res = tokio::task::spawn_blocking(move || {
            run_windows_sandbox_capture(
                policy_str.as_str(),
                &sandbox_cwd,
                base_dir.as_path(),
                command_vec,
                &cwd_clone,
                env_map,
                None,
            )
        })
        .await;

        let capture = match res {
            Ok(Ok(v)) => v,
            Ok(Err(err)) => {
                eprintln!("windows sandbox failed: {err}");
                std::process::exit(1);
            }
            Err(join_err) => {
                eprintln!("windows sandbox join error: {join_err}");
                std::process::exit(1);
            }
        };

        if !capture.stdout.is_empty() {
            use std::io::Write;
            let _ = std::io::stdout().write_all(&capture.stdout);
        }
        if !capture.stderr.is_empty() {
            use std::io::Write;
            let _ = std::io::stderr().write_all(&capture.stderr);
        }

        std::process::exit(capture.exit_code);
    }

    #[cfg(target_os = "macos")]
    let status = {
        let mut denial_logger = log_denials.then(DenialLogger::new).flatten();
        let mut child = spawn_command_under_seatbelt(
            command,
            cwd,
            &config.sandbox_policy,
            sandbox_policy_cwd.as_path(),
            stdio_policy,
            env,
        )
        .await?;
        if let Some(denial_logger) = &mut denial_logger {
            denial_logger.on_child_spawn(&child);
        }

        let status = child.wait().await?;

        if let Some(denial_logger) = denial_logger {
            let denials = denial_logger.finish().await;
            eprintln!("\n=== Sandbox denials ===");
            if denials.is_empty() {
                eprintln!("None found.");
            } else {
                for seatbelt::SandboxDenial { name, capability } in denials {
                    eprintln!("({name}) {capability}");
                }
            }
        }

        status
    };
    #[cfg(target_os = "linux")]
    let status = {
        #[expect(clippy::expect_used)]
        let codex_linux_sandbox_exe = config
            .codex_linux_sandbox_exe
            .expect("codex-linux-sandbox executable not found");
        let mut child = spawn_command_under_linux_sandbox(
            codex_linux_sandbox_exe,
            command,
            cwd,
            &config.sandbox_policy,
            sandbox_policy_cwd.as_path(),
            stdio_policy,
            env,
        )
        .await?;
        child.wait().await?
    };

    handle_exit_status(status);
}

pub fn create_sandbox_mode(full_auto: bool) -> SandboxMode {
    if full_auto {
        SandboxMode::WorkspaceWrite
    } else {
        SandboxMode::ReadOnly
    }
}
