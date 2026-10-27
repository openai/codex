//! Turn-scoped safety recourse: match the terminal policy error and offer safer rewrites.

mod metadata;

use super::*;
use codex_app_server_protocol::TurnError;
use codex_app_server_protocol::TurnModerationMetadataNotification;
use metadata::Domain;
use metadata::Recourse;

pub(super) const VIEW_ID: &str = "safety-recourse";

pub(super) struct PendingRecourse {
    domain: Domain,
    pub(super) notice: Box<dyn HistoryCell>,
    result: Option<Recourse>,
    pub(super) deferred_error: Option<TurnError>,
}

pub(super) fn on_metadata(chat: &mut ChatWidget, notification: TurnModerationMetadataNotification) {
    if !chat.turn_lifecycle.agent_turn_running
        || !matches_turn(chat, &notification.thread_id, &notification.turn_id)
        || chat.side_conversation_active()
        || chat.config_ref().ephemeral
        || chat.blocks_direct_input
        || (chat.safety_recourse.is_some() && chat.has_queued_follow_up_messages())
    {
        return;
    }
    let Some((domain, result)) = metadata::parse(notification.metadata) else {
        return;
    };
    let pending = chat.safety_recourse.get_or_insert(PendingRecourse {
        domain,
        notice: Box::new(match domain {
            Domain::Bio => history_cell::new_safety_access_block_event(),
            Domain::Cyber => history_cell::new_cyber_policy_error_event(chat.cyber_policy_notice()),
        }),
        result: None,
        deferred_error: None,
    });
    if pending.domain == domain && pending.result.is_none() {
        pending.result = result;
        chat.clear_safety_buffering();
        chat.bottom_pane.ensure_status_indicator();
        chat.set_status_header("Finding an explanation and safer ways to continue…".into());
    }
}

fn matches_turn(chat: &ChatWidget, thread_id: &str, turn_id: &str) -> bool {
    chat.last_turn_id() == Some(turn_id)
        && chat
            .thread_id()
            .is_some_and(|id| id.to_string() == thread_id)
}

fn matches_error(domain: Domain, error: &TurnError) -> bool {
    match domain {
        Domain::Bio => super::turn_runtime::is_bio_policy_error(&error.message),
        Domain::Cyber => error.codex_error_info == Some(AppServerCodexErrorInfo::CyberPolicy),
    }
}

pub(super) fn defer_error(chat: &mut ChatWidget, notification: &ErrorNotification) -> bool {
    if matches_turn(chat, &notification.thread_id, &notification.turn_id)
        && let Some(pending) = &mut chat.safety_recourse
        && matches_error(pending.domain, &notification.error)
    {
        pending.deferred_error = Some(notification.error.clone());
        return true;
    }
    false
}

pub(super) fn render_failure(
    chat: &mut ChatWidget,
    pending: Option<PendingRecourse>,
    error: &TurnError,
) -> bool {
    let Some(pending) = pending else {
        return false;
    };
    let Some(result) = pending.result else {
        return false;
    };
    if chat.has_queued_follow_up_messages() || !matches_error(pending.domain, error) {
        return false;
    }
    let (Some(thread_id), Some(turn_id)) = (chat.thread_id(), chat.last_turn_id()) else {
        return false;
    };
    let mut items = Vec::new();
    let subtitle = (!result.suggestions.is_empty())
        .then(|| "Choosing a rewrite sends it in a new chat.".into());
    for (label, prompt) in result.suggestions {
        let turn_id = turn_id.to_string();
        items.push(SelectionItem {
            name: label,
            actions: vec![Box::new(move |tx| {
                tx.send(AppEvent::ContinueSafetyRecourse {
                    thread_id,
                    turn_id: turn_id.clone(),
                    prompt: prompt.clone(),
                })
            })],
            ..Default::default()
        });
    }
    let mut cell = match pending.domain {
        Domain::Bio => history_cell::new_safety_access_block_event(),
        Domain::Cyber => {
            let notice = chat.cyber_policy_notice();
            if notice == crate::daybreak::Notice::Disabled {
                items.push(SelectionItem {
                    name: "Enable Daybreak".into(),
                    actions: vec![Box::new(move |tx| {
                        tx.send(AppEvent::PersistDaybreakSelection {
                            thread_id,
                            enabled: true,
                        })
                    })],
                    dismiss_on_select: true,
                    ..Default::default()
                });
            }
            history_cell::new_cyber_policy_error_event(notice)
        }
    };
    cell.explanation = Some(result.explanation);
    chat.input_queue.submit_pending_steers_after_interrupt = false;
    chat.finalize_turn();
    chat.add_to_history(cell);
    if !items.is_empty() {
        chat.bottom_pane.show_selection_view(SelectionViewParams {
            view_id: Some(VIEW_ID),
            title: Some("Here are a few ways I can help:".into()),
            subtitle,
            items,
            ..SelectionViewParams::picker()
        });
        chat.defer_input_until_settings_applied();
    }
    chat.request_redraw();
    true
}
