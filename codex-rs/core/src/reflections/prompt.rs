use std::path::Path;

use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;

use super::storage::WINDOW_DIR_PATTERN;

const NEAR_LIMIT_REMINDER_HEADROOM_TOKENS: i64 = 4096;
const NEAR_LIMIT_REMINDER_PREFIX: &str = "Your context window is nearly exhausted";

pub(crate) fn usage_hint(
    context_window_size: Option<i64>,
    sidecar_path: &Path,
    custom_text: Option<&str>,
) -> String {
    if let Some(custom_text) = custom_text {
        return custom_text.to_string();
    }

    format!(
        "{context_window_text}\n\n\
Reflections is enabled. Codex automatically records visible messages and tool events from each context window under:\n\n\
{logs_path}/{window_dir_pattern}/transcript.md\n\n\
Use this directory for durable recovery notes:\n\n\
{notes_path}\n\n\
You may want to keep concise notes about your progress incrementally so you can more easily resume after the context window resets. Having things in context is useful, but when details may be needed later, prefer storing references to specific messages, files, commands, findings, or important decisions in notes rather than repeatedly reading complete files or full transcript logs.\n\n\
Future context windows will not automatically include the full previous transcript. If the current task, current user request, important tool result, or relevant instruction detail may matter later, record a concise note and reference the relevant transcript path and message heading.",
        context_window_text = context_window_text(context_window_size),
        logs_path = sidecar_path.join("logs").display(),
        notes_path = sidecar_path.join("notes").display(),
        window_dir_pattern = WINDOW_DIR_PATTERN,
    )
}

pub(crate) fn near_limit_reminder(remaining_tokens: Option<i64>) -> ResponseItem {
    let opening = match remaining_tokens {
        Some(remaining_tokens) => format!(
            "{NEAR_LIMIT_REMINDER_PREFIX} ({remaining_tokens} tokens remain before your context will be reset)."
        ),
        None => format!("{NEAR_LIMIT_REMINDER_PREFIX} and will be reset soon."),
    };
    let text = format!(
        "{opening}\n\n\
You may want to pause task work and write concise recovery notes under the Reflections notes directory before continuing. Include the current task, progress made, important files, commands, findings, decisions, and next steps. If you have not finished the user's task, you are advised NOT to send a final answer and to use this time to write or clean up notes so you can resume work in the next context window after the context reset.\n\n\
After saving notes, you may call `reflections_new_context_window` to start a fresh context window. If you continue without calling it, your context will automatically reset once the compaction limit is reached."
    );

    ResponseItem::Message {
        id: None,
        role: "developer".to_string(),
        content: vec![ContentItem::InputText { text }],
        end_turn: None,
        phase: None,
    }
}

pub(crate) fn near_limit_reminder_threshold(auto_compact_limit: i64) -> i64 {
    auto_compact_limit
        .saturating_sub(NEAR_LIMIT_REMINDER_HEADROOM_TOKENS)
        .max(0)
}

pub(crate) fn is_near_limit_reminder(item: &ResponseItem) -> bool {
    let ResponseItem::Message { role, content, .. } = item else {
        return false;
    };
    role == "developer"
        && content.iter().any(|item| {
            matches!(
                item,
                ContentItem::InputText { text } if text.starts_with(NEAR_LIMIT_REMINDER_PREFIX)
            )
        })
}

pub(crate) fn post_compaction_handoff(
    context_window_size: Option<i64>,
    logs_path: &Path,
    notes_path: &Path,
) -> String {
    format!(
        "{context_window_text}\n\n\
Reflections is enabled. Codex automatically recorded visible messages and tool events from previous context windows here:\n\n\
{logs_path}\n\n\
Use this directory for durable recovery notes:\n\n\
{notes_path}\n\n\
Your context window was reset, and you are continuing in a fresh context window. The current task may only be available in the transcript logs from previous context windows. There may be no new user message in this context window, so do not assume one exists.\n\n\
To recover context, first inspect `notes/`. If `notes/` is empty, missing, or does not clearly identify the current task and status, inspect or search the full conversation context logs under `logs/` for the previous context windows. The logs are organized by context window as `logs/{window_dir_pattern}/transcript.md`. Prefer recovering only the details you need rather than rereading every transcript file.",
        context_window_text = context_window_text(context_window_size),
        logs_path = logs_path.display(),
        notes_path = notes_path.display(),
        window_dir_pattern = WINDOW_DIR_PATTERN,
    )
}

fn context_window_text(context_window_size: Option<i64>) -> String {
    match context_window_size {
        Some(context_window_size) => {
            format!("Your context window size is {context_window_size} tokens.")
        }
        None => "Your context window size is not available for this model.".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::is_near_limit_reminder;
    use super::near_limit_reminder;
    use super::post_compaction_handoff;
    use super::usage_hint;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ResponseItem;
    use std::path::Path;

    #[test]
    fn usage_hint_mentions_sidecar_paths() {
        let hint = usage_hint(Some(98304), Path::new("/tmp/rollout.reflections"), None);

        assert!(hint.contains("Your context window size is 98304 tokens."));
        assert!(hint.contains("/tmp/rollout.reflections/logs/cwNNNNN/transcript.md"));
        assert!(hint.contains("/tmp/rollout.reflections/notes"));
    }

    #[test]
    fn handoff_mentions_logs_and_notes() {
        let handoff = post_compaction_handoff(
            None,
            Path::new("/tmp/rollout.reflections/logs"),
            Path::new("/tmp/rollout.reflections/notes"),
        );

        assert!(handoff.contains("Your context window size is not available for this model."));
        assert!(handoff.contains("previous context windows here:"));
        assert!(handoff.contains("/tmp/rollout.reflections/logs"));
        assert!(handoff.contains("logs/cwNNNNN/transcript.md"));
        assert!(!handoff.contains("/tmp/rollout.reflections/logs/cw00000/transcript.md"));
        assert!(handoff.contains("/tmp/rollout.reflections/notes"));
    }

    #[test]
    fn near_limit_reminder_is_developer_message() {
        let reminder = near_limit_reminder(Some(4058));
        let ResponseItem::Message { role, content, .. } = &reminder else {
            panic!("near-limit reminder should be a message");
        };
        assert_eq!(role, "developer");
        let [ContentItem::InputText { text }] = content.as_slice() else {
            panic!("near-limit reminder should contain exactly one text item");
        };
        assert!(text.contains(
            "Your context window is nearly exhausted (4058 tokens remain before your context will be reset)."
        ));
        assert!(text.contains("advised NOT to send a final answer"));
        assert!(text.contains("reflections_new_context_window"));
        assert!(is_near_limit_reminder(&reminder));
    }

    #[test]
    fn near_limit_reminder_supports_unknown_remaining_tokens() {
        let reminder = near_limit_reminder(None);
        let ResponseItem::Message { content, .. } = &reminder else {
            panic!("near-limit reminder should be a message");
        };
        let [ContentItem::InputText { text }] = content.as_slice() else {
            panic!("near-limit reminder should contain exactly one text item");
        };
        assert!(
            text.starts_with("Your context window is nearly exhausted and will be reset soon.")
        );
    }

    #[test]
    fn near_limit_reminder_threshold_never_goes_negative() {
        assert_eq!(super::near_limit_reminder_threshold(10_000), 5904);
        assert_eq!(super::near_limit_reminder_threshold(100), 0);
    }
}
