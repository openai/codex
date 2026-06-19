#[cfg(target_os = "macos")]
mod pid_tracker;
#[cfg(target_os = "macos")]
mod seatbelt;

use std::path::PathBuf;
use std::process::Stdio;

use codex_config::LoaderOverrides;
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
use codex_sandboxing::landlock::allow_network_for_proxy;
use codex_sandboxing::landlock::create_linux_sandbox_command_args_for_permission_profile;
#[cfg(target_os = "macos")]
use codex_sandboxing::seatbelt::CreateSeatbeltCommandArgsParams;
#[cfg(target_os = "macos")]
use codex_sandboxing::seatbelt::create_seatbelt_command_args;
use codex_sandboxing::with_managed_mitm_ca_readable_root;
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
    loader_overrides: LoaderOverrides,
) -> anyhow::Result<()> {
    let SeatbeltCommand {
        permissions_profile,
        config_profile: _,
        cwd,
        include_managed_config,
        allow_unix_sockets,
        log_denials,
        config_overrides,
        command,
    } = command;
    let managed_requirements_mode = ManagedRequirementsMode::for_profile_invocation(
        &permissions_profile,
        include_managed_config,
    );
    run_command_under_sandbox(
        DebugSandboxConfigOptions {
            permissions_profile,
            cwd,
            managed_requirements_mode,
            loader_overrides,
        },
        command,
        config_overrides,
        codex_linux_sandbox_exe,
        SandboxType::Seatbelt,
        log_denials,
        &allow_unix_sockets,
    )
    .await
}

#[cfg(not(target_os = "macos"))]
pub async fn run_command_under_seatbelt(
    _command: SeatbeltCommand,
    _codex_linux_sandbox_exe: Option<PathBuf>,
    _loader_overrides: LoaderOverrides,
) -> anyhow::Result<()> {
    anyhow::bail!("Seatbelt sandbox is only available on macOS");
}

pub async fn run_command_under_landlock(
    command: LandlockCommand,
    codex_linux_sandbox_exe: Option<PathBuf>,
    loader_overrides: LoaderOverrides,
) -> anyhow::Result<()> {
    let LandlockCommand {
        permissions_profile,
        config_profile: _,
        cwd,
        include_managed_config,
        config_overrides,
        command,
    } = command;
    let managed_requirements_mode = ManagedRequirementsMode::for_profile_invocation(
        &permissions_profile,
        include_managed_config,
    );
    run_command_under_sandbox(
        DebugSandboxConfigOptions {
            permissions_profile,
            cwd,
            managed_requirements_mode,
            loader_overrides,
        },
        command,
        config_overrides,
        codex_linux_sandbox_exe,
        SandboxType::Landlock,
        /*log_denials*/ false,
        &[],
    )
    .await
}

pub async fn run_command_under_windows_sandbox(
    command: WindowsCommand,
    codex_linux_sandbox_exe: Option<PathBuf>,
    loader_overrides: LoaderOverrides,
) -> anyhow::Result<()> {
    let WindowsCommand {
        permissions_profile,
        config_profile: _,
        cwd,
        include_managed_config,
        config_overrides,
        command,
    } = command;
    let managed_requirements_mode = ManagedRequirementsMode::for_profile_invocation(
        &permissions_profile,
        include_managed_config,
    );
    run_command_under_sandbox(
        DebugSandboxConfigOptions {
            permissions_profile,
            cwd,
            managed_requirements_mode,
            loader_overrides,
        },
        command,
        config_overrides,
        codex_linux_sandbox_exe,
        SandboxType::Windows,
        /*log_denials*/ false,
        &[],
    )
    .await
}

enum SandboxType {
    #[cfg(target_os = "macos")]
    Seatbelt,
    Landlock,
    Windows,
}

#[derive(Debug)]
struct DebugSandboxConfigOptions {
    permissions_profile: Option<String>,
    cwd: Option<PathBuf>,
    managed_requirements_mode: ManagedRequirementsMode,
    loader_overrides: LoaderOverrides,
}

#[derive(Debug, Clone, Copy)]
enum ManagedRequirementsMode {
    Include,
    Ignore,
}

impl ManagedRequirementsMode {
    fn for_profile_invocation(
        permissions_profile: &Option<String>,
        include_managed_config: bool,
    ) -> Self {
        if permissions_profile.is_some() && !include_managed_config {
            Self::Ignore
        } else {
            Self::Include
        }
    }
}

async fn run_command_under_sandbox(
    config_options: DebugSandboxConfigOptions,
    command: Vec<String>,
    config_overrides: CliConfigOverrides,
    codex_linux_sandbox_exe: Option<PathBuf>,
    sandbox_type: SandboxType,
    log_denials: bool,
    #[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
    allow_unix_sockets: &[AbsolutePathBuf],
) -> anyhow::Result<()> {
    let config = load_debug_sandbox_config(
        config_overrides
            .parse_overrides()
            .map_err(anyhow::Error::msg)?,
        codex_linux_sandbox_exe,
        config_options,
        /*strict_config*/ false,
    )
    .await?;

    // In practice, this should be `std::env::current_dir()` because this CLI
    // does not support `--cwd`, but let's use the config value for consistency.
    let cwd = config.cwd.clone();
    // Non-Windows sandbox launchers still use `sandbox_policy_cwd` for any
    // remaining cwd-dependent policy resolution. `:workspace_roots` entries in
    // the effective profile have already been materialized from config roots.
    let sandbox_policy_cwd = cwd.clone();
    #[cfg(target_os = "windows")]
    let workspace_roots = config.effective_workspace_roots();

    let env = create_env(
        &config.permissions.shell_environment_policy,
        /*thread_id*/ None,
    );

    // Special-case Windows sandbox: execute and exit the process to emulate inherited stdio.
    if let SandboxType::Windows = sandbox_type {
        #[cfg(target_os = "windows")]
        {
            run_command_under_windows_session(&config, command, cwd, workspace_roots, env).await;
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
                config.permissions.permission_profile(),
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
    // Proxy containment depends on whether a proxy is active, not whether its
    // policy came from managed requirements.
    let enforce_managed_network = network.is_some();
    let managed_mitm_ca_trust_bundle_path = match network.as_ref() {
        Some(network) => network.managed_mitm_ca_trust_bundle_path(),
        None => None,
    };
    let runtime_permission_profile = with_managed_mitm_ca_readable_root(
        config.permissions.effective_permission_profile(),
        managed_mitm_ca_trust_bundle_path.as_ref(),
        sandbox_policy_cwd.as_path(),
    );

    let mut child = match sandbox_type {
        #[cfg(target_os = "macos")]
        SandboxType::Seatbelt => {
            let (file_system_sandbox_policy, network_sandbox_policy) =
                runtime_permission_profile.to_runtime_permissions();
            let args = create_seatbelt_command_args(CreateSeatbeltCommandArgsParams {
                command,
                file_system_sandbox_policy: &file_system_sandbox_policy,
                network_sandbox_policy,
                sandbox_policy_cwd: sandbox_policy_cwd.as_path(),
                enforce_managed_network,
                environment_id: None,
                network: network.as_ref(),
                extra_allow_unix_sockets: allow_unix_sockets,
            })
            .map_err(|err| anyhow::anyhow!(err))?;
            spawn_debug_sandbox_child(
                PathBuf::from("/usr/bin/sandbox-exec"),
                args,
                /*arg0*/ None,
                cwd.to_path_buf(),
                network_sandbox_policy,
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
            let network_sandbox_policy = runtime_permission_profile.network_sandbox_policy();
            let args = create_linux_sandbox_command_args_for_permission_profile(
                command,
                cwd.as_path(),
                &runtime_permission_profile,
                sandbox_policy_cwd.as_path(),
                use_legacy_landlock,
                allow_network_for_proxy(enforce_managed_network),
            );
            spawn_debug_sandbox_child(
                codex_linux_sandbox_exe,
                args,
                Some("codex-linux-sandbox"),
                cwd.to_path_buf(),
                network_sandbox_policy,
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

#[cfg(target_os = "windows")]
async fn run_command_under_windows_session(
    config: &Config,
    command: Vec<String>,
    cwd: AbsolutePathBuf,
    workspace_roots: Vec<AbsolutePathBuf>,
    env: std::collections::HashMap<String, String>,
) -> ! {
    use codex_core::windows_sandbox::WindowsSandboxLevelExt;
    use codex_protocol::config_types::WindowsSandboxLevel;
    use codex_windows_sandbox::WindowsSandboxSessionRequest;
    use codex_windows_sandbox::spawn_windows_sandbox_session_for_level;

    let permission_profile = config.permissions.effective_permission_profile();
    let empty_paths: &[AbsolutePathBuf] = &[];
    let spawned = spawn_windows_sandbox_session_for_level(WindowsSandboxSessionRequest {
        permission_profile: &permission_profile,
        workspace_roots: workspace_roots.as_slice(),
        codex_home: config.codex_home.as_path(),
        command,
        cwd: cwd.as_path(),
        env_map: env,
        windows_sandbox_level: WindowsSandboxLevel::from_config(config),
        proxy_enforced: false,
        timeout_ms: None,
        read_roots_override: None,
        read_roots_include_platform_defaults: false,
        write_roots_override: None,
        deny_read_paths_override: empty_paths,
        deny_write_paths_override: empty_paths,
        tty: false,
        stdin_open: true,
        use_private_desktop: config.permissions.windows_sandbox_private_desktop,
    })
    .await;

    let spawned = match spawned {
        Ok(spawned) => spawned,
        Err(err) => {
            eprintln!("windows sandbox failed: {err}");
            std::process::exit(1);
        }
    };

    let exit_code = codex_windows_sandbox::forward_sandbox_session_stdio(spawned).await;
    std::process::exit(exit_code);
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
    options: DebugSandboxConfigOptions,
    strict_config: bool,
) -> anyhow::Result<Config> {
    load_debug_sandbox_config_with_codex_home(
        cli_overrides,
        codex_linux_sandbox_exe,
        options,
        /*codex_home*/ None,
        strict_config,
    )
    .await
}

async fn load_debug_sandbox_config_with_codex_home(
    cli_overrides: Vec<(String, TomlValue)>,
    codex_linux_sandbox_exe: Option<PathBuf>,
    options: DebugSandboxConfigOptions,
    codex_home: Option<PathBuf>,
    strict_config: bool,
) -> anyhow::Result<Config> {
    let DebugSandboxConfigOptions {
        permissions_profile,
        cwd,
        managed_requirements_mode,
        loader_overrides,
    } = options;
    let mut cli_overrides = cli_overrides;

    if let Some(permissions_profile) = permissions_profile {
        cli_overrides.push((
            "default_permissions".to_string(),
            TomlValue::String(permissions_profile),
        ));
    }

    // For legacy configs, `codex sandbox` historically defaulted to read-only
    // instead of inheriting ambient `sandbox_mode` settings from user/system
    // config. Keep that behavior unless this invocation explicitly passes a
    // legacy `sandbox_mode` CLI override for compatibility with older callers.
    let uses_legacy_sandbox_mode_override = cli_overrides_use_legacy_sandbox_mode(&cli_overrides);
    let config = build_debug_sandbox_config_with_loader_overrides(
        cli_overrides.clone(),
        ConfigOverrides {
            cwd: cwd.clone(),
            codex_linux_sandbox_exe: codex_linux_sandbox_exe.clone(),
            ..Default::default()
        },
        codex_home.clone(),
        managed_requirements_mode,
        loader_overrides.clone(),
        strict_config,
    )
    .await?;

    if config_uses_permission_profiles(&config) || uses_legacy_sandbox_mode_override {
        return Ok(config);
    }

    build_debug_sandbox_config_with_loader_overrides(
        cli_overrides,
        ConfigOverrides {
            sandbox_mode: Some(SandboxMode::ReadOnly),
            cwd,
            codex_linux_sandbox_exe,
            ..Default::default()
        },
        codex_home,
        managed_requirements_mode,
        loader_overrides,
        strict_config,
    )
    .await
    .map_err(Into::into)
}

async fn build_debug_sandbox_config_with_loader_overrides(
    cli_overrides: Vec<(String, TomlValue)>,
    harness_overrides: ConfigOverrides,
    codex_home: Option<PathBuf>,
    managed_requirements_mode: ManagedRequirementsMode,
    mut loader_overrides: LoaderOverrides,
    strict_config: bool,
) -> std::io::Result<Config> {
    let mut builder = ConfigBuilder::default()
        .cli_overrides(cli_overrides)
        .harness_overrides(harness_overrides)
        .strict_config(strict_config);
    if matches!(managed_requirements_mode, ManagedRequirementsMode::Ignore) {
        loader_overrides.ignore_managed_requirements = true;
    }
    builder = builder.loader_overrides(loader_overrides);
    if let Some(codex_home) = codex_home {
        builder = builder
            .codex_home(codex_home.clone())
            .fallback_cwd(Some(codex_home));
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

fn cli_overrides_use_legacy_sandbox_mode(cli_overrides: &[(String, TomlValue)]) -> bool {
    cli_overrides.iter().any(|(key, _)| key == "sandbox_mode")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    async fn build_debug_sandbox_config(
        cli_overrides: Vec<(String, TomlValue)>,
        harness_overrides: ConfigOverrides,
        codex_home: Option<PathBuf>,
        managed_requirements_mode: ManagedRequirementsMode,
        strict_config: bool,
    ) -> std::io::Result<Config> {
        build_debug_sandbox_config_with_loader_overrides(
            cli_overrides,
            harness_overrides,
            codex_home,
            managed_requirements_mode,
            LoaderOverrides::default(),
            strict_config,
        )
        .await
    }

    fn escape_toml_path(path: &std::path::Path) -> String {
        path.display().to_string().replace('\\', "\\\\")
    }

    fn write_permissions_profile_config(
        codex_home: &TempDir,
        docs: &std::path::Path,
        private: &std::path::Path,
    ) -> std::io::Result<()> {
        write_permissions_profile_config_to_path(
            &codex_home.path().join("config.toml"),
            docs,
            private,
        )
    }

    fn write_permissions_profile_config_to_path(
        config_path: &std::path::Path,
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
        std::fs::write(config_path, config)?;
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
            ManagedRequirementsMode::Include,
            /*strict_config*/ false,
        )
        .await?;
        let legacy_config = build_debug_sandbox_config(
            Vec::new(),
            ConfigOverrides {
                sandbox_mode: Some(SandboxMode::ReadOnly),
                ..Default::default()
            },
            Some(codex_home_path.clone()),
            ManagedRequirementsMode::Include,
            /*strict_config*/ false,
        )
        .await?;

        let config = load_debug_sandbox_config_with_codex_home(
            Vec::new(),
            /*codex_linux_sandbox_exe*/ None,
            DebugSandboxConfigOptions {
                permissions_profile: None,
                cwd: None,
                managed_requirements_mode: ManagedRequirementsMode::Include,
                loader_overrides: LoaderOverrides::default(),
            },
            Some(codex_home_path),
            /*strict_config*/ false,
        )
        .await?;

        assert!(config_uses_permission_profiles(&config));
        assert!(
            profile_config.permissions.file_system_sandbox_policy()
                != legacy_config.permissions.file_system_sandbox_policy(),
            "test fixture should distinguish profile syntax from legacy sandbox_mode"
        );
        assert_eq!(
            config.permissions.file_system_sandbox_policy(),
            profile_config.permissions.file_system_sandbox_policy(),
        );
        assert_ne!(
            config.permissions.file_system_sandbox_policy(),
            legacy_config.permissions.file_system_sandbox_policy(),
        );

        Ok(())
    }

    #[tokio::test]
    async fn debug_sandbox_honors_config_profile_loader_overrides() -> anyhow::Result<()> {
        let codex_home = TempDir::new()?;
        let sandbox_paths = TempDir::new()?;
        let docs = sandbox_paths.path().join("docs");
        let private = docs.join("private");
        let profile_path = codex_home.path().join("work.config.toml");
        write_permissions_profile_config_to_path(&profile_path, &docs, &private)?;
        let codex_home_path = codex_home.path().to_path_buf();
        let loader_overrides = LoaderOverrides {
            user_config_path: Some(AbsolutePathBuf::from_absolute_path(&profile_path)?),
            user_config_profile: Some("work".parse().expect("profile name should parse")),
            ..LoaderOverrides::default()
        };

        let profile_config = build_debug_sandbox_config_with_loader_overrides(
            Vec::new(),
            ConfigOverrides::default(),
            Some(codex_home_path.clone()),
            ManagedRequirementsMode::Include,
            loader_overrides.clone(),
            /*strict_config*/ false,
        )
        .await?;
        let read_only_config = build_debug_sandbox_config(
            Vec::new(),
            ConfigOverrides {
                sandbox_mode: Some(SandboxMode::ReadOnly),
                ..Default::default()
            },
            Some(codex_home_path.clone()),
            ManagedRequirementsMode::Include,
            /*strict_config*/ false,
        )
        .await?;

        let config = load_debug_sandbox_config_with_codex_home(
            Vec::new(),
            /*codex_linux_sandbox_exe*/ None,
            DebugSandboxConfigOptions {
                permissions_profile: None,
                cwd: None,
                managed_requirements_mode: ManagedRequirementsMode::Include,
                loader_overrides,
            },
            Some(codex_home_path),
            /*strict_config*/ false,
        )
        .await?;

        assert!(config_uses_permission_profiles(&config));
        assert_ne!(
            profile_config.permissions.file_system_sandbox_policy(),
            read_only_config.permissions.file_system_sandbox_policy(),
            "test fixture should distinguish the profile config from read-only"
        );
        assert_eq!(
            config.permissions.file_system_sandbox_policy(),
            profile_config.permissions.file_system_sandbox_policy(),
        );

        Ok(())
    }

    #[tokio::test]
    async fn debug_sandbox_honors_explicit_legacy_sandbox_mode() -> anyhow::Result<()> {
        let codex_home = TempDir::new()?;
        let codex_home_path = codex_home.path().to_path_buf();
        let cli_overrides = vec![(
            "sandbox_mode".to_string(),
            TomlValue::String("workspace-write".to_string()),
        )];

        let workspace_write_config = build_debug_sandbox_config(
            cli_overrides.clone(),
            ConfigOverrides::default(),
            Some(codex_home_path.clone()),
            ManagedRequirementsMode::Include,
            /*strict_config*/ false,
        )
        .await?;
        let read_only_config = build_debug_sandbox_config(
            Vec::new(),
            ConfigOverrides {
                sandbox_mode: Some(SandboxMode::ReadOnly),
                ..Default::default()
            },
            Some(codex_home_path.clone()),
            ManagedRequirementsMode::Include,
            /*strict_config*/ false,
        )
        .await?;

        let config = load_debug_sandbox_config_with_codex_home(
            cli_overrides,
            /*codex_linux_sandbox_exe*/ None,
            DebugSandboxConfigOptions {
                permissions_profile: None,
                cwd: None,
                managed_requirements_mode: ManagedRequirementsMode::Include,
                loader_overrides: LoaderOverrides::default(),
            },
            Some(codex_home_path),
            /*strict_config*/ false,
        )
        .await?;

        if cfg!(target_os = "windows") {
            assert_eq!(
                workspace_write_config
                    .permissions
                    .file_system_sandbox_policy(),
                read_only_config.permissions.file_system_sandbox_policy(),
                "workspace-write downgrades to read-only when the Windows sandbox is disabled"
            );
        } else {
            assert_ne!(
                workspace_write_config
                    .permissions
                    .file_system_sandbox_policy(),
                read_only_config.permissions.file_system_sandbox_policy(),
                "test fixture should distinguish explicit workspace-write from read-only"
            );
        }
        assert_eq!(
            config.permissions.file_system_sandbox_policy(),
            workspace_write_config
                .permissions
                .file_system_sandbox_policy(),
        );

        Ok(())
    }

    #[tokio::test]
    async fn debug_sandbox_defaults_legacy_configs_to_read_only() -> anyhow::Result<()> {
        let codex_home = TempDir::new()?;
        let codex_home_path = codex_home.path().to_path_buf();

        let read_only_config = build_debug_sandbox_config(
            Vec::new(),
            ConfigOverrides {
                sandbox_mode: Some(SandboxMode::ReadOnly),
                ..Default::default()
            },
            Some(codex_home_path.clone()),
            ManagedRequirementsMode::Include,
            /*strict_config*/ false,
        )
        .await?;

        let config = load_debug_sandbox_config_with_codex_home(
            Vec::new(),
            /*codex_linux_sandbox_exe*/ None,
            DebugSandboxConfigOptions {
                permissions_profile: None,
                cwd: None,
                managed_requirements_mode: ManagedRequirementsMode::Include,
                loader_overrides: LoaderOverrides::default(),
            },
            Some(codex_home_path),
            /*strict_config*/ false,
        )
        .await?;

        assert!(!config_uses_permission_profiles(&config));
        assert_eq!(
            config.permissions.file_system_sandbox_policy(),
            read_only_config.permissions.file_system_sandbox_policy(),
        );

        Ok(())
    }

    #[tokio::test]
    async fn debug_sandbox_honors_explicit_builtin_permission_profile() -> anyhow::Result<()> {
        let codex_home = TempDir::new()?;

        let config = load_debug_sandbox_config_with_codex_home(
            Vec::new(),
            /*codex_linux_sandbox_exe*/ None,
            DebugSandboxConfigOptions {
                permissions_profile: Some(":workspace".to_string()),
                cwd: None,
                managed_requirements_mode: ManagedRequirementsMode::Ignore,
                loader_overrides: LoaderOverrides::default(),
            },
            Some(codex_home.path().to_path_buf()),
            /*strict_config*/ false,
        )
        .await?;

        let actual = config
            .permissions
            .permission_profile()
            .file_system_sandbox_policy();
        let expected = codex_protocol::models::PermissionProfile::workspace_write()
            .file_system_sandbox_policy();
        assert!(
            expected
                .entries
                .iter()
                .all(|entry| actual.entries.contains(entry)),
            "explicit workspace profile should preserve the built-in workspace rules"
        );

        Ok(())
    }

    #[tokio::test]
    async fn debug_sandbox_honors_explicit_named_permission_profile() -> anyhow::Result<()> {
        let codex_home = TempDir::new()?;
        let sandbox_paths = TempDir::new()?;
        let docs = sandbox_paths.path().join("docs");
        let private = docs.join("private");
        write_permissions_profile_config(&codex_home, &docs, &private)?;

        let config = load_debug_sandbox_config_with_codex_home(
            Vec::new(),
            /*codex_linux_sandbox_exe*/ None,
            DebugSandboxConfigOptions {
                permissions_profile: Some("limited-read-test".to_string()),
                cwd: None,
                managed_requirements_mode: ManagedRequirementsMode::Ignore,
                loader_overrides: LoaderOverrides::default(),
            },
            Some(codex_home.path().to_path_buf()),
            /*strict_config*/ false,
        )
        .await?;

        let expected = build_debug_sandbox_config(
            vec![(
                "default_permissions".to_string(),
                TomlValue::String("limited-read-test".to_string()),
            )],
            ConfigOverrides::default(),
            Some(codex_home.path().to_path_buf()),
            ManagedRequirementsMode::Include,
            /*strict_config*/ false,
        )
        .await?;

        assert_eq!(
            config.permissions.file_system_sandbox_policy(),
            expected.permissions.file_system_sandbox_policy()
        );

        Ok(())
    }

    #[tokio::test]
    async fn debug_sandbox_uses_explicit_cwd() -> anyhow::Result<()> {
        let codex_home = TempDir::new()?;
        let cwd = TempDir::new()?;

        let config = load_debug_sandbox_config_with_codex_home(
            Vec::new(),
            /*codex_linux_sandbox_exe*/ None,
            DebugSandboxConfigOptions {
                permissions_profile: Some(":workspace".to_string()),
                cwd: Some(cwd.path().to_path_buf()),
                managed_requirements_mode: ManagedRequirementsMode::Ignore,
                loader_overrides: LoaderOverrides::default(),
            },
            Some(codex_home.path().to_path_buf()),
            /*strict_config*/ false,
        )
        .await?;

        assert_eq!(config.cwd.as_path(), cwd.path());

        Ok(())
    }
}
