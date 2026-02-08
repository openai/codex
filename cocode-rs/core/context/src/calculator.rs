//! Sync token estimation and budget computation.
//!
//! Provides approximate token counting based on character-to-token ratios,
//! and computes token budgets for the context window.

use crate::budget::BudgetCategory;
use crate::budget::ContextBudget;
use crate::conversation_context::MemoryFile;
use crate::environment::EnvironmentInfo;

/// Default characters-per-token ratio for estimation.
const DEFAULT_CHARS_PER_TOKEN: f32 = 4.0;

/// Default reserved safety margin (percentage of input budget).
const DEFAULT_RESERVED_PCT: f32 = 0.05;

/// Sync token estimator and budget calculator.
#[derive(Debug, Clone)]
pub struct ContextCalculator {
    /// Characters-per-token ratio for estimation.
    chars_per_token: f32,
}

impl Default for ContextCalculator {
    fn default() -> Self {
        Self {
            chars_per_token: DEFAULT_CHARS_PER_TOKEN,
        }
    }
}

impl ContextCalculator {
    /// Create a new calculator with custom chars-per-token ratio.
    pub fn new(chars_per_token: f32) -> Self {
        Self { chars_per_token }
    }

    /// Estimate token count for a text string.
    pub fn estimate_tokens(&self, text: &str) -> i32 {
        if text.is_empty() {
            return 0;
        }
        (text.len() as f32 / self.chars_per_token).ceil() as i32
    }

    /// Compute a context budget based on environment and content.
    pub fn compute_budget(
        &self,
        env: &EnvironmentInfo,
        system_prompt: &str,
        tool_definitions: &[String],
        memory_files: &[MemoryFile],
    ) -> ContextBudget {
        let mut budget = ContextBudget::new(env.context_window, env.max_output_tokens);

        // Reserve safety margin
        let reserved = (budget.input_budget() as f32 * DEFAULT_RESERVED_PCT) as i32;
        budget.set_allocation(BudgetCategory::Reserved, reserved);
        budget.record_usage(BudgetCategory::Reserved, reserved);

        // System prompt
        let system_tokens = self.estimate_tokens(system_prompt);
        budget.set_allocation(BudgetCategory::SystemPrompt, system_tokens);
        budget.record_usage(BudgetCategory::SystemPrompt, system_tokens);

        // Tool definitions
        let tool_tokens: i32 = tool_definitions
            .iter()
            .map(|t| self.estimate_tokens(t))
            .sum();
        budget.set_allocation(BudgetCategory::ToolDefinitions, tool_tokens);
        budget.record_usage(BudgetCategory::ToolDefinitions, tool_tokens);

        // Memory files
        let memory_tokens: i32 = memory_files
            .iter()
            .map(|m| self.estimate_tokens(&m.content))
            .sum();
        budget.set_allocation(BudgetCategory::MemoryFiles, memory_tokens);
        budget.record_usage(BudgetCategory::MemoryFiles, memory_tokens);

        // Remaining goes to conversation history
        let conversation_budget = budget.available();
        budget.set_allocation(BudgetCategory::ConversationHistory, conversation_budget);

        budget
    }

    /// Check if context needs compaction based on utilization threshold.
    pub fn needs_compaction(&self, budget: &ContextBudget, threshold: f32) -> bool {
        budget.utilization() >= threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens_empty() {
        let calc = ContextCalculator::default();
        assert_eq!(calc.estimate_tokens(""), 0);
    }

    #[test]
    fn test_estimate_tokens_basic() {
        let calc = ContextCalculator::default();
        // "hello" = 5 chars / 4.0 = 1.25 -> ceil = 2
        assert_eq!(calc.estimate_tokens("hello"), 2);

        // 100 chars / 4.0 = 25
        let text = "a".repeat(100);
        assert_eq!(calc.estimate_tokens(&text), 25);
    }

    #[test]
    fn test_estimate_tokens_custom_ratio() {
        let calc = ContextCalculator::new(3.0);
        // "hello" = 5 chars / 3.0 = 1.67 -> ceil = 2
        assert_eq!(calc.estimate_tokens("hello"), 2);

        // 90 chars / 3.0 = 30
        let text = "a".repeat(90);
        assert_eq!(calc.estimate_tokens(&text), 30);
    }

    #[test]
    fn test_compute_budget() {
        let env = EnvironmentInfo::builder()
            .cwd("/tmp/test")
            .model("test-model")
            .context_window(100000)
            .max_output_tokens(10000)
            .build()
            .unwrap();

        let calc = ContextCalculator::default();
        let system_prompt = "a".repeat(4000); // ~1000 tokens
        let tool_defs = vec!["a".repeat(400)]; // ~100 tokens
        let memory = vec![MemoryFile {
            path: "CLAUDE.md".to_string(),
            content: "a".repeat(2000),
            priority: 0,
        }]; // ~500 tokens

        let budget = calc.compute_budget(&env, &system_prompt, &tool_defs, &memory);

        assert_eq!(budget.total_tokens, 100000);
        assert_eq!(budget.output_reserved, 10000);
        assert!(budget.total_used() > 0);
        assert!(budget.available() > 0);

        // Conversation history should get the rest
        assert!(budget.remaining_for(BudgetCategory::ConversationHistory) > 0);
    }

    #[test]
    fn test_needs_compaction() {
        let calc = ContextCalculator::default();

        let mut budget = ContextBudget::new(100000, 10000);
        assert!(!calc.needs_compaction(&budget, 0.8));

        // Use 80% of input budget
        budget.record_usage(BudgetCategory::ConversationHistory, 72000);
        assert!(calc.needs_compaction(&budget, 0.8));
    }
}
