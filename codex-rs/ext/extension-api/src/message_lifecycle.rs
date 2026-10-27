//! Capture publication state once; consume it independently for each delivery.

use crate::ExtensionData;
use codex_protocol::ResponseItemId;
use codex_protocol::ThreadId;

/// State shared by all deliveries of a publication, including recipients resolved later.
pub struct MessageLifecycleInput {
    pub message_id: ResponseItemId,
    pub sender: ThreadId,
    /// Capture contributor-specific state at publication; read it at consumption.
    pub data: ExtensionData,
}

/// Observes messages from local agent control and the initial task of a host-spawned child.
/// Callbacks are synchronous and must not block. Replaying history or submitting raw
/// protocol input does not create a publication.
pub trait MessageLifecycleContributor: Send + Sync {
    /// Called once before the message is exposed to any recipient.
    fn on_publish(&self, _input: &MessageLifecycleInput) {}
    /// Called at most once per logical delivery, at history commitment before sampling.
    fn on_consumed(&self, _input: &MessageLifecycleInput, _recipient: ThreadId) {}
}
