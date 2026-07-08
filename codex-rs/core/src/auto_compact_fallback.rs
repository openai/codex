use std::sync::Arc;

use codex_extension_api::AutoCompactFallbackContributionInput;
use codex_features::Feature;
use codex_mcp::McpConnectionManager;
use codex_protocol::error::Result as CodexResult;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::client::ModelClientSession;
use crate::context_manager::updates::build_developer_update_item;
use crate::responses_metadata::CodexResponsesRequestKind;
use crate::session::session::Session;
use crate::session::step_context::StepContext;
use crate::session::turn::SamplingRequestOptions;
use crate::session::turn::run_sampling_request;
use crate::turn_diff_tracker::TurnDiffTracker;

/// Runs the optional, extension-configured turn immediately before an automatic compaction
/// rollover. Tool follow-up inference is ignored.
///
/// The returned step refreshes request-scoped world state after any fallback tool side effects while
/// retaining the model that successfully performed compaction.
pub(crate) async fn run_auto_compact_fallback(
    sess: &Arc<Session>,
    step_context: &Arc<StepContext>,
    client_session: &mut ModelClientSession,
) -> CodexResult<Option<Arc<StepContext>>> {
    let turn_context = &step_context.turn;
    if !turn_context
        .config
        .features
        .enabled(Feature::AutoCompactFallback)
    {
        return Ok(None);
    }

    let mut prompt_sections = Vec::new();
    for contributor in sess.services.extensions.context_contributors() {
        if let Some(section) = contributor
            .contribute_auto_compact_fallback_prompt(AutoCompactFallbackContributionInput {
                thread_id: sess.thread_id(),
                turn_id: turn_context.sub_id.as_str(),
                session_store: &sess.services.session_extension_data,
                thread_store: &sess.services.thread_extension_data,
                turn_store: turn_context.extension_data.as_ref(),
                model_context_window: turn_context.model_context_window(),
            })
            .await
            && !section.trim().is_empty()
        {
            prompt_sections.push(section);
        }
    }

    let Some(developer_message) = build_developer_update_item(prompt_sections) else {
        return Ok(None);
    };
    sess.record_conversation_items(turn_context, std::slice::from_ref(&developer_message))
        .await;

    let restricted_turn = Arc::new(turn_context.with_approvals_disabled());
    let restricted_step = Arc::new(step_context.with_turn(Arc::clone(&restricted_turn)));
    let window_id = sess.current_window_id().await;
    let responses_metadata = restricted_turn.turn_metadata_state.to_responses_metadata(
        sess.installation_id.clone(),
        window_id,
        CodexResponsesRequestKind::AutoCompactFallback,
    );
    let input = sess
        .clone_history()
        .await
        .for_prompt(&restricted_turn.model_info.input_modalities);
    let turn_diff_tracker = Arc::new(Mutex::new(TurnDiffTracker::new()));
    let cancellation_token = sess
        .active_turn_context_and_cancellation_token()
        .await
        .map(|(_, token)| token)
        .unwrap_or_else(CancellationToken::new);

    let _elicitation_guard = McpElicitationAutoDenyGuard::new(restricted_step.mcp.manager_arc());
    run_sampling_request(
        Arc::clone(sess),
        restricted_step,
        Arc::clone(&restricted_turn.extension_data),
        turn_diff_tracker,
        client_session,
        &responses_metadata,
        input,
        SamplingRequestOptions::auto_compact_fallback(),
        cancellation_token,
    )
    .await?;

    Ok(Some(
        sess.capture_step_context(Arc::clone(turn_context)).await,
    ))
}

struct McpElicitationAutoDenyGuard {
    manager: Arc<McpConnectionManager>,
    previous: bool,
}

impl McpElicitationAutoDenyGuard {
    fn new(manager: Arc<McpConnectionManager>) -> Self {
        let previous = manager.elicitations_auto_deny();
        manager.set_elicitations_auto_deny(true);
        Self { manager, previous }
    }
}

impl Drop for McpElicitationAutoDenyGuard {
    fn drop(&mut self) {
        self.manager.set_elicitations_auto_deny(self.previous);
    }
}
