use std::collections::HashMap;
use std::sync::Arc;

use codex_app_server_protocol::McpServerOauthLoginCompletedNotification;
use codex_app_server_protocol::ServerNotification;
use codex_config::types::McpServerConfig;
use codex_core::config::Config;
use codex_mcp::mcp::auth::McpOAuthLoginSupport;
use codex_mcp::mcp::auth::oauth_login_support;
use codex_mcp::mcp::auth::resolve_oauth_scopes;
use codex_mcp::mcp::auth::should_retry_without_scopes;
use codex_rmcp_client::perform_oauth_login_silent;
use tracing::warn;

use super::CodexMessageProcessor;

impl CodexMessageProcessor {
    pub(super) async fn start_plugin_mcp_oauth_logins(
        &self,
        config: &Config,
        plugin_mcp_servers: HashMap<String, McpServerConfig>,
    ) {
        for (name, server) in plugin_mcp_servers {
            let oauth_config = match oauth_login_support(&server.transport).await {
                McpOAuthLoginSupport::Supported(config) => config,
                McpOAuthLoginSupport::Unsupported => continue,
                McpOAuthLoginSupport::Unknown(err) => {
                    warn!(
                        "MCP server may or may not require login for plugin install {name}: {err}"
                    );
                    continue;
                }
            };

            let resolved_scopes = resolve_oauth_scopes(
                /*explicit_scopes*/ None,
                server.scopes.clone(),
                oauth_config.discovered_scopes.clone(),
            );

            let store_mode = config.mcp_oauth_credentials_store_mode;
            let callback_port = config.mcp_oauth_callback_port;
            let callback_url = config.mcp_oauth_callback_url.clone();
            let outgoing = Arc::clone(&self.outgoing);
            let notification_name = name.clone();

            tokio::spawn(async move {
                let first_attempt = perform_oauth_login_silent(
                    &name,
                    &oauth_config.url,
                    store_mode,
                    oauth_config.http_headers.clone(),
                    oauth_config.env_http_headers.clone(),
                    &resolved_scopes.scopes,
                    server.oauth_resource.as_deref(),
                    callback_port,
                    callback_url.as_deref(),
                )
                .await;

                let final_result = match first_attempt {
                    Err(err) if should_retry_without_scopes(&resolved_scopes, &err) => {
                        perform_oauth_login_silent(
                            &name,
                            &oauth_config.url,
                            store_mode,
                            oauth_config.http_headers,
                            oauth_config.env_http_headers,
                            &[],
                            server.oauth_resource.as_deref(),
                            callback_port,
                            callback_url.as_deref(),
                        )
                        .await
                    }
                    result => result,
                };

                let (success, error) = match final_result {
                    Ok(()) => (true, None),
                    Err(err) => (false, Some(err.to_string())),
                };

                let notification = ServerNotification::McpServerOauthLoginCompleted(
                    McpServerOauthLoginCompletedNotification {
                        name: notification_name,
                        success,
                        error,
                    },
                );
                outgoing.send_server_notification(notification).await;
            });
        }
    }
}
