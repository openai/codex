//! Plan mode utilities for cocode-rs.
//!
//! This crate provides the plan file management and slug generation
//! aligned with Claude Code v2.1.7 behavior:
//!
//! - Plan files are stored at `~/.cocode/plans/{slug}.md`
//! - Slugs follow the format `{adjective}-{action}-{noun}` (e.g., "mossy-orbiting-donut")
//! - Session-based slug caching prevents regeneration on re-entry
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                        cocode-plan-mode                             │
//! ├─────────────────────────────────────────────────────────────────────┤
//! │  plan_slug          │  plan_file          │  state                  │
//! │  - word lists       │  - get_plan_dir()   │  - PlanModeState        │
//! │  - generate_slug()  │  - read_plan_file() │  - is_safe_file()       │
//! │  - session cache    │  - is_plan_file()   │                         │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```

mod error;
pub mod plan_file;
pub mod plan_slug;
pub mod state;

// Re-export primary types
pub use error::PlanModeError;
pub use error::Result;
pub use plan_file::PlanFileManager;
pub use plan_file::get_plan_dir;
pub use plan_file::get_plan_file_path;
pub use plan_file::is_plan_file;
pub use plan_file::read_plan_file;
pub use plan_slug::generate_slug;
pub use plan_slug::get_unique_slug;
pub use state::PlanModeState;
pub use state::is_safe_file;
