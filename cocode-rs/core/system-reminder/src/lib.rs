//! cocode-system-reminder - Dynamic context injection for agent conversations.
//!
//! This crate provides the system reminder infrastructure for injecting dynamic
//! contextual metadata into agent conversations. It mirrors Claude Code's
//! attachment system with XML-tagged `<system-reminder>` messages.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                    cocode-system-reminder                           │
//! ├─────────────────────────────────────────────────────────────────────┤
//! │  Orchestrator          │  Generators           │  Types            │
//! │  - parallel execution  │  - ChangedFiles       │  - AttachmentType │
//! │  - timeout protection  │  - PlanMode*          │  - ReminderTier   │
//! │  - tier filtering      │  - TodoReminders      │  - XmlTag         │
//! │  - throttle management │  - LspDiagnostics     │  - SystemReminder │
//! │                        │  - NestedMemory       │                   │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # System Prompt vs System Reminder
//!
//! | System | Type | When | What | Where |
//! |--------|------|------|------|-------|
//! | core/prompt | Static | Build time | System prompt template | Main system message |
//! | system-reminder | Dynamic | Per-turn | Contextual metadata | Conversation history |
//!
//! They are complementary:
//! - `core/prompt` builds the **static base prompt** (identity, tool policy, etc.)
//! - `system-reminder` injects **dynamic context** (file changes, plan mode, diagnostics)
//!
//! # Quick Start
//!
//! ```ignore
//! use cocode_system_reminder::{
//!     SystemReminderOrchestrator, SystemReminderConfig, GeneratorContext,
//! };
//!
//! // Create orchestrator with default config
//! let config = SystemReminderConfig::default();
//! let orchestrator = SystemReminderOrchestrator::new(config);
//!
//! // Build context for this turn
//! let ctx = GeneratorContext::builder()
//!     .turn_number(5)
//!     .is_main_agent(true)
//!     .has_user_input(true)
//!     .build();
//!
//! // Generate all applicable reminders
//! let reminders = orchestrator.generate_all(&ctx).await;
//!
//! // Inject into message history
//! inject_reminders(reminders, &mut messages, turn_id);
//! ```

pub mod config;
pub mod error;
pub mod file_tracker;
pub mod file_watcher;
pub mod generator;
pub mod generators;
pub mod inject;
pub mod orchestrator;
pub mod parsing;
pub mod throttle;
pub mod types;
pub mod xml;

// Re-export main types at crate root
pub use config::SystemReminderConfig;
pub use error::Result;
pub use error::SystemReminderError;
pub use file_tracker::FileTracker;
pub use file_tracker::ReadFileState;
pub use file_watcher::FileChangeEvent;
pub use file_watcher::FileChangeKind;
pub use file_watcher::FileSystemWatcher;
pub use file_watcher::FileWatcherConfig;
pub use generator::AttachmentGenerator;
pub use generator::GeneratorContext;
pub use generator::GeneratorContextBuilder;
pub use generator::QueuedCommandInfo;
pub use inject::combine_reminders;
pub use inject::inject_reminders;
pub use orchestrator::SystemReminderOrchestrator;
pub use throttle::ThrottleConfig;
pub use throttle::ThrottleManager;
pub use types::AttachmentType;
pub use types::ReminderTier;
pub use types::SystemReminder;
pub use types::XmlTag;
pub use xml::extract_system_reminder;
pub use xml::wrap_system_reminder;
pub use xml::wrap_with_tag;

// Parsing utilities
pub use parsing::AgentMention;
pub use parsing::FileMention;
pub use parsing::ParsedMentions;
pub use parsing::parse_agent_mentions;
pub use parsing::parse_file_mentions;
pub use parsing::parse_mentions;

/// Prelude module for convenient imports.
pub mod prelude {
    pub use crate::config::SystemReminderConfig;
    pub use crate::file_tracker::FileTracker;
    pub use crate::file_watcher::FileChangeEvent;
    pub use crate::file_watcher::FileSystemWatcher;
    pub use crate::generator::AttachmentGenerator;
    pub use crate::generator::GeneratorContext;
    pub use crate::inject::inject_reminders;
    pub use crate::orchestrator::SystemReminderOrchestrator;
    pub use crate::types::AttachmentType;
    pub use crate::types::ReminderTier;
    pub use crate::types::SystemReminder;
    pub use crate::types::XmlTag;
    pub use crate::xml::wrap_system_reminder;
}
