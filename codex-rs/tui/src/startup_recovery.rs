//! Retain one startup draft in memory until the intended session accepts the handoff.
//!
//! Recovery belongs to the startup task, never a process-global file, log, or session. After a
//! startup error the caller restores the terminal before showing the unsent text. Printing takes
//! the snapshot, so nested fatal/error paths cannot print it twice.

use std::cell::RefCell;
use std::future::Future;
use std::io::Write;

use crate::bottom_pane::ChatComposer;
use crate::bottom_pane::ComposerDraftSnapshot;
use crate::bottom_pane::InputResult;

tokio::task_local! {
    static DRAFT: RefCell<Option<RecoveryDraft>>;
}

struct RecoveryDraft {
    snapshot: ComposerDraftSnapshot,
    stage: RecoveryStage,
}

enum RecoveryStage {
    Startup,
    HandedOff,
    Submitted,
    Dispatched(String),
}

/// Keep draft recovery isolated to this startup, including futures polled during initialization.
pub(crate) async fn scope<F: Future>(future: F) -> F::Output {
    DRAFT.scope(RefCell::new(/*value*/ None), future).await
}

/// Replace the single recovery copy after input changes or before handing the draft off.
pub(crate) fn remember(snapshot: ComposerDraftSnapshot) {
    let _ = DRAFT.try_with(|draft| {
        *draft.borrow_mut() = Some(RecoveryDraft {
            snapshot,
            stage: RecoveryStage::Startup,
        });
    });
}

/// Track the merged destination only once startup has transferred its draft into ChatWidget.
pub(crate) fn handed_off(snapshot: impl FnOnce() -> ComposerDraftSnapshot) {
    let _ = DRAFT.try_with(|draft| {
        if let Some(draft) = draft.borrow_mut().as_mut() {
            draft.snapshot = snapshot();
            draft.stage = RecoveryStage::HandedOff;
        }
    });
}

/// Refresh pending recovery after local edits without retaining any post-startup input.
pub(crate) fn refresh_after_handoff(snapshot: impl FnOnce() -> ComposerDraftSnapshot) {
    let _ = DRAFT.try_with(|draft| {
        if let Some(draft) = draft.borrow_mut().as_mut()
            && matches!(draft.stage, RecoveryStage::HandedOff)
        {
            draft.snapshot = snapshot();
        }
    });
}

/// Retain actual submission output, including burst text flushed while consuming the composer.
pub(crate) fn submitted(result: &InputResult) {
    let submission = match result {
        InputResult::Submitted {
            text,
            text_elements,
        } => Some((text, text_elements, &[][..])),
        InputResult::Queued {
            text,
            text_elements,
            pending_pastes,
            ..
        } => Some((text, text_elements, pending_pastes.as_slice())),
        InputResult::CommandWithArgs(crate::slash_command::SlashCommand::Plan, ..) => None,
        InputResult::None
        | InputResult::ParentOwnedInputBlocked
        | InputResult::Command(_)
        | InputResult::ServiceTierCommand(_)
        | InputResult::CommandWithArgs(..) => return,
    };
    let _ = DRAFT.try_with(|draft| {
        if let Some(draft) = draft.borrow_mut().as_mut()
            && matches!(draft.stage, RecoveryStage::HandedOff)
        {
            if let Some((text, text_elements, pending_pastes)) = submission {
                draft.snapshot.text.clone_from(text);
                draft.snapshot.text_elements.clone_from(text_elements);
                draft.snapshot.pending_pastes = pending_pastes.to_vec();
                draft.snapshot.cursor = text.len();
            }
            // Later editor input is a different draft; preserve this one until acknowledgment.
            draft.stage = RecoveryStage::Submitted;
        }
    });
}

/// Associate the recovered submission with its actual queued request, not an earlier CLI prompt.
pub(crate) fn bind_submission(text: &str, client_id: &str) {
    let _ = DRAFT.try_with(|draft| {
        if let Some(draft) = draft.borrow_mut().as_mut()
            && matches!(draft.stage, RecoveryStage::Submitted)
        {
            let expanded = ChatComposer::expand_pending_pastes(
                &draft.snapshot.text,
                draft.snapshot.text_elements.clone(),
                &draft.snapshot.pending_pastes,
            )
            .0;
            // /plan submits its arguments while recovery retains the original command.
            if expanded == text
                || crate::bottom_pane::prompt_args::parse_slash_name(&expanded)
                    .is_some_and(|(name, args, _)| name == "plan" && args.trim() == text)
            {
                draft.stage = RecoveryStage::Dispatched(client_id.to_owned());
            }
        }
    });
}

/// Only acceptance of the recovered message releases its unsent copy.
pub(crate) fn acknowledged(client_id: &str) {
    let _ = DRAFT.try_with(|draft| {
        let mut draft = draft.borrow_mut();
        if draft.as_ref().is_some_and(
            |draft| matches!(&draft.stage, RecoveryStage::Dispatched(id) if id == client_id),
        ) {
            draft.take();
        }
    });
}

/// Forget recovery once startup safely transfers the draft or the user cancels the session.
pub(crate) fn clear() {
    let _ = DRAFT.try_with(|draft| {
        draft.borrow_mut().take();
    });
}

fn take_unsent_text() -> Option<String> {
    let snapshot = DRAFT
        .try_with(|draft| draft.borrow_mut().take())
        .ok()??
        .snapshot;
    let (text, _) = ChatComposer::expand_pending_pastes(
        &snapshot.text,
        snapshot.text_elements,
        &snapshot.pending_pastes,
    );
    if text.is_empty() {
        return None;
    }
    // Keep the typed content recoverable without letting pasted control bytes change the terminal.
    let mut visible = String::with_capacity(text.len());
    for character in text.chars() {
        if character.is_control() && !matches!(character, '\n' | '\t') {
            visible.extend(character.escape_default());
        } else {
            visible.push(character);
        }
    }
    Some(visible)
}

/// Display unsent text only after the caller has restored normal terminal modes.
pub(crate) fn print_unsent_draft() {
    if let Some(text) = take_unsent_text() {
        let _ = writeln!(std::io::stderr().lock(), "\nUnsent draft:\n{text}\n");
    }
}

#[cfg(test)]
#[path = "startup_recovery_tests.rs"]
mod tests;
