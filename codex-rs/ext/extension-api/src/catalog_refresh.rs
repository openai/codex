//! Thread-owned cloud catalog retry cooldowns scoped to auth and resource generations.

use codex_mcp::McpResourceClientAuthKey;
use codex_mcp::McpResourceServerCacheKey;
use std::time::Duration;
use tokio::time::Instant;

/// Store per catalog and thread; clear on success or configuration reset.
#[derive(Clone)]
pub struct CatalogRefreshBackoff {
    auth: Option<McpResourceClientAuthKey>,
    resources: Option<McpResourceServerCacheKey>,
    delay: Duration,
    retry_at: Instant,
}

impl std::fmt::Debug for CatalogRefreshBackoff {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CatalogRefreshBackoff")
            .field("delay", &self.delay)
            .field("retry_at", &self.retry_at)
            .finish_non_exhaustive()
    }
}

impl CatalogRefreshBackoff {
    pub fn is_pending(
        &self,
        auth: Option<&McpResourceClientAuthKey>,
        resources: Option<&McpResourceServerCacheKey>,
    ) -> bool {
        self.auth.as_ref() == auth
            && self.resources.as_ref() == resources
            && Instant::now() < self.retry_at
    }

    pub fn after_failure(
        auth: Option<McpResourceClientAuthKey>,
        resources: Option<McpResourceServerCacheKey>,
        previous: Option<&Self>,
    ) -> Self {
        let delay = previous
            .filter(|previous| previous.auth == auth && previous.resources == resources)
            .map_or(Duration::from_secs(30), |previous| {
                (previous.delay * 2).min(Duration::from_secs(300))
            });
        Self {
            auth,
            resources,
            delay,
            retry_at: Instant::now() + delay,
        }
    }
}
