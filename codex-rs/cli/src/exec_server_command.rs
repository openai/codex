//! Command-line startup for `codex exec-server`.
//!
//! Transport, configuration, authentication, and shutdown are kept together.

use std::sync::Arc;

use clap::Parser;
use codex_arg0::Arg0DispatchPaths;
use codex_cloud_config::cloud_config_bundle_loader_for_storage;
use codex_config::LoaderOverrides;
use codex_core::config::Config;
use codex_core::config::ConfigBuilder;
use codex_core::config::ConfigLoadOptions;
use codex_core::config::bootstrap_auth_config;
use codex_core::config::find_codex_home;
use codex_core::config::load_config_toml_with_layer_stack;
use codex_exec_server::ExecServerRuntimePaths;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_login::is_workload_identity_selected;
use codex_login::read_codex_access_token_from_env;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_cli::CliConfigOverrides;
use codex_websocket_auth::WebsocketAuthArgs;

use crate::exec_server_auth;
use crate::exec_server_telemetry;

/// [EXPERIMENTAL] Run the standalone exec-server service.
#[derive(Debug, Parser)]
pub(super) struct ExecServerCommand {
    #[command(subcommand)]
    pub(super) command: Option<ExecServerSubcommand>,

    /// Error out when config.toml contains fields that are not recognized by this version of Codex.
    #[arg(
        id = "exec_server_strict_config",
        long = "strict-config",
        default_value_t = false,
        global = true
    )]
    pub(super) strict_config: bool,

    /// Linux PID namespace: isolate (default) or inherit. Inherit allows signals to
    /// other same-UID processes; enable only when provisioning a dedicated environment.
    #[arg(long, value_name = "MODE", default_value = "isolate", global = true)]
    linux_sandbox_pid_namespace: codex_sandboxing::LinuxSandboxPidNamespace,

    /// Maximum number of requests to process concurrently on each connection.
    #[arg(
        long = "concurrent-requests",
        value_name = "COUNT",
        default_value = "1"
    )]
    request_dispatch_mode: codex_exec_server::RequestDispatchMode,

    /// Transport endpoint URL. Supported values: `ws://IP:PORT` (default), `stdio`, `stdio://`.
    #[arg(
        long = "listen",
        value_name = "URL",
        conflicts_with = "exec_server_remote"
    )]
    listen: Option<String>,

    #[command(flatten)]
    websocket_auth: WebsocketAuthArgs,

    /// Register this exec-server as a remote environment using the given base URL.
    #[arg(
        long = "remote",
        id = "exec_server_remote",
        value_name = "URL",
        requires = "environment_id",
        global = true
    )]
    pub(super) remote: Option<String>,

    /// Transport used for the remote executor connection.
    #[arg(
        long = "remote-transport",
        value_enum,
        default_value_t = ExecServerRemoteTransport::Noise,
        requires = "exec_server_remote",
        requires_if("direct", "aws_sigv4"),
        global = true
    )]
    pub(super) remote_transport: ExecServerRemoteTransport,

    /// Environment id to attach to when registering remotely.
    #[arg(long = "environment-id", value_name = "ID", global = true)]
    pub(super) environment_id: Option<String>,

    /// Human-readable environment name.
    #[arg(long = "name", value_name = "NAME", global = true)]
    pub(super) name: Option<String>,

    /// Use Agent Identity auth from CODEX_ACCESS_TOKEN for remote registration.
    #[arg(
        long = "use-agent-identity-auth",
        requires = "exec_server_remote",
        conflicts_with = "aws_sigv4",
        global = true
    )]
    pub(super) use_agent_identity_auth: bool,

    /// Sign Direct registration and WebSocket handshake requests with AWS SigV4.
    #[arg(long = "aws-sigv4", requires = "exec_server_remote", global = true)]
    pub(super) aws_sigv4: bool,

    /// AWS profile used for SigV4 authentication.
    #[arg(
        long = "aws-profile",
        value_name = "PROFILE",
        requires = "aws_sigv4",
        global = true
    )]
    pub(super) aws_profile: Option<String>,

    /// AWS signing region. Uses the SDK region chain when omitted.
    #[arg(
        long = "aws-region",
        value_name = "REGION",
        requires = "aws_sigv4",
        global = true
    )]
    pub(super) aws_region: Option<String>,

    /// AWS signing service.
    #[arg(
        long = "aws-service",
        value_name = "SERVICE",
        default_value = "execute-api",
        requires = "aws_sigv4",
        global = true
    )]
    pub(super) aws_service: String,

    /// Exit when the parent-owned standard-input pipe closes.
    #[arg(
        long = "exit-on-stdin-close",
        env = codex_exec_server::CODEX_EXEC_SERVER_EXIT_ON_STDIN_CLOSE_ENV_VAR,
        requires_if("true", "exec_server_remote"),
        global = true
    )]
    exit_on_stdin_close: bool,
}

impl ExecServerCommand {
    /// Starts the executor with the given runtime paths and configuration overrides.
    pub(super) async fn run(
        mut self,
        arg0_paths: &Arg0DispatchPaths,
        root_config_overrides: &CliConfigOverrides,
    ) -> anyhow::Result<()> {
        let strict_config = self.strict_config;
        self.validate_remote_transport()?;
        let websocket_auth = self.websocket_auth.try_into_settings()?;
        if websocket_auth.config.is_some() && (self.remote.is_some() || self.command.is_some()) {
            anyhow::bail!("WebSocket listener auth cannot be used with --remote or forward");
        }
        let codex_self_exe = arg0_paths
            .codex_self_exe
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Codex executable path is not configured"))?;
        let runtime_paths = ExecServerRuntimePaths::new(
            codex_self_exe,
            arg0_paths.codex_linux_sandbox_exe.clone(),
        )?
        .with_linux_sandbox_pid_namespace(self.linux_sandbox_pid_namespace);
        if let Some(base_url) = self.remote.take() {
            let environment_id = self.environment_id.take().ok_or_else(|| {
                anyhow::anyhow!("--environment-id is required when --remote is set")
            })?;
            let config = load_exec_server_config(
                root_config_overrides,
                strict_config,
                /*enable_workload_identity*/ true,
            )
            .await?;
            let direct_transport = self.remote_transport == ExecServerRemoteTransport::Direct;
            let (_otel, telemetry) = exec_server_telemetry::init(Some(&config));
            let auth_provider = if self.aws_sigv4 {
                exec_server_auth::aws_sigv4_auth_provider(
                    codex_aws_auth::AwsAuthConfig {
                        profile: self.aws_profile,
                        region: self.aws_region,
                        service: self.aws_service,
                    },
                    config.http_client_factory(),
                )
                .await?
            } else {
                load_exec_server_remote_auth_provider(
                    &config,
                    &base_url,
                    self.use_agent_identity_auth,
                )
                .await?
            };
            let mut remote_config = codex_exec_server::RemoteEnvironmentConfig::new_with_transport(
                base_url,
                environment_id,
                if direct_transport {
                    codex_exec_server::RemoteEnvironmentTransport::Direct
                } else {
                    codex_exec_server::RemoteEnvironmentTransport::Noise
                },
                auth_provider,
                config.http_client_factory(),
            )?;
            if let Some(name) = self.name {
                remote_config.name = name;
            }
            remote_config.request_dispatch_mode = self.request_dispatch_mode;
            let remote_config = remote_config.with_telemetry(telemetry);
            let parent_lifetime = if self.exit_on_stdin_close {
                exec_server_telemetry::ParentLifetime::StdinPipe
            } else {
                exec_server_telemetry::ParentLifetime::Independent
            };
            let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();
            #[cfg(target_os = "macos")]
            let runtime_paths = runtime_paths.with_allowed_symlinked_codex_home(
                codex_config::allowed_symlinked_codex_home(
                    &config.config_layer_stack,
                    &config.codex_home,
                ),
            );
            exec_server_telemetry::run_until_shutdown(
                async move {
                    let shutdown = async move {
                        let _ = shutdown_receiver.await;
                    };
                    match self.command {
                        Some(ExecServerSubcommand::Forward { connect }) => {
                            codex_exec_server::run_remote_environment_forward_until_shutdown(
                                remote_config,
                                connect,
                                shutdown,
                            )
                            .await
                        }
                        None => {
                            codex_exec_server::run_remote_environment_until_shutdown(
                                remote_config,
                                runtime_paths,
                                shutdown,
                            )
                            .await
                        }
                    }
                    .map_err(anyhow::Error::new)
                },
                parent_lifetime,
                exec_server_telemetry::ShutdownBehavior::Graceful(shutdown_sender),
            )
            .await
        } else {
            let config_result = load_exec_server_config(
                root_config_overrides,
                strict_config,
                /*enable_workload_identity*/ false,
            )
            .await;
            let config = if strict_config {
                Some(config_result?)
            } else {
                config_result.ok()
            };
            let (_otel, telemetry) = exec_server_telemetry::init(config.as_ref());
            #[cfg(target_os = "macos")]
            let runtime_paths = runtime_paths.with_allowed_symlinked_codex_home(
                config.as_ref().and_then(|config| {
                    codex_config::allowed_symlinked_codex_home(
                        &config.config_layer_stack,
                        &config.codex_home,
                    )
                }),
            );
            let http_client_factory = config
                .as_ref()
                .map(Config::http_client_factory)
                .unwrap_or_else(|| HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault));
            let listen_url = self
                .listen
                .unwrap_or_else(|| codex_exec_server::DEFAULT_LISTEN_URL.to_string());
            let run = exec_server_telemetry::run_until_shutdown(
                codex_exec_server::run_main_with_telemetry(
                    &listen_url,
                    runtime_paths,
                    telemetry,
                    http_client_factory,
                    self.request_dispatch_mode,
                    websocket_auth,
                ),
                exec_server_telemetry::ParentLifetime::Independent,
                exec_server_telemetry::ShutdownBehavior::Immediate,
            );
            run.await.map_err(anyhow::Error::from_boxed)
        }
    }

    pub(super) fn validate_remote_transport(&self) -> anyhow::Result<()> {
        match (self.remote_transport, self.aws_sigv4) {
            (ExecServerRemoteTransport::Noise, true) => {
                anyhow::bail!("--aws-sigv4 requires --remote-transport direct");
            }
            (ExecServerRemoteTransport::Direct, false) => {
                anyhow::bail!("--remote-transport direct requires --aws-sigv4");
            }
            (ExecServerRemoteTransport::Noise, false)
            | (ExecServerRemoteTransport::Direct, true) => {}
        }
        if self.remote_transport == ExecServerRemoteTransport::Direct
            && matches!(
                self.command.as_ref(),
                Some(ExecServerSubcommand::Forward { .. })
            )
        {
            anyhow::bail!("direct exec-server transport does not support forwarding");
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
pub(super) enum ExecServerRemoteTransport {
    #[default]
    Noise,
    Direct,
}

#[derive(Debug, clap::Subcommand)]
pub(super) enum ExecServerSubcommand {
    /// Register an existing WebSocket exec-server as a remote environment.
    Forward {
        /// Destination exec-server WebSocket URL.
        #[arg(long, value_name = "URL", requires = "exec_server_remote")]
        connect: String,
    },
}

async fn load_exec_server_remote_auth_provider(
    config: &codex_core::config::Config,
    base_url: &str,
    use_agent_identity_auth: bool,
) -> anyhow::Result<codex_api::SharedAuthProvider> {
    if use_agent_identity_auth {
        read_codex_access_token_from_env().ok_or_else(|| {
            anyhow::anyhow!("CODEX_ACCESS_TOKEN is required when --use-agent-identity-auth is set")
        })?;
        let auth = AuthManager::shared_from_config(config, /*enable_codex_api_key_env*/ false)
            .await?
            .auth()
            .await
            .ok_or_else(|| anyhow::anyhow!("Agent Identity authentication is unavailable"))?;
        if !matches!(auth, CodexAuth::AgentIdentity(_)) {
            anyhow::bail!(
                "CODEX_ACCESS_TOKEN did not provide permitted Agent Identity authentication"
            );
        }
        return Ok(codex_model_provider::auth_provider_from_auth(&auth));
    }

    let (auth_manager, auth) = load_exec_server_remote_auth(
        config,
        "remote exec-server registration requires ChatGPT authentication or API key authentication; run `codex login` or set CODEX_API_KEY",
    )
    .await?;

    if !is_supported_exec_server_remote_auth(&auth) {
        anyhow::bail!(
            "remote exec-server registration requires ChatGPT authentication or API key authentication; Agent Identity auth requires --use-agent-identity-auth"
        );
    }

    if auth.is_api_key_auth() {
        validate_api_key_remote_host(base_url)?;
    }

    if auth_manager.is_workload_identity_selected() {
        Ok(codex_model_provider::auth_provider_from_auth_manager(
            auth_manager,
            &auth,
        ))
    } else {
        Ok(codex_model_provider::auth_provider_from_auth(&auth))
    }
}

pub(super) fn is_supported_exec_server_remote_auth(auth: &CodexAuth) -> bool {
    auth.is_chatgpt_auth() || auth.is_api_key_auth()
}

pub(super) fn validate_api_key_remote_host(base_url: &str) -> anyhow::Result<()> {
    let url = url::Url::parse(base_url)
        .map_err(|err| anyhow::anyhow!("invalid remote exec-server registration URL: {err}"))?;
    let host = url.host().ok_or_else(|| {
        anyhow::anyhow!("remote exec-server registration URL must include a host")
    })?;

    let is_loopback = match &host {
        url::Host::Domain(host) => host.eq_ignore_ascii_case("localhost"),
        url::Host::Ipv4(ip) => ip.is_loopback(),
        url::Host::Ipv6(ip) => ip.is_loopback(),
    };
    let is_openai_host = match &host {
        url::Host::Domain(host) => ["openai.com", "openai.org"].into_iter().any(|domain| {
            host.eq_ignore_ascii_case(domain)
                || host.to_ascii_lowercase().ends_with(&format!(".{domain}"))
        }),
        _ => false,
    };
    let is_allowed = match url.scheme() {
        "https" => is_loopback || is_openai_host,
        "http" => is_loopback,
        _ => false,
    };

    if !is_allowed {
        anyhow::bail!(
            "remote exec-server API-key authentication is restricted to HTTPS openai.com and openai.org hosts and subdomains or loopback hosts"
        );
    }

    Ok(())
}

async fn load_exec_server_config(
    root_config_overrides: &CliConfigOverrides,
    strict_config: bool,
    enable_workload_identity: bool,
) -> anyhow::Result<codex_core::config::Config> {
    let cli_kv_overrides = root_config_overrides
        .parse_overrides()
        .map_err(anyhow::Error::msg)?;
    let bootstrap_cli_overrides = cli_kv_overrides.clone();
    let mut builder = ConfigBuilder::default()
        .cli_overrides(cli_kv_overrides)
        .strict_config(strict_config);
    if enable_workload_identity && is_workload_identity_selected() {
        let codex_home = find_codex_home()?;
        let bootstrap_cwd = AbsolutePathBuf::current_dir()?;
        let bootstrap_config = load_config_toml_with_layer_stack(
            &codex_home,
            Some(&bootstrap_cwd),
            bootstrap_cli_overrides,
            ConfigLoadOptions {
                loader_overrides: LoaderOverrides::default(),
                strict_config,
                cloud_config_bundle: Default::default(),
            },
        )
        .await?;
        let bootstrap_auth_config = bootstrap_auth_config(&codex_home, &bootstrap_config)?;
        let cloud_config_bundle = cloud_config_bundle_loader_for_storage(
            bootstrap_auth_config,
            /*enable_codex_api_key_env*/ false,
        )
        .await?;
        builder = builder.cloud_config_bundle(cloud_config_bundle);
    }
    Ok(builder.build().await?)
}

async fn load_exec_server_remote_auth(
    config: &codex_core::config::Config,
    missing_auth_error: &'static str,
) -> anyhow::Result<(Arc<AuthManager>, codex_login::CodexAuth)> {
    let auth_manager =
        AuthManager::shared_from_config(config, /*enable_codex_api_key_env*/ true).await?;

    let auth = match auth_manager.auth().await {
        Some(auth) => auth,
        None => {
            auth_manager.reload().await;
            auth_manager
                .auth()
                .await
                .ok_or_else(|| anyhow::anyhow!(missing_auth_error))?
        }
    };

    Ok((auth_manager, auth))
}
