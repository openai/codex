use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::Mutex as StdMutex;
use std::time::Instant;

use crate::AppBranding;
use crate::AppMetadata;
use crate::CONNECTORS_CACHE_TTL;

/// Metadata returned by the app batch-read API.
///
/// This intentionally excludes connector runtime state, tools, actions, model descriptions, and
/// icon fields. Consumers that need those concepts must use their owning APIs instead of growing
/// this cache boundary.
#[derive(Debug, Clone, PartialEq)]
pub struct ConnectorMetadata {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub distribution_channel: Option<String>,
    pub branding: Option<AppBranding>,
    pub app_metadata: Option<AppMetadata>,
    pub labels: Option<HashMap<String, String>>,
    pub install_url: Option<String>,
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

    /// Returns only unexpired records for the requested ids.
    ///
    /// Expired entries are deliberately left in place so a failed refresh cannot mutate prior
    /// cache state.
    pub fn fresh_records(&self, ids: &[String]) -> HashMap<String, ConnectorMetadata> {
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
                    .filter(|record| now < record.expires_at)
                    .map(|record| (id.clone(), record.metadata.clone()))
            })
            .collect()
    }

    /// Commits successfully fetched records to this store's captured scope.
    pub fn commit(&self, records: &[ConnectorMetadata]) {
        if records.is_empty() {
            return;
        }

        let expires_at = Instant::now() + CONNECTORS_CACHE_TTL;
        let mut cache = CONNECTOR_METADATA_CACHE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let scoped_records = cache.entry(self.scope.clone()).or_default();
        for metadata in records {
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
