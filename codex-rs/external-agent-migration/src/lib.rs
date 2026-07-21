//! Migration helpers for importing external-agent configuration into Codex.

mod config_values;
mod detect;
mod hooks_cla;
mod hooks_common;
mod hooks_cur;
mod mcp;
mod memory;
mod memory_import;
mod migration_source;
mod model;
mod plugins;
mod reporting;
mod rewrite;
mod scope;
mod service;
pub mod sessions;
mod source;
mod source_cla;
mod source_cur;
mod subagents;
mod utils;

use std::io;

pub use hooks_cla::hook_migration_event_names_cla;
pub use hooks_cla::hooks_migration_description_cla;
pub use hooks_cla::import_hooks_cla;
#[cfg(test)]
use hooks_common::EXTERNAL_AGENT_HOOKS_SUBDIR;
#[cfg(test)]
use hooks_common::EXTERNAL_AGENT_MIGRATED_HOOKS_SUBDIR;
#[cfg(test)]
use hooks_common::SOURCE_EXTERNAL_AGENT_NAME;
#[cfg(test)]
use hooks_common::copy_hook_scripts;
pub(crate) use hooks_common::external_agent_config_dir;
#[cfg(test)]
use hooks_common::external_agent_project_dir_env_var;
pub(crate) use hooks_common::json_u64;
pub(crate) use hooks_common::rewrite_hook_command_for_source;
#[cfg(test)]
use hooks_common::shell_single_quote;
pub(crate) use hooks_common::write_hook_migration;
pub use hooks_cur::hook_migration_event_names_cur;
pub use hooks_cur::import_hooks_cur;
#[cfg(test)]
use mcp::EXTERNAL_AGENT_MCP_CONFIG_FILE;
pub use mcp::build_mcp_config_from_external;
pub use mcp::build_mcp_config_from_json_file;
#[cfg(test)]
use mcp::external_agent_project_config_file;
#[cfg(test)]
use mcp::parse_env_placeholder;
pub use memory::ExternalMemoryFile;
pub use memory::discover_external_memory_files;
pub use rewrite::RewriteProfile;
pub use service::ExternalAgentConfigDetectOptions;
pub use service::ExternalAgentConfigImportItemResult;
pub use service::ExternalAgentConfigImportOutcome;
pub use service::ExternalAgentConfigImportRawError;
pub use service::ExternalAgentConfigImportSuccess;
pub use service::ExternalAgentConfigMigrationItem;
pub use service::ExternalAgentConfigMigrationItemType;
pub use service::ExternalAgentConfigService;
pub use service::ExternalAgentSessionImportLimits;
pub use service::MigrationDetails;
pub use service::NamedMigration;
pub use service::PendingPluginImport;
pub use service::PluginImportOutcome;
pub use service::PluginsMigration;
pub use service::record_import_error;
pub(crate) use source::ClaSource;
pub(crate) use source::CurSource;
pub(crate) use source::InstructionSourceGroup;
#[cfg(test)]
use subagents::FrontmatterValue;
#[cfg(test)]
use subagents::agent_metadata;
pub use subagents::count_missing_subagents;
pub use subagents::import_subagents_with_rewrite_profile;
pub use subagents::missing_subagent_names;
#[cfg(test)]
use subagents::parse_document_content;
#[cfg(test)]
use subagents::render_agent_toml;
#[cfg(test)]
use subagents::subagent_target_file;

fn invalid_data_error(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
