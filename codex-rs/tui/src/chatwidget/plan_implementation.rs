use codex_protocol::config_types::CollaborationModeMask;

use crate::app_event::AppEvent;
use crate::bottom_pane::SelectionAction;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionViewParams;
use crate::bottom_pane::popup_consts::standard_popup_hint_line;

pub(super) const PLAN_IMPLEMENTATION_TITLE: &str = "Implement this plan?";
const PLAN_IMPLEMENTATION_YES: &str = "Yes, implement this plan";
const PLAN_IMPLEMENTATION_CLEAR_CONTEXT: &str = "Yes, clear context and implement";
const PLAN_IMPLEMENTATION_NO: &str = "No, stay in Plan mode";
pub(super) const PLAN_IMPLEMENTATION_CODING_MESSAGE: &str = "Implement the plan.";
pub(super) const PLAN_IMPLEMENTATION_CLEAR_CONTEXT_PREFIX: &str = concat!(
    "A previous agent produced the plan below to accomplish the user's task. ",
    "Implement the plan in a fresh context. Treat the plan as the source of ",
    "user intent, re-read files as needed, and carry the work through ",
    "implementation and verification."
);
pub(super) const PLAN_IMPLEMENTATION_DEFAULT_UNAVAILABLE: &str = "Default mode unavailable";
pub(super) const PLAN_IMPLEMENTATION_NO_APPROVED_PLAN: &str = "No approved plan available";

pub(super) fn selection_view_params(
    default_mask: Option<CollaborationModeMask>,
    plan_markdown: Option<&str>,
) -> SelectionViewParams {
    let (implement_actions, implement_disabled_reason) = match default_mask.clone() {
        Some(mask) => {
            let user_text = PLAN_IMPLEMENTATION_CODING_MESSAGE.to_string();
            let actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
                tx.send(AppEvent::SubmitUserMessageWithMode {
                    text: user_text.clone(),
                    collaboration_mode: mask.clone(),
                });
            })];
            (actions, None)
        }
        None => (
            Vec::new(),
            Some(PLAN_IMPLEMENTATION_DEFAULT_UNAVAILABLE.to_string()),
        ),
    };

    let (clear_context_actions, clear_context_disabled_reason) = match (default_mask, plan_markdown)
    {
        (None, _) => (
            Vec::new(),
            Some(PLAN_IMPLEMENTATION_DEFAULT_UNAVAILABLE.to_string()),
        ),
        (Some(_), Some(plan_markdown)) if !plan_markdown.trim().is_empty() => {
            let user_text =
                format!("{PLAN_IMPLEMENTATION_CLEAR_CONTEXT_PREFIX}\n\n{plan_markdown}");
            let actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
                tx.send(AppEvent::ClearUiAndSubmitUserMessage {
                    text: user_text.clone(),
                });
            })];
            (actions, None)
        }
        (Some(_), _) => (
            Vec::new(),
            Some(PLAN_IMPLEMENTATION_NO_APPROVED_PLAN.to_string()),
        ),
    };

    SelectionViewParams {
        title: Some(PLAN_IMPLEMENTATION_TITLE.to_string()),
        subtitle: None,
        footer_hint: Some(standard_popup_hint_line()),
        items: vec![
            SelectionItem {
                name: PLAN_IMPLEMENTATION_YES.to_string(),
                description: Some("Switch to Default and start coding.".to_string()),
                selected_description: None,
                is_current: false,
                actions: implement_actions,
                disabled_reason: implement_disabled_reason,
                dismiss_on_select: true,
                ..Default::default()
            },
            SelectionItem {
                name: PLAN_IMPLEMENTATION_CLEAR_CONTEXT.to_string(),
                description: Some("Fresh thread with this plan.".to_string()),
                selected_description: None,
                is_current: false,
                actions: clear_context_actions,
                disabled_reason: clear_context_disabled_reason,
                dismiss_on_select: true,
                ..Default::default()
            },
            SelectionItem {
                name: PLAN_IMPLEMENTATION_NO.to_string(),
                description: Some("Continue planning with the model.".to_string()),
                selected_description: None,
                is_current: false,
                actions: Vec::new(),
                dismiss_on_select: true,
                ..Default::default()
            },
        ],
        ..Default::default()
    }
}
