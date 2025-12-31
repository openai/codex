//! Attachment generators for system reminders.
//!
//! Each generator produces a specific type of system reminder.

mod agent_mentions;
mod agent_task;
mod at_mentioned_files;
mod changed_files;
mod critical_instruction;
mod lsp_diagnostics;
mod nested_memory;
mod output_style;
mod plan_approved;
mod plan_mode;
mod plan_tool_reminder;
mod shell_task;

pub use agent_mentions::AgentMentionsGenerator;
pub use agent_task::AgentTaskGenerator;
pub use at_mentioned_files::AtMentionedFilesGenerator;
pub use changed_files::ChangedFilesGenerator;
pub use critical_instruction::CriticalInstructionGenerator;
pub use lsp_diagnostics::LspDiagnosticsGenerator;
pub use nested_memory::NestedMemoryGenerator;
pub use output_style::OutputStyleGenerator;
pub use plan_approved::PlanApprovedGenerator;
pub use plan_mode::PlanModeGenerator;
pub use plan_tool_reminder::PlanToolReminderGenerator;
pub use shell_task::ShellTaskGenerator;
