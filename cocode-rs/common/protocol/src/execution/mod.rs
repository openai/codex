//! Execution context types for LLM inference.
//!
//! This module provides types for managing model selection and inference context:
//!
//! - [`ExecutionIdentity`]: How to resolve a model (by role, spec, or inherit)
//! - [`AgentKind`]: Type of agent making the inference call
//! - [`InferenceContext`]: Complete context for an inference request
//!
//! # Design Philosophy
//!
//! These types replace the fragmented approach of passing `Option<String>` model
//! identifiers throughout the codebase. Instead:
//!
//! 1. **Intent-driven selection**: Use `ExecutionIdentity` to express _how_ to
//!    find a model, not just _what_ model to use.
//!
//! 2. **Full context propagation**: `InferenceContext` carries everything needed
//!    to build a request, enabling centralized parameter assembly.
//!
//! 3. **Explicit inheritance**: `ExecutionIdentity::Inherit` makes parent model
//!    inheritance explicit rather than implicit via `None`.

mod agent_kind;
mod identity;
mod inference_context;

pub use agent_kind::AgentKind;
pub use identity::ExecutionIdentity;
pub use inference_context::InferenceContext;
