use super::*;
use codex_extension_api::ExtensionData;
use codex_extension_api::TurnInputContext;
use codex_extension_api::TurnInputContributor;
use codex_extension_api::TurnItemContributor;
use codex_protocol::items::AgentMessageContent;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use std::sync::Mutex;
use tokio_util::sync::CancellationToken;

struct RewriteAgentMessageContributor;

struct CaptureTurnInputContributor {
    input: Arc<Mutex<Option<TurnInputContext>>>,
}

impl TurnInputContributor for CaptureTurnInputContributor {
    fn contribute<'a>(
        &'a self,
        input: TurnInputContext,
        _session_store: &'a ExtensionData,
        _thread_store: &'a ExtensionData,
        _turn_store: &'a ExtensionData,
    ) -> codex_extension_api::ExtensionFuture<'a, Vec<Box<dyn ContextualUserFragment + Send>>> {
        Box::pin(async move {
            *self.input.lock().expect("capture input lock") = Some(input);
            Vec::new()
        })
    }
}

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
        id: Some("msg-1".to_string()),
        role: "assistant".to_string(),
        content: vec![ContentItem::OutputText {
            text: text.to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    }
}

#[tokio::test]
async fn plan_mode_uses_contributed_turn_item_for_last_agent_message() {
    let (mut session, turn_context) = crate::session::tests::make_session_and_context().await;
    let mut builder = codex_extension_api::ExtensionRegistryBuilder::new();
    builder.turn_item_contributor(Arc::new(RewriteAgentMessageContributor));
    session.services.extensions = Arc::new(builder.build());
    let turn_store = ExtensionData::new(turn_context.sub_id.clone());
    let mut state = PlanModeStreamState::new(&turn_context.sub_id);
    let mut last_agent_message = None;
    let item = assistant_output_text("original assistant text");

    let handled = handle_assistant_item_done_in_plan_mode(
        &session,
        &turn_context,
        &turn_store,
        &item,
        &mut state,
        /*previously_active_item*/ None,
        &mut last_agent_message,
    )
    .await;

    assert!(handled);
    assert_eq!(
        last_agent_message.as_deref(),
        Some("plan contributed assistant text")
    );
}

#[tokio::test]
async fn turn_input_contributors_receive_step_environments() {
    let (mut session, mut turn_context) = crate::session::tests::make_session_and_context().await;
    let step_environments = turn_context.environments.clone();
    turn_context.environments = Default::default();
    let turn_context = Arc::new(turn_context);
    let step_context = StepContext::new(
        Arc::clone(&turn_context),
        step_environments.clone(),
        /*loaded_agents_md*/ None,
    );
    let captured_input = Arc::new(Mutex::new(None));
    let mut builder = codex_extension_api::ExtensionRegistryBuilder::new();
    builder.turn_input_contributor(Arc::new(CaptureTurnInputContributor {
        input: Arc::clone(&captured_input),
    }));
    session.services.extensions = Arc::new(builder.build());

    build_extension_turn_input_items(
        &Arc::new(session),
        &step_context,
        &[],
        &CancellationToken::new(),
    )
    .await
    .expect("contributor should complete");

    let input = captured_input
        .lock()
        .expect("capture input lock")
        .take()
        .expect("contributor should receive input");
    let [environment] = input.environments.as_slice() else {
        panic!("expected one contributed environment");
    };
    let expected_environment = step_environments.primary().expect("step environment");
    let expected_cwd = expected_environment
        .cwd()
        .to_abs_path()
        .expect("test cwd should be host native");
    assert_eq!(
        (
            environment.environment_id.as_str(),
            environment.cwd.as_path(),
            environment.is_primary,
        ),
        (
            expected_environment.environment_id.as_str(),
            expected_cwd.as_path(),
            true,
        )
    );
}
