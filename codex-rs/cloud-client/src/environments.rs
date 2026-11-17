//! Reads the desktop Cloud picker's runnable configurations for the bound account.

use crate::Client;
use crate::Error;
use crate::prewarm::DEFAULT_ENVIRONMENT_CONFIG_ID;
use codex_http_client::bounded_response_bytes;
use serde::Deserialize;
use std::collections::HashSet;
use std::time::Duration;

const CATALOG_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_PAGE_BYTES: usize = 1024 * 1024;

/// A saved environment available to the current Cloud account.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct SavedEnvironment {
    pub id: String,
    pub name: String,
    #[serde(skip)]
    pub workspace_shared: bool,
}

#[derive(Deserialize)]
struct Page {
    data: Vec<SavedEnvironment>,
    next_cursor: Option<String>,
}

impl Client {
    /// Fetch both personal and workspace environments through the authenticated picker API.
    pub async fn list_saved_environments(&self) -> Result<Vec<SavedEnvironment>, Error> {
        tokio::time::timeout(CATALOG_TIMEOUT, async {
            let mut environments = self.list_saved_environment_scope("user").await?;
            environments.extend(self.list_saved_environment_scope("workspace").await?);
            Ok(environments)
        })
        .await
        .map_err(|_| Error::EnvironmentCatalogUnavailable)?
    }

    async fn list_saved_environment_scope(
        &self,
        scope: &str,
    ) -> Result<Vec<SavedEnvironment>, Error> {
        let mut url = self.model_catalog_url.clone();
        url.set_path("/v1/environment-configs");
        let mut configs = Vec::new();
        let mut seen_cursors = HashSet::new();
        let mut cursor: Option<String> = None;
        loop {
            url.set_query(None);
            {
                let mut query = url.query_pairs_mut();
                query.extend_pairs([
                    ("scope", scope),
                    ("limit", "100"),
                    ("omitDraft", "true"),
                    ("view", "picker"),
                ]);
                if let Some(cursor) = cursor.as_deref() {
                    query.append_pair("cursor", cursor);
                }
            }
            let response = self
                .http
                .get(url.clone())
                .headers(self.credentials.headers.clone())
                .header("x-openai-product-sku", "codex")
                .send()
                .await
                .and_then(codex_http_client::HttpResponse::error_for_status)
                .map_err(|_| Error::EnvironmentCatalogUnavailable)?;
            let body = bounded_response_bytes(response, MAX_PAGE_BYTES)
                .await
                .map_err(|_| Error::EnvironmentCatalogUnavailable)?;
            let page: Page =
                serde_json::from_slice(&body).map_err(|_| Error::EnvironmentCatalogUnavailable)?;
            configs.extend(page.data.into_iter().filter_map(|mut config| {
                config.workspace_shared = scope == "workspace";
                (!config.id.is_empty()
                    && config.id != DEFAULT_ENVIRONMENT_CONFIG_ID
                    && !config.name.trim().is_empty())
                .then_some(config)
            }));
            let Some(next_cursor) = page.next_cursor else {
                return Ok(configs);
            };
            if !seen_cursors.insert(next_cursor.clone()) {
                return Err(Error::EnvironmentCatalogUnavailable);
            }
            cursor = Some(next_cursor);
        }
    }
}
