//! Account-bound Daybreak intent for one live goal generation.

use super::SavedIntent;
use super::eligible_owner;
use super::saved_choice;
use super::turn::SavedProgramContext;
use super::turn::SavedProgramResolution;
use super::turn::explicit_program_for_spawn;
use super::turn::select_program;
use crate::config::Config;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use codex_features::Feature;
use codex_login::AuthOwner;
use codex_protocol::openai_models::SavedDaybreakChoice;

const ACCOUNT_CHANGED: &str =
    "The account for this goal's Daybreak choice has changed. Start a new goal.";

/// A live goal's request preference, not an entitlement. Only Core can produce
/// this snapshot, and it cannot be persisted or used after the account changes.
#[derive(Clone, Debug)]
pub struct GoalDaybreakSnapshot {
    owner: AuthOwner,
    choice: Result<SavedDaybreakChoice, String>,
}

/// Host attachment consumed at idle admission, never saved as a thread default.
pub(crate) struct GoalDaybreakAdmission(pub(crate) GoalDaybreakSnapshot);

impl GoalDaybreakSnapshot {
    pub(crate) async fn capture_saved(session: &Session) -> Option<Self> {
        let owner = eligible_owner(session)?;
        let config = session.get_config().await;
        if !supported_config(&config)
            || !codex_model_provider::provider_uses_first_party_auth_path(&session.provider().await)
        {
            return None;
        }
        let choice = match saved_choice(session).await {
            Ok(Some(enabled)) => Ok(SavedDaybreakChoice::from(Some(enabled))),
            Ok(None) => return None,
            Err(error) => Err(error),
        };
        Some(Self { owner, choice })
    }

    pub(crate) fn from_turn(turn: &TurnContext) -> Option<Self> {
        // An explicit choice is governed only by the explicit continuation path.
        if explicit_program_for_spawn(turn).is_some() {
            return None;
        }
        if let Some(snapshot) = turn.extension_data.get::<Self>() {
            return Some(snapshot.as_ref().clone());
        }
        let intent = turn.extension_data.get::<SavedIntent>()?;
        Some(Self {
            owner: intent.owner.clone(),
            choice: intent.choice.clone(),
        })
    }

    pub(crate) fn apply(&self, session: &Session, turn: &mut TurnContext) {
        // Keep Core's ordinary explicit request path authoritative.
        if turn.cyber_access_program.is_some() || !supported_turn(turn) {
            return;
        }
        let resolved =
            if eligible_owner(session).as_ref() != Some(&self.owner) || !self.owner.is_current() {
                Err(ACCOUNT_CHANGED.to_owned())
            } else {
                self.choice.clone().and_then(|choice| {
                    select_program(turn.model_info(), choice, SavedProgramContext::Goal)
                })
            };
        let bound_owner = resolved.map(|program| {
            turn.cyber_access_program = program;
            self.owner.clone()
        });
        turn.extension_data
            .insert(SavedProgramResolution::new(bound_owner, ACCOUNT_CHANGED));
        turn.extension_data.insert(self.clone());
    }
}

fn supported_config(config: &Config) -> bool {
    config.features.enabled(Feature::DaybreakProgramPropagation)
        && config.model_provider_id == codex_model_provider_info::OPENAI_PROVIDER_ID
        && config
            .model_provider
            .supports_cyber_access_programs(&config.model_provider_id)
}

fn supported_turn(turn: &TurnContext) -> bool {
    supported_config(&turn.config)
        && codex_model_provider::provider_uses_first_party_auth_path(turn.provider.info())
}
