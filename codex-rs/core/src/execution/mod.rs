//! Execution module for Blueprint Mode
//!
//! Provides switchable execution strategies (Single, Orchestrated, Competition).

pub mod engine;

pub use engine::ExecutionEngine;
pub use engine::ExecutionResult;
