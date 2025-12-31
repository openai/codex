//! Agent mentions generator.
//!
//! Parses @agent-type mentions from user prompt.
//! Matches Claude Code's agent_mentions attachment (UserPrompt tier).

use crate::config::system_reminder::SystemReminderConfig;
use crate::error::Result;
use crate::system_reminder::generator::AttachmentGenerator;
use crate::system_reminder::generator::GeneratorContext;
use crate::system_reminder::generator_ext::parse_agent_mentions;
use crate::system_reminder::throttle::ThrottleConfig;
use crate::system_reminder::types::AttachmentType;
use crate::system_reminder::types::SystemReminder;
use async_trait::async_trait;

/// Agent mentions generator.
///
/// Parses user prompt for @agent-type mentions and generates invocation instructions.
#[derive(Debug)]
pub struct AgentMentionsGenerator;

impl AgentMentionsGenerator {
    /// Create a new agent mentions generator.
    pub fn new() -> Self {
        Self
    }

    /// Format agent invocation instruction (matching Claude Code format).
    fn format_invocation(&self, agent_type: &str) -> String {
        format!(
            "The user has expressed a desire to invoke the agent \"{agent_type}\". \
             Please invoke the agent appropriately, passing in the required context to it."
        )
    }
}

impl Default for AgentMentionsGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AttachmentGenerator for AgentMentionsGenerator {
    fn name(&self) -> &str {
        "agent_mentions"
    }

    fn attachment_type(&self) -> AttachmentType {
        AttachmentType::AgentMentions
    }

    async fn generate(&self, ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>> {
        // Need user prompt to parse mentions
        let user_prompt = match ctx.user_prompt {
            Some(p) => p,
            None => return Ok(None),
        };

        // Parse agent mentions
        let mentions = parse_agent_mentions(user_prompt);
        if mentions.is_empty() {
            return Ok(None);
        }

        // Format invocation instructions
        let parts: Vec<String> = mentions
            .iter()
            .map(|m| self.format_invocation(&m.agent_type))
            .collect();

        tracing::info!(
            generator = "agent_mentions",
            agent_count = parts.len(),
            agents = ?mentions.iter().map(|m| &m.agent_type).collect::<Vec<_>>(),
            "Generating agent mentions reminder"
        );

        Ok(Some(SystemReminder::new(
            AttachmentType::AgentMentions,
            parts.join("\n\n"),
        )))
    }

    fn is_enabled(&self, config: &SystemReminderConfig) -> bool {
        config.enabled && config.attachments.agent_mentions
    }

    fn throttle_config(&self) -> ThrottleConfig {
        // No throttling for user prompt tier
        ThrottleConfig {
            min_turns_between: 0,
            min_turns_after_trigger: 0,
            max_per_session: None,
        }
    }
}

// ============================================
// Tests
// ============================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::system_reminder::LspDiagnosticsMinSeverity;
    use crate::system_reminder::file_tracker::FileTracker;
    use crate::system_reminder::generator::PlanState;
    use crate::system_reminder::types::ReminderTier;
    use std::path::Path;

    fn make_context<'a>(
        user_prompt: Option<&'a str>,
        file_tracker: &'a FileTracker,
        plan_state: &'a PlanState,
    ) -> GeneratorContext<'a> {
        GeneratorContext {
            turn_number: 1,
            is_main_agent: true,
            has_user_input: true,
            user_prompt,
            cwd: Path::new("/test"),
            agent_id: "test-agent",
            file_tracker,
            is_plan_mode: false,
            plan_file_path: None,
            is_plan_reentry: false,
            plan_state,
            background_tasks: &[],
            critical_instruction: None,
            diagnostics_store: None,
            lsp_diagnostics_min_severity: LspDiagnosticsMinSeverity::default(),
            output_style: None,
            approved_plan: None,
        }
    }

    #[tokio::test]
    async fn test_returns_none_without_user_prompt() {
        let generator = AgentMentionsGenerator::new();
        let tracker = FileTracker::new();
        let plan_state = PlanState::default();
        let ctx = make_context(None, &tracker, &plan_state);

        let result = generator.generate(&ctx).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_returns_none_without_mentions() {
        let generator = AgentMentionsGenerator::new();
        let tracker = FileTracker::new();
        let plan_state = PlanState::default();
        let ctx = make_context(Some("Hello, no agent mentions here"), &tracker, &plan_state);

        let result = generator.generate(&ctx).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_parses_agent_mention() {
        let generator = AgentMentionsGenerator::new();
        let tracker = FileTracker::new();
        let plan_state = PlanState::default();
        let ctx = make_context(
            Some("Use @agent-search to find files"),
            &tracker,
            &plan_state,
        );

        let result = generator.generate(&ctx).await.unwrap();
        assert!(result.is_some());

        let reminder = result.unwrap();
        assert_eq!(reminder.attachment_type, AttachmentType::AgentMentions);
        assert!(reminder.content.contains("search"));
        assert!(reminder.content.contains("invoke the agent"));
    }

    #[tokio::test]
    async fn test_parses_multiple_agent_mentions() {
        let generator = AgentMentionsGenerator::new();
        let tracker = FileTracker::new();
        let plan_state = PlanState::default();
        let ctx = make_context(
            Some("Use @agent-search and @agent-edit"),
            &tracker,
            &plan_state,
        );

        let result = generator.generate(&ctx).await.unwrap();
        assert!(result.is_some());

        let reminder = result.unwrap();
        assert!(reminder.content.contains("search"));
        assert!(reminder.content.contains("edit"));
    }

    #[test]
    fn test_attachment_type() {
        let generator = AgentMentionsGenerator::new();
        assert_eq!(generator.attachment_type(), AttachmentType::AgentMentions);
        assert_eq!(generator.tier(), ReminderTier::UserPrompt);
    }

    #[test]
    fn test_no_throttling() {
        let generator = AgentMentionsGenerator::new();
        let config = generator.throttle_config();
        assert_eq!(config.min_turns_between, 0);
        assert_eq!(config.min_turns_after_trigger, 0);
    }
}
