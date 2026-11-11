//! Capture the saved Daybreak choice for an admitted native helper turn.
use super::Enrollment;
use super::SavedIntent;
use super::eligible_owner;
use super::saved_choice;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use codex_features::Feature;
use codex_login::AuthOwner;
use codex_protocol::openai_models::SavedDaybreakChoice;

/// The chat account at admission, captured before preparing the new turn.
pub(crate) struct HelperTurnAdmission(pub(crate) AuthOwner);

impl HelperTurnAdmission {
    pub(crate) fn capture(session: &Session) -> Option<Self> {
        let enrollment = session.services.thread_extension_data.get::<Enrollment>()?;
        if *enrollment != Enrollment::NativeHelper {
            return None;
        }
        eligible_owner(session).map(Self)
    }

    pub(crate) async fn capture_intent(&self, session: &Session, turn: &TurnContext) {
        if !turn
            .config
            .features
            .enabled(Feature::DaybreakProgramPropagation)
            || turn.config.model_provider_id != codex_model_provider_info::OPENAI_PROVIDER_ID
            || !codex_model_provider::provider_uses_first_party_auth_path(turn.provider.info())
        {
            return;
        }
        let (choice, owner) = match saved_choice(session).await {
            Ok(Some(enabled)) => (Ok(SavedDaybreakChoice::from(Some(enabled))), self.0.clone()),
            Ok(None) => {
                let Some(inherited) = session.services.thread_extension_data.get::<SavedIntent>()
                else {
                    return;
                };
                (inherited.choice.clone(), inherited.owner.clone())
            }
            Err(_) if turn.cyber_access_program.is_some() => return,
            Err(error) => (Err(error), self.0.clone()),
        };
        turn.extension_data.insert(SavedIntent { choice, owner });
    }
}
