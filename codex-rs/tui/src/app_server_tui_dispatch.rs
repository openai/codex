use std::future::Future;

use crate::Cli;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::features::Feature;

pub(crate) fn app_server_tui_config_inputs(
    cli: &Cli,
) -> std::io::Result<(Vec<(String, toml::Value)>, ConfigOverrides)> {
    let mut raw_overrides = cli.config_overrides.raw_overrides.clone();
    if cli.web_search {
        raw_overrides.push("web_search=\"live\"".to_string());
    }

    let cli_kv_overrides = codex_utils_cli::CliConfigOverrides { raw_overrides }
        .parse_overrides()
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))?;

    let config_overrides = ConfigOverrides {
        cwd: cli.cwd.clone(),
        config_profile: cli.config_profile.clone(),
        ..Default::default()
    };

    Ok((cli_kv_overrides, config_overrides))
}

pub(crate) async fn should_use_app_server_tui_with<F, Fut>(
    cli: &Cli,
    load_config: F,
) -> std::io::Result<bool>
where
    F: FnOnce(Vec<(String, toml::Value)>, ConfigOverrides) -> Fut,
    Fut: Future<Output = std::io::Result<Config>>,
{
    let (cli_kv_overrides, config_overrides) = app_server_tui_config_inputs(cli)?;
    let config = load_config(cli_kv_overrides, config_overrides).await?;

    Ok(config.features.enabled(Feature::TuiAppServer))
}

pub async fn should_use_app_server_tui(cli: &Cli) -> std::io::Result<bool> {
    should_use_app_server_tui_with(cli, Config::load_with_cli_overrides_and_harness_overrides).await
}
