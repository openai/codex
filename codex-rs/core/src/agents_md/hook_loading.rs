use codex_extension_api::UserInstructions;
use codex_hooks::Hooks;
use codex_hooks::UserInstructionsRequest;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::HookCompletedEvent;
use codex_protocol::protocol::HookStartedEvent;
use codex_protocol::protocol::WarningEvent;

use crate::session::INITIAL_SUBMIT_ID;

pub(crate) struct UserInstructionsResolution {
    pub instructions: Option<UserInstructions>,
    pub events: Vec<Event>,
}

/// Resolves a fresh user-instruction snapshot from the configured hook.
pub(crate) async fn resolve_user_instructions(
    hooks: &Hooks,
    request: UserInstructionsRequest,
    mut instructions: Option<UserInstructions>,
    mut on_completed: impl FnMut(&HookCompletedEvent),
) -> UserInstructionsResolution {
    let mut events = hooks
        .preview_user_instructions(&request)
        .into_iter()
        .map(|run| {
            event(EventMsg::HookStarted(HookStartedEvent {
                turn_id: None,
                run,
            }))
        })
        .collect::<Vec<_>>();

    let outcome = hooks.run_user_instructions(request).await;
    for completed in outcome.hook_events {
        on_completed(&completed);
        events.push(event(EventMsg::HookCompleted(completed)));
    }
    events.extend(
        outcome
            .warnings
            .into_iter()
            .map(|message| event(EventMsg::Warning(WarningEvent { message }))),
    );

    let mut texts = Vec::new();
    let mut sources = Vec::new();
    for result in outcome.results {
        match result.source_path.to_abs_path() {
            Ok(source) => {
                texts.push(result.text);
                if !sources.contains(&source) {
                    sources.push(source);
                }
            }
            Err(error) => events.push(event(EventMsg::Warning(WarningEvent {
                message: format!(
                    "UserInstructions hook source `{}` cannot be resolved on this host: {error}; ignoring hook output",
                    result.source_path
                ),
            }))),
        }
    }
    if !texts.is_empty() {
        if let Some(existing) = &instructions
            && let Some(source) = existing.sources.first()
        {
            events.push(event(EventMsg::Warning(WarningEvent {
                message: format!(
                    "UserInstructions hook output overrides user-level instructions from `{}`.",
                    source.display()
                ),
            })));
        }
        instructions = Some(UserInstructions {
            text: texts.join("\n\n"),
            sources,
        });
    }

    UserInstructionsResolution {
        instructions,
        events,
    }
}

fn event(msg: EventMsg) -> Event {
    Event {
        id: INITIAL_SUBMIT_ID.to_owned(),
        msg,
    }
}
