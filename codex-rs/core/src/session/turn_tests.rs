use super::*;
use codex_extension_api::ExtensionData;
use codex_extension_api::TurnItemContributor;
use codex_protocol::ResponseItemId;
use codex_protocol::items::AgentMessageContent;
use codex_protocol::protocol::ItemCompletedEvent;
use codex_protocol::protocol::ItemStartedEvent;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use tracing_subscriber::prelude::*;

struct RewriteAgentMessageContributor;

impl TurnItemContributor for RewriteAgentMessageContributor {
    fn contribute<'a>(
        &'a self,
        _thread_store: &'a ExtensionData,
        _turn_store: &'a ExtensionData,
        item: &'a mut TurnItem,
    ) -> codex_extension_api::ExtensionFuture<'a, Result<(), String>> {
        Box::pin(async move {
            if let TurnItem::AgentMessage(agent_message) = item {
                agent_message.content = vec![AgentMessageContent::Text {
                    text: "plan contributed assistant text".to_string(),
                }];
            }
            Ok(())
        })
    }
}

fn assistant_output_text(text: &str) -> ResponseItem {
    ResponseItem::Message {
        status: None,
        encrypted_content: None,
        id: Some(ResponseItemId::with_suffix("msg", "1")),
        role: "assistant".to_string(),
        content: vec![ContentItem::OutputText {
            annotations: None,
            logprobs: None,
            text: text.to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    }
}

#[test]
fn post_sampling_token_estimate_is_disabled_by_always_on_sinks() {
    let feedback = codex_feedback::CodexFeedback::new();
    let subscriber = tracing_subscriber::registry()
        .with(feedback.logger_layer())
        .with(tracing_subscriber::fmt::layer().with_filter(codex_state::log_db::default_filter()));

    static METADATA: tracing::Metadata<'static> = tracing::metadata! {
        name: "post sampling token estimate filter probe",
        target: POST_SAMPLING_TOKEN_ESTIMATE_TARGET,
        level: tracing::Level::TRACE,
        fields: &["turn_id", "estimated_token_count", "message"],
        callsite: &CALLSITE,
        kind: tracing::metadata::Kind::EVENT.hint(),
    };
    static CALLSITE: tracing::callsite::DefaultCallsite =
        tracing::callsite::DefaultCallsite::new(&METADATA);

    assert!(tracing::Subscriber::register_callsite(&subscriber, &METADATA).is_never());
}

#[tokio::test]
async fn plan_mode_uses_contributed_turn_item_for_last_agent_message() {
    let (mut session, turn_context) = crate::session::tests::make_session_and_context().await;
    let mut builder = codex_extension_api::ExtensionRegistryBuilder::new();
    builder.turn_item_contributor(Arc::new(RewriteAgentMessageContributor));
    session.services.extensions = Arc::new(builder.build());
    let turn_store = ExtensionData::new(turn_context.sub_id.clone());
    let mut state = PlanModeStreamState::new(/*unfinished_plan_id*/ None);
    let mut last_agent_message = None;
    let item = assistant_output_text("original assistant text");

    let step_context = StepContext::for_test(Arc::new(turn_context));
    let handled = handle_assistant_item_done_in_plan_mode(
        &session,
        &step_context,
        &turn_store,
        &item,
        &mut state,
        /*previously_active_item*/ None,
        &mut last_agent_message,
        /*moderation*/ None,
    )
    .await;

    assert!(handled);
    assert_eq!(
        last_agent_message.as_deref(),
        Some("plan contributed assistant text")
    );
}

/// A model retry can reuse an unfinished plan. A later completion must use its own ID.
#[tokio::test]
async fn plan_attempts_only_reuse_unfinished_item_ids() {
    let (session, turn, events) = crate::session::tests::make_session_and_context_with_rx().await;
    let mut first = PlanModeStreamState::new(/*unfinished_plan_id*/ None).plan_item_state;
    let first_id = first.item_id.clone();
    first.start(&session, &turn).await;
    first.push_delta(&session, &turn, "Partial").await;

    let mut retry = PlanModeStreamState::new(first.unfinished_id()).plan_item_state;
    retry.start(&session, &turn).await;
    retry.push_delta(&session, &turn, "Plan A").await;
    retry
        .complete_with_text(&session, &turn, "Plan A".into())
        .await;
    retry
        .complete_with_text(&session, &turn, "Repeated A".into())
        .await;

    let mut next = PlanModeStreamState::new(retry.unfinished_id()).plan_item_state;
    next.start(&session, &turn).await;
    next.push_delta(&session, &turn, "Plan B").await;
    next.complete_with_text(&session, &turn, "Plan B".into())
        .await;

    let mut observed = Vec::new();
    while let Ok(event) = events.try_recv() {
        match event.msg {
            EventMsg::ItemStarted(ItemStartedEvent {
                item: TurnItem::Plan(plan),
                ..
            }) => {
                observed.push(("start", plan.id, plan.text));
            }
            EventMsg::PlanDelta(event) => observed.push(("delta", event.item_id, event.delta)),
            EventMsg::ItemCompleted(ItemCompletedEvent {
                item: TurnItem::Plan(plan),
                ..
            }) => {
                observed.push(("complete", plan.id, plan.text));
            }
            _ => {}
        }
    }
    let next_id = next.item_id;
    assert_eq!(Uuid::parse_str(&first_id).unwrap().get_version_num(), 7);
    assert_ne!(first_id, next_id);
    assert_eq!(
        observed,
        vec![
            ("start", first_id.clone(), String::new()),
            ("delta", first_id.clone(), "Partial".into()),
            ("start", first_id.clone(), String::new()),
            ("delta", first_id.clone(), "Plan A".into()),
            ("complete", first_id, "Plan A".into()),
            ("start", next_id.clone(), String::new()),
            ("delta", next_id.clone(), "Plan B".into()),
            ("complete", next_id, "Plan B".into()),
        ],
    );
}

#[test]
fn realtime_user_verification_notice_excludes_request_payload() {
    let event = EventMsg::ElicitationRequest(codex_protocol::approvals::ElicitationRequestEvent {
        turn_id: None,
        server_name: "private-server-name".to_string(),
        id: codex_protocol::mcp::RequestId::String("private-request-id".to_string()),
        request: codex_protocol::approvals::ElicitationRequest::UserVerification {
            meta: None,
            title: "private-title".to_string(),
            description: "private-description".to_string(),
            challenge: "private-challenge".to_string(),
        },
    });
    assert_eq!(
        realtime_text_for_event(&event),
        Some(RealtimeEventText::Handoff(
            "<user_verification_notice>User verification is required. Please respond in the app.</user_verification_notice>".to_string(),
            None,
        )),
    );
}
