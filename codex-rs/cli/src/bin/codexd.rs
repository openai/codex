use clap::Parser;
use codex_app_server::AppServerTransport;
use codex_app_server::run_main_with_transport;
use codex_arg0::Arg0DispatchPaths;
use codex_arg0::arg0_dispatch_or_else;
use codex_core::config::Config;
use codex_core::config::ConfigBuilder;
use codex_core::config_loader::LoaderOverrides;
use codex_utils_cli::CliConfigOverrides;
use std::io::ErrorKind;
use toml::Value;

#[derive(Debug, Parser)]
#[clap(
    author,
    version,
    bin_name = "codexd",
    override_usage = "codexd [OPTIONS]"
)]
struct CodexdCli {
    #[clap(flatten)]
    config_overrides: CliConfigOverrides,
}

fn default_remote_control_url(chatgpt_base_url: &str) -> String {
    let chatgpt_base_url = chatgpt_base_url.trim_end_matches('/');
    if chatgpt_base_url.contains("/backend-api") {
        format!("{chatgpt_base_url}/wham")
    } else {
        format!("{chatgpt_base_url}/backend-api/wham")
    }
}

fn main() -> anyhow::Result<()> {
    arg0_dispatch_or_else(|arg0_paths: Arg0DispatchPaths| async move {
        let cli = CodexdCli::parse();
        let cli_kv_overrides = cli.config_overrides.parse_overrides().map_err(|err| {
            std::io::Error::new(
                ErrorKind::InvalidInput,
                format!("error parsing -c overrides: {err}"),
            )
        })?;
        let config = match ConfigBuilder::default()
            .cli_overrides(cli_kv_overrides.clone())
            .loader_overrides(LoaderOverrides::default())
            .build()
            .await
        {
            Ok(config) => config,
            Err(_err) => Config::load_default_with_cli_overrides(cli_kv_overrides).map_err(
                |fallback_err| {
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        format!("error loading default config after config error: {fallback_err}"),
                    )
                },
            )?,
        };
        let mut config_overrides = cli.config_overrides.clone();
        if config.experimental_app_server_remote_control_url.is_none() {
            config_overrides.raw_overrides.push(format!(
                "experimental_app_server_remote_control_url={}",
                Value::String(default_remote_control_url(&config.chatgpt_base_url))
            ));
        }

        run_main_with_transport(
            arg0_paths,
            config_overrides,
            LoaderOverrides::default(),
            false,
            AppServerTransport::Headless,
        )
        .await?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::CodexdCli;
    use super::default_remote_control_url;
    use clap::Parser;
    use pretty_assertions::assert_eq;

    #[test]
    fn codexd_parses_root_config_overrides() {
        let cli = CodexdCli::try_parse_from([
            "codexd",
            "-c",
            "chatgpt_base_url=\"http://localhost:10000\"",
            "-c",
            "model=\"gpt-5.1\"",
        ])
        .expect("codexd args should parse");

        assert_eq!(
            cli.config_overrides.raw_overrides,
            vec![
                "chatgpt_base_url=\"http://localhost:10000\"".to_string(),
                "model=\"gpt-5.1\"".to_string(),
            ]
        );
    }

    #[test]
    fn default_remote_control_url_adds_backend_api_wham_for_chatgpt_roots() {
        assert_eq!(
            default_remote_control_url("https://chatgpt.com"),
            "https://chatgpt.com/backend-api/wham"
        );
        assert_eq!(
            default_remote_control_url("http://localhost:10000"),
            "http://localhost:10000/backend-api/wham"
        );
    }

    #[test]
    fn default_remote_control_url_keeps_existing_backend_api_prefixes() {
        assert_eq!(
            default_remote_control_url("https://chatgpt.com/backend-api"),
            "https://chatgpt.com/backend-api/wham"
        );
        assert_eq!(
            default_remote_control_url("https://chatgpt.com/backend-api/"),
            "https://chatgpt.com/backend-api/wham"
        );
    }
}
