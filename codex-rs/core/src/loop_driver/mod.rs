//! Loop driver for iterative agent execution.
//!
//! This module provides loop/time-based execution control for the agent.
//!
//! # Features
//!
//! - **Count-based loops**: Run N iterations
//! - **Time-based loops**: Run until duration elapsed
//! - **Git-based prompts**: Auto-Coder style iterative improvement
//! - **Continue-on-error**: Iterations continue after single failure
//! - **Cancellation support**: Graceful shutdown via CancellationToken
//!
//! # Usage Patterns
//!
//! This module provides two usage patterns:
//!
//! 1. **exec mode** (inline loop): Used in `exec/src/lib.rs` for tight integration
//!    with approval flow and event processing. Manually controls iteration logic
//!    using `should_continue()`, `build_query()`, and `mark_iteration_complete()`.
//!
//! 2. **spawn_task mode** (LoopDriver): Used by `SpawnAgent` for isolated execution.
//!    Uses `run_with_loop()` for clean encapsulation where the driver handles
//!    the entire loop lifecycle.
//!
//! Both patterns share the same core logic (`LoopCondition`, `LoopPromptBuilder`).
//!
//! # Example
//!
//! ```rust,ignore
//! use codex_core::loop_driver::{LoopCondition, LoopDriver};
//!
//! let condition = LoopCondition::parse("5")?;  // or "1h"
//! let driver = LoopDriver::new(condition, token);
//!
//! let result = driver.run_with_loop(&codex, "query", None).await;
//!
//! // Check results including any failed iterations
//! println!("Completed {}/{} iterations ({} failed)",
//!     result.iterations_succeeded,
//!     result.iterations_attempted,
//!     result.iterations_failed
//! );
//! ```

mod condition;
mod context;
mod driver;
pub mod git_ops;
mod prompt;
pub mod summarizer;

pub use condition::LoopCondition;
pub use context::IterationRecord;
pub use context::LoopContext;
pub use driver::LoopDriver;
pub use driver::LoopProgress;
pub use driver::LoopResult;
pub use driver::LoopStopReason;
pub use driver::SummarizerContext;
pub use prompt::LoopPromptBuilder;
pub use prompt::build_enhanced_prompt;
