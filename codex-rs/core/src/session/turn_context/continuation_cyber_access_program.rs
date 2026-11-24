//! Carry explicit Cyber selection within live work owned by the same account and model.

use super::TurnContext;
use crate::CodexThread;
use crate::agent::helper_daybreak::Enrollment;
use crate::cyber_access_program;
use crate::session::session::Session;
use codex_features::Feature;
use codex_login::AuthOwner;
use codex_model_provider::request_auth_owner;
use codex_protocol::turn_input::CyberAccessProgram;

/// An explicit program captured for a live continuation on the same account and model.
pub struct ContinuationCyberAccessProgram {
    program: CyberAccessProgram,
    owner: AuthOwner,
    model: String,
    provider_id: String,
}

impl ContinuationCyberAccessProgram {
    pub(super) fn capture(session: &Session, turn: &TurnContext) -> Option<Self> {
        session.services.thread_extension_data.get::<Enrollment>()?;
        let program = turn.cyber_access_program?;
        if !turn
            .config
            .features
            .enabled(Feature::DaybreakProgramPropagation)
        {
            return None;
        }
        Some(Self {
            program,
            owner: request_auth_owner(turn.provider.as_ref())?,
            model: turn
                .initial_settings
                .selected_collaboration_mode()
                .model()
                .to_string(),
            provider_id: turn.config.model_provider_id.clone(),
        })
    }

    /// Reuse the selection only while the current turn configuration still matches its source.
    pub async fn for_thread(&self, thread: &CodexThread) -> Option<CyberAccessProgram> {
        if !thread.daybreak_program_propagation_enrolled() {
            return None;
        }
        let state = thread.session.state.lock().await;
        let current = &state.session_configuration;
        let config = &current.original_config_do_not_use;
        if !config.features.enabled(Feature::DaybreakProgramPropagation)
            || config.model_provider_id != self.provider_id
            || current.step_settings.collaboration_mode.model() != self.model
            || request_auth_owner(current.provider.as_ref()).as_ref() != Some(&self.owner)
        {
            return None;
        }
        cyber_access_program::for_provider(config, Some(self.program))
    }
}
