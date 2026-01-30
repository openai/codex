use clap::Parser;
use codex_app_server::run_main;
use codex_arg0::arg0_dispatch_or_else;
use codex_common::CliConfigOverrides;
use codex_core::config_loader::LoaderOverrides;
use std::path::PathBuf;

// Debug-only test hook: lets integration tests point the server at a temporary
// managed config file without writing to /etc.
const MANAGED_CONFIG_PATH_ENV_VAR: &str = "CODEX_APP_SERVER_MANAGED_CONFIG_PATH";

#[derive(Debug, Parser, Default, Clone)]
#[command(bin_name = "codex-app-server")]
struct AppServerCli {
    #[clap(flatten)]
    config_overrides: CliConfigOverrides,

    /// Seed ChatGPT proxy auth (tokenless) on startup when no auth is present.
    #[arg(long = "default-chatgpt-proxy-auth")]
    default_chatgpt_proxy_auth: bool,
}

fn main() -> anyhow::Result<()> {
    arg0_dispatch_or_else(|codex_linux_sandbox_exe| async move {
        let cli = AppServerCli::parse();
        let managed_config_path = managed_config_path_from_debug_env();
        let loader_overrides = LoaderOverrides {
            managed_config_path,
            ..Default::default()
        };

        run_main(
            codex_linux_sandbox_exe,
            cli.config_overrides,
            loader_overrides,
            false,
            cli.default_chatgpt_proxy_auth,
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
