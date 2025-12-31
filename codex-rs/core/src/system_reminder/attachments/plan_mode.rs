//! Plan mode generator.
//!
//! Plan mode instructions and re-entry handling (P0).
//! Matches VH5() in Claude Code chunks.107.mjs:1886-1908
//! And Sb3()/_b3() in chunks.153.mjs:2890-2977.

use crate::config::system_reminder::SystemReminderConfig;
use crate::error::Result;
use crate::system_reminder::generator::AttachmentGenerator;
use crate::system_reminder::generator::GeneratorContext;
use crate::system_reminder::throttle::ThrottleConfig;
use crate::system_reminder::throttle::default_throttle_config;
use crate::system_reminder::types::AttachmentType;
use crate::system_reminder::types::SystemReminder;
use async_trait::async_trait;
use std::path::Path;

/// Plan mode generator.
///
/// Generates plan mode instructions and re-entry guidance.
#[derive(Debug)]
pub struct PlanModeGenerator;

impl PlanModeGenerator {
    /// Create a new plan mode generator.
    pub fn new() -> Self {
        Self
    }

    /// Build plan mode content for main agent (matches Sb3).
    fn build_main_agent_content(&self, ctx: &GeneratorContext<'_>) -> String {
        let plan_file_info = if let Some(path) = ctx.plan_file_path {
            let plan_exists = Path::new(path).exists();
            if plan_exists {
                format!(
                    "## Plan File Info:\n\
                     A plan file already exists at {path}. You can read it and make tweakcc \
                     edits using the Edit tool."
                )
            } else {
                format!(
                    "## Plan File Info:\n\
                     No plan file exists yet. You should create your plan at {path} using the Write tool."
                )
            }
        } else {
            String::new()
        };

        format!(
            "Plan mode is active. The user indicated that they do not want you to execute yet -- \
             you MUST NOT make any edits (with the exception of the plan file mentioned below), \
             run any non-readonly tools (including changing configs or making commits), \
             or otherwise make any changes to the system. This supercedes any other instructions \
             you have received.\n\n\
             {plan_file_info}\n\
             You should build your plan incrementally by writing to or editing this file. \
             NOTE that this is the only file you are allowed to edit - other than this you are \
             only allowed to take READ-ONLY actions.\n\n\
             ## Plan Workflow\n\n\
             ### Phase 1: Initial Understanding\n\
             Goal: Gain a comprehensive understanding of the user's request by reading through code \
             and asking them questions. Critical: In this phase you should only use the Explore \
             subagent type.\n\n\
             1. Focus on understanding the user's request and the code associated with their request\n\
             2. Launch Explore agents to efficiently explore the codebase\n\
             3. After exploring the code, use the AskUserQuestion tool to clarify ambiguities\n\n\
             ### Phase 2: Design\n\
             Goal: Design an implementation approach.\n\n\
             Launch Plan agent(s) to design the implementation based on your exploration results.\n\n\
             ### Phase 3: Review\n\
             Goal: Review the plan(s) and ensure alignment with the user's intentions.\n\n\
             1. Read the critical files identified by agents\n\
             2. Ensure that the plans align with the user's original request\n\
             3. Use AskUserQuestion to clarify any remaining questions\n\n\
             ### Phase 4: Final Plan\n\
             Goal: Write your final plan to the plan file.\n\n\
             - Include only your recommended approach\n\
             - Ensure the plan is concise but detailed enough to execute\n\
             - Include paths of critical files to be modified\n\n\
             ### Phase 5: Call ExitPlanMode\n\
             At the very end of your turn, once you have asked the user questions and are happy \
             with your final plan file - you should always call ExitPlanMode to indicate to the \
             user that you are done planning.\n\n\
             This is critical - your turn should only end with either asking the user a question \
             (using AskUserQuestion) or calling ExitPlanMode. Do not stop unless it's for these \
             2 reasons.\n\n\
             NOTE: At any point in this workflow you should feel free to ask the user questions \
             or clarifications using AskUserQuestion. Don't make large assumptions about user intent. \
             The goal is to present a well-researched plan to the user, and tie any loose ends \
             before implementation begins."
        )
    }

    /// Build plan mode re-entry content.
    fn build_reentry_content(&self, plan_file_path: &str) -> String {
        format!(
            "## Re-entering Plan Mode\n\n\
             You are returning to plan mode after having previously exited it. \
             A plan file exists at {plan_file_path} from your previous planning session.\n\n\
             **Before proceeding with any new planning, you should:**\n\
             1. Read the existing plan file to understand what was previously planned\n\
             2. Evaluate the user's current request against that plan\n\
             3. Decide how to proceed:\n\
                - **Different task**: If the user's request is for a different task—even if it's similar \
                  or related—start fresh by overwriting the existing plan\n\
                - **Same task, continuing**: If this is explicitly a continuation or refinement of \
                  the exact same task, modify the existing plan while cleaning up outdated sections\n\
             4. Continue on with the plan process and most importantly you should always edit the \
                plan file one way or the other before calling ExitPlanMode\n\n\
             Treat this as a fresh planning session. Do not assume the existing plan is relevant \
             without evaluating it first."
        )
    }
}

impl Default for PlanModeGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AttachmentGenerator for PlanModeGenerator {
    fn name(&self) -> &str {
        "plan_mode"
    }

    fn attachment_type(&self) -> AttachmentType {
        AttachmentType::PlanMode
    }

    async fn generate(&self, ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>> {
        if !ctx.is_plan_mode {
            return Ok(None);
        }

        // Build content based on mode
        let content = if ctx.is_plan_reentry {
            if let Some(path) = ctx.plan_file_path {
                // Both re-entry content and main content
                format!(
                    "{}\n\n{}",
                    self.build_reentry_content(path),
                    self.build_main_agent_content(ctx)
                )
            } else {
                self.build_main_agent_content(ctx)
            }
        } else {
            self.build_main_agent_content(ctx)
        };

        tracing::info!(
            generator = "plan_mode",
            is_reentry = ctx.is_plan_reentry,
            "Generating plan mode reminder"
        );
        Ok(Some(SystemReminder::new(AttachmentType::PlanMode, content)))
    }

    fn is_enabled(&self, config: &SystemReminderConfig) -> bool {
        config.enabled && config.attachments.plan_mode
    }

    fn throttle_config(&self) -> ThrottleConfig {
        default_throttle_config(AttachmentType::PlanMode)
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

    fn make_context<'a>(
        is_plan_mode: bool,
        plan_file_path: Option<&'a str>,
        is_plan_reentry: bool,
        file_tracker: &'a FileTracker,
        plan_state: &'a PlanState,
    ) -> GeneratorContext<'a> {
        GeneratorContext {
            turn_number: 1,
            is_main_agent: true,
            has_user_input: true,
            user_prompt: None,
            cwd: Path::new("/test"),
            agent_id: "test-agent",
            file_tracker,
            is_plan_mode,
            plan_file_path,
            is_plan_reentry,
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
    async fn test_generates_when_plan_mode_active() {
        let generator = PlanModeGenerator::new();
        let tracker = FileTracker::new();
        let plan_state = PlanState::default();
        let ctx = make_context(true, Some("/path/to/plan.md"), false, &tracker, &plan_state);

        let result = generator.generate(&ctx).await.unwrap();
        assert!(result.is_some());

        let reminder = result.unwrap();
        assert_eq!(reminder.attachment_type, AttachmentType::PlanMode);
        assert!(reminder.content.contains("Plan mode is active"));
    }

    #[tokio::test]
    async fn test_returns_none_when_not_plan_mode() {
        let generator = PlanModeGenerator::new();
        let tracker = FileTracker::new();
        let plan_state = PlanState::default();
        let ctx = make_context(false, None, false, &tracker, &plan_state);

        let result = generator.generate(&ctx).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_includes_reentry_content() {
        let generator = PlanModeGenerator::new();
        let tracker = FileTracker::new();
        let plan_state = PlanState::default();
        let ctx = make_context(true, Some("/path/to/plan.md"), true, &tracker, &plan_state);

        let result = generator.generate(&ctx).await.unwrap();
        assert!(result.is_some());

        let reminder = result.unwrap();
        assert!(reminder.content.contains("Re-entering Plan Mode"));
        assert!(reminder.content.contains("Plan mode is active"));
    }

    #[test]
    fn test_throttle_config() {
        let generator = PlanModeGenerator::new();
        let config = generator.throttle_config();
        assert_eq!(config.min_turns_between, 5);
    }

    #[test]
    fn test_attachment_type() {
        let generator = PlanModeGenerator::new();
        assert_eq!(generator.attachment_type(), AttachmentType::PlanMode);
        assert_eq!(generator.tier(), ReminderTier::Core);
    }
}
