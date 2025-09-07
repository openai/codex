use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::history_cell;
use crate::markdown::extract_text_without_code;
use codex_core::ConversationManager;
use codex_core::config::Config;
use codex_core::model_family::find_family_for_model;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_protocol::config_types::ReasoningEffort;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::config_types::Verbosity;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
struct JudgeResponse {
    #[serde(default)]
    follow_plan: bool,
    #[serde(default)]
    reason: Option<String>,
}

/// Spawn a lightweight secondary session to evaluate the last assistant reply
/// and decide whether to follow the plan.
pub(crate) async fn run_post_turn_evaluation(
    server: Arc<ConversationManager>,
    base_config: &Config,
    prompt_text: &str,
    last_agent_message: Option<String>,
    app_event_tx: AppEventSender,
    main_session_id: Option<uuid::Uuid>,
    autopilot_on_follow: bool,
) {
    // Prepare a safe, tool-free config for the judge session.
    let mut cfg = base_config.clone();
    cfg.include_plan_tool = false;
    cfg.include_apply_patch_tool = false;
    cfg.include_view_image_tool = false;
    cfg.tools_web_search_request = false;
    // Force a lean GPT‑5 configuration for the background judge session.
    cfg.model = "gpt-5".to_string();
    if let Some(fam) = find_family_for_model(&cfg.model) {
        cfg.model_family = fam;
    }
    cfg.model_reasoning_effort = ReasoningEffort::Minimal;
    // Use Detailed summary to satisfy provider constraints for GPT-5.
    cfg.model_reasoning_summary = ReasoningSummary::Detailed;
    cfg.model_verbosity = Some(Verbosity::Low);

    // Create the new conversation.
    let (conv, conv_id) = match server.new_conversation(cfg).await {
        Ok(newc) => (newc.conversation, newc.conversation_id),
        Err(e) => {
            let cell = history_cell::new_turn_judge_result(
                false,
                Some(format!("falha ao iniciar avaliação: {e}")),
            );
            app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(cell)));
            return;
        }
    };

    // Build a concise user message combining the supplied prompt and the last answer.
    let mut user_text = String::new();
    user_text.push_str(prompt_text);
    if let Some(ans) = last_agent_message {
        let prose = extract_text_without_code(&ans);
        user_text.push_str("\n\nResposta do assistente (somente texto):\n---\n");
        user_text.push_str(&prose);
        user_text.push_str("\n---\n");
    }

    // Submit the user message to the judge session.
    let _ = conv
        .submit(Op::UserInput {
            items: vec![codex_core::protocol::InputItem::Text { text: user_text }],
        })
        .await;

    // Collect until TaskComplete to capture the final message.
    let mut final_text: Option<String> = None;
    while let Ok(event) = conv.next_event().await {
        if let EventMsg::TaskComplete(ev) = event.msg {
            final_text = ev.last_agent_message;
            break;
        }
    }

    // Parse the judge output.
    let (follow, reason) = match final_text.as_deref() {
        Some(text) => parse_judge_output(text),
        None => (false, Some("sem resposta do avaliador".to_string())),
    };

    // Push a compact decision cell into the main transcript.
    let cell = history_cell::new_turn_judge_result(follow, reason);
    app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(cell)));

    // If approved and autopilot is enabled, submit a follow-up user message to the main session.
    if follow
        && autopilot_on_follow
        && let Some(sess_id) = main_session_id
        && let Ok(conv) = server.get_conversation(sess_id).await
    {
        let _ = conv
            .submit(Op::UserInput {
                items: vec![codex_core::protocol::InputItem::Text {
                    text: "Siga o plano e prossiga para a próxima etapa.".to_string(),
                }],
            })
            .await;
    }

    // Best-effort cleanup of the short-lived judge conversation.
    server.remove_conversation(conv_id).await;
}

fn parse_judge_output(text: &str) -> (bool, Option<String>) {
    // First try JSON with { "follow_plan": bool, "reason": string }
    if let Ok(v) = serde_json::from_str::<JudgeResponse>(text) {
        return (v.follow_plan, v.reason);
    }

    // Heuristic fallback: look for clear yes/no markers.
    let lower = text.to_ascii_lowercase();
    let yes_markers = [
        "follow_plan: yes",
        "seguir o plano: sim",
        "seguir o plano: yes",
        "seguir o plano: true",
        "follow",
        "sim",
    ];
    let no_markers = [
        "follow_plan: no",
        "seguir o plano: não",
        "seguir o plano: nao",
        "seguir o plano: no",
        "não seguir",
        "nao seguir",
        "no",
    ];

    if yes_markers.iter().any(|m| lower.contains(m)) {
        (true, Some(text.trim().to_string()))
    } else if no_markers.iter().any(|m| lower.contains(m)) {
        (false, Some(text.trim().to_string()))
    } else {
        // Default to not following when unclear.
        (false, Some(text.trim().to_string()))
    }
}
