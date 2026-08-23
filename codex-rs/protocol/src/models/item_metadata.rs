use serde::Deserialize;
use serde::Serialize;

/// Harness-owned classification for one position in an item's content array.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct ContentItemKind(pub String);

#[cfg(test)]
#[path = "item_metadata_tests.rs"]
mod tests;
