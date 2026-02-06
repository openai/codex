//! Orchestrator for parallel generator execution.
//!
//! This module provides the main orchestration logic for running
//! multiple generators in parallel with timeout protection.

use std::sync::Arc;
use std::time::Duration;

use futures::future;
use tokio::time::timeout;
use tracing::debug;
use tracing::warn;

use crate::config::SystemReminderConfig;
use crate::generator::AttachmentGenerator;
use crate::generator::GeneratorContext;
use crate::generators::AlreadyReadFilesGenerator;
use crate::generators::AvailableSkillsGenerator;
use crate::generators::BudgetUsdGenerator;
use crate::generators::ChangedFilesGenerator;
use crate::generators::CollabNotificationsGenerator;
use crate::generators::CompactFileReferenceGenerator;
use crate::generators::DelegateModeGenerator;
use crate::generators::LspDiagnosticsGenerator;
use crate::generators::NestedMemoryGenerator;
use crate::generators::OutputStyleGenerator;
use crate::generators::PlanModeApprovedGenerator;
use crate::generators::PlanModeEnterGenerator;
use crate::generators::PlanModeExitGenerator;
use crate::generators::PlanToolReminderGenerator;
use crate::generators::PlanVerificationGenerator;
use crate::generators::QueuedCommandsGenerator;
use crate::generators::SecurityGuidelinesGenerator;
use crate::generators::TodoRemindersGenerator;
use crate::generators::TokenUsageGenerator;
use crate::generators::UnifiedTasksGenerator;
use crate::throttle::ThrottleManager;
use crate::types::ReminderTier;
use crate::types::SystemReminder;

/// Default timeout for generator execution (1 second).
const DEFAULT_TIMEOUT_MS: i64 = 1000;

/// Orchestrator for running system reminder generators.
///
/// The orchestrator manages a collection of generators, running them
/// in parallel with timeout protection and tier-based filtering.
pub struct SystemReminderOrchestrator {
    /// Registered generators.
    generators: Vec<Arc<dyn AttachmentGenerator>>,
    /// Throttle manager for rate limiting.
    throttle_manager: ThrottleManager,
    /// Timeout duration for each generator.
    timeout_duration: Duration,
    /// Configuration.
    config: SystemReminderConfig,
}

impl std::fmt::Debug for SystemReminderOrchestrator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SystemReminderOrchestrator")
            .field("generator_count", &self.generators.len())
            .field("timeout_ms", &self.timeout_duration.as_millis())
            .finish()
    }
}

impl SystemReminderOrchestrator {
    /// Create a new orchestrator with the given configuration.
    pub fn new(config: SystemReminderConfig) -> Self {
        let timeout_ms = if config.timeout_ms > 0 {
            config.timeout_ms
        } else {
            DEFAULT_TIMEOUT_MS
        };

        let generators = Self::create_default_generators();

        Self {
            generators,
            throttle_manager: ThrottleManager::new(),
            timeout_duration: Duration::from_millis(timeout_ms as u64),
            config,
        }
    }

    /// Create the default set of generators.
    fn create_default_generators() -> Vec<Arc<dyn AttachmentGenerator>> {
        vec![
            // Core tier
            Arc::new(SecurityGuidelinesGenerator),
            Arc::new(ChangedFilesGenerator),
            Arc::new(PlanModeEnterGenerator),
            Arc::new(PlanModeApprovedGenerator),
            Arc::new(PlanModeExitGenerator),
            Arc::new(PlanToolReminderGenerator),
            Arc::new(NestedMemoryGenerator),
            // MainAgentOnly tier
            Arc::new(AvailableSkillsGenerator),
            Arc::new(LspDiagnosticsGenerator),
            Arc::new(OutputStyleGenerator),
            Arc::new(TodoRemindersGenerator),
            Arc::new(UnifiedTasksGenerator),
            Arc::new(DelegateModeGenerator),
            Arc::new(CollabNotificationsGenerator),
            Arc::new(PlanVerificationGenerator),
            Arc::new(TokenUsageGenerator),
            Arc::new(QueuedCommandsGenerator),
            // New generators for enhanced features
            Arc::new(AlreadyReadFilesGenerator),
            Arc::new(BudgetUsdGenerator),
            Arc::new(CompactFileReferenceGenerator),
        ]
    }

    /// Add a custom generator.
    pub fn add_generator(&mut self, generator: Arc<dyn AttachmentGenerator>) {
        self.generators.push(generator);
    }

    /// Generate all applicable reminders for the current context.
    ///
    /// Generators are filtered by:
    /// 1. Global enable flag
    /// 2. Per-generator enable flag
    /// 3. Tier requirements (Core, MainAgentOnly, UserPrompt)
    /// 4. Throttle rules
    ///
    /// All applicable generators run in parallel with timeout protection.
    pub async fn generate_all(&self, ctx: &GeneratorContext<'_>) -> Vec<SystemReminder> {
        if !self.config.enabled {
            debug!("System reminders disabled globally");
            return Vec::new();
        }

        // Filter generators that should run
        let applicable_generators: Vec<_> = self
            .generators
            .iter()
            .filter(|g| self.should_run_generator(g.as_ref(), ctx))
            .cloned()
            .collect();

        if applicable_generators.is_empty() {
            debug!("No applicable generators for this turn");
            return Vec::new();
        }

        debug!(
            "Running {} generators for turn {}",
            applicable_generators.len(),
            ctx.turn_number
        );

        // Run all generators in parallel with timeout
        let futures: Vec<_> = applicable_generators
            .iter()
            .map(|g| {
                let generator = Arc::clone(g);
                let timeout_duration = self.timeout_duration;
                async move {
                    let name = generator.name().to_string();
                    let attachment_type = generator.attachment_type();

                    match timeout(timeout_duration, generator.generate(ctx)).await {
                        Ok(Ok(Some(reminder))) => {
                            debug!("Generator '{}' produced reminder", name);
                            Some((attachment_type, reminder))
                        }
                        Ok(Ok(None)) => {
                            debug!("Generator '{}' produced no output", name);
                            None
                        }
                        Ok(Err(e)) => {
                            warn!("Generator '{}' failed: {}", name, e);
                            None
                        }
                        Err(_) => {
                            warn!(
                                "Generator '{}' timed out after {}ms",
                                name,
                                timeout_duration.as_millis()
                            );
                            None
                        }
                    }
                }
            })
            .collect();

        let results = future::join_all(futures).await;

        // Mark successful generations and collect reminders
        let mut reminders = Vec::new();
        for result in results.into_iter().flatten() {
            let (attachment_type, reminder) = result;
            self.throttle_manager
                .mark_generated(attachment_type, ctx.turn_number);
            reminders.push(reminder);
        }

        debug!(
            "Generated {} reminders for turn {}",
            reminders.len(),
            ctx.turn_number
        );

        reminders
    }

    /// Check if a generator should run for the current context.
    fn should_run_generator(
        &self,
        generator: &dyn AttachmentGenerator,
        ctx: &GeneratorContext<'_>,
    ) -> bool {
        // Check if generator is enabled in config
        if !generator.is_enabled(&self.config) {
            return false;
        }

        // Check tier requirements
        let tier = generator.tier();
        match tier {
            ReminderTier::Core => {
                // Always run for all agents
            }
            ReminderTier::MainAgentOnly => {
                if !ctx.is_main_agent {
                    return false;
                }
            }
            ReminderTier::UserPrompt => {
                if !ctx.has_user_input {
                    return false;
                }
            }
        }

        // Check throttle
        let throttle_config = generator.throttle_config();
        if !self.throttle_manager.should_generate(
            generator.attachment_type(),
            &throttle_config,
            ctx.turn_number,
        ) {
            debug!(
                "Generator '{}' throttled at turn {}",
                generator.name(),
                ctx.turn_number
            );
            return false;
        }

        true
    }

    /// Get a reference to the throttle manager.
    pub fn throttle_manager(&self) -> &ThrottleManager {
        &self.throttle_manager
    }

    /// Reset the throttle manager (e.g., at session start).
    pub fn reset_throttle(&self) {
        self.throttle_manager.reset();
    }

    /// Get the number of registered generators.
    pub fn generator_count(&self) -> usize {
        self.generators.len()
    }

    /// Get the timeout duration.
    pub fn timeout_duration(&self) -> Duration {
        self.timeout_duration
    }

    /// Get the configuration.
    pub fn config(&self) -> &SystemReminderConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_config() -> SystemReminderConfig {
        SystemReminderConfig::default()
    }

    fn test_ctx(config: &SystemReminderConfig) -> GeneratorContext<'_> {
        GeneratorContext::builder()
            .config(config)
            .turn_number(1)
            .is_main_agent(true)
            .has_user_input(true)
            .cwd(PathBuf::from("/tmp/test"))
            .build()
    }

    #[test]
    fn test_orchestrator_creation() {
        let config = test_config();
        let orchestrator = SystemReminderOrchestrator::new(config);

        assert!(orchestrator.generator_count() > 0);
        assert_eq!(orchestrator.timeout_duration().as_millis(), 1000);
    }

    #[test]
    fn test_orchestrator_disabled() {
        let config = SystemReminderConfig {
            enabled: false,
            ..Default::default()
        };
        let orchestrator = SystemReminderOrchestrator::new(config.clone());
        let ctx = test_ctx(&config);

        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let reminders = rt.block_on(orchestrator.generate_all(&ctx));

        assert!(reminders.is_empty());
    }

    #[test]
    fn test_tier_filtering_subagent() {
        let config = test_config();
        let orchestrator = SystemReminderOrchestrator::new(config.clone());

        // Create context as a subagent
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .is_main_agent(false) // subagent
            .has_user_input(true)
            .cwd(PathBuf::from("/tmp"))
            .build();

        // MainAgentOnly generators should not run for subagents
        for g in &orchestrator.generators {
            if g.tier() == ReminderTier::MainAgentOnly {
                assert!(!orchestrator.should_run_generator(g.as_ref(), &ctx));
            }
        }
    }

    #[test]
    fn test_tier_filtering_no_user_input() {
        let config = test_config();
        let orchestrator = SystemReminderOrchestrator::new(config.clone());

        // Create context without user input
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .is_main_agent(true)
            .has_user_input(false) // no user input
            .cwd(PathBuf::from("/tmp"))
            .build();

        // UserPrompt generators should not run without user input
        for g in &orchestrator.generators {
            if g.tier() == ReminderTier::UserPrompt {
                assert!(!orchestrator.should_run_generator(g.as_ref(), &ctx));
            }
        }
    }

    #[tokio::test]
    async fn test_generate_all_basic() {
        let config = test_config();
        let orchestrator = SystemReminderOrchestrator::new(config.clone());
        let ctx = test_ctx(&config);

        // Should run without panicking
        let reminders = orchestrator.generate_all(&ctx).await;

        // Most generators will return None without proper setup,
        // but the orchestrator should handle that gracefully
        assert!(reminders.len() <= orchestrator.generator_count());
    }

    #[test]
    fn test_throttle_reset() {
        let config = test_config();
        let orchestrator = SystemReminderOrchestrator::new(config);

        // Mark some generation
        orchestrator
            .throttle_manager()
            .mark_generated(crate::types::AttachmentType::ChangedFiles, 1);

        // State should exist
        assert!(
            orchestrator
                .throttle_manager()
                .get_state(crate::types::AttachmentType::ChangedFiles)
                .is_some()
        );

        // Reset
        orchestrator.reset_throttle();

        // State should be gone
        assert!(
            orchestrator
                .throttle_manager()
                .get_state(crate::types::AttachmentType::ChangedFiles)
                .is_none()
        );
    }
}
