//! Plan Mode module for Codex.
//!
//! Provides structured workflow that enforces exploration and planning before
//! executing complex tasks. In Plan Mode:
//! - Only read-only tools are allowed (with plan file exception)
//! - User must approve plan before implementation can begin
//!
//! Entry: `/plan` slash command (user-initiated only)
//! Exit: `exit_plan_mode` tool + user approval
//!
//! ## Plan File Naming (aligned with Claude Code)
//!
//! - Format: `{adjective}-{action}-{noun}.md` (e.g., "bright-exploring-aurora.md")
//! - Location: `~/.codex/plans/`
//! - Key: Uses **session-to-slug caching** - same session = same file always
//!
//! This enables proper re-entry detection: when user calls `/plan` again after
//! approving a plan, the system can detect re-entry because the file path is stable.

mod file_management;
mod state;
mod wordlist;

pub use file_management::cleanup_plan_slug;
pub use file_management::get_plan_file_path;
pub use file_management::get_plan_slug;
pub use file_management::get_plans_directory;
pub use file_management::plan_file_exists;
pub use file_management::read_plan_file;
pub use state::PlanModeState;

#[cfg(test)]
mod tests;
