use codex_protocol::models::ResponseItem;

use crate::ExtensionData;

/// Input supplied immediately before the host builds one model sampling request.
///
/// `request_input` is a request-local clone of the conversation history. Mutations
/// affect only the current outbound request and are not persisted to canonical
/// history automatically.
pub struct SamplingInputContext<'a> {
    /// Stable host-owned turn identifier.
    pub turn_id: &'a str,
    /// Store scoped to the host session runtime.
    pub session_store: &'a ExtensionData,
    /// Store scoped to this thread runtime.
    pub thread_store: &'a ExtensionData,
    /// Store scoped to this turn runtime.
    pub turn_store: &'a ExtensionData,
    /// Model input for the current sampling request.
    pub request_input: &'a mut Vec<ResponseItem>,
}
