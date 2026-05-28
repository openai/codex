//! Scheduling glue for extension hooks and turns that should run when a session is idle.

use std::sync::Arc;

use codex_extension_api::ResponseInjectionItem;
use codex_extension_api::ThreadIdleRequest;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;

use crate::session::TurnInput;
use crate::session::session::Session;
use crate::state::ActiveTurn;

use super::RegularTask;

const MAX_IDLE_EXTENSION_PROMPT_TOKENS: usize = 4_000;

pub(super) fn schedule_turn(session: &Arc<Session>) {
    if session
        .services
        .extensions
        .thread_lifecycle_contributors()
        .is_empty()
        && session
            .services
            .extensions
            .thread_idle_turn_contributors()
            .is_empty()
    {
        return;
    }

    let session = Arc::clone(session);
    let _handle = tokio::spawn(async move {
        maybe_start_turn(session).await;
    });
}

pub(crate) async fn maybe_start_turn(session: Arc<Session>) {
    // Give queued user-visible work the first chance to wake the session.
    session.maybe_start_turn_for_pending_work().await;
    if has_active_or_pending_work(&session).await {
        return;
    }

    notify_thread_idle(&session).await;

    // Lifecycle callbacks can race with user input or mailbox delivery. Re-check before
    // claiming an idle slot, and let the normal pending-work path handle anything that appeared.
    if has_active_or_pending_work(&session).await {
        session.maybe_start_turn_for_pending_work().await;
        return;
    }

    // Probe before reserving so a thread with no idle-turn work stays freely available.
    if next_idle_turn_candidate(&session).await.is_none() {
        return;
    }

    // Reserve the active turn after the final quiet check so another task cannot start while
    // the turn context is being built.
    if !reserve_idle_turn(&session).await {
        return;
    }

    // Re-read the idle request after reservation. This drops any stale prompt that was produced
    // before a goal/status/config change raced with the scheduler.
    let Some(candidate) = next_idle_turn_candidate(&session).await else {
        clear_reserved_idle_turn(&session).await;
        return;
    };

    if has_pending_work(&session).await {
        clear_reserved_idle_turn(&session).await;
        session.maybe_start_turn_for_pending_work().await;
        return;
    }

    if !should_start_idle_turn(&session, &candidate).await {
        clear_reserved_idle_turn(&session).await;
        return;
    }

    if has_pending_work(&session).await {
        clear_reserved_idle_turn(&session).await;
        session.maybe_start_turn_for_pending_work().await;
        return;
    }

    let input = vec![idle_extension_input(candidate.request)];

    // Treat extension-provided prompts as the new turn's initial input rather than stashing them
    // in turn-state pending input; this keeps rollback logic out of the scheduler.
    let turn_context = session
        .new_default_turn_with_sub_id(uuid::Uuid::new_v4().to_string())
        .await;
    session
        .maybe_emit_unknown_model_warning_for_turn(turn_context.as_ref())
        .await;
    session
        .start_task(turn_context, input, RegularTask::new())
        .await;
}

async fn notify_thread_idle(session: &Session) {
    for contributor in session.services.extensions.thread_lifecycle_contributors() {
        contributor
            .on_thread_idle(codex_extension_api::ThreadIdleInput {
                session_store: &session.services.session_extension_data,
                thread_store: &session.services.thread_extension_data,
            })
            .await;
    }
}

async fn has_active_or_pending_work(session: &Session) -> bool {
    session.active_turn.lock().await.is_some() || has_pending_work(session).await
}

async fn has_pending_work(session: &Session) -> bool {
    session
        .input_queue
        .has_queued_response_items_for_next_turn()
        .await
        || session.input_queue.has_trigger_turn_mailbox_items().await
}

struct IdleTurnCandidate {
    contributor_index: usize,
    request: ThreadIdleRequest,
}

async fn next_idle_turn_candidate(session: &Session) -> Option<IdleTurnCandidate> {
    let collaboration_mode = session.collaboration_mode().await;
    for (contributor_index, contributor) in session
        .services
        .extensions
        .thread_idle_turn_contributors()
        .iter()
        .enumerate()
    {
        if !idle_turn_policy_allows_mode(contributor.idle_turn_policy(), &collaboration_mode) {
            continue;
        }
        let Some(request) = contributor
            .request_thread_idle_turn(codex_extension_api::ThreadIdleInput {
                session_store: &session.services.session_extension_data,
                thread_store: &session.services.thread_extension_data,
            })
            .await
        else {
            continue;
        };
        if is_non_empty_idle_input(&request.item) {
            return Some(IdleTurnCandidate {
                contributor_index,
                request,
            });
        }
    }

    None
}

fn is_non_empty_idle_input(item: &ResponseInjectionItem) -> bool {
    match item {
        ResponseInjectionItem::HiddenContext(context) => !context.body().trim().is_empty(),
        ResponseInjectionItem::Raw(_) => true,
    }
}

fn idle_turn_policy_allows_mode(
    policy: codex_extension_api::IdleTurnPolicy,
    collaboration_mode: &CollaborationMode,
) -> bool {
    !matches!(collaboration_mode.mode, ModeKind::Plan) || policy.allows_plan_mode()
}

async fn should_start_idle_turn(session: &Session, candidate: &IdleTurnCandidate) -> bool {
    let Some(contributor) = session
        .services
        .extensions
        .thread_idle_turn_contributors()
        .get(candidate.contributor_index)
    else {
        return false;
    };
    let collaboration_mode = session.collaboration_mode().await;
    if !idle_turn_policy_allows_mode(contributor.idle_turn_policy(), &collaboration_mode) {
        return false;
    }
    contributor
        .should_start_thread_idle_turn(codex_extension_api::ThreadIdleTurnStartInput {
            request: &candidate.request,
            session_store: &session.services.session_extension_data,
            thread_store: &session.services.thread_extension_data,
        })
        .await
}

async fn reserve_idle_turn(session: &Session) -> bool {
    let mut active_turn = session.active_turn.lock().await;
    if active_turn.is_some() {
        return false;
    }
    *active_turn = Some(ActiveTurn::default());
    true
}

async fn clear_reserved_idle_turn(session: &Session) {
    let mut active_turn = session.active_turn.lock().await;
    if active_turn
        .as_ref()
        .is_some_and(|active_turn| active_turn.task.is_none())
    {
        *active_turn = None;
    }
}

fn idle_extension_input(request: ThreadIdleRequest) -> TurnInput {
    match request.item {
        ResponseInjectionItem::HiddenContext(context) => {
            let (marker, body) = context.into_parts();
            let prompt = codex_utils_string::truncate_middle_with_token_budget(
                &body,
                MAX_IDLE_EXTENSION_PROMPT_TOKENS,
            )
            .0;
            TurnInput::ResponseInputItem(
                codex_extension_api::HiddenContext::new(marker, prompt).into_response_input_item(),
            )
        }
        ResponseInjectionItem::Raw(item) => TurnInput::ResponseInputItem(item),
    }
}

#[cfg(test)]
mod tests {
    use codex_extension_api::HiddenContext;
    use codex_extension_api::HiddenContextMarker;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ResponseInputItem;

    use super::*;

    #[test]
    fn idle_extension_input_truncates_large_prompts() {
        let prompt = format!(
            "start {} end",
            "repeat ".repeat(MAX_IDLE_EXTENSION_PROMPT_TOKENS * 3)
        );
        let original_len = prompt.len();

        let request = ThreadIdleRequest::new(HiddenContext::new(
            HiddenContextMarker::new("<test_context>", "</test_context>"),
            prompt,
        ));
        let TurnInput::ResponseInputItem(ResponseInputItem::Message { content, .. }) =
            idle_extension_input(request)
        else {
            panic!("expected message input");
        };
        let [ContentItem::InputText { text }] = content.as_slice() else {
            panic!("expected one text content item");
        };

        assert!(text.starts_with("<test_context>"));
        assert!(text.contains("start"));
        assert!(text.contains("end"));
        assert!(text.len() < original_len);
    }
}
