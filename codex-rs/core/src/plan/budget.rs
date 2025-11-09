//! Budget enforcement for Plan execution
//!
//! Tracks and enforces token and time budgets to prevent runaway costs.

use super::schema::Budget;
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;
use thiserror::Error;

/// Budget tracker for a Plan execution session
#[derive(Debug, Clone)]
pub struct BudgetTracker {
    budget: Budget,
    tokens_used: Arc<AtomicU64>,
    start_time: Instant,
}

/// Budget usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetUsage {
    /// Tokens used so far
    pub tokens_used: u64,

    /// Elapsed time in seconds
    pub elapsed_secs: f64,

    /// Token budget remaining
    pub tokens_remaining: Option<u64>,

    /// Time budget remaining in seconds
    pub time_remaining_secs: Option<f64>,

    /// Whether token budget is exceeded
    pub tokens_exceeded: bool,

    /// Whether time budget is exceeded
    pub time_exceeded: bool,
}

/// Budget enforcement errors
#[derive(Debug, Error)]
pub enum BudgetError {
    #[error("Token budget exceeded: used {used}, cap {cap}")]
    TokensExceeded { used: u64, cap: u64 },

    #[error("Time budget exceeded: elapsed {elapsed:.1}min, cap {cap}min")]
    TimeExceeded { elapsed: f64, cap: f64 },

    #[error("Step token budget exceeded: used {used}, cap {cap}")]
    StepTokensExceeded { used: u64, cap: u64 },
}

impl BudgetTracker {
    /// Create a new budget tracker
    pub fn new(budget: Budget) -> Self {
        Self {
            budget,
            tokens_used: Arc::new(AtomicU64::new(0)),
            start_time: Instant::now(),
        }
    }

    /// Record tokens used in this step
    pub fn record_tokens(&self, tokens: u64) -> Result<()> {
        let total = self.tokens_used.fetch_add(tokens, Ordering::SeqCst) + tokens;

        // Check step limit
        if let Some(max_step) = self.budget.max_step {
            if tokens > max_step {
                return Err(BudgetError::StepTokensExceeded {
                    used: tokens,
                    cap: max_step,
                }
                .into());
            }
        }

        // Check session limit
        if let Some(session_cap) = self.budget.session_cap {
            if total > session_cap {
                return Err(BudgetError::TokensExceeded {
                    used: total,
                    cap: session_cap,
                }
                .into());
            }
        }

        Ok(())
    }

    /// Get current budget usage
    pub fn usage(&self) -> BudgetUsage {
        let tokens_used = self.tokens_used.load(Ordering::SeqCst);
        let elapsed = self.start_time.elapsed();
        let elapsed_secs = elapsed.as_secs_f64();
        let elapsed_mins = elapsed_secs / 60.0;

        let tokens_remaining = self
            .budget
            .session_cap
            .map(|cap| cap.saturating_sub(tokens_used));

        let time_remaining_secs = self
            .budget
            .cap_min
            .map(|cap| (cap as f64 * 60.0) - elapsed_secs);

        let tokens_exceeded = self
            .budget
            .session_cap
            .map_or(false, |cap| tokens_used > cap);

        let time_exceeded = self
            .budget
            .cap_min
            .map_or(false, |cap| elapsed_mins > cap as f64);

        BudgetUsage {
            tokens_used,
            elapsed_secs,
            tokens_remaining,
            time_remaining_secs,
            tokens_exceeded,
            time_exceeded,
        }
    }

    /// Check if execution should continue
    pub fn check(&self) -> Result<()> {
        let usage = self.usage();

        if usage.tokens_exceeded {
            if let Some(cap) = self.budget.session_cap {
                return Err(BudgetError::TokensExceeded {
                    used: usage.tokens_used,
                    cap,
                }
                .into());
            }
        }

        if usage.time_exceeded {
            if let Some(cap) = self.budget.cap_min {
                return Err(BudgetError::TimeExceeded {
                    elapsed: usage.elapsed_secs / 60.0,
                    cap: cap as f64,
                }
                .into());
            }
        }

        Ok(())
    }

    /// Get elapsed time
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Get tokens used
    pub fn tokens_used(&self) -> u64 {
        self.tokens_used.load(Ordering::SeqCst)
    }

    /// Check if within estimate
    pub fn within_estimate(&self) -> bool {
        let elapsed_mins = self.start_time.elapsed().as_secs_f64() / 60.0;

        if let Some(estimate) = self.budget.estimate_min {
            elapsed_mins <= estimate as f64
        } else {
            true
        }
    }

    /// Get budget utilization percentage (0.0-1.0+)
    pub fn utilization(&self) -> (Option<f64>, Option<f64>) {
        let tokens_used = self.tokens_used.load(Ordering::SeqCst);
        let elapsed_mins = self.start_time.elapsed().as_secs_f64() / 60.0;

        let token_utilization = self
            .budget
            .session_cap
            .map(|cap| tokens_used as f64 / cap as f64);

        let time_utilization = self.budget.cap_min.map(|cap| elapsed_mins / cap as f64);

        (token_utilization, time_utilization)
    }
}

/// Format budget usage for display
pub fn format_usage(usage: &BudgetUsage) -> String {
    let mut parts = Vec::new();

    parts.push(format!("Tokens: {}", usage.tokens_used));
    if let Some(remaining) = usage.tokens_remaining {
        parts.push(format!("({} remaining)", remaining));
    }

    let elapsed_mins = usage.elapsed_secs / 60.0;
    parts.push(format!("Time: {:.1}min", elapsed_mins));
    if let Some(remaining_secs) = usage.time_remaining_secs {
        let remaining_mins = remaining_secs / 60.0;
        parts.push(format!("({:.1}min remaining)", remaining_mins));
    }

    if usage.tokens_exceeded || usage.time_exceeded {
        parts.push("⚠️ BUDGET EXCEEDED".to_string());
    }

    parts.join(" | ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration as StdDuration;

    #[test]
    fn test_token_tracking() {
        let budget = Budget {
            max_step: Some(1000),
            session_cap: Some(5000),
            estimate_min: Some(10),
            cap_min: Some(20),
        };

        let tracker = BudgetTracker::new(budget);

        // Record tokens
        tracker.record_tokens(500).unwrap();
        assert_eq!(tracker.tokens_used(), 500);

        tracker.record_tokens(300).unwrap();
        assert_eq!(tracker.tokens_used(), 800);

        let usage = tracker.usage();
        assert_eq!(usage.tokens_used, 800);
        assert_eq!(usage.tokens_remaining, Some(4200));
        assert!(!usage.tokens_exceeded);
    }

    #[test]
    fn test_token_budget_exceeded() {
        let budget = Budget {
            max_step: Some(1000),
            session_cap: Some(2000),
            estimate_min: None,
            cap_min: None,
        };

        let tracker = BudgetTracker::new(budget);

        tracker.record_tokens(1000).unwrap();
        tracker.record_tokens(900).unwrap();

        // Should exceed budget
        let result = tracker.record_tokens(200);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err().downcast::<BudgetError>().unwrap(),
            BudgetError::TokensExceeded { .. }
        ));
    }

    #[test]
    fn test_step_budget_exceeded() {
        let budget = Budget {
            max_step: Some(100),
            session_cap: Some(1000),
            estimate_min: None,
            cap_min: None,
        };

        let tracker = BudgetTracker::new(budget);

        // Single step exceeds limit
        let result = tracker.record_tokens(150);
        assert!(result.is_err());
    }

    #[test]
    fn test_time_tracking() {
        let budget = Budget {
            max_step: None,
            session_cap: None,
            estimate_min: Some(1),
            cap_min: Some(2),
        };

        let tracker = BudgetTracker::new(budget);

        // Sleep a bit
        thread::sleep(StdDuration::from_millis(100));

        let usage = tracker.usage();
        assert!(usage.elapsed_secs > 0.0);
        assert!(!usage.time_exceeded);
    }

    #[test]
    fn test_utilization() {
        let budget = Budget {
            max_step: None,
            session_cap: Some(1000),
            estimate_min: None,
            cap_min: Some(10),
        };

        let tracker = BudgetTracker::new(budget);
        tracker.record_tokens(500).unwrap();

        let (token_util, time_util) = tracker.utilization();
        assert_eq!(token_util, Some(0.5));
        assert!(time_util.unwrap() < 1.0); // Should be well under time budget
    }

    #[test]
    fn test_format_usage() {
        let usage = BudgetUsage {
            tokens_used: 1500,
            elapsed_secs: 120.0,
            tokens_remaining: Some(500),
            time_remaining_secs: Some(180.0),
            tokens_exceeded: false,
            time_exceeded: false,
        };

        let formatted = format_usage(&usage);
        assert!(formatted.contains("1500"));
        assert!(formatted.contains("2.0min"));
        assert!(!formatted.contains("EXCEEDED"));
    }
}
