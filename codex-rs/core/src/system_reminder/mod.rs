//! System reminder module.
//!
//! Provides contextual injection of metadata, state information, and instructions
//! into conversations at strategic points. This mechanism:
//! - Provides rich context to the LLM without cluttering user-visible output
//! - Uses XML-tagged messages (`<system-reminder>`, `<system-notification>`, etc.)
//! - Runs parallel generators with timeout protection (1 second max)
//! - Supports throttling to avoid spam
//!
//! Based on Claude Code v2.0.59's attachment system.

pub mod attachments;
pub mod file_tracker;
pub mod generator;
pub mod generator_ext;
pub mod throttle;
pub mod types;

pub use file_tracker::FileTracker;
pub use generator::ApprovedPlanInfo;
pub use generator::AttachmentGenerator;
pub use generator::BackgroundTaskInfo;
pub use generator::BackgroundTaskStatus;
pub use generator::BackgroundTaskType;
pub use generator::GeneratorContext;
pub use generator::PlanState;
pub use generator::PlanStep;
pub use throttle::ThrottleConfig;
pub use throttle::ThrottleManager;
pub use types::AttachmentType;
pub use types::ReminderTier;
pub use types::SYSTEM_NOTIFICATION_CLOSE_TAG;
pub use types::SYSTEM_NOTIFICATION_OPEN_TAG;
pub use types::SYSTEM_REMINDER_CLOSE_TAG;
pub use types::SYSTEM_REMINDER_OPEN_TAG;
pub use types::SystemReminder;
pub use types::XmlTag;

use crate::config::system_reminder::SystemReminderConfig;
use attachments::AgentMentionsGenerator;
use attachments::AgentTaskGenerator;
use attachments::AtMentionedFilesGenerator;
use attachments::ChangedFilesGenerator;
use attachments::CriticalInstructionGenerator;
use attachments::LspDiagnosticsGenerator;
use attachments::NestedMemoryGenerator;
use attachments::OutputStyleGenerator;
use attachments::PlanApprovedGenerator;
use attachments::PlanModeGenerator;
use attachments::PlanToolReminderGenerator;
use attachments::ShellTaskGenerator;
use futures::future::join_all;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

/// Default timeout for generator execution (1 second).
const DEFAULT_TIMEOUT_MS: i64 = 1000;

/// Telemetry sampling rate (5%).
const TELEMETRY_SAMPLE_RATE: f64 = 0.05;

/// Main system reminder orchestrator.
///
/// Matches JH5() in Claude Code chunks.107.mjs:1813-1829.
pub struct SystemReminderOrchestrator {
    generators: Vec<Arc<dyn AttachmentGenerator>>,
    throttle_manager: ThrottleManager,
    timeout_duration: Duration,
    config: SystemReminderConfig,
}

impl SystemReminderOrchestrator {
    /// Create a new orchestrator with the given configuration.
    pub fn new(config: SystemReminderConfig) -> Self {
        let timeout_ms = config.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);

        let generators: Vec<Arc<dyn AttachmentGenerator>> = vec![
            // Core tier
            Arc::new(CriticalInstructionGenerator::new()),
            Arc::new(PlanApprovedGenerator::new()),
            Arc::new(PlanModeGenerator::new()),
            Arc::new(PlanToolReminderGenerator::new()),
            Arc::new(ChangedFilesGenerator::new()),
            Arc::new(NestedMemoryGenerator::new(config.nested_memory.clone())),
            // MainAgentOnly tier
            Arc::new(ShellTaskGenerator::new()),
            Arc::new(AgentTaskGenerator::new()),
            Arc::new(LspDiagnosticsGenerator::new()),
            Arc::new(OutputStyleGenerator::new()),
            // UserPrompt tier
            Arc::new(AtMentionedFilesGenerator::new()),
            Arc::new(AgentMentionsGenerator::new()),
        ];

        Self {
            generators,
            throttle_manager: ThrottleManager::new(),
            timeout_duration: Duration::from_millis(timeout_ms as u64),
            config,
        }
    }

    /// Generate all applicable system reminders for a turn.
    ///
    /// Matches JH5 execution flow in Claude Code.
    pub async fn generate_all(&self, ctx: &GeneratorContext<'_>) -> Vec<SystemReminder> {
        // Step 1: Check global disable
        if !self.config.enabled {
            return Vec::new();
        }

        // Step 2: Build futures for all applicable generators
        let futures: Vec<_> = self
            .generators
            .iter()
            .filter(|g| self.should_run(g.as_ref(), ctx))
            .map(|g| {
                let g = Arc::clone(g);
                let timeout_duration = self.timeout_duration;
                let should_sample = rand::random::<f64>() < TELEMETRY_SAMPLE_RATE;
                let start_time = std::time::Instant::now();

                async move {
                    // Step 3: Execute with timeout (1 second max)
                    let result = match timeout(timeout_duration, g.generate(ctx)).await {
                        Ok(Ok(Some(reminder))) => {
                            tracing::info!(
                                generator = g.name(),
                                attachment_type = %reminder.attachment_type,
                                "System reminder generated"
                            );
                            Some(reminder)
                        }
                        Ok(Ok(None)) => {
                            tracing::trace!("Generator {} returned None", g.name());
                            None
                        }
                        Ok(Err(e)) => {
                            // Graceful degradation
                            tracing::warn!("Generator {} failed: {}", g.name(), e);
                            None
                        }
                        Err(_) => {
                            tracing::warn!("Generator {} timed out", g.name());
                            None
                        }
                    };

                    // Step 4: Record telemetry (5% sample)
                    if should_sample {
                        let duration = start_time.elapsed();
                        tracing::info!(
                            target: "telemetry",
                            generator = g.name(),
                            duration_ms = duration.as_millis() as i64,
                            success = result.is_some(),
                            "attachment_compute_duration"
                        );
                    }

                    result
                }
            })
            .collect();

        // Step 5: Run all generators in parallel
        let results: Vec<SystemReminder> = join_all(futures).await.into_iter().flatten().collect();

        // Step 6: Mark successful generations in throttle manager
        for reminder in &results {
            self.throttle_manager
                .mark_generated(reminder.attachment_type, ctx.turn_number);
        }

        results
    }

    /// Check if a generator should run.
    fn should_run(&self, generator: &dyn AttachmentGenerator, ctx: &GeneratorContext<'_>) -> bool {
        // Check if enabled in config
        if !generator.is_enabled(&self.config) {
            return false;
        }

        // Check tier requirements
        let tier_ok = match generator.tier() {
            ReminderTier::Core => true,
            ReminderTier::MainAgentOnly => ctx.is_main_agent,
            ReminderTier::UserPrompt => ctx.has_user_input,
        };
        if !tier_ok {
            return false;
        }

        // Check throttle rules
        let trigger_turn = self.get_trigger_turn(generator.attachment_type(), ctx);
        self.throttle_manager.should_generate(
            generator.attachment_type(),
            ctx.turn_number,
            trigger_turn,
        )
    }

    /// Get the trigger turn for a given attachment type.
    ///
    /// For PlanToolReminder, this is the last time update_plan was called.
    fn get_trigger_turn(
        &self,
        attachment_type: AttachmentType,
        ctx: &GeneratorContext<'_>,
    ) -> Option<i32> {
        match attachment_type {
            AttachmentType::PlanToolReminder => Some(ctx.plan_state.last_update_count),
            _ => None,
        }
    }

    /// Reset orchestrator state (call at session start).
    pub fn reset(&self) {
        self.throttle_manager.reset();
    }

    /// Get reference to the throttle manager.
    pub fn throttle_manager(&self) -> &ThrottleManager {
        &self.throttle_manager
    }

    /// Get reference to the config.
    pub fn config(&self) -> &SystemReminderConfig {
        &self.config
    }
}

impl Default for SystemReminderOrchestrator {
    fn default() -> Self {
        Self::new(SystemReminderConfig::default())
    }
}

impl std::fmt::Debug for SystemReminderOrchestrator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SystemReminderOrchestrator")
            .field("generator_count", &self.generators.len())
            .field("timeout_ms", &self.timeout_duration.as_millis())
            .field("enabled", &self.config.enabled)
            .finish()
    }
}

// ============================================
// Tests
// ============================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::system_reminder::AttachmentSettings;
    use crate::config::system_reminder::LspDiagnosticsMinSeverity;
    use std::path::Path;

    fn make_context(
        _is_main_agent: bool,
        _is_plan_mode: bool,
    ) -> (FileTracker, PlanState, Vec<BackgroundTaskInfo>) {
        (FileTracker::new(), PlanState::default(), vec![])
    }

    #[tokio::test]
    async fn test_orchestrator_disabled() {
        let config = SystemReminderConfig {
            enabled: false,
            ..Default::default()
        };
        let orchestrator = SystemReminderOrchestrator::new(config);

        let (tracker, plan_state, bg_tasks) = make_context(true, false);
        let ctx = GeneratorContext {
            turn_number: 1,
            is_main_agent: true,
            has_user_input: true,
            user_prompt: None,
            cwd: Path::new("/test"),
            agent_id: "test",
            file_tracker: &tracker,
            is_plan_mode: false,
            plan_file_path: None,
            is_plan_reentry: false,
            plan_state: &plan_state,
            background_tasks: &bg_tasks,
            critical_instruction: Some("test instruction"),
            diagnostics_store: None,
            lsp_diagnostics_min_severity: LspDiagnosticsMinSeverity::default(),
            output_style: None,
            approved_plan: None,
        };

        let reminders = orchestrator.generate_all(&ctx).await;
        assert!(reminders.is_empty());
    }

    #[tokio::test]
    async fn test_orchestrator_generates_critical_instruction() {
        let config = SystemReminderConfig::default();
        let orchestrator = SystemReminderOrchestrator::new(config);

        let (tracker, plan_state, bg_tasks) = make_context(true, false);
        let ctx = GeneratorContext {
            turn_number: 1,
            is_main_agent: true,
            has_user_input: true,
            user_prompt: None,
            cwd: Path::new("/test"),
            agent_id: "test",
            file_tracker: &tracker,
            is_plan_mode: false,
            plan_file_path: None,
            is_plan_reentry: false,
            plan_state: &plan_state,
            background_tasks: &bg_tasks,
            critical_instruction: Some("Always run tests"),
            diagnostics_store: None,
            lsp_diagnostics_min_severity: LspDiagnosticsMinSeverity::default(),
            output_style: None,
            approved_plan: None,
        };

        let reminders = orchestrator.generate_all(&ctx).await;

        // Should have at least the critical instruction
        assert!(
            reminders
                .iter()
                .any(|r| r.attachment_type == AttachmentType::CriticalInstruction)
        );
    }

    #[tokio::test]
    async fn test_orchestrator_generates_plan_mode() {
        let config = SystemReminderConfig::default();
        let orchestrator = SystemReminderOrchestrator::new(config);

        let (tracker, plan_state, bg_tasks) = make_context(true, true);
        let ctx = GeneratorContext {
            turn_number: 1,
            is_main_agent: true,
            has_user_input: true,
            user_prompt: None,
            cwd: Path::new("/test"),
            agent_id: "test",
            file_tracker: &tracker,
            is_plan_mode: true,
            plan_file_path: Some("/path/to/plan.md"),
            is_plan_reentry: false,
            plan_state: &plan_state,
            background_tasks: &bg_tasks,
            critical_instruction: None,
            diagnostics_store: None,
            lsp_diagnostics_min_severity: LspDiagnosticsMinSeverity::default(),
            output_style: None,
            approved_plan: None,
        };

        let reminders = orchestrator.generate_all(&ctx).await;

        // Should have plan mode reminder
        assert!(
            reminders
                .iter()
                .any(|r| r.attachment_type == AttachmentType::PlanMode)
        );
    }

    #[tokio::test]
    async fn test_orchestrator_respects_attachment_settings() {
        let config = SystemReminderConfig {
            enabled: true,
            attachments: AttachmentSettings {
                critical_instruction: false,
                plan_mode: true,
                plan_tool_reminder: false,
                changed_files: false,
                background_task: false,
                lsp_diagnostics: false,
                lsp_diagnostics_min_severity: LspDiagnosticsMinSeverity::default(),
                nested_memory: false,
                at_mentioned_files: false,
                agent_mentions: false,
                output_style: true,
            },
            ..Default::default()
        };
        let orchestrator = SystemReminderOrchestrator::new(config);

        let (tracker, plan_state, bg_tasks) = make_context(true, false);
        let ctx = GeneratorContext {
            turn_number: 1,
            is_main_agent: true,
            has_user_input: true,
            user_prompt: None,
            cwd: Path::new("/test"),
            agent_id: "test",
            file_tracker: &tracker,
            is_plan_mode: false,
            plan_file_path: None,
            is_plan_reentry: false,
            plan_state: &plan_state,
            background_tasks: &bg_tasks,
            critical_instruction: Some("test"),
            diagnostics_store: None,
            lsp_diagnostics_min_severity: LspDiagnosticsMinSeverity::default(),
            output_style: None,
            approved_plan: None,
        };

        let reminders = orchestrator.generate_all(&ctx).await;

        // Critical instruction should NOT be generated
        assert!(
            !reminders
                .iter()
                .any(|r| r.attachment_type == AttachmentType::CriticalInstruction)
        );
    }

    #[test]
    fn test_orchestrator_reset() {
        let orchestrator = SystemReminderOrchestrator::default();
        orchestrator
            .throttle_manager()
            .mark_generated(AttachmentType::PlanToolReminder, 1);
        orchestrator.reset();

        // After reset, throttle state should be cleared
        assert!(orchestrator.throttle_manager().should_generate(
            AttachmentType::PlanToolReminder,
            2,
            None
        ));
    }

    #[test]
    fn test_orchestrator_default() {
        let orchestrator = SystemReminderOrchestrator::default();
        assert!(orchestrator.config.enabled);
        assert_eq!(orchestrator.generators.len(), 12);
    }

    #[tokio::test]
    async fn test_plan_tool_reminder_throttle() {
        // Test that plan_tool_reminder respects min_turns_between = 3
        let config = SystemReminderConfig::default();
        let orchestrator = SystemReminderOrchestrator::new(config);

        let tracker = FileTracker::new();
        let bg_tasks = vec![];

        // Plan state: not empty, last update at turn 0
        // This means calls_since_update = turn_number - 0 >= 5 will trigger
        let plan_state = PlanState {
            is_empty: false,
            last_update_count: 0,
            steps: vec![PlanStep {
                step: "Test step".to_string(),
                status: "pending".to_string(),
            }],
        };

        // Turn 6: calls_since_update = 6 - 0 = 6 >= 5, should generate
        let ctx1 = GeneratorContext {
            turn_number: 6,
            is_main_agent: true,
            has_user_input: true,
            user_prompt: None,
            cwd: Path::new("/test"),
            agent_id: "test",
            file_tracker: &tracker,
            is_plan_mode: false,
            plan_file_path: None,
            is_plan_reentry: false,
            plan_state: &plan_state,
            background_tasks: &bg_tasks,
            critical_instruction: None,
            diagnostics_store: None,
            lsp_diagnostics_min_severity: LspDiagnosticsMinSeverity::default(),
            output_style: None,
            approved_plan: None,
        };

        let reminders1 = orchestrator.generate_all(&ctx1).await;
        let has_plan_reminder1 = reminders1
            .iter()
            .any(|r| r.attachment_type == AttachmentType::PlanToolReminder);
        assert!(
            has_plan_reminder1,
            "Turn 6: should generate plan tool reminder"
        );

        // Turn 7: only 1 turn since last reminder (< min_turns_between=3), should NOT generate
        let ctx2 = GeneratorContext {
            turn_number: 7,
            is_main_agent: true,
            has_user_input: true,
            user_prompt: None,
            cwd: Path::new("/test"),
            agent_id: "test",
            file_tracker: &tracker,
            is_plan_mode: false,
            plan_file_path: None,
            is_plan_reentry: false,
            plan_state: &plan_state,
            background_tasks: &bg_tasks,
            critical_instruction: None,
            diagnostics_store: None,
            lsp_diagnostics_min_severity: LspDiagnosticsMinSeverity::default(),
            output_style: None,
            approved_plan: None,
        };

        let reminders2 = orchestrator.generate_all(&ctx2).await;
        let has_plan_reminder2 = reminders2
            .iter()
            .any(|r| r.attachment_type == AttachmentType::PlanToolReminder);
        assert!(
            !has_plan_reminder2,
            "Turn 7: should NOT generate (only 1 turn since last, need 3)"
        );

        // Turn 10: 4 turns since last reminder (>= min_turns_between=3), should generate
        let ctx3 = GeneratorContext {
            turn_number: 10,
            is_main_agent: true,
            has_user_input: true,
            user_prompt: None,
            cwd: Path::new("/test"),
            agent_id: "test",
            file_tracker: &tracker,
            is_plan_mode: false,
            plan_file_path: None,
            is_plan_reentry: false,
            plan_state: &plan_state,
            background_tasks: &bg_tasks,
            critical_instruction: None,
            diagnostics_store: None,
            lsp_diagnostics_min_severity: LspDiagnosticsMinSeverity::default(),
            output_style: None,
            approved_plan: None,
        };

        let reminders3 = orchestrator.generate_all(&ctx3).await;
        let has_plan_reminder3 = reminders3
            .iter()
            .any(|r| r.attachment_type == AttachmentType::PlanToolReminder);
        assert!(
            has_plan_reminder3,
            "Turn 10: should generate (4 turns since last, >= 3)"
        );
    }

    #[tokio::test]
    async fn test_plan_tool_reminder_respects_trigger_turn() {
        // Test that plan_tool_reminder requires min_turns_after_trigger = 5
        let config = SystemReminderConfig::default();
        let orchestrator = SystemReminderOrchestrator::new(config);

        let tracker = FileTracker::new();
        let bg_tasks = vec![];

        // Plan state: last update at turn 3
        let plan_state = PlanState {
            is_empty: false,
            last_update_count: 3,
            steps: vec![PlanStep {
                step: "Test step".to_string(),
                status: "pending".to_string(),
            }],
        };

        // Turn 5: calls_since_update = 5 - 3 = 2 < 5, should NOT generate
        let ctx1 = GeneratorContext {
            turn_number: 5,
            is_main_agent: true,
            has_user_input: true,
            user_prompt: None,
            cwd: Path::new("/test"),
            agent_id: "test",
            file_tracker: &tracker,
            is_plan_mode: false,
            plan_file_path: None,
            is_plan_reentry: false,
            plan_state: &plan_state,
            background_tasks: &bg_tasks,
            critical_instruction: None,
            diagnostics_store: None,
            lsp_diagnostics_min_severity: LspDiagnosticsMinSeverity::default(),
            output_style: None,
            approved_plan: None,
        };

        let reminders1 = orchestrator.generate_all(&ctx1).await;
        let has_plan_reminder1 = reminders1
            .iter()
            .any(|r| r.attachment_type == AttachmentType::PlanToolReminder);
        assert!(
            !has_plan_reminder1,
            "Turn 5: should NOT generate (2 calls since update, need 5)"
        );

        // Turn 9: calls_since_update = 9 - 3 = 6 >= 5, should generate
        let ctx2 = GeneratorContext {
            turn_number: 9,
            is_main_agent: true,
            has_user_input: true,
            user_prompt: None,
            cwd: Path::new("/test"),
            agent_id: "test",
            file_tracker: &tracker,
            is_plan_mode: false,
            plan_file_path: None,
            is_plan_reentry: false,
            plan_state: &plan_state,
            background_tasks: &bg_tasks,
            critical_instruction: None,
            diagnostics_store: None,
            lsp_diagnostics_min_severity: LspDiagnosticsMinSeverity::default(),
            output_style: None,
            approved_plan: None,
        };

        let reminders2 = orchestrator.generate_all(&ctx2).await;
        let has_plan_reminder2 = reminders2
            .iter()
            .any(|r| r.attachment_type == AttachmentType::PlanToolReminder);
        assert!(
            has_plan_reminder2,
            "Turn 9: should generate (6 calls since update, >= 5)"
        );
    }
}
