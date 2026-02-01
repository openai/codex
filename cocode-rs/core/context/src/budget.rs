//! Token budget tracking for context management.
//!
//! Tracks token allocations across categories to ensure the context window
//! is used efficiently and does not overflow.

use serde::Deserialize;
use serde::Serialize;

/// Categories for token budget allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetCategory {
    /// System prompt tokens.
    SystemPrompt,
    /// Conversation history tokens.
    ConversationHistory,
    /// Tool definition tokens.
    ToolDefinitions,
    /// Memory file tokens (CLAUDE.md, etc.).
    MemoryFiles,
    /// Injected content tokens.
    Injections,
    /// Safety margin reserved tokens.
    Reserved,
}

impl BudgetCategory {
    /// Get the string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            BudgetCategory::SystemPrompt => "system_prompt",
            BudgetCategory::ConversationHistory => "conversation_history",
            BudgetCategory::ToolDefinitions => "tool_definitions",
            BudgetCategory::MemoryFiles => "memory_files",
            BudgetCategory::Injections => "injections",
            BudgetCategory::Reserved => "reserved",
        }
    }
}

impl std::fmt::Display for BudgetCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A single budget allocation for a category.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetAllocation {
    /// Which category this allocation is for.
    pub category: BudgetCategory,
    /// Allocated token count.
    pub allocated: i32,
    /// Currently used token count.
    pub used: i32,
}

impl BudgetAllocation {
    /// Remaining tokens in this allocation.
    pub fn remaining(&self) -> i32 {
        self.allocated - self.used
    }
}

/// Token budget tracker for the context window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextBudget {
    /// Total context window tokens.
    pub total_tokens: i32,
    /// Tokens reserved for model output.
    pub output_reserved: i32,
    /// Per-category allocations.
    allocations: Vec<BudgetAllocation>,
}

impl ContextBudget {
    /// Create a new context budget.
    pub fn new(total_tokens: i32, output_reserved: i32) -> Self {
        Self {
            total_tokens,
            output_reserved,
            allocations: Vec::new(),
        }
    }

    /// Input token budget (total minus output reserved).
    pub fn input_budget(&self) -> i32 {
        self.total_tokens - self.output_reserved
    }

    /// Total tokens currently used across all categories.
    pub fn total_used(&self) -> i32 {
        self.allocations.iter().map(|a| a.used).sum()
    }

    /// Available tokens (input budget minus total used).
    pub fn available(&self) -> i32 {
        self.input_budget() - self.total_used()
    }

    /// Set allocation for a category.
    pub fn set_allocation(&mut self, category: BudgetCategory, allocated: i32) {
        if let Some(alloc) = self.allocations.iter_mut().find(|a| a.category == category) {
            alloc.allocated = allocated;
        } else {
            self.allocations.push(BudgetAllocation {
                category,
                allocated,
                used: 0,
            });
        }
    }

    /// Remaining tokens for a specific category.
    pub fn remaining_for(&self, category: BudgetCategory) -> i32 {
        self.allocations
            .iter()
            .find(|a| a.category == category)
            .map_or(0, BudgetAllocation::remaining)
    }

    /// Record token usage for a category.
    pub fn record_usage(&mut self, category: BudgetCategory, tokens: i32) {
        if let Some(alloc) = self.allocations.iter_mut().find(|a| a.category == category) {
            alloc.used += tokens;
        } else {
            self.allocations.push(BudgetAllocation {
                category,
                allocated: 0,
                used: tokens,
            });
        }
    }

    /// Context utilization ratio (0.0 to 1.0).
    pub fn utilization(&self) -> f32 {
        let budget = self.input_budget();
        if budget <= 0 {
            return 1.0;
        }
        self.total_used() as f32 / budget as f32
    }

    /// Get all allocations.
    pub fn allocations(&self) -> &[BudgetAllocation] {
        &self.allocations
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_budget_new() {
        let budget = ContextBudget::new(200000, 16384);
        assert_eq!(budget.total_tokens, 200000);
        assert_eq!(budget.output_reserved, 16384);
        assert_eq!(budget.input_budget(), 200000 - 16384);
        assert_eq!(budget.total_used(), 0);
        assert_eq!(budget.available(), 200000 - 16384);
    }

    #[test]
    fn test_budget_allocation_and_usage() {
        let mut budget = ContextBudget::new(200000, 16384);

        budget.set_allocation(BudgetCategory::SystemPrompt, 10000);
        budget.set_allocation(BudgetCategory::ToolDefinitions, 5000);

        assert_eq!(budget.remaining_for(BudgetCategory::SystemPrompt), 10000);

        budget.record_usage(BudgetCategory::SystemPrompt, 3000);
        assert_eq!(budget.remaining_for(BudgetCategory::SystemPrompt), 7000);
        assert_eq!(budget.total_used(), 3000);

        budget.record_usage(BudgetCategory::ToolDefinitions, 2000);
        assert_eq!(budget.total_used(), 5000);
        assert_eq!(budget.available(), 200000 - 16384 - 5000);
    }

    #[test]
    fn test_budget_utilization() {
        let mut budget = ContextBudget::new(100000, 10000);
        assert!((budget.utilization() - 0.0).abs() < f32::EPSILON);

        budget.record_usage(BudgetCategory::SystemPrompt, 45000);
        assert!((budget.utilization() - 0.5).abs() < f32::EPSILON);

        budget.record_usage(BudgetCategory::ConversationHistory, 45000);
        assert!((budget.utilization() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_budget_record_usage_auto_creates() {
        let mut budget = ContextBudget::new(100000, 10000);
        budget.record_usage(BudgetCategory::Injections, 500);
        assert_eq!(budget.total_used(), 500);
        // allocated is 0 but used is 500
        assert_eq!(budget.remaining_for(BudgetCategory::Injections), -500);
    }

    #[test]
    fn test_budget_category_display() {
        assert_eq!(BudgetCategory::SystemPrompt.to_string(), "system_prompt");
        assert_eq!(
            BudgetCategory::ConversationHistory.to_string(),
            "conversation_history"
        );
        assert_eq!(
            BudgetCategory::ToolDefinitions.to_string(),
            "tool_definitions"
        );
    }
}
