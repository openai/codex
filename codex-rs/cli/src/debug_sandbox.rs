#[cfg(target_os = "macos")]
mod pid_tracker;
#[cfg(target_os = "macos")]
mod seatbelt;

use std::path::PathBuf;
use std::process::Stdio;

use codex_core::config::Config;
use codex_core::config::ConfigBuilder;
use codex_core::config::ConfigOverrides;
use codex_core::config::NetworkProxyAuditMetadata;
use codex_core::exec_env::create_env;
#[cfg(target_os = "macos")]
use codex_core::spawn::CODEX_SANDBOX_ENV_VAR;
use codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use codex_protocol::config_types::SandboxMode;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_sandboxing::landlock::create_linux_sandbox_command_args_for_policies;
#[cfg(target_os = "macos")]
use codex_sandboxing::seatbelt::create_seatbelt_command_args_for_policies;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_cli::CliConfigOverrides;
use tokio::process::Child;
use tokio::process::Command as TokioCommand;
use toml::Value as TomlValue;

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
        SandboxType::Seatbelt,
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
        SandboxType::Landlock,
        /*log_denials*/ false,
    )
    .await
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
        SandboxType::Windows,
        /*log_denials*/ false,
    )
    .await
}

enum SandboxType {
    #[cfg(target_os = "macos")]
    Seatbelt,
    Landlock,
    Windows,
}

async fn run_command_under_sandbox(
    full_auto: bool,
    command: Vec<String>,
    config_overrides: CliConfigOverrides,
    codex_linux_sandbox_exe: Option<PathBuf>,
    sandbox_type: SandboxType,
    log_denials: bool,
) -> anyhow::Result<()> {
    let config = load_debug_sandbox_config(
        config_overrides
            .parse_overrides()
            .map_err(anyhow::Error::msg)?,
        codex_linux_sandbox_exe,
        full_auto,
    )
    .await?;

    // In practice, this should be `std::env::current_dir()` because this CLI
    // does not support `--cwd`, but let's use the config value for consistency.
    let cwd = config.cwd.clone();
    // For now, we always use the same cwd for both the command and the
    // sandbox policy. In the future, we could add a CLI option to set them
    // separately.
    let sandbox_policy_cwd = cwd.clone();

    let env = create_env(
        &config.permissions.shell_environment_policy,
        /*thread_id*/ None,
    );

    // Special-case Windows sandbox: execute and exit the process to emulate inherited stdio.
    if let SandboxType::Windows = sandbox_type {
        #[cfg(target_os = "windows")]
        {
            use codex_core::windows_sandbox::WindowsSandboxLevelExt;
            use codex_protocol::config_types::WindowsSandboxLevel;
            use codex_windows_sandbox::run_windows_sandbox_capture;
            use codex_windows_sandbox::run_windows_sandbox_capture_elevated;

            let policy_str = serde_json::to_string(config.permissions.sandbox_policy.get())?;

            let sandbox_cwd = sandbox_policy_cwd.clone();
            let cwd_clone = cwd.clone();
            let env_map = env.clone();
            let command_vec = command.clone();
            let base_dir = config.codex_home.clone();
            let use_elevated = matches!(
                WindowsSandboxLevel::from_config(&config),
                WindowsSandboxLevel::Elevated
            );

            // Preflight audit is invoked elsewhere at the appropriate times.
            let res = tokio::task::spawn_blocking(move || {
                if use_elevated {
                    run_windows_sandbox_capture_elevated(
                        codex_windows_sandbox::ElevatedSandboxCaptureRequest {
                            policy_json_or_preset: policy_str.as_str(),
                            sandbox_policy_cwd: &sandbox_cwd,
                            codex_home: base_dir.as_path(),
                            command: command_vec,
                            cwd: &cwd_clone,
                            env_map,
                            timeout_ms: None,
                            use_private_desktop: config.permissions.windows_sandbox_private_desktop,
                            proxy_enforced: false,
                        },
                    )
                } else {
                    run_windows_sandbox_capture(
                        policy_str.as_str(),
                        &sandbox_cwd,
                        base_dir.as_path(),
                        command_vec,
                        &cwd_clone,
                        env_map,
                        /*timeout_ms*/ None,
                        config.permissions.windows_sandbox_private_desktop,
                    )
                }
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
        #[cfg(not(target_os = "windows"))]
        {
            anyhow::bail!("Windows sandbox is only available on Windows");
        }
    }

    #[cfg(target_os = "macos")]
    let mut denial_logger = log_denials.then(DenialLogger::new).flatten();
    #[cfg(not(target_os = "macos"))]
    let _ = log_denials;

    let managed_network_requirements_enabled = config.managed_network_requirements_enabled();

    // This proxy should only live for the lifetime of the child process.
    let network_proxy = match config.permissions.network.as_ref() {
        Some(spec) => Some(
            spec.start_proxy(
                config.permissions.sandbox_policy.get(),
                /*policy_decider*/ None,
                /*blocked_request_observer*/ None,
                managed_network_requirements_enabled,
                NetworkProxyAuditMetadata::default(),
            )
            .await
            .map_err(|err| anyhow::anyhow!("failed to start managed network proxy: {err}"))?,
        ),
        None => None,
    };
    let network = network_proxy
        .as_ref()
        .map(codex_core::config::StartedNetworkProxy::proxy);

    let mut child = match sandbox_type {
        #[cfg(target_os = "macos")]
        SandboxType::Seatbelt => {
            let args = create_seatbelt_command_args_for_policies(
                command,
                &config.permissions.file_system_sandbox_policy,
                config.permissions.network_sandbox_policy,
                sandbox_policy_cwd.as_path(),
                /*enforce_managed_network*/ false,
                network.as_ref(),
            );
            let network_policy = config.permissions.network_sandbox_policy;
            spawn_debug_sandbox_child(
                PathBuf::from("/usr/bin/sandbox-exec"),
                args,
                /*arg0*/ None,
                cwd.to_path_buf(),
                network_policy,
                env,
                |env_map| {
                    env_map.insert(CODEX_SANDBOX_ENV_VAR.to_string(), "seatbelt".to_string());
                    if let Some(network) = network.as_ref() {
                        network.apply_to_env(env_map);
                    }
                },
            )
            .await?
        }
        SandboxType::Landlock => {
            #[expect(clippy::expect_used)]
            let codex_linux_sandbox_exe = config
                .codex_linux_sandbox_exe
                .expect("codex-linux-sandbox executable not found");
            let use_legacy_landlock = config.features.use_legacy_landlock();
            let args = create_linux_sandbox_command_args_for_policies(
                command,
                cwd.as_path(),
                config.permissions.sandbox_policy.get(),
                &config.permissions.file_system_sandbox_policy,
                config.permissions.network_sandbox_policy,
                sandbox_policy_cwd.as_path(),
                use_legacy_landlock,
                /*allow_network_for_proxy*/ false,
            );
            let network_policy = config.permissions.network_sandbox_policy;
            spawn_debug_sandbox_child(
                codex_linux_sandbox_exe,
                args,
                Some("codex-linux-sandbox"),
                cwd.to_path_buf(),
                network_policy,
                env,
                |env_map| {
                    if let Some(network) = network.as_ref() {
                        network.apply_to_env(env_map);
                    }
                },
            )
            .await?
        }
        SandboxType::Windows => {
            unreachable!("Windows sandbox should have been handled above");
        }
    };

    #[cfg(target_os = "macos")]
    if let Some(denial_logger) = &mut denial_logger {
        denial_logger.on_child_spawn(&child);
    }

    let status = child.wait().await?;

    #[cfg(target_os = "macos")]
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

    handle_exit_status(status);
}

pub fn create_sandbox_mode(full_auto: bool) -> SandboxMode {
    if full_auto {
        SandboxMode::WorkspaceWrite
    } else {
        SandboxMode::ReadOnly
    }
}

async fn spawn_debug_sandbox_child(
    program: PathBuf,
    args: Vec<String>,
    arg0: Option<&str>,
    cwd: PathBuf,
    network_sandbox_policy: NetworkSandboxPolicy,
    mut env: std::collections::HashMap<String, String>,
    apply_env: impl FnOnce(&mut std::collections::HashMap<String, String>),
) -> std::io::Result<Child> {
    let mut cmd = TokioCommand::new(&program);
    #[cfg(unix)]
    cmd.arg0(arg0.map_or_else(|| program.to_string_lossy().to_string(), String::from));
    #[cfg(not(unix))]
    let _ = arg0;
    cmd.args(args);
    cmd.current_dir(cwd);
    apply_env(&mut env);
    cmd.env_clear();
    cmd.envs(env);

    if !network_sandbox_policy.is_enabled() {
        cmd.env(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR, "1");
    }

    cmd.stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
}

async fn load_debug_sandbox_config(
    cli_overrides: Vec<(String, TomlValue)>,
    codex_linux_sandbox_exe: Option<PathBuf>,
    full_auto: bool,
) -> anyhow::Result<Config> {
    load_debug_sandbox_config_with_codex_home(
        cli_overrides,
        codex_linux_sandbox_exe,
        full_auto,
        /*codex_home*/ None,
    )
    .await
}

async fn load_debug_sandbox_config_with_codex_home(
    cli_overrides: Vec<(String, TomlValue)>,
    codex_linux_sandbox_exe: Option<PathBuf>,
    full_auto: bool,
    codex_home: Option<PathBuf>,
) -> anyhow::Result<Config> {
    let config = build_debug_sandbox_config(
        cli_overrides.clone(),
        ConfigOverrides {
            codex_linux_sandbox_exe: codex_linux_sandbox_exe.clone(),
            ..Default::default()
        },
        codex_home.clone(),
    )
    .await?;

    if config_uses_permission_profiles(&config) {
        if full_auto {
            anyhow::bail!(
                "`codex sandbox --full-auto` is only supported for legacy `sandbox_mode` configs; choose a writable `[permissions]` profile instead"
            );
        }
        return Ok(config);
    }

    build_debug_sandbox_config(
        cli_overrides,
        ConfigOverrides {
            sandbox_mode: Some(create_sandbox_mode(full_auto)),
            codex_linux_sandbox_exe,
            ..Default::default()
        },
        codex_home,
    )
    .await
    .map_err(Into::into)
}

async fn build_debug_sandbox_config(
    cli_overrides: Vec<(String, TomlValue)>,
    harness_overrides: ConfigOverrides,
    codex_home: Option<PathBuf>,
) -> std::io::Result<Config> {
    let mut builder = ConfigBuilder::default()
        .cli_overrides(cli_overrides)
        .harness_overrides(harness_overrides);
    if let Some(codex_home) = codex_home {
        let fallback_cwd = AbsolutePathBuf::relative_to_current_dir(&codex_home)?;
        builder = builder
            .codex_home(codex_home)
            .fallback_cwd(Some(fallback_cwd));
    }
    builder.build().await
}

fn config_uses_permission_profiles(config: &Config) -> bool {
    config
        .config_layer_stack
        .effective_config()
        .get("default_permissions")
        .is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn escape_toml_path(path: &std::path::Path) -> String {
        path.display().to_string().replace('\\', "\\\\")
    }

    fn write_permissions_profile_config(
        codex_home: &TempDir,
        docs: &std::path::Path,
        private: &std::path::Path,
    ) -> std::io::Result<()> {
        std::fs::create_dir_all(private)?;
        let config = format!(
            "default_permissions = \"limited-read-test\"\n\
             [permissions.limited-read-test.filesystem]\n\
             \":minimal\" = \"read\"\n\
             \"{}\" = \"read\"\n\
             \"{}\" = \"none\"\n\
             \n\
             [permissions.limited-read-test.network]\n\
             enabled = true\n",
            escape_toml_path(docs),
            escape_toml_path(private),
        );
        std::fs::write(codex_home.path().join("config.toml"), config)?;
        Ok(())
    }

    #[tokio::test]
    async fn debug_sandbox_honors_active_permission_profiles() -> anyhow::Result<()> {
        let codex_home = TempDir::new()?;
        let sandbox_paths = TempDir::new()?;
        let docs = sandbox_paths.path().join("docs");
        let private = docs.join("private");
        write_permissions_profile_config(&codex_home, &docs, &private)?;
        let codex_home_path = codex_home.path().to_path_buf();

        let profile_config = build_debug_sandbox_config(
            Vec::new(),
            ConfigOverrides::default(),
            Some(codex_home_path.clone()),
        )
        .await?;
        let legacy_config = build_debug_sandbox_config(
            Vec::new(),
            ConfigOverrides {
                sandbox_mode: Some(create_sandbox_mode(/*full_auto*/ false)),
                ..Default::default()
            },
            Some(codex_home_path.clone()),
        )
        .await?;

        let config = load_debug_sandbox_config_with_codex_home(
            Vec::new(),
            /*codex_linux_sandbox_exe*/ None,
            /*full_auto*/ false,
            Some(codex_home_path),
        )
        .await?;

        assert!(config_uses_permission_profiles(&config));
        assert!(
            profile_config.permissions.file_system_sandbox_policy
                != legacy_config.permissions.file_system_sandbox_policy,
            "test fixture should distinguish profile syntax from legacy sandbox_mode"
        );
        assert_eq!(
            config.permissions.file_system_sandbox_policy,
            profile_config.permissions.file_system_sandbox_policy,
        );
        assert_ne!(
            config.permissions.file_system_sandbox_policy,
            legacy_config.permissions.file_system_sandbox_policy,
        );

        Ok(())
    }

    #[tokio::test]
    async fn debug_sandbox_rejects_full_auto_for_permission_profiles() -> anyhow::Result<()> {
        let codex_home = TempDir::new()?;
        let sandbox_paths = TempDir::new()?;
        let docs = sandbox_paths.path().join("docs");
        let private = docs.join("private");
        write_permissions_profile_config(&codex_home, &docs, &private)?;

        let err = load_debug_sandbox_config_with_codex_home(
            Vec::new(),
            /*codex_linux_sandbox_exe*/ None,
            /*full_auto*/ true,
            Some(codex_home.path().to_path_buf()),
        )
        .await
        .expect_err("full-auto should be rejected for active permission profiles");

        assert!(
            err.to_string().contains("--full-auto"),
            "unexpected error: {err}"
        );

        Ok(())
    }
}
