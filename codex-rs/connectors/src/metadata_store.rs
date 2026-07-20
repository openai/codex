use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::Mutex as StdMutex;
use std::time::Instant;

use crate::CONNECTOR_METADATA_CACHE_TTL;

/// Display-only summary of one app tool returned by the app batch-read API.
#[derive(Debug, Clone, PartialEq)]
pub struct ConnectorToolSummary {
    pub name: String,
    pub title: Option<String>,
    pub description: String,
}

/// Metadata returned by the app batch-read API.
///
/// This intentionally excludes connector runtime state, full actions, and model descriptions.
/// Tool summaries contain display text only, and icon URLs are already projected as public URLs by
/// the backend.
#[derive(Debug, Clone, PartialEq)]
pub struct ConnectorMetadata {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    pub icon_url_dark: Option<String>,
    pub distribution_channel: Option<String>,
    pub tool_summaries: Option<Vec<ConnectorToolSummary>>,
}

/// A view of the process-wide metadata cache bound to one backend and auth identity.
///
/// The active ChatGPT account id represents the selected personal account or workspace, while the
/// ChatGPT user id identifies the account principal. Keeping both plus workspace classification
/// matches the existing connector-directory cache partition.
pub struct ConnectorMetadataStore {
    scope: ConnectorMetadataStoreScope,
}

impl ConnectorMetadataStore {
    pub fn new(
        backend_base_url: String,
        account_id: Option<String>,
        chatgpt_user_id: Option<String>,
        is_workspace_account: bool,
    ) -> Self {
        Self {
            scope: ConnectorMetadataStoreScope {
                backend_base_url,
                account_id,
                chatgpt_user_id,
                is_workspace_account,
            },
        }
    }

    /// Returns only unexpired records for the requested ids, requiring tool summaries when asked.
    ///
    /// Expired entries are deliberately left in place so a failed refresh cannot mutate prior
    /// cache state.
    pub fn fresh_records(
        &self,
        ids: &[String],
        include_tools: bool,
    ) -> HashMap<String, ConnectorMetadata> {
        let cache = CONNECTOR_METADATA_CACHE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(records) = cache.get(&self.scope) else {
            return HashMap::new();
        };
        let now = Instant::now();
        ids.iter()
            .filter_map(|id| {
                records
                    .get(id)
                    .filter(|record| {
                        now < record.expires_at
                            && (!include_tools || record.metadata.tool_summaries.is_some())
                    })
                    .map(|record| (id.clone(), record.metadata.clone()))
            })
            .collect()
    }

    /// Commits successfully fetched records without letting a late metadata-only response
    /// replace fresh tool summaries.
    pub fn commit(&self, records: &[ConnectorMetadata]) {
        if records.is_empty() {
            return;
        }

        let now = Instant::now();
        let expires_at = now + CONNECTOR_METADATA_CACHE_TTL;
        let mut cache = CONNECTOR_METADATA_CACHE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let scoped_records = cache.entry(self.scope.clone()).or_default();
        for metadata in records {
            if metadata.tool_summaries.is_none()
                && scoped_records.get(&metadata.id).is_some_and(|record| {
                    now < record.expires_at && record.metadata.tool_summaries.is_some()
                })
            {
                continue;
            }
            scoped_records.insert(
                metadata.id.clone(),
                CachedConnectorMetadata {
                    metadata: metadata.clone(),
                    expires_at,
                },
            );
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ConnectorMetadataStoreScope {
    backend_base_url: String,
    account_id: Option<String>,
    chatgpt_user_id: Option<String>,
    is_workspace_account: bool,
}

struct CachedConnectorMetadata {
    metadata: ConnectorMetadata,
    expires_at: Instant,
}

static CONNECTOR_METADATA_CACHE: LazyLock<
    StdMutex<HashMap<ConnectorMetadataStoreScope, HashMap<String, CachedConnectorMetadata>>>,
> = LazyLock::new(|| StdMutex::new(HashMap::new()));

#[cfg(test)]
#[path = "metadata_store_tests.rs"]
mod tests;
