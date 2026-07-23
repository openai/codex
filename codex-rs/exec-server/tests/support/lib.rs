use codex_exec_server::EnvironmentManager;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;

/// Builds a manager without environments using the legacy outbound HTTP policy.
pub fn environment_manager_without_environments() -> EnvironmentManager {
    EnvironmentManager::without_environments(HttpClientFactory::new(
        OutboundProxyPolicy::ReqwestDefault,
    ))
}
