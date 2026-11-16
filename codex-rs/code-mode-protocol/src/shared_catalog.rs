//! Shares immutable tool definitions across sessions in the same process. Each session
//! negotiates its own revision and owns its snapshot independently.

use std::borrow::Cow;
use std::collections::HashMap;
use std::hash::BuildHasher;
use std::hash::RandomState;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::sync::Weak;

use codex_protocol::ToolName;

use crate::EnabledToolMetadata;
use crate::ToolDefinition;

type ToolIndex<T> = LazyLock<Mutex<HashMap<u64, Vec<Weak<T>>>>>;

impl ToolDefinition {
    /// Shares matching definitions while preserving the client's catalog order.
    pub fn shared_catalog(tools: &[Self]) -> Vec<Arc<Self>> {
        static INDEX: ToolIndex<ToolDefinition> = LazyLock::new(Mutex::default);
        share(&INDEX, tools.iter().map(Cow::Borrowed), |tool| {
            &tool.tool_name
        })
    }
}

impl EnabledToolMetadata {
    /// Shares matching runtime metadata while preserving the cell's callback indices.
    pub fn shared_catalog(tools: Vec<Self>) -> Vec<Arc<Self>> {
        static INDEX: ToolIndex<EnabledToolMetadata> = LazyLock::new(Mutex::default);
        share(&INDEX, tools.into_iter().map(Cow::Owned), |tool| {
            &tool.tool_name
        })
    }
}

fn share<'a, T: 'a + Clone + PartialEq>(
    index: &ToolIndex<T>,
    tools: impl Iterator<Item = Cow<'a, T>>,
    name: impl Fn(&T) -> &ToolName,
) -> Vec<Arc<T>> {
    static HASHER: LazyLock<RandomState> = LazyLock::new(RandomState::new);
    let mut index = index.lock().unwrap_or_else(PoisonError::into_inner);
    tools
        .map(|tool| {
            // Hash only the name. Versions of the same tool share a bucket, and
            // exact equality checks the descriptions, schemas and other fields.
            let hash = HASHER.hash_one(name(&tool));
            if index.len() == index.capacity() && !index.contains_key(&hash) {
                index.retain(|_, versions| versions.iter().any(|tool| tool.strong_count() > 0));
            }
            let versions = index.entry(hash).or_default();
            if let Some(existing) = versions
                .iter()
                .filter_map(Weak::upgrade)
                .find(|existing| existing.as_ref() == tool.as_ref())
            {
                return existing;
            }
            versions.retain(|tool| tool.strong_count() > 0);
            let shared = Arc::new(tool.into_owned());
            versions.push(Arc::downgrade(&shared));
            shared
        })
        .collect()
}
