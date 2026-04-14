use async_trait::async_trait;

use crate::ConfigDocumentRead;
use crate::ConfigStoreResult;
use crate::ReadConfigDocumentParams;

/// Storage-neutral reader for path-addressed config documents.
///
/// Implementations should only read and parse the requested document. Codex config loading remains
/// responsible for deciding what the document represents, how missing documents are handled, how
/// parse errors interact with project trust, how relative paths are resolved, and how layers are
/// ordered and merged.
#[async_trait]
pub trait ConfigDocumentStore: Send + Sync {
    /// Reads one config document addressed by path.
    async fn read_config_document(
        &self,
        params: ReadConfigDocumentParams,
    ) -> ConfigStoreResult<ConfigDocumentRead>;
}
