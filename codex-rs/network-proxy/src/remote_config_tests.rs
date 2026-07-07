use pretty_assertions::assert_eq;

use super::RemoteNetworkProxyConfig;
use crate::NetworkMode;
use crate::NetworkProxyConfig;

#[test]
fn round_trip_preserves_supported_effective_settings() {
    let mut config = NetworkProxyConfig::default();
    config.network.enabled = true;
    config.network.enable_socks5 = false;
    config.network.enable_socks5_udp = false;
    config.network.allow_upstream_proxy = false;
    config.network.dangerously_allow_all_unix_sockets = true;
    config.network.mode = NetworkMode::Limited;
    config
        .network
        .set_allowed_domains(vec!["example.com".into()]);
    config
        .network
        .set_denied_domains(vec!["blocked.example.com".into()]);
    config
        .network
        .set_allow_unix_sockets(vec!["/var/run/example.sock".into()]);
    config.network.allow_local_binding = true;

    let remote =
        RemoteNetworkProxyConfig::from_effective_config(&config).expect("supported remote config");
    let round_trip = remote.into_network_proxy_config();

    assert_eq!(round_trip, config);
}

#[test]
fn rejects_mitm_configuration() {
    let mut config = NetworkProxyConfig::default();
    config.network.mitm = true;

    let error = RemoteNetworkProxyConfig::from_effective_config(&config)
        .expect_err("MITM must not cross the remote executor boundary");

    assert_eq!(
        error.to_string(),
        "remote exec-server network proxy does not support MITM, credential injection, or MITM hooks"
    );
}
