//! System reminder integration for conversation flow.
//!
//! Minimal integration hooks - bulk logic in system_reminder/ module.

use crate::config::output_style::OutputStyle;
use crate::config::system_reminder::LspDiagnosticsMinSeverity;
use crate::system_reminder::ApprovedPlanInfo;
use crate::system_reminder::BackgroundTaskInfo;
use crate::system_reminder::FileTracker;
use crate::system_reminder::GeneratorContext;
use crate::system_reminder::PlanState;
use crate::system_reminder::SystemReminder;
use crate::system_reminder::SystemReminderOrchestrator;
use crate::user_instructions::UserInstructions;
use codex_lsp::DiagnosticsStore;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use std::path::Path;
use std::sync::Arc;

/// Inject system reminders into conversation history.
///
/// Called from conversation flow before prompt assembly.
/// Matches attachment injection in Claude Code chunks.121.mjs.
pub async fn inject_system_reminders(
    history: &mut Vec<ResponseItem>,
    orchestrator: &SystemReminderOrchestrator,
    ctx: &GeneratorContext<'_>,
) {
    let reminders = orchestrator.generate_all(ctx).await;

    if reminders.is_empty() {
        return;
    }

    // Find insertion position (after environment_context and user_instructions)
    let insert_pos = find_insert_position(history);

    tracing::info!(
        count = reminders.len(),
        position = insert_pos,
        "Injecting system reminders into conversation"
    );

    // Insert reminders in reverse order to maintain order
    for reminder in reminders.into_iter().rev() {
        history.insert(insert_pos, reminder.into());
    }
}

/// Find the position to insert system reminders.
///
/// Reminders should appear after environment_context and user_instructions.
fn find_insert_position(history: &[ResponseItem]) -> usize {
    history
        .iter()
        .position(|item| {
            if let ResponseItem::Message { content, .. } = item {
                !is_environment_context(content)
                    && !UserInstructions::is_user_instructions(content)
                    && !SystemReminder::is_system_reminder(content)
            } else {
                true
            }
        })
        .unwrap_or(history.len())
}

/// Check if message content is an environment context message.
fn is_environment_context(content: &[ContentItem]) -> bool {
    if let [ContentItem::InputText { text }] = content {
        text.starts_with("<environment_context>")
    } else {
        false
    }
}

/// Build GeneratorContext from turn state.
///
/// Helper to construct GeneratorContext from various session components.
#[allow(clippy::too_many_arguments)]
pub fn build_generator_context<'a>(
    turn_number: i32,
    agent_id: &'a str,
    is_main_agent: bool,
    has_user_input: bool,
    user_prompt: Option<&'a str>,
    cwd: &'a Path,
    is_plan_mode: bool,
    plan_file_path: Option<&'a str>,
    is_plan_reentry: bool,
    file_tracker: &'a FileTracker,
    plan_state: &'a PlanState,
    background_tasks: &'a [BackgroundTaskInfo],
    critical_instruction: Option<&'a str>,
    diagnostics_store: Option<Arc<DiagnosticsStore>>,
    lsp_diagnostics_min_severity: LspDiagnosticsMinSeverity,
    output_style: Option<&'a OutputStyle>,
    approved_plan: Option<ApprovedPlanInfo>,
) -> GeneratorContext<'a> {
    GeneratorContext {
        turn_number,
        agent_id,
        is_main_agent,
        has_user_input,
        user_prompt,
        cwd,
        is_plan_mode,
        plan_file_path,
        is_plan_reentry,
        file_tracker,
        plan_state,
        background_tasks,
        critical_instruction,
        diagnostics_store,
        lsp_diagnostics_min_severity,
        output_style,
        approved_plan,
    }
}

// ============================================
// Tests
// ============================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::system_reminder::SystemReminderConfig;

    fn make_environment_context_message() -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "<environment_context>\n<cwd>/test</cwd>\n</environment_context>".to_string(),
            }],
        }
    }

    fn make_user_instructions_message() -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "# AGENTS.md instructions for /test\n\n<INSTRUCTIONS>test</INSTRUCTIONS>"
                    .to_string(),
            }],
        }
    }

    fn make_user_message(text: &str) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: text.to_string(),
            }],
        }
    }

    #[test]
    fn test_find_insert_position_empty() {
        let history: Vec<ResponseItem> = vec![];
        assert_eq!(find_insert_position(&history), 0);
    }

    #[test]
    fn test_find_insert_position_after_env_context() {
        let history = vec![
            make_environment_context_message(),
            make_user_message("Hello"),
        ];
        assert_eq!(find_insert_position(&history), 1);
    }

    #[test]
    fn test_find_insert_position_after_instructions() {
        let history = vec![
            make_environment_context_message(),
            make_user_instructions_message(),
            make_user_message("Hello"),
        ];
        assert_eq!(find_insert_position(&history), 2);
    }

    #[test]
    fn test_is_environment_context() {
        let content = vec![ContentItem::InputText {
            text: "<environment_context>\n<cwd>/test</cwd>\n</environment_context>".to_string(),
        }];
        assert!(is_environment_context(&content));

        let not_env = vec![ContentItem::InputText {
            text: "Hello world".to_string(),
        }];
        assert!(!is_environment_context(&not_env));
    }

    #[tokio::test]
    async fn test_inject_system_reminders() {
        let config = SystemReminderConfig::default();
        let orchestrator = SystemReminderOrchestrator::new(config);

        let mut history = vec![
            make_environment_context_message(),
            make_user_instructions_message(),
            make_user_message("Hello"),
        ];

        let tracker = FileTracker::new();
        let plan_state = PlanState::default();
        let bg_tasks = vec![];

        let ctx = build_generator_context(
            1,
            "test-agent",
            true,
            true,
            None, // user_prompt
            Path::new("/test"),
            false,
            None,
            false,
            &tracker,
            &plan_state,
            &bg_tasks,
            Some("Critical: Always test"),
            None,
            LspDiagnosticsMinSeverity::default(),
            None, // output_style
            None, // approved_plan
        );

        inject_system_reminders(&mut history, &orchestrator, &ctx).await;

        // History should have grown (at least the critical instruction)
        assert!(history.len() >= 4);

        // Check that a system reminder was inserted at position 2
        if let ResponseItem::Message { content, .. } = &history[2] {
            if let [ContentItem::InputText { text }] = &content[..] {
                assert!(
                    text.starts_with("<system-reminder>"),
                    "Expected system reminder at position 2"
                );
            }
        }
    }
}
