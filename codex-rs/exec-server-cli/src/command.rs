//! Command-line startup shared by `codex exec-server` and `codex-exec-server`.
//!
//! Transport, configuration, authentication, and shutdown are kept together.

use std::sync::Arc;

use clap::Parser;
use codex_arg0::Arg0DispatchPaths;
use codex_core::config::Config;
use codex_exec_server::ExecServerRuntimeOptions;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_utils_cli::CliConfigOverrides;
use codex_websocket_auth::WebsocketAuthArgs;

use crate::config::load_exec_server_config;
use crate::config::load_exec_server_remote_auth_provider;
use crate::exec_server_auth;
use crate::exec_server_credential::ExecutorCredentialPipe;
use crate::exec_server_telemetry;

/// [EXPERIMENTAL] Run the standalone exec-server service.
#[derive(Debug, Parser)]
pub struct ExecServerCommand {
    #[command(subcommand)]
    command: Option<ExecServerSubcommand>,

    /// Error out when config.toml contains fields that are not recognized by this version of Codex.
    #[arg(
        id = "exec_server_strict_config",
        long = "strict-config",
        default_value_t = false,
        global = true
    )]
    pub strict_config: bool,

    /// Linux PID namespace: isolate (default) or inherit. Inherit allows signals to
    /// other same-UID processes; enable only when provisioning a dedicated environment.
    #[arg(long, value_name = "MODE", default_value = "isolate", global = true)]
    linux_sandbox_pid_namespace: codex_sandboxing::LinuxSandboxPidNamespace,

    /// Allow permitted private IP destinations to use the configured upstream proxy.
    /// If no valid upstream proxy applies to the request protocol, connect directly.
    /// Loopback stays local. This flag does not require upstream routing.
    #[arg(
        long,
        env = "CODEX_EXEC_SERVER_PROXY_PRIVATE_IPS_VIA_UPSTREAM",
        global = true
    )]
    proxy_private_ips_via_upstream: bool,

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
    remote: Option<String>,

    /// Transport used for the remote executor connection.
    #[arg(
        long = "remote-transport",
        value_enum,
        default_value_t = ExecServerRemoteTransport::Noise,
        requires = "exec_server_remote",
        requires_if("direct", "aws_sigv4"),
        global = true
    )]
    remote_transport: ExecServerRemoteTransport,

    /// Environment id to attach to when registering remotely.
    #[arg(long = "environment-id", value_name = "ID", global = true)]
    environment_id: Option<String>,

    /// Human-readable environment name.
    #[arg(long = "name", value_name = "NAME", global = true)]
    name: Option<String>,

    /// Use Agent Identity auth from CODEX_ACCESS_TOKEN for remote registration.
    #[arg(
        long = "use-agent-identity-auth",
        requires = "exec_server_remote",
        conflicts_with = "aws_sigv4",
        global = true
    )]
    use_agent_identity_auth: bool,

    /// Read rotating executor credentials from stdin; exit when the pipe closes.
    #[arg(
        long = "executor-credential-stdin",
        requires = "exec_server_remote",
        conflicts_with_all = ["use_agent_identity_auth", "aws_sigv4"],
        global = true
    )]
    executor_credential_stdin: bool,

    /// Sign Direct registration and WebSocket handshake requests with AWS SigV4.
    #[arg(long = "aws-sigv4", requires = "exec_server_remote", global = true)]
    aws_sigv4: bool,

    /// AWS profile used for SigV4 authentication.
    #[arg(
        long = "aws-profile",
        value_name = "PROFILE",
        requires = "aws_sigv4",
        global = true
    )]
    aws_profile: Option<String>,

    /// AWS signing region. Uses the SDK region chain when omitted.
    #[arg(
        long = "aws-region",
        value_name = "REGION",
        requires = "aws_sigv4",
        global = true
    )]
    aws_region: Option<String>,

    /// AWS signing service.
    #[arg(
        long = "aws-service",
        value_name = "SERVICE",
        default_value = "execute-api",
        requires = "aws_sigv4",
        global = true
    )]
    aws_service: String,

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
    pub async fn run(
        mut self,
        arg0_paths: &Arg0DispatchPaths,
        root_config_overrides: &CliConfigOverrides,
    ) -> anyhow::Result<()> {
        let strict_config = self.strict_config;
        self.validate_remote_transport()?;
        let enable_cloud_config = self.should_load_cloud_config();
        let websocket_auth = self.websocket_auth.try_into_settings()?;
        if websocket_auth.config.is_some() && (self.remote.is_some() || self.command.is_some()) {
            anyhow::bail!("WebSocket listener auth cannot be used with --remote or forward");
        }
        let codex_self_exe = arg0_paths
            .codex_self_exe
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Codex executable path is not configured"))?;
        let mut runtime_paths = ExecServerRuntimeOptions::new(
            codex_self_exe,
            arg0_paths.codex_linux_sandbox_exe.clone(),
        )?
        .with_linux_sandbox_pid_namespace(self.linux_sandbox_pid_namespace)
        .with_proxy_private_ips_via_upstream(self.proxy_private_ips_via_upstream);
        // Config loading below owns error handling (non-strict stdio can proceed
        // without config). Never retain other startup flags in the executor.
        if let Ok(overrides) = root_config_overrides.parse_overrides() {
            let cli_flags = codex_config::build_cli_overrides_layer(&overrides);
            runtime_paths.prefer_mxc = cli_flags
                .get("features")
                .and_then(|features| features.get("prefer_mxc"))
                .and_then(toml::Value::as_bool);
        }
        if let Some(base_url) = self.remote.take() {
            let environment_id = self.environment_id.take().ok_or_else(|| {
                anyhow::anyhow!("--environment-id is required when --remote is set")
            })?;
            let config =
                load_exec_server_config(root_config_overrides, strict_config, enable_cloud_config)
                    .await?;
            let direct_transport = self.remote_transport == ExecServerRemoteTransport::Direct;
            // Configured exporters may contain account-wide credentials or client keys.
            // The scoped executor must not initialize those exporters.
            let telemetry_config = (!self.executor_credential_stdin).then_some(&config);
            let (_otel, telemetry) = exec_server_telemetry::init(telemetry_config);
            let mut credential_provider = None;
            let mut parent_lifetime = if self.exit_on_stdin_close {
                exec_server_telemetry::ParentLifetime::StdinPipe
            } else {
                exec_server_telemetry::ParentLifetime::Independent
            };
            let auth_provider: codex_api::SharedAuthProvider = if self.executor_credential_stdin {
                let ExecutorCredentialPipe {
                    provider,
                    parent_closed,
                } = ExecutorCredentialPipe::from_stdin(environment_id.trim().to_string());
                provider.wait_for_initial().await?;
                parent_lifetime =
                    exec_server_telemetry::ParentLifetime::MonitoredStdin(parent_closed);
                credential_provider = Some(Arc::clone(&provider));
                provider
            } else if self.aws_sigv4 {
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
            if self.executor_credential_stdin {
                remote_config = remote_config.with_executor_credential_auth()?;
            }
            if let Some(name) = self.name {
                remote_config.name = name;
            }
            remote_config.request_dispatch_mode = self.request_dispatch_mode;
            let remote_config = remote_config.with_telemetry(telemetry);
            let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();
            #[cfg(target_os = "macos")]
            let runtime_paths = runtime_paths.with_allowed_symlinked_codex_home(
                codex_config::allowed_symlinked_codex_home(
                    &config.config_layer_stack,
                    &config.codex_home,
                ),
            );
            let result = exec_server_telemetry::run_until_shutdown(
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
            .await;
            if let Some(provider) = credential_provider {
                provider.check_pipe_failure()?;
            }
            result
        } else {
            let config_result =
                load_exec_server_config(root_config_overrides, strict_config, enable_cloud_config)
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

    fn should_load_cloud_config(&self) -> bool {
        self.remote.is_some()
            && self.command.is_none()
            && !self.executor_credential_stdin
            && !self.aws_sigv4
            && !self.use_agent_identity_auth
    }

    fn validate_remote_transport(&self) -> anyhow::Result<()> {
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
enum ExecServerRemoteTransport {
    #[default]
    Noise,
    Direct,
}

#[derive(Debug, clap::Subcommand)]
enum ExecServerSubcommand {
    /// Register an existing WebSocket exec-server as a remote environment.
    Forward {
        /// Destination exec-server WebSocket URL.
        #[arg(long, value_name = "URL", requires = "exec_server_remote")]
        connect: String,
    },
}

#[cfg(test)]
#[path = "exec_server_args_tests.rs"]
mod tests;
