//! System reminder generators.
//!
//! This module contains all the individual generator implementations
//! for different types of system reminders.

pub mod agent_mentions;
pub mod already_read_files;
pub mod at_mentioned_files;
pub mod available_skills;
pub mod budget_usd;
pub mod changed_files;
pub mod collab_notifications;
pub mod compact_file_reference;
pub mod delegate_mode;
pub mod hook_response;
pub mod invoked_skills;
pub mod lsp_diagnostics;
pub mod nested_memory;
pub mod output_style;
pub mod plan_mode;
pub mod plan_mode_exit;
pub mod plan_verification;
pub mod queued_commands;
pub mod security_guidelines;
pub mod todo_reminders;
pub mod token_usage;
pub mod unified_tasks;

// Re-export generators
pub use agent_mentions::AgentMentionsGenerator;
pub use already_read_files::AlreadyReadFilesGenerator;
pub use at_mentioned_files::AtMentionedFilesGenerator;
pub use available_skills::AVAILABLE_SKILLS_KEY;
pub use available_skills::AvailableSkillsGenerator;
pub use available_skills::SkillInfo;
pub use budget_usd::BudgetUsdGenerator;
pub use changed_files::ChangedFilesGenerator;
pub use collab_notifications::CollabNotificationsGenerator;
pub use compact_file_reference::CompactFileReferenceGenerator;
pub use delegate_mode::DelegateModeGenerator;
pub use hook_response::ASYNC_HOOK_RESPONSES_KEY;
pub use hook_response::AsyncHookResponseGenerator;
pub use hook_response::AsyncHookResponseInfo;
pub use hook_response::HOOK_BLOCKING_KEY;
pub use hook_response::HOOK_CONTEXT_KEY;
pub use hook_response::HookAdditionalContextGenerator;
pub use hook_response::HookBlockingErrorGenerator;
pub use hook_response::HookBlockingInfo;
pub use hook_response::HookContextInfo;
pub use invoked_skills::INVOKED_SKILLS_KEY;
pub use invoked_skills::InvokedSkillInfo;
pub use invoked_skills::InvokedSkillsGenerator;
pub use lsp_diagnostics::LspDiagnosticsGenerator;
pub use nested_memory::NestedMemoryGenerator;
pub use output_style::OutputStyleGenerator;
pub use plan_mode::PlanModeApprovedGenerator;
pub use plan_mode::PlanModeEnterGenerator;
pub use plan_mode::PlanToolReminderGenerator;
pub use plan_mode_exit::PlanModeExitGenerator;
pub use plan_verification::PlanVerificationGenerator;
pub use queued_commands::QueuedCommandsGenerator;
pub use security_guidelines::SecurityGuidelinesGenerator;
pub use todo_reminders::TodoRemindersGenerator;
pub use token_usage::TokenUsageGenerator;
pub use unified_tasks::UNIFIED_TASKS_KEY;
pub use unified_tasks::UnifiedTasksGenerator;
