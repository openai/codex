use crate::legacy_core::config::Config;
use crate::legacy_core::config::ConfigBuilder;
use crate::legacy_core::config::ConfigOverrides;
use crate::session_state::SessionNetworkProxyRuntime;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::NetworkSandboxPolicy;
use pretty_assertions::assert_eq;

use super::TerminalBrowserNetworkAvailability;

async fn terminal_browser_config(managed_network: bool) -> Config {
    let codex_home = tempfile::tempdir().expect("terminal-browser config home");
    let mut cli_overrides = Vec::new();
    if managed_network {
        cli_overrides.push((
            "features.network_proxy".to_string(),
            toml::Value::Boolean(true),
        ));
    }
    ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .loader_overrides(codex_config::LoaderOverrides::without_managed_config_for_tests())
        .cli_overrides(cli_overrides)
        .harness_overrides(ConfigOverrides {
            permission_profile: Some(PermissionProfile::workspace_write_with(
                &[],
                NetworkSandboxPolicy::Enabled,
                /*exclude_tmpdir_env_var*/ false,
                /*exclude_slash_tmp*/ false,
            )),
            ..ConfigOverrides::default()
        })
        .build()
        .await
        .expect("terminal-browser config should build")
}

fn runtime(http_addr: &str, mitm: bool) -> SessionNetworkProxyRuntime {
    SessionNetworkProxyRuntime {
        http_addr: http_addr.to_string(),
        socks_addr: "127.0.0.1:43129".to_string(),
        mitm,
    }
}

#[tokio::test]
async fn direct_network_does_not_require_runtime_proxy_details() {
    let config = terminal_browser_config(/*managed_network*/ false).await;

    assert_eq!(
        TerminalBrowserNetworkAvailability::from_config_and_runtime(
            &config, /*network_proxy*/ None,
        ),
        TerminalBrowserNetworkAvailability::Direct
    );
}

#[tokio::test]
async fn managed_network_requires_a_non_mitm_loopback_runtime_proxy() {
    let config = terminal_browser_config(/*managed_network*/ true).await;
    assert!(config.permissions.network.is_some());

    assert_eq!(
        TerminalBrowserNetworkAvailability::from_config_and_runtime(
            &config, /*network_proxy*/ None,
        ),
        TerminalBrowserNetworkAvailability::ManagedProxyUnavailable
    );
    for unavailable in [
        runtime("not-an-address", /*mitm*/ false),
        runtime("192.0.2.10:43128", /*mitm*/ false),
        runtime("127.0.0.1:0", /*mitm*/ false),
    ] {
        assert_eq!(
            TerminalBrowserNetworkAvailability::from_config_and_runtime(
                &config,
                Some(&unavailable),
            ),
            TerminalBrowserNetworkAvailability::ManagedProxyUnavailable
        );
    }
    assert_eq!(
        TerminalBrowserNetworkAvailability::from_config_and_runtime(
            &config,
            Some(&runtime("127.0.0.1:43128", /*mitm*/ true)),
        ),
        TerminalBrowserNetworkAvailability::ManagedProxyMitmUnsupported
    );
    assert_eq!(
        TerminalBrowserNetworkAvailability::from_config_and_runtime(
            &config,
            Some(&runtime("127.0.0.1:43128", /*mitm*/ false)),
        ),
        TerminalBrowserNetworkAvailability::ManagedProxy {
            http_addr: "127.0.0.1:43128".parse().expect("proxy address"),
        }
    );
}
