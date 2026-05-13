use codex_extension_api::ExtensionData;
use codex_protocol::protocol::TurnAbortReason;

use crate::session::session::Session;

impl Session {
    pub(super) fn emit_turn_start_lifecycle(&self, turn_store: &ExtensionData) {
        for contributor in self.services.extensions.turn_lifecycle_contributors() {
            contributor.on_turn_start(codex_extension_api::TurnStartInput {
                session_store: &self.services.session_extension_data,
                thread_store: &self.services.thread_extension_data,
                turn_store,
            });
        }
    }

    pub(super) fn emit_turn_stop_lifecycle(&self, turn_store: &ExtensionData) {
        for contributor in self.services.extensions.turn_lifecycle_contributors() {
            contributor.on_turn_stop(codex_extension_api::TurnStopInput {
                session_store: &self.services.session_extension_data,
                thread_store: &self.services.thread_extension_data,
                turn_store,
            });
        }
    }

    pub(super) fn emit_turn_abort_lifecycle(
        &self,
        reason: TurnAbortReason,
        turn_store: &ExtensionData,
    ) {
        for contributor in self.services.extensions.turn_lifecycle_contributors() {
            contributor.on_turn_abort(codex_extension_api::TurnAbortInput {
                reason: reason.clone(),
                session_store: &self.services.session_extension_data,
                thread_store: &self.services.thread_extension_data,
                turn_store,
            });
        }
    }
}
