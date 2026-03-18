use codex_protocol::protocol::HookCompletedEvent;
use codex_protocol::protocol::HookOutputEntry;
use codex_protocol::protocol::HookOutputEntryKind;
use codex_protocol::protocol::HookRunStatus;

use crate::engine::ConfiguredHandler;
use crate::engine::dispatcher;

pub(crate) fn join_text_chunks(chunks: Vec<String>) -> Option<String> {
    if chunks.is_empty() {
        None
    } else {
        Some(chunks.join("\n\n"))
    }
}

pub(crate) fn trimmed_non_empty(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub(crate) fn append_additional_context(
    entries: &mut Vec<HookOutputEntry>,
    additional_contexts_for_model: &mut Vec<String>,
    additional_context: String,
) {
    entries.push(HookOutputEntry {
        kind: HookOutputEntryKind::Context,
        text: additional_context.clone(),
    });
    additional_contexts_for_model.push(additional_context);
}

pub(crate) fn flatten_additional_contexts<'a>(
    additional_contexts: impl IntoIterator<Item = &'a [String]>,
) -> Vec<String> {
    additional_contexts
        .into_iter()
        .flat_map(|chunk| chunk.iter().cloned())
        .collect()
}

pub(crate) fn serialization_failure_hook_events(
    handlers: Vec<ConfiguredHandler>,
    turn_id: Option<String>,
    error_message: String,
) -> Vec<HookCompletedEvent> {
    handlers
        .into_iter()
        .map(|handler| {
            let mut run = dispatcher::running_summary(&handler);
            run.status = HookRunStatus::Failed;
            run.completed_at = Some(run.started_at);
            run.duration_ms = Some(0);
            run.entries = vec![HookOutputEntry {
                kind: HookOutputEntryKind::Error,
                text: error_message.clone(),
            }];
            HookCompletedEvent {
                turn_id: turn_id.clone(),
                run,
            }
        })
        .collect()
}
