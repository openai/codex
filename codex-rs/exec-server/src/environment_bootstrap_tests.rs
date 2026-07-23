use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use pretty_assertions::assert_eq;

use super::PreparedEnvironmentManager;
use super::PreparedEnvironmentSource;
use crate::DefaultEnvironmentProvider;
use crate::Environment;
use crate::EnvironmentConnectionState;
use crate::ExecServerRuntimePaths;
use crate::LOCAL_ENVIRONMENT_ID;
use crate::REMOTE_ENVIRONMENT_ID;
use crate::environment_provider::EnvironmentDefault;
use crate::environment_provider::EnvironmentProviderSnapshot;

#[test]
fn prepared_remote_environment_is_detected_without_starting_a_connection() {
    let environment = Environment::create_for_tests(Some("ws://127.0.0.1:8765".to_string()))
        .expect("remote environment");
    let connection_state = environment
        .subscribe_connection_state()
        .expect("remote connection state");
    let prepared = PreparedEnvironmentManager {
        source: PreparedEnvironmentSource::Snapshot(EnvironmentProviderSnapshot {
            environments: vec![(REMOTE_ENVIRONMENT_ID.to_string(), environment)],
            default: EnvironmentDefault::EnvironmentId(REMOTE_ENVIRONMENT_ID.to_string()),
            include_local: false,
        }),
    };

    assert!(prepared.default_environment_is_remote());
    assert_eq!(
        *connection_state.borrow(),
        EnvironmentConnectionState::Disconnected
    );
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
            DefaultEnvironmentProvider::new(/*exec_server_url*/ None).snapshot_inner(),
        ),
    };
    let runtime_paths = ExecServerRuntimePaths::new(
        std::env::current_exe().expect("current exe"),
        /*codex_linux_sandbox_exe*/ None,
    )
    .expect("runtime paths");
    let manager = prepared
        .build(
            Some(runtime_paths),
            HttpClientFactory::new(OutboundProxyPolicy::RespectSystemProxy),
        )
        .expect("environment manager");

    assert_eq!(
        manager.http_client_factory().outbound_proxy_policy(),
        OutboundProxyPolicy::RespectSystemProxy
    );
}
