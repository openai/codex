use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::Ordering;
use std::time::SystemTime;

use crate::rmcp_client::AsyncManagedClient;
use crate::tools::ToolInfo;

pub(super) struct ToolInventoryCache {
    codex_apps_cache_path: Option<PathBuf>,
    cached: Mutex<Option<CachedToolInventory>>,
}

#[derive(Clone)]
struct CachedToolInventory {
    revision: ToolInventoryRevision,
    tools: Vec<ToolInfo>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ToolInventoryRevision {
    server_startup: Vec<(String, bool)>,
    codex_apps_cache_file: Option<CodexAppsCacheFileRevision>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum CodexAppsCacheFileRevision {
    Missing,
    Present { len: u64, modified: SystemTime },
}

impl ToolInventoryCache {
    pub(super) fn new(codex_apps_cache_path: Option<PathBuf>) -> Self {
        Self {
            codex_apps_cache_path,
            cached: Mutex::new(None),
        }
    }

    pub(super) fn get(&self, revision: &ToolInventoryRevision) -> Option<Vec<ToolInfo>> {
        self.cached
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .filter(|cached| &cached.revision == revision)
            .map(|cached| cached.tools.clone())
    }

    pub(super) fn insert_if_unchanged(
        &self,
        revision_before: Option<ToolInventoryRevision>,
        clients: &HashMap<String, AsyncManagedClient>,
        tools: &[ToolInfo],
    ) {
        let revision_after = self.revision(clients);
        if let (Some(revision_before), Some(revision_after)) = (revision_before, revision_after)
            && revision_before == revision_after
        {
            self.cached
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .replace(CachedToolInventory {
                    revision: revision_after,
                    tools: tools.to_vec(),
                });
        }
    }

    pub(super) fn clear(&self) {
        self.cached
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
    }

    pub(super) fn revision(
        &self,
        clients: &HashMap<String, AsyncManagedClient>,
    ) -> Option<ToolInventoryRevision> {
        let mut server_startup = clients
            .iter()
            .map(|(server_name, client)| {
                (
                    server_name.clone(),
                    client.startup_complete.load(Ordering::Acquire),
                )
            })
            .collect::<Vec<_>>();
        server_startup.sort_unstable_by(|left, right| left.0.cmp(&right.0));

        let codex_apps_cache_file = match self.codex_apps_cache_path.as_ref() {
            Some(path) => Some(match std::fs::metadata(path) {
                Ok(metadata) => metadata
                    .modified()
                    .map(|modified| CodexAppsCacheFileRevision::Present {
                        len: metadata.len(),
                        modified,
                    })
                    .ok(),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    Some(CodexAppsCacheFileRevision::Missing)
                }
                Err(_) => None,
            }?),
            None => None,
        };

        Some(ToolInventoryRevision {
            server_startup,
            codex_apps_cache_file,
        })
    }
}
