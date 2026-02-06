//! Budget USD generator.
//!
//! This generator reports budget warnings when the session budget is low.

use async_trait::async_trait;

use crate::Result;
use crate::config::SystemReminderConfig;
use crate::generator::AttachmentGenerator;
use crate::generator::GeneratorContext;
use crate::throttle::ThrottleConfig;
use crate::types::AttachmentType;
use crate::types::SystemReminder;

/// Threshold percentage for low budget warning.
const LOW_BUDGET_THRESHOLD: f64 = 10.0;

/// Generator for budget USD warnings.
///
/// Reports budget information only when the budget is low (< 10% remaining).
/// This helps the model be aware of budget constraints and adjust behavior.
#[derive(Debug)]
pub struct BudgetUsdGenerator;

#[async_trait]
impl AttachmentGenerator for BudgetUsdGenerator {
    fn name(&self) -> &str {
        "BudgetUsdGenerator"
    }

    fn attachment_type(&self) -> AttachmentType {
        AttachmentType::BudgetUsd
    }

    fn is_enabled(&self, config: &SystemReminderConfig) -> bool {
        config.attachments.budget_usd
    }

    fn throttle_config(&self) -> ThrottleConfig {
        // Check every turn when budget is low
        ThrottleConfig::none()
    }

    async fn generate(&self, ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>> {
        let Some(budget) = &ctx.budget else {
            return Ok(None);
        };

        // Only generate if budget is low
        let remaining_percent = if budget.total_usd > 0.0 {
            (budget.remaining_usd / budget.total_usd) * 100.0
        } else {
            100.0 // No budget set
        };

        if remaining_percent > LOW_BUDGET_THRESHOLD && !budget.is_low {
            return Ok(None);
        }

        // Build the warning message
        let used_percent = if budget.total_usd > 0.0 {
            (budget.used_usd / budget.total_usd) * 100.0
        } else {
            0.0
        };

        let content = format!(
            "**Budget Warning:** ${:.2} remaining of ${:.2} ({:.1}% used)\n\n\
            Please be mindful of API costs. Consider:\n\
            - Being more concise in responses\n\
            - Avoiding unnecessary tool calls\n\
            - Completing the current task efficiently",
            budget.remaining_usd, budget.total_usd, used_percent
        );

        Ok(Some(SystemReminder::text(
            AttachmentType::BudgetUsd,
            content,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SystemReminderConfig;
    use crate::generator::BudgetInfo;
    use crate::types::ReminderTier;
    use std::path::PathBuf;

    fn test_config() -> SystemReminderConfig {
        SystemReminderConfig::default()
    }

    #[tokio::test]
    async fn test_no_budget() {
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .cwd(PathBuf::from("/tmp"))
            // No budget
            .build();

        let generator = BudgetUsdGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_budget_not_low() {
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .cwd(PathBuf::from("/tmp"))
            .budget(BudgetInfo {
                total_usd: 10.0,
                used_usd: 5.0,
                remaining_usd: 5.0,
                is_low: false, // 50% remaining
            })
            .build();

        let generator = BudgetUsdGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_budget_low() {
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .cwd(PathBuf::from("/tmp"))
            .budget(BudgetInfo {
                total_usd: 10.0,
                used_usd: 9.5,
                remaining_usd: 0.5,
                is_low: true, // 5% remaining
            })
            .build();

        let generator = BudgetUsdGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_some());

        let reminder = result.expect("reminder");
        assert_eq!(reminder.attachment_type, AttachmentType::BudgetUsd);
        assert!(reminder.is_text());
        assert!(reminder.content().unwrap().contains("Budget Warning"));
        assert!(reminder.content().unwrap().contains("$0.50"));
        assert!(reminder.content().unwrap().contains("95.0%"));
    }

    #[tokio::test]
    async fn test_budget_below_threshold() {
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .cwd(PathBuf::from("/tmp"))
            .budget(BudgetInfo {
                total_usd: 100.0,
                used_usd: 92.0,
                remaining_usd: 8.0,
                is_low: false, // 8% remaining, below 10% threshold
            })
            .build();

        let generator = BudgetUsdGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_some()); // Should generate because below threshold

        let reminder = result.expect("reminder");
        assert!(reminder.content().unwrap().contains("$8.00"));
    }

    #[test]
    fn test_generator_properties() {
        let generator = BudgetUsdGenerator;
        assert_eq!(generator.name(), "BudgetUsdGenerator");
        assert_eq!(generator.attachment_type(), AttachmentType::BudgetUsd);
        assert_eq!(generator.tier(), ReminderTier::MainAgentOnly);

        // No throttle for budget warnings
        let throttle = generator.throttle_config();
        assert_eq!(throttle.min_turns_between, 0);
    }
}
