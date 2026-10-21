//! Storage namespace selected by the board's owner.
//! Owners supply the same namespace when reopening storage.

use super::invalid;
use codex_protocol::error::Result;

pub(super) const DEFAULT_STORAGE_NAMESPACE: &str = "agent_message_board";

/// Host-selected storage, independent of tool selection and connection permissions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalBoardOptions {
    /// A namespace isolates boards with the same session identity in separate databases.
    /// Use 1–64 lowercase ASCII letters, digits, underscores or hyphens.
    pub storage_namespace: String,
}

impl Default for LocalBoardOptions {
    fn default() -> Self {
        Self {
            storage_namespace: DEFAULT_STORAGE_NAMESPACE.into(),
        }
    }
}

impl LocalBoardOptions {
    pub(super) fn database_file(&self) -> Result<String> {
        if self.storage_namespace.is_empty()
            || self.storage_namespace.len() > 64
            || !self.storage_namespace.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
            })
        {
            return Err(invalid(
                "board storage namespaces require 1–64 lowercase ASCII letters, digits, underscores or hyphens",
            ));
        }
        if self.storage_namespace == DEFAULT_STORAGE_NAMESPACE {
            return Ok("agent_message_board_1.sqlite".into());
        }
        // Custom boards must not share filenames with other runtime databases.
        Ok(format!("message_board_{}_1.sqlite", self.storage_namespace))
    }
}
