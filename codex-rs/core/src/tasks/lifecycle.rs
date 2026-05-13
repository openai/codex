use codex_extension_api::ExtensionData;
use codex_protocol::protocol::TurnAbortReason;

use crate::session::session::Session;
use crate::session::turn_context::TurnContext;

impl Session {
    pub(super) fn emit_turn_start_lifecycle(
        &self,
        turn_context: &TurnContext,
        turn_store: &ExtensionData,
    ) {
        for contributor in self.services.extensions.turn_lifecycle_contributors() {
            contributor.on_turn_start(codex_extension_api::TurnStartInput {
                thread_id: self.conversation_id,
                turn_id: &turn_context.sub_id,
                session_store: &self.services.session_extension_data,
                thread_store: &self.services.thread_extension_data,
                turn_store,
            });
        }
    }

    pub(super) fn emit_turn_stop_lifecycle(
        &self,
        turn_context: &TurnContext,
        turn_store: &ExtensionData,
    ) {
        for contributor in self.services.extensions.turn_lifecycle_contributors() {
            contributor.on_turn_stop(codex_extension_api::TurnStopInput {
                thread_id: self.conversation_id,
                turn_id: &turn_context.sub_id,
                session_store: &self.services.session_extension_data,
                thread_store: &self.services.thread_extension_data,
                turn_store,
            });
        }
    }

    pub(super) fn emit_turn_abort_lifecycle(
        &self,
        turn_context: &TurnContext,
        reason: TurnAbortReason,
        turn_store: &ExtensionData,
    ) {
        for contributor in self.services.extensions.turn_lifecycle_contributors() {
            contributor.on_turn_abort(codex_extension_api::TurnAbortInput {
                thread_id: self.conversation_id,
                turn_id: &turn_context.sub_id,
                reason: reason.clone(),
                session_store: &self.services.session_extension_data,
                thread_store: &self.services.thread_extension_data,
                turn_store,
            });
        }
    }
}
