use clap::Parser;
use codex_app_server::AppServerTransport;
use codex_app_server::run_main_with_transport;
use codex_arg0::arg0_dispatch_or_else;
use codex_core::config_loader::LoaderOverrides;
use codex_utils_cli::CliConfigOverrides;
use std::path::PathBuf;

// Debug-only test hook: lets integration tests point the server at a temporary
// managed config file without writing to /etc.
const MANAGED_CONFIG_PATH_ENV_VAR: &str = "CODEX_APP_SERVER_MANAGED_CONFIG_PATH";

#[derive(Debug, Parser)]
struct AppServerArgs {
    /// Transport endpoint URL. Supported values: `stdio://` (default),
    /// `ws://IP:PORT`.
    #[arg(
        long = "listen",
        value_name = "URL",
        default_value = AppServerTransport::DEFAULT_LISTEN_URL
    )]
    listen: AppServerTransport,
}

fn main() -> anyhow::Result<()> {
    arg0_dispatch_or_else(|codex_linux_sandbox_exe| async move {
        let args = AppServerArgs::parse();
        let managed_config_path = managed_config_path_from_debug_env();
        let loader_overrides = LoaderOverrides {
            managed_config_path,
            ..Default::default()
        };
        let transport = args.listen;

        run_main_with_transport(
            codex_linux_sandbox_exe,
            CliConfigOverrides::default(),
            loader_overrides,
            false,
            transport,
        )
        .await?;
        Ok(())
    })
}

fn managed_config_path_from_debug_env() -> Option<PathBuf> {
    #[cfg(debug_assertions)]
    {
        if let Ok(value) = std::env::var(MANAGED_CONFIG_PATH_ENV_VAR) {
            return if value.is_empty() {
                None
            } else {
                Some(PathBuf::from(value))
            };
        }
    }

    None
}
