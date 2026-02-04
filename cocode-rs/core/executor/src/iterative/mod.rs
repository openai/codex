//! Iterative execution module for running agents through multiple iterations.
//!
//! This module provides:
//! - `IterativeExecutor`: Main executor for iterative agent execution
//! - `IterationContext`: Cross-iteration state management
//! - `IterationCondition`: Loop termination conditions
//! - `git_ops`: Git operations for context passing
//! - `prompt_builder`: Enhanced prompt construction with context injection
//! - `Summarizer`: Iteration summarization utilities
//!
//! # LLM Callbacks for Summarization
//!
//! The executor supports optional LLM callbacks for generating iteration summaries
//! and commit messages. Use `with_summarize_fn` and `with_commit_msg_fn` to provide
//! LLM-powered callbacks. If not provided, fallback to file-based summaries.
//!
//! For creating default LLM callbacks with hyper-sdk, see:
//! - [`summarizer::create_summarize_fn`] - Creates a summarization callback
//! - [`summarizer::create_commit_msg_fn`] - Creates a commit message callback

pub mod condition;
pub mod context;
pub mod executor;
pub mod git_ops;
pub mod prompt_builder;
pub mod summarizer;

// Re-export main types
pub use condition::IterationCondition;
pub use context::IterationContext;
pub use context::IterationRecord;
pub use executor::ContextPassingConfig;
pub use executor::IterationExecuteFn;
pub use executor::IterationInput;
pub use executor::IterationOutput;
pub use executor::IterationProgress;
pub use executor::IterativeExecutor;
pub use executor::SimpleIterationExecuteFn;
pub use prompt_builder::IterativePromptBuilder;
pub use summarizer::CommitMessageFn;
pub use summarizer::SummarizeFn;
pub use summarizer::Summarizer;
pub use summarizer::create_commit_msg_fn;
pub use summarizer::create_summarize_fn;
