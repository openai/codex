use super::RemotePluginCatalogError;
use super::RemotePluginListResponse;
use super::RemotePluginScope;
use super::RemotePluginServiceConfig;
use super::RemotePluginSummary;
use super::authenticated_request;
use super::build_remote_plugin_summary;
use super::ensure_chatgpt_auth;
use super::send_and_decode;
use codex_login::CodexAuth;
use http::Method;
use tracing::instrument;
use url::Url;

/// Search parameters forwarded directly to plugin-service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemotePluginSearchRequest<'a> {
    pub query: &'a str,
    pub scope: Option<RemotePluginScope>,
    pub limit: u32,
    pub page_token: Option<&'a str>,
}

/// One uncached page of remote plugin search results.
#[derive(Debug, Clone, PartialEq)]
pub struct RemotePluginSearchPage {
    pub plugins: Vec<RemotePluginSummary>,
    pub next_page_token: Option<String>,
}

/// Searches plugin-service without reading or populating the remote catalog cache.
#[instrument(
    level = "debug",
    skip_all,
    fields(plugin.scope = ?search.scope, plugin.limit = search.limit)
)]
pub async fn search_remote_plugins(
    config: &RemotePluginServiceConfig,
    auth: Option<&CodexAuth>,
    search: RemotePluginSearchRequest<'_>,
) -> Result<RemotePluginSearchPage, RemotePluginCatalogError> {
    let auth = ensure_chatgpt_auth(auth)?;
    let base_url = config.chatgpt_base_url.trim_end_matches('/');
    let mut url = Url::parse(&format!("{base_url}/ps/plugins/search"))
        .map_err(RemotePluginCatalogError::InvalidBaseUrl)?;
    // Search terms and page tokens can contain user data. Keep the queryless endpoint for
    // diagnostics so neither value is exposed through errors or telemetry.
    let url_for_error = url.to_string();

    {
        let mut query_pairs = url.query_pairs_mut();
        query_pairs.append_pair("q", search.query);
        if let Some(scope) = search.scope {
            query_pairs.append_pair("scope", scope.api_value());
        }
        query_pairs.append_pair("limit", &search.limit.to_string());
        if let Some(page_token) = search.page_token {
            query_pairs.append_pair("pageToken", page_token);
        }
    }

    let url = url.to_string();
    let request = authenticated_request(config.http_request(Method::GET, &url), auth);
    let response: RemotePluginListResponse = send_and_decode(request, &url_for_error)
        .await
        .map_err(|error| match error {
            RemotePluginCatalogError::Request { url, source } => {
                RemotePluginCatalogError::Request {
                    url,
                    source: source.without_url(),
                }
            }
            other => other,
        })?;
    let plugins = response
        .plugins
        .iter()
        // Search intentionally does not join against `/ps/plugins/installed`, so these
        // summaries always report `installed: false`.
        .map(|plugin| build_remote_plugin_summary(plugin, /*installed_plugin*/ None))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(RemotePluginSearchPage {
        plugins,
        next_page_token: response.pagination.next_page_token,
    })
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
