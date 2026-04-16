use crate::config::Config;
use codex_core_plugins::remote::RemotePluginFetchError;
use codex_core_plugins::remote::RemotePluginMutationError;
use codex_core_plugins::remote::RemotePluginServiceConfig;
use codex_core_plugins::remote::RemotePluginStatusSummary;
use codex_login::CodexAuth;
use codex_protocol::protocol::Product;

fn remote_plugin_service_config(config: &Config) -> RemotePluginServiceConfig {
    RemotePluginServiceConfig {
        chatgpt_base_url: config.chatgpt_base_url.clone(),
    }
}

pub(crate) async fn fetch_remote_plugin_status(
    config: &Config,
    auth: Option<&CodexAuth>,
) -> Result<Vec<RemotePluginStatusSummary>, RemotePluginFetchError> {
    codex_core_plugins::remote::fetch_remote_plugin_status(
        &remote_plugin_service_config(config),
        auth,
    )
    .await
}

pub(crate) async fn fetch_remote_featured_plugin_ids(
    config: &Config,
    auth: Option<&CodexAuth>,
    product: Option<Product>,
) -> Result<Vec<String>, RemotePluginFetchError> {
    codex_core_plugins::remote::fetch_remote_featured_plugin_ids(
        &remote_plugin_service_config(config),
        auth,
        product,
    )
    .await
}

pub(crate) async fn enable_remote_plugin(
    config: &Config,
    auth: Option<&CodexAuth>,
    plugin_id: &str,
) -> Result<(), RemotePluginMutationError> {
    codex_core_plugins::remote::enable_remote_plugin(
        &remote_plugin_service_config(config),
        auth,
        plugin_id,
    )
    .await
}

pub(crate) async fn uninstall_remote_plugin(
    config: &Config,
    auth: Option<&CodexAuth>,
    plugin_id: &str,
) -> Result<(), RemotePluginMutationError> {
    codex_core_plugins::remote::uninstall_remote_plugin(
        &remote_plugin_service_config(config),
        auth,
        plugin_id,
    )
    .await
}
