use super::*;
use codex_app_server_protocol::PluginSearchParams;
use codex_app_server_protocol::PluginSearchResponse;
use codex_app_server_protocol::PluginSearchResult;
use codex_app_server_protocol::PluginSearchScope;
use codex_core_plugins::remote::RemotePluginSearchRequest;
use codex_core_plugins::remote::search_remote_plugins;

const DEFAULT_PLUGIN_SEARCH_LIMIT: u32 = 16;
const MAX_PLUGIN_SEARCH_LIMIT: u32 = 1_000;

impl PluginRequestProcessor {
    pub(crate) async fn plugin_search(
        &self,
        params: PluginSearchParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.plugin_search_response(params)
            .await
            .map(|response| Some(response.into()))
    }

    async fn plugin_search_response(
        &self,
        params: PluginSearchParams,
    ) -> Result<PluginSearchResponse, JSONRPCErrorError> {
        let PluginSearchParams {
            search_term,
            scope,
            cwds: _,
            cursor,
            limit,
        } = params;
        let search_term = search_term.trim();
        let empty_response = || PluginSearchResponse {
            data: Vec::new(),
            next_cursor: None,
        };
        if search_term.is_empty() {
            return Ok(empty_response());
        }

        let config = self.load_latest_config(/*fallback_cwd*/ None).await?;
        if !config.features.enabled(Feature::Plugins) {
            return Ok(empty_response());
        }
        let scope = if config.features.enabled(Feature::RemotePlugin) {
            scope
        } else {
            match scope {
                None | Some(PluginSearchScope::Workspace) => Some(PluginSearchScope::Workspace),
                Some(PluginSearchScope::Global | PluginSearchScope::Personal) => {
                    return Ok(empty_response());
                }
            }
        };
        let plugin_sharing_enabled = config.features.enabled(Feature::PluginSharing);

        let auth = self.auth_manager.auth().await;
        if !self
            .workspace_codex_plugins_enabled(&config, auth.as_ref())
            .await
            || !auth
                .as_ref()
                .map(CodexAuth::api_auth_mode)
                .is_some_and(DomainAuthMode::uses_codex_backend)
        {
            return Ok(empty_response());
        }

        let scope = scope.map(|scope| match scope {
            PluginSearchScope::Global => RemotePluginScope::Global,
            PluginSearchScope::Workspace => RemotePluginScope::Workspace,
            PluginSearchScope::Personal => RemotePluginScope::User,
        });
        let limit = limit
            .unwrap_or(DEFAULT_PLUGIN_SEARCH_LIMIT)
            .clamp(1, MAX_PLUGIN_SEARCH_LIMIT);
        let page = search_remote_plugins(
            &remote_plugin_service_config(&config),
            auth.as_ref(),
            RemotePluginSearchRequest {
                query: search_term,
                scope,
                limit,
                page_token: cursor.as_deref(),
            },
        )
        .await
        .map_err(|err| {
            remote_plugin_catalog_error_to_jsonrpc(err, "search remote plugin catalog")
        })?;

        let next_cursor = page.next_page_token;
        let mut data = Vec::with_capacity(page.plugins.len());
        for plugin in page.plugins {
            let plugin_id = PluginId::parse(&plugin.id).map_err(|err| {
                internal_error(format!("invalid remote plugin search result id: {err}"))
            })?;

            // NOTE: (brisebois) filter out plugins from the results that belong to "shared"
            // marketplaces if plugin sharing is disabled. There is a chance that this filters
            // out all results and returns an empty list to the client. Ideally this filtering
            // would be done server-side to avoid this problem.
            if !plugin_sharing_enabled
                && matches!(
                    plugin_id.marketplace_name.as_str(),
                    REMOTE_WORKSPACE_SHARED_WITH_ME_MARKETPLACE_NAME
                        | REMOTE_WORKSPACE_SHARED_WITH_ME_PRIVATE_MARKETPLACE_NAME
                        | REMOTE_WORKSPACE_SHARED_WITH_ME_UNLISTED_MARKETPLACE_NAME
                )
            {
                continue;
            }
            data.push(PluginSearchResult {
                plugin: remote_plugin_summary_to_info(plugin),
                marketplace_name: plugin_id.marketplace_name,
                marketplace_path: None,
            });
        }

        Ok(PluginSearchResponse { data, next_cursor })
    }
}
