mod experimental_api;
#[cfg(test)]
mod export;
mod precomputed_exports;
#[cfg(test)]
#[path = "precomputed_exports_tests.rs"]
mod precomputed_exports_tests;
mod protocol;
pub mod rpc;
#[cfg(test)]
mod schema_fixtures;
#[cfg(test)]
#[path = "schema_fixtures_tests.rs"]
mod schema_fixtures_tests;

pub use experimental_api::*;
pub use precomputed_exports::GenerateTsOptions;
pub use precomputed_exports::generate_internal_json_schema;
pub use precomputed_exports::generate_json;
pub use precomputed_exports::generate_json_with_experimental;
pub use precomputed_exports::generate_ts;
pub use precomputed_exports::generate_ts_with_options;
pub use precomputed_exports::generate_types;
pub use protocol::common::*;
pub use protocol::event_mapping::*;
pub use protocol::item_builders::*;
pub use protocol::thread_history::*;
pub use protocol::thread_history_projection::*;
pub use protocol::v1::ApplyPatchApprovalParams;
pub use protocol::v1::ApplyPatchApprovalResponse;
pub use protocol::v1::ClientInfo;
pub use protocol::v1::ConversationGitInfo;
pub use protocol::v1::ConversationSummary;
pub use protocol::v1::ExecCommandApprovalParams;
pub use protocol::v1::ExecCommandApprovalResponse;
pub use protocol::v1::GetAuthStatusParams;
pub use protocol::v1::GetAuthStatusResponse;
pub use protocol::v1::GetConversationSummaryParams;
pub use protocol::v1::GetConversationSummaryResponse;
pub use protocol::v1::GitDiffToRemoteParams;
pub use protocol::v1::GitDiffToRemoteResponse;
pub use protocol::v1::GitSha;
pub use protocol::v1::InitializeCapabilities;
pub use protocol::v1::InitializeParams;
pub use protocol::v1::InitializeResponse;
pub use protocol::v1::InterruptConversationResponse;
pub use protocol::v1::LoginApiKeyParams;
pub use protocol::v1::SandboxSettings;
pub use protocol::v1::Tools;
pub use protocol::v1::UserSavedConfig;
pub use protocol::v2::*;
pub use rpc::*;
#[cfg(test)]
pub use schema_fixtures::SchemaFixtureOptions;
#[cfg(test)]
pub use schema_fixtures::read_schema_fixture_subtree;
#[cfg(test)]
pub use schema_fixtures::read_schema_fixture_tree;
#[cfg(test)]
pub use schema_fixtures::write_schema_fixtures;
#[cfg(test)]
pub use schema_fixtures::write_schema_fixtures_with_options;

#[cfg(not(test))]
pub(crate) use codex_app_server_protocol_noop_macros::JsonSchema;
#[cfg(not(test))]
pub(crate) use codex_app_server_protocol_noop_macros::TS;
#[cfg(test)]
pub(crate) use schemars::JsonSchema;
#[cfg(test)]
pub(crate) use ts_rs::TS;
