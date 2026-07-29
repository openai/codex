use anyhow::Context;
use anyhow::Result;
use codex_cloud_config::cloud_config_bundle_loader_for_storage;
use codex_config::CloudConfigBundleLoader;
use codex_config::ConfigLoadOptions;
use codex_core::config::Config;
use codex_core::config::ConfigBuilder;
use codex_core::config::LoaderOverrides;
use codex_core::config::find_codex_home;
use codex_core::config::load_config_toml_with_layer_stack;
use codex_core::config::resolve_bootstrap_auth_keyring_backend_kind;
use codex_core::config::resolve_bootstrap_auth_route_config;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_cli::CliConfigOverrides;

pub(super) async fn load_mcp_config(
    config_overrides: &CliConfigOverrides,
    loader_overrides: LoaderOverrides,
) -> Result<Config> {
    let cli_overrides = config_overrides
        .parse_overrides()
        .map_err(anyhow::Error::msg)?;
    let codex_home = find_codex_home().context("failed to resolve CODEX_HOME")?;
    let cwd = AbsolutePathBuf::current_dir().context("failed to resolve current directory")?;
    let bootstrap_config = load_config_toml_with_layer_stack(
        codex_home.as_path(),
        Some(&cwd),
        cli_overrides.clone(),
        ConfigLoadOptions {
            loader_overrides: loader_overrides.clone(),
            strict_config: false,
            cloud_config_bundle: CloudConfigBundleLoader::default(),
        },
    )
    .await
    .context("failed to load bootstrap configuration")?;
    let bootstrap_config_toml = &bootstrap_config.config_toml;
    let auth_route_config = resolve_bootstrap_auth_route_config(
        bootstrap_config_toml,
        bootstrap_config
            .config_layer_stack
            .requirements()
            .feature_requirements
            .as_ref(),
    )
    .context("failed to resolve cloud configuration authentication")?;
    let cloud_config_bundle = cloud_config_bundle_loader_for_storage(
        codex_home.to_path_buf(),
        /*enable_codex_api_key_env*/ false,
        bootstrap_config_toml
            .cli_auth_credentials_store
            .unwrap_or_default(),
        resolve_bootstrap_auth_keyring_backend_kind(&bootstrap_config)
            .context("failed to resolve cloud configuration credential storage")?,
        bootstrap_config_toml
            .chatgpt_base_url
            .clone()
            .unwrap_or_else(|| "https://chatgpt.com/backend-api/".to_string()),
        auth_route_config,
    )
    .await;

    ConfigBuilder::default()
        .codex_home(codex_home.to_path_buf())
        .cli_overrides(cli_overrides)
        .loader_overrides(loader_overrides)
        .cloud_config_bundle(cloud_config_bundle)
        .build()
        .await
        .context("failed to load configuration")
}
