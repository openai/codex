//! Skill system for cocode-rs.
//!
//! This crate implements a skill loading and management system that supports:
//! - Scanning directories for `SKILL.toml` files
//! - Validating skill metadata (name, description, prompt)
//! - Bundled skills with SHA-256 fingerprinting
//! - Deduplication of loaded skills by name
//!
//! # Architecture
//!
//! Skills are discovered from multiple sources (bundled, project-local,
//! user-global, plugin) and loaded through a pipeline:
//!
//! 1. **Scan** - [`scanner::SkillScanner`] discovers skill directories
//! 2. **Load** - [`loader`] reads and parses `SKILL.toml` files
//! 3. **Validate** - [`validator`] checks constraints on skill metadata
//! 4. **Dedup** - [`dedup`] removes duplicate skills by name

pub mod bundled;
pub mod command;
pub mod dedup;
pub mod interface;
pub mod loader;
pub mod manager;
pub mod outcome;
pub mod scanner;
pub mod source;
pub mod validator;

mod error;

// Re-export primary types
pub use bundled::{BundledSkill, bundled_skills, compute_fingerprint};
pub use command::{CommandType, SkillPromptCommand, SlashCommand};
pub use dedup::{SkillDeduplicator, dedup_skills};
pub use interface::SkillInterface;
pub use loader::{load_all_skills, load_skills_from_dir};
pub use manager::{
    SkillExecutionResult, SkillLoadResult, SkillManager, execute_skill, parse_skill_command,
};
pub use outcome::SkillLoadOutcome;
pub use scanner::SkillScanner;
pub use source::{LoadedFrom, SkillSource};
pub use validator::validate_skill;

// Re-export the error type
pub use error::{Result, SkillError};
