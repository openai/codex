use std::future::Future;
use std::path::PathBuf;

/// Supplies the bounded memory summary used by prompt injection.
pub(crate) trait MemoryPromptSource: Clone + Send + Sync + 'static {
    fn read_summary(&self) -> impl Future<Output = Option<MemoryPromptSummary>> + Send;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MemoryPromptSummary {
    pub(crate) base_path: PathBuf,
    pub(crate) content: String,
}
