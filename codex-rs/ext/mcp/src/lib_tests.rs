use pretty_assertions::assert_eq;

use super::AppsMcpServerTarget;
use super::apps_mcp_server_target;

#[test]
fn default_uses_hosted_plugin_runtime() {
    assert_eq!(
        apps_mcp_server_target(
            "https://chatgpt.com",
            /*apps_mcp_base_url_override*/ None,
        ),
        AppsMcpServerTarget::HostedPluginRuntime("https://chatgpt.com"),
    );
}

#[test]
fn empty_override_uses_hosted_plugin_runtime() {
    assert_eq!(
        apps_mcp_server_target("https://chatgpt.com", Some("  ")),
        AppsMcpServerTarget::HostedPluginRuntime("https://chatgpt.com"),
    );
}

#[test]
fn explicit_override_uses_local_codex_apps_endpoint() {
    assert_eq!(
        apps_mcp_server_target("https://chatgpt.com", Some("http://127.0.0.1:8061"),),
        AppsMcpServerTarget::CodexApps("http://127.0.0.1:8061"),
    );
}
