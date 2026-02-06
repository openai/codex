//! Token usage generator.
//!
//! This generator reports token usage statistics and budget information
//! to help the model be aware of context limits.

use async_trait::async_trait;

use crate::Result;
use crate::config::SystemReminderConfig;
use crate::generator::AttachmentGenerator;
use crate::generator::GeneratorContext;
use crate::throttle::ThrottleConfig;
use crate::types::AttachmentType;
use crate::types::SystemReminder;

/// Generator for token usage statistics.
///
/// Reports token consumption and budget information. Triggered periodically
/// or when context usage is high.
#[derive(Debug)]
pub struct TokenUsageGenerator;

/// Threshold for high context usage (80%).
const HIGH_CONTEXT_THRESHOLD: f64 = 80.0;

/// Threshold for critical context usage (95%).
const CRITICAL_CONTEXT_THRESHOLD: f64 = 95.0;

#[async_trait]
impl AttachmentGenerator for TokenUsageGenerator {
    fn name(&self) -> &str {
        "TokenUsageGenerator"
    }

    fn attachment_type(&self) -> AttachmentType {
        AttachmentType::TokenUsage
    }

    fn is_enabled(&self, config: &SystemReminderConfig) -> bool {
        config.attachments.token_usage
    }

    fn throttle_config(&self) -> ThrottleConfig {
        // Report every 10 turns normally
        ThrottleConfig {
            min_turns_between: 10,
            min_turns_after_trigger: 0,
            max_per_session: None,
        }
    }

    async fn generate(&self, ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>> {
        let Some(usage) = &ctx.token_usage else {
            return Ok(None);
        };

        // Always generate if context usage is high (overrides throttle)
        let is_high_usage = usage.context_usage_percent >= HIGH_CONTEXT_THRESHOLD;
        let is_critical = usage.context_usage_percent >= CRITICAL_CONTEXT_THRESHOLD;

        // Build the content
        let mut lines = vec!["## Token Usage".to_string()];

        // Context usage with warning if high
        if is_critical {
            lines.push(format!(
                "\n**CRITICAL: Context usage at {:.1}%** - Consider summarizing the conversation",
                usage.context_usage_percent
            ));
        } else if is_high_usage {
            lines.push(format!(
                "\n**Warning: Context usage at {:.1}%** - Be mindful of context limits",
                usage.context_usage_percent
            ));
        } else {
            lines.push(format!(
                "\nContext usage: {:.1}%",
                usage.context_usage_percent
            ));
        }

        // Token details
        lines.push(format!(
            "- Session tokens: {} / {}",
            format_tokens(usage.total_session_tokens),
            format_tokens(usage.context_capacity)
        ));

        if usage.input_tokens > 0 || usage.output_tokens > 0 {
            lines.push(format!(
                "- This turn: {} input, {} output",
                format_tokens(usage.input_tokens),
                format_tokens(usage.output_tokens)
            ));
        }

        if usage.cache_read_tokens > 0 {
            lines.push(format!(
                "- Cache: {} read, {} write",
                format_tokens(usage.cache_read_tokens),
                format_tokens(usage.cache_write_tokens)
            ));
        }

        // Budget information
        if let Some(budget) = &ctx.budget {
            lines.push(String::new());
            if budget.is_low {
                lines.push(format!(
                    "**Budget Warning:** ${:.2} remaining of ${:.2} ({:.1}% used)",
                    budget.remaining_usd,
                    budget.total_usd,
                    (budget.used_usd / budget.total_usd) * 100.0
                ));
            } else {
                lines.push(format!(
                    "Budget: ${:.2} / ${:.2} used",
                    budget.used_usd, budget.total_usd
                ));
            }
        }

        Ok(Some(SystemReminder::new(
            AttachmentType::TokenUsage,
            lines.join("\n"),
        )))
    }
}

/// Format token count for display.
fn format_tokens(tokens: i64) -> String {
    if tokens >= 1_000_000 {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{:.1}K", tokens as f64 / 1_000.0)
    } else {
        tokens.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::BudgetInfo;
    use crate::generator::TokenUsageStats;
    use std::path::PathBuf;

    fn test_config() -> SystemReminderConfig {
        SystemReminderConfig::default()
    }

    #[tokio::test]
    async fn test_not_triggered_without_usage() {
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .cwd(PathBuf::from("/tmp"))
            // No token_usage
            .build();

        let generator = TokenUsageGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_normal_usage() {
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .cwd(PathBuf::from("/tmp"))
            .token_usage(TokenUsageStats {
                input_tokens: 5000,
                output_tokens: 2000,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
                total_session_tokens: 50000,
                context_capacity: 200000,
                context_usage_percent: 25.0,
            })
            .build();

        let generator = TokenUsageGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_some());

        let reminder = result.expect("reminder");
        assert!(reminder.content().unwrap().contains("Token Usage"));
        assert!(reminder.content().unwrap().contains("25.0%"));
        assert!(!reminder.content().unwrap().contains("Warning"));
    }

    #[tokio::test]
    async fn test_high_usage_warning() {
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .cwd(PathBuf::from("/tmp"))
            .token_usage(TokenUsageStats {
                input_tokens: 5000,
                output_tokens: 2000,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
                total_session_tokens: 170000,
                context_capacity: 200000,
                context_usage_percent: 85.0,
            })
            .build();

        let generator = TokenUsageGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_some());

        let reminder = result.expect("reminder");
        assert!(reminder.content().unwrap().contains("Warning"));
        assert!(reminder.content().unwrap().contains("85.0%"));
    }

    #[tokio::test]
    async fn test_critical_usage() {
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .cwd(PathBuf::from("/tmp"))
            .token_usage(TokenUsageStats {
                input_tokens: 5000,
                output_tokens: 2000,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
                total_session_tokens: 195000,
                context_capacity: 200000,
                context_usage_percent: 97.5,
            })
            .build();

        let generator = TokenUsageGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_some());

        let reminder = result.expect("reminder");
        assert!(reminder.content().unwrap().contains("CRITICAL"));
        assert!(reminder.content().unwrap().contains("summarizing"));
    }

    #[tokio::test]
    async fn test_with_budget_warning() {
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .cwd(PathBuf::from("/tmp"))
            .token_usage(TokenUsageStats {
                context_usage_percent: 50.0,
                context_capacity: 200000,
                ..Default::default()
            })
            .budget(BudgetInfo {
                total_usd: 10.0,
                used_usd: 9.5,
                remaining_usd: 0.5,
                is_low: true,
            })
            .build();

        let generator = TokenUsageGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_some());

        let reminder = result.expect("reminder");
        assert!(reminder.content().unwrap().contains("Budget Warning"));
        assert!(reminder.content().unwrap().contains("$0.50"));
    }

    #[test]
    fn test_format_tokens() {
        assert_eq!(format_tokens(500), "500");
        assert_eq!(format_tokens(1500), "1.5K");
        assert_eq!(format_tokens(1_500_000), "1.5M");
    }

    #[test]
    fn test_throttle_config() {
        let generator = TokenUsageGenerator;
        let throttle = generator.throttle_config();
        assert_eq!(throttle.min_turns_between, 10);
    }
}
