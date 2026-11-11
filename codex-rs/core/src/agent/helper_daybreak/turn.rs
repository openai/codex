//! Resolve a native helper's saved Daybreak choice once, before its turn starts.
//! The existing turn program then travels with normal and compaction requests.
use super::Enrollment;
use super::SavedIntent;
use super::eligible_owner;
use super::saved_choice;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use codex_features::Feature;
use codex_login::AuthOwner;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::SavedDaybreakChoice;
use codex_protocol::turn_input::CyberAccessProgram;

const ACCOUNT_CHANGED: &str =
    "The account for this subagent's saved Daybreak choice has changed. Start a new subagent.";

/// An implicit program stays bound to the account whose choice selected it.
pub(crate) struct SavedProgramResolution(Result<AuthOwner, String>);

impl SavedProgramResolution {
    pub(crate) fn validate(&self) -> CodexResult<()> {
        match &self.0 {
            Ok(owner) if owner.is_current() => Ok(()),
            Ok(_) => Err(CodexErr::InvalidRequest(ACCOUNT_CHANGED.to_owned())),
            Err(error) => Err(CodexErr::InvalidRequest(error.clone())),
        }
    }
}

/// A helper resolves automatic choices from saved intent on its own turn.
pub(crate) fn explicit_program_for_spawn(turn: &TurnContext) -> Option<CyberAccessProgram> {
    if turn
        .extension_data
        .get::<SavedProgramResolution>()
        .is_some()
    {
        None
    } else {
        turn.cyber_access_program
    }
}

/// The chat account at admission, captured before preparing the new turn.
pub(crate) struct HelperTurnAdmission(pub(crate) AuthOwner);

impl HelperTurnAdmission {
    pub(crate) fn capture(session: &Session) -> Option<Self> {
        let enrollment = session.services.thread_extension_data.get::<Enrollment>()?;
        if *enrollment != Enrollment::NativeHelper {
            return None;
        }
        eligible_owner(session)
            .or_else(|| {
                let inherited = session
                    .services
                    .thread_extension_data
                    .get::<SavedIntent>()?;
                (!inherited.owner.is_current()).then(|| inherited.owner.clone())
            })
            .map(Self)
    }

    pub(crate) async fn capture_intent(&self, session: &Session, turn: &mut TurnContext) {
        if !turn
            .config
            .features
            .enabled(Feature::DaybreakProgramPropagation)
            || turn.config.model_provider_id != codex_model_provider_info::OPENAI_PROVIDER_ID
            || !codex_model_provider::provider_uses_first_party_auth_path(turn.provider.info())
            || !turn
                .config
                .model_provider
                .supports_cyber_access_programs(&turn.config.model_provider_id)
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
        if turn.cyber_access_program.is_none() {
            let resolution = if owner != self.0 || eligible_owner(session).as_ref() != Some(&self.0)
            {
                Err(ACCOUNT_CHANGED.to_owned())
            } else {
                choice
                    .clone()
                    .and_then(|choice| select_program(turn.model_info(), choice))
            };
            let bound_owner = resolution.map(|program| {
                turn.cyber_access_program = program;
                owner.clone()
            });
            turn.extension_data
                .insert(SavedProgramResolution(bound_owner));
        }
        turn.extension_data.insert(SavedIntent { choice, owner });
    }
}

pub(crate) fn select_program(
    model: &ModelInfo,
    choice: SavedDaybreakChoice,
) -> Result<Option<CyberAccessProgram>, String> {
    let Some(programs) = model.available_access_programs.as_ref() else {
        return Ok(None);
    };
    let selected = programs.select_saved(choice);
    if selected.is_none() {
        match choice {
            SavedDaybreakChoice::On if !programs.cyber.is_empty() => {
                return Err(format!(
                    "Model {} does not advertise Daybreak support. Turn Daybreak off or choose a compatible model.",
                    model.slug
                ));
            }
            SavedDaybreakChoice::Off => {
                return Err(format!(
                    "Model {} does not advertise Standard. Choose a model that supports turning Daybreak off.",
                    model.slug
                ));
            }
            SavedDaybreakChoice::On | SavedDaybreakChoice::Unset => {}
        }
    }
    Ok(selected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::openai_models::ModelAccessPrograms;

    #[test]
    fn missing_metadata_and_unset_choice_preserve_implicit_selection() {
        let mut model = codex_models_manager::bundled_models_response()
            .unwrap()
            .models
            .remove(0);
        model.available_access_programs = None;
        assert_eq!(select_program(&model, SavedDaybreakChoice::Off), Ok(None));
        for cyber in [vec![], vec![CyberAccessProgram::DaybreakRed]] {
            model.available_access_programs = Some(ModelAccessPrograms { cyber });
            assert_eq!(select_program(&model, SavedDaybreakChoice::Unset), Ok(None));
            assert!(
                select_program(&model, SavedDaybreakChoice::Off)
                    .unwrap_err()
                    .contains("does not advertise Standard")
            );
        }
    }
}
