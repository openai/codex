use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use pretty_assertions::assert_eq;

use super::PreparedEnvironmentManager;
use super::PreparedEnvironmentSource;
use crate::DefaultEnvironmentProvider;
use crate::LOCAL_ENVIRONMENT_ID;
use crate::REMOTE_ENVIRONMENT_ID;
use crate::client_api::DEFAULT_REMOTE_EXEC_SERVER_CONNECT_TIMEOUT;
use crate::client_api::ExecServerTransportParams;
use crate::environment_provider::EnvironmentDefault;
use crate::environment_provider::EnvironmentProviderSnapshot;
use crate::remote::NoiseRendezvousEnvironmentConfig;

#[test]
fn prepared_remote_environment_is_detected_without_constructing_a_connection() {
    let prepared = PreparedEnvironmentManager {
        source: PreparedEnvironmentSource::Snapshot(EnvironmentProviderSnapshot {
            environments: vec![(
                REMOTE_ENVIRONMENT_ID.to_string(),
                ExecServerTransportParams::websocket_url(
                    "ws://username:password@executor.example/private?token=secret".to_string(),
                    DEFAULT_REMOTE_EXEC_SERVER_CONNECT_TIMEOUT,
                ),
            )],
            default: EnvironmentDefault::EnvironmentId(REMOTE_ENVIRONMENT_ID.to_string()),
            include_local: false,
        }),
    };

    assert!(prepared.default_environment_is_remote());

    let debug = format!("{prepared:?}");
    assert!(!debug.contains("username"));
    assert!(!debug.contains("password"));
    assert!(!debug.contains("executor.example"));
    assert!(!debug.contains("secret"));
}

#[test]
fn prepared_noise_environment_is_detected_before_http_policy_is_resolved() {
    let config = NoiseRendezvousEnvironmentConfig::new(
        "https://registry-user:registry-password@registry.example/api?access_token=query-secret#fragment-secret"
            .to_string(),
        "environment-requested".to_string(),
        "registry-token".to_string(),
        Some("workspace-123".to_string()),
    )
    .expect("Noise environment configuration");
    let prepared = PreparedEnvironmentManager {
        source: PreparedEnvironmentSource::Noise(config),
    };

    assert!(prepared.default_environment_is_remote());

    let debug = format!("{prepared:?}");
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("registry-token"));
    assert!(!debug.contains("workspace-123"));
    assert!(!debug.contains("registry-user"));
    assert!(!debug.contains("registry-password"));
    assert!(!debug.contains("registry.example"));
    assert!(!debug.contains("query-secret"));
    assert!(!debug.contains("fragment-secret"));
}

#[test]
fn prepared_noise_environment_rejects_invalid_configuration() {
    let invalid_configs = [
        ("", "environment-requested", "registry-token", None),
        ("https://registry.example", "", "registry-token", None),
        (
            "https://registry.example",
            "environment-requested",
            "",
            None,
        ),
        (
            "https://registry.example",
            "environment-requested",
            "registry\ntoken",
            None,
        ),
        (
            "https://registry.example",
            "environment-requested",
            "registry-token",
            Some("workspace\n123"),
        ),
    ];

    for (registry_url, environment_id, auth_token, chatgpt_account_id) in invalid_configs {
        let result = NoiseRendezvousEnvironmentConfig::new(
            registry_url.to_string(),
            environment_id.to_string(),
            auth_token.to_string(),
            chatgpt_account_id.map(str::to_string),
        );

        assert!(result.is_err());
    }
}

#[test]
fn prepared_local_and_disabled_environments_are_not_remote() {
    let local = PreparedEnvironmentManager {
        source: PreparedEnvironmentSource::Snapshot(EnvironmentProviderSnapshot {
            environments: Vec::new(),
            default: EnvironmentDefault::EnvironmentId(LOCAL_ENVIRONMENT_ID.to_string()),
            include_local: true,
        }),
    };
    let disabled = PreparedEnvironmentManager {
        source: PreparedEnvironmentSource::Snapshot(EnvironmentProviderSnapshot {
            environments: Vec::new(),
            default: EnvironmentDefault::Disabled,
            include_local: false,
        }),
    };

    assert_eq!(
        [
            local.default_environment_is_remote(),
            disabled.default_environment_is_remote()
        ],
        [false, false]
    );
}

#[tokio::test]
async fn prepared_environment_manager_builds_with_the_explicit_http_policy() {
    let prepared = PreparedEnvironmentManager {
        source: PreparedEnvironmentSource::Snapshot(
            DefaultEnvironmentProvider::new(Some("ws://127.0.0.1:8765".to_string()))
                .snapshot_inner(),
        ),
    };
    let manager = prepared
        .build(
            /*local_runtime_paths*/ None,
            HttpClientFactory::new(OutboundProxyPolicy::RespectSystemProxy),
        )
        .expect("environment manager");

    assert_eq!(
        manager.default_environment_id(),
        Some(REMOTE_ENVIRONMENT_ID)
    );
    assert!(
        manager
            .default_environment()
            .expect("remote environment")
            .is_remote()
    );
    assert_eq!(
        manager.http_client_factory().outbound_proxy_policy(),
        OutboundProxyPolicy::RespectSystemProxy
    );
}
