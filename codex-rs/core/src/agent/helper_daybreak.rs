//! Native helpers inherit the spawning turn's saved Daybreak choice.

pub(crate) mod turn;

use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use codex_login::AuthOwner;
use codex_protocol::openai_models::SavedDaybreakChoice;
use codex_thread_store::PendingDaybreakPreference;
use codex_thread_store::ThreadStoreError;

/// Enrollment is local to a fresh root or a native child admitted at spawn.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Enrollment {
    Root,
    NativeHelper,
}

/// The saved choice and the account under which the choice was observed.
#[derive(Clone, Debug)]
pub(crate) struct SavedIntent {
    pub(crate) choice: Result<SavedDaybreakChoice, String>,
    pub(crate) owner: AuthOwner,
}

pub(crate) fn eligible_owner(session: &Session) -> Option<AuthOwner> {
    let manager = &session.services.auth_manager;
    let credential = manager
        .auth_cached()
        .filter(codex_login::CodexAuth::is_chatgpt_auth)?;
    manager.auth_owner_for(&credential)
}

pub(crate) async fn spawning_intent(
    parent: &Session,
    turn: &TurnContext,
    enrollment: &Enrollment,
    spawn_owner: Option<&AuthOwner>,
) -> Option<SavedIntent> {
    let owner = spawn_owner?;
    if turn.config.model_provider_id != codex_model_provider_info::OPENAI_PROVIDER_ID
        || !codex_model_provider::provider_uses_first_party_auth_path(turn.provider.info())
    {
        return None;
    }
    if *enrollment == Enrollment::NativeHelper {
        // A nested spawn inherits the choice recorded for the sending turn, including
        // when a concrete override is forwarded independently in the spawn options.
        return turn
            .extension_data
            .get::<SavedIntent>()
            .map(|intent| intent.as_ref().clone());
    }
    let choice = match saved_choice(parent).await {
        Ok(Some(enabled)) => Ok(SavedDaybreakChoice::from(Some(enabled))),
        Ok(None) => return None,
        Err(_) if turn.cyber_access_program.is_some() => return None,
        Err(error) => Err(error),
    };
    Some(SavedIntent {
        choice,
        owner: owner.clone(),
    })
}

pub(crate) async fn saved_choice(session: &Session) -> Result<Option<bool>, String> {
    let thread_id = session.thread_id();
    let read_error =
        |error| format!("Could not read the Daybreak preference for thread {thread_id}: {error}");
    let pending =
        PendingDaybreakPreference::read(session.services.thread_store.as_ref(), thread_id)
            .await
            .map_err(read_error)?;
    let Some(live) = session.live_thread() else {
        return Ok(None);
    };
    let stored = match live
        .read_thread(
            /*include_archived*/ false, /*include_history*/ false,
        )
        .await
    {
        Ok(thread) => thread.daybreak_enabled,
        // The live handle already selects the thread for a typed not-found response.
        Err(error)
            if matches!(error, ThreadStoreError::ThreadNotFound { .. })
                || error.is_missing_for_read(thread_id) =>
        {
            None
        }
        Err(error) => return Err(read_error(error)),
    };
    Ok(pending.resolve(stored))
}
