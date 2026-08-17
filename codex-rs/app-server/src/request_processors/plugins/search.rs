use super::*;
use codex_app_server_protocol::PluginSearchParams;
use codex_app_server_protocol::PluginSearchResponse;
use codex_app_server_protocol::PluginSearchResult;
use codex_app_server_protocol::PluginSearchScope;
use codex_core_plugins::OPENAI_BUNDLED_MARKETPLACE_NAME;
use codex_core_plugins::remote::RemotePluginSearchRequest;
use codex_core_plugins::remote::search_remote_plugins;

const DEFAULT_PLUGIN_SEARCH_LIMIT: u32 = 16;
const MAX_PLUGIN_SEARCH_LIMIT: u32 = 1_000;
const MAX_LOCAL_PLUGIN_SEARCH_RESULTS: usize = 100;
const PLUGIN_SEARCH_NO_MATCH_RANK: usize = 6;

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
            cwds,
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
        let plugin_sharing_enabled = config.features.enabled(Feature::PluginSharing);

        let auth = self.auth_manager.auth().await;
        let auth_mode = auth.as_ref().map(CodexAuth::api_auth_mode);
        self.thread_manager
            .plugins_manager()
            .set_auth_mode(auth_mode);
        let remote_plugin_enabled = config.features.enabled(Feature::RemotePlugin);
        let use_remote_global_catalog =
            remote_plugin_enabled && auth_mode.is_some_and(DomainAuthMode::uses_codex_backend);
        let remote_scope = if remote_plugin_enabled {
            Some(scope.map(|scope| match scope {
                PluginSearchScope::Global => RemotePluginScope::Global,
                PluginSearchScope::Workspace => RemotePluginScope::Workspace,
                PluginSearchScope::Personal => RemotePluginScope::User,
            }))
        } else {
            match scope {
                None | Some(PluginSearchScope::Workspace) => {
                    Some(Some(RemotePluginScope::Workspace))
                }
                Some(PluginSearchScope::Global | PluginSearchScope::Personal) => None,
            }
        };
        let limit = limit
            .unwrap_or(DEFAULT_PLUGIN_SEARCH_LIMIT)
            .clamp(1, MAX_PLUGIN_SEARCH_LIMIT);
        let mut next_cursor = None;
        let mut remote_results = Vec::new();
        if auth_mode.is_some_and(DomainAuthMode::uses_codex_backend)
            && let Some(remote_scope) = remote_scope
        {
            let page = search_remote_plugins(
                &remote_plugin_service_config(&config),
                auth.as_ref(),
                RemotePluginSearchRequest {
                    query: search_term,
                    scope: remote_scope,
                    limit,
                    page_token: cursor.as_deref(),
                },
            )
            .await
            .map_err(|err| {
                remote_plugin_catalog_error_to_jsonrpc(err, "search remote plugin catalog")
            })?;

            next_cursor = page.next_page_token;
            remote_results.reserve(page.plugins.len());
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
                remote_results.push(plugin_search_result(
                    remote_plugin_summary_to_info(plugin),
                    plugin_id.marketplace_name,
                    /*marketplace_path*/ None,
                ));
            }
        }

        // All local results are stitched into the first page; if
        // we are not on the first page, don't even check local
        if cursor.is_some() {
            return Ok(PluginSearchResponse {
                data: remote_results,
                next_cursor,
            });
        }

        let normalized_search_term = normalize_search_text(search_term);
        let mut local_results = Vec::new();
        if scope != Some(PluginSearchScope::Workspace) {
            let roots = cwds.unwrap_or_default();
            let plugins_input = config.plugins_config_input();
            let plugins_manager = self.thread_manager.plugins_manager();
            let shared_plugin_ids_by_local_path = load_shared_plugin_ids_by_local_path(&config)
                .unwrap_or_else(|err| {
                    warn!(
                        error = %err.message,
                        "plugin/search could not load shared plugin identities"
                    );
                    Default::default()
                });
            let outcome = tokio::task::spawn_blocking(move || {
                plugins_manager.list_marketplaces_for_config(
                    &plugins_input,
                    &roots,
                    /*include_openai_curated*/ !use_remote_global_catalog,
                )
            })
            .await
            .map_err(|err| internal_error(format!("failed to list marketplace plugins: {err}")))?
            .map_err(|err| Self::marketplace_error(err, "list marketplace plugins"))?;

            for error in outcome.errors {
                warn!(
                    marketplace_path = %error.path.as_path().display(),
                    error = %error.message,
                    "plugin/search skipped a local marketplace that could not be loaded"
                );
            }

            for marketplace in outcome.marketplaces {
                if !marketplace_matches_search_scope(&marketplace.name, scope) {
                    continue;
                }

                for plugin in marketplace.plugins {
                    let plugin = convert_configured_marketplace_plugin_to_plugin_summary(
                        plugin,
                        &shared_plugin_ids_by_local_path,
                    );
                    local_results.push(plugin_search_result(
                        plugin,
                        marketplace.name.clone(),
                        Some(marketplace.path.clone()),
                    ));
                }
            }
        }

        let mut data = Vec::with_capacity(
            local_results.len().min(MAX_LOCAL_PLUGIN_SEARCH_RESULTS) + remote_results.len(),
        );
        let mut local_matches = Vec::new();
        let mut remote_results = remote_results.into_iter().map(Some).collect::<Vec<_>>();
        let mut seen_local_plugin_identities = HashSet::new();

        for local_result in local_results {
            let local_remote_plugin_id =
                local_result.plugin.remote_plugin_id.as_deref().or_else(|| {
                    local_result
                        .plugin
                        .share_context
                        .as_ref()
                        .map(|context| context.remote_plugin_id.as_str())
                });
            let remote_result_index = remote_results.iter().position(|remote_result| {
                remote_result.as_ref().is_some_and(|remote_result| {
                    local_remote_plugin_id.is_some_and(|remote_plugin_id| {
                        remote_result.plugin.remote_plugin_id.as_deref() == Some(remote_plugin_id)
                    }) || local_result.plugin.id == remote_result.plugin.id
                        || (is_openai_curated_marketplace_name(&local_result.marketplace_name)
                            && remote_result.marketplace_name == REMOTE_GLOBAL_MARKETPLACE_NAME
                            && local_result.plugin.name == remote_result.plugin.name)
                })
            });

            if remote_result_index.is_none()
                && plugin_search_match_rank(&local_result.plugin, &normalized_search_term).is_none()
            {
                continue;
            }

            let local_identity = local_remote_plugin_id.map_or_else(
                || {
                    if is_openai_curated_marketplace_name(&local_result.marketplace_name) {
                        format!("curated:{}", local_result.plugin.name)
                    } else {
                        format!("plugin:{}", local_result.plugin.id)
                    }
                },
                |remote_plugin_id| format!("remote:{remote_plugin_id}"),
            );
            if !seen_local_plugin_identities.insert(local_identity) {
                continue;
            }

            if let Some(remote_result_index) = remote_result_index
                && let Some(mut remote_result) = remote_results[remote_result_index].take()
            {
                remote_result.plugin.installed = local_result.plugin.installed;
                remote_result.plugin.local_version = local_result.plugin.local_version;
                data.push(remote_result);
            } else {
                local_matches.push(local_result);
            }
        }

        local_matches.sort_by_key(|result| {
            plugin_search_match_rank(&result.plugin, &normalized_search_term)
                .unwrap_or(PLUGIN_SEARCH_NO_MATCH_RANK)
        });
        local_matches.truncate(MAX_LOCAL_PLUGIN_SEARCH_RESULTS);
        data.extend(local_matches);
        data.extend(remote_results.into_iter().flatten());
        data.sort_by_key(|result| {
            plugin_search_match_rank(&result.plugin, &normalized_search_term)
                .unwrap_or(PLUGIN_SEARCH_NO_MATCH_RANK)
        });

        Ok(PluginSearchResponse { data, next_cursor })
    }
}

/// Plugin discovery does not resolve effective activation, so all results explicitly report
/// `enabled: false`, regardless of their source, installation state, or page.
fn plugin_search_result(
    mut plugin: PluginSummary,
    marketplace_name: String,
    marketplace_path: Option<AbsolutePathBuf>,
) -> PluginSearchResult {
    plugin.enabled = false;

    PluginSearchResult {
        plugin,
        marketplace_name,
        marketplace_path,
    }
}

fn marketplace_matches_search_scope(
    marketplace_name: &str,
    scope: Option<PluginSearchScope>,
) -> bool {
    let is_built_in = is_openai_curated_marketplace_name(marketplace_name)
        || matches!(
            marketplace_name,
            OPENAI_BUNDLED_MARKETPLACE_NAME
                | "openai-bundled-alpha"
                | "codex-official"
                | "openai-curated-remote"
                | "openai-primary-runtime"
        );

    match scope {
        None => true,
        Some(PluginSearchScope::Global) => is_built_in,
        Some(PluginSearchScope::Workspace) => false,
        Some(PluginSearchScope::Personal) => !is_built_in,
    }
}

fn normalize_search_text(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    for character in value.to_lowercase().chars() {
        if character.is_alphanumeric() {
            normalized.push(character);
        } else if !normalized.is_empty() && !normalized.ends_with(' ') {
            normalized.push(' ');
        }
    }
    if normalized.ends_with(' ') {
        normalized.pop();
    }
    normalized
}

fn plugin_search_match_rank(plugin: &PluginSummary, normalized_query: &str) -> Option<usize> {
    if normalized_query.is_empty() {
        return None;
    }

    let visible_name = normalize_search_text(
        plugin
            .interface
            .as_ref()
            .and_then(|interface| interface.display_name.as_deref())
            .unwrap_or_default(),
    );
    let internal_name = normalize_search_text(&plugin.name);
    let names = [&visible_name, &internal_name];
    let keywords = plugin
        .keywords
        .iter()
        .map(|keyword| normalize_search_text(keyword))
        .collect::<Vec<_>>();
    let joined_search_values = normalize_search_text(&format!(
        "{internal_name} {visible_name} {}",
        keywords.join(" ")
    ));

    [
        visible_name == normalized_query,
        internal_name == normalized_query,
        names.iter().any(|name| name.starts_with(normalized_query)),
        names.iter().any(|name| name.contains(normalized_query)),
        keywords.iter().any(|keyword| keyword == normalized_query),
        keywords
            .iter()
            .any(|keyword| keyword.contains(normalized_query))
            || joined_search_values.contains(normalized_query),
    ]
    .iter()
    .position(|matches| *matches)
}
