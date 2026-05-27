use codex_client::OutboundProxyConfig;
use codex_client::OutboundProxyMode;
use codex_config::types::NetworkConfigToml;
use codex_config::types::NetworkProxyMode;

pub(crate) fn outbound_proxy_config_from_network_config(
    network: &NetworkConfigToml,
) -> OutboundProxyConfig {
    let mode = match network.proxy_mode.unwrap_or_default() {
        NetworkProxyMode::Auto => OutboundProxyMode::Auto,
        NetworkProxyMode::Env => OutboundProxyMode::Env,
        NetworkProxyMode::System => OutboundProxyMode::System,
        NetworkProxyMode::Direct => OutboundProxyMode::Direct,
    };
    OutboundProxyConfig {
        mode,
        proxy_url: network.proxy_url.clone(),
    }
}
