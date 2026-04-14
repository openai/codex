use clap::Parser;
use codex_app_server::AppServerTransport;
use codex_app_server::AppServerWebsocketAuthArgs;
use codex_app_server::run_main_with_transport;
use codex_arg0::Arg0DispatchPaths;
use codex_arg0::arg0_dispatch_or_else;
use codex_core::config_loader::LoaderOverrides;
use codex_protocol::protocol::SessionSource;
use codex_utils_cli::CliConfigOverrides;

// Debug-only test hook: lets integration tests disable host-managed config or
// point the server at a temporary managed config file without writing to /etc.
#[cfg(debug_assertions)]
const MANAGED_CONFIG_PATH_ENV_VAR: &str = "CODEX_APP_SERVER_MANAGED_CONFIG_PATH";
#[cfg(debug_assertions)]
const DISABLE_MANAGED_CONFIG_ENV_VAR: &str = "CODEX_APP_SERVER_DISABLE_MANAGED_CONFIG";

#[derive(Debug, Parser)]
struct AppServerArgs {
    /// Transport endpoint URL. Supported values: `stdio://` (default),
    /// `ws://IP:PORT`, `off`.
    #[arg(
        long = "listen",
        value_name = "URL",
        default_value = AppServerTransport::DEFAULT_LISTEN_URL
    )]
    listen: AppServerTransport,

    /// Session source used to derive product restrictions and metadata.
    #[arg(
        long = "session-source",
        value_name = "SOURCE",
        default_value = "vscode",
        value_parser = SessionSource::from_startup_arg
    )]
    session_source: SessionSource,

    #[command(flatten)]
    auth: AppServerWebsocketAuthArgs,
}

fn main() -> anyhow::Result<()> {
    arg0_dispatch_or_else(|arg0_paths: Arg0DispatchPaths| async move {
        let args = AppServerArgs::parse();
        let loader_overrides = loader_overrides_from_debug_env();
        let transport = args.listen;
        let session_source = args.session_source;
        let auth = args.auth.try_into_settings()?;

        run_main_with_transport(
            arg0_paths,
            CliConfigOverrides::default(),
            loader_overrides,
            /*default_analytics_enabled*/ false,
            transport,
            session_source,
            auth,
        )
        .await?;
        Ok(())
    })
}

fn loader_overrides_from_debug_env() -> LoaderOverrides {
    #[cfg(debug_assertions)]
    {
        if disable_managed_config_from_debug_env() {
            return LoaderOverrides::without_managed_config_for_tests();
        }
        if let Ok(value) = std::env::var(MANAGED_CONFIG_PATH_ENV_VAR) {
            return if value.is_empty() {
                LoaderOverrides::without_managed_config_for_tests()
            } else {
                LoaderOverrides {
                    managed_config_path: Some(std::path::PathBuf::from(value)),
                    ..Default::default()
                }
            };
        }
    }

    LoaderOverrides::default()
}

#[cfg(debug_assertions)]
fn disable_managed_config_from_debug_env() -> bool {
    if let Ok(value) = std::env::var(DISABLE_MANAGED_CONFIG_ENV_VAR) {
        return matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES");
    }

    false
}
