//! Bounded, host-rendered sender evidence attached to a delivered task message.

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

const MAX_ID_BYTES: usize = 128;

/// Rendered evidence and the receiver identities needed for replay and rollback.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SenderUserMessages {
    pub receiver_turn_id: String,
    pub receiver_message_id: String,
    pub text: String,
}

impl SenderUserMessages {
    /// Bounds the identities used to recover the delivery and roll it back.
    pub fn bound(&mut self) {
        for id in [&mut self.receiver_turn_id, &mut self.receiver_message_id] {
            id.truncate(id.floor_char_boundary(MAX_ID_BYTES));
        }
    }
}

impl std::fmt::Debug for SenderUserMessages {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SenderUserMessages")
            .finish_non_exhaustive()
    }
}
