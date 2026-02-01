//! Session management for cocode multi-provider LLM CLI.
//!
//! This crate provides session lifecycle management including:
//! - Session metadata tracking (id, timestamps, model, provider)
//! - Session state aggregation (history, tools, hooks, skills)
//! - Multi-session management with persistence
//! - Turn execution with streaming support
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                      cocode-session                             │
//! ├─────────────────────────────────────────────────────────────────┤
//! │  Session           │  SessionState       │  SessionManager      │
//! │  - id, timestamps  │  - message_history  │  - create/load/save  │
//! │  - model, provider │  - tool_registry    │  - multi-session     │
//! │  - working_dir     │  - hook_registry    │  - persistence       │
//! │                    │  - run_turn()       │                      │
//! └────────────────────┴─────────────────────┴──────────────────────┘
//! ```
//!
//! # Example
//!
//! ```ignore
//! use cocode_session::{Session, SessionState, SessionManager};
//! use cocode_config::ConfigManager;
//! use std::path::PathBuf;
//!
//! // Create a new session
//! let session = Session::new(
//!     PathBuf::from("."),
//!     "gpt-5",
//!     cocode_protocol::ProviderType::Openai,
//! );
//!
//! // Build session state
//! let config = ConfigManager::from_default()?;
//! let mut state = SessionState::new(session, &config).await?;
//!
//! // Run a turn
//! let result = state.run_turn("Hello, world!").await?;
//! println!("Response: {}", result.final_text);
//! ```

pub mod manager;
pub mod persistence;
pub mod session;
pub mod state;

// Re-exports
pub use manager::SessionManager;
pub use persistence::load_session_from_file;
pub use persistence::save_session_to_file;
pub use session::Session;
pub use state::SessionState;
pub use state::TurnResult;
