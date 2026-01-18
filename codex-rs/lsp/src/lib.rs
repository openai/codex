#![deny(clippy::print_stdout, clippy::print_stderr)]

pub mod client;
pub mod config;
pub mod detect;
pub mod diagnostics;
mod manager;
pub mod registry;
pub mod text;
mod uri;
mod workspace_edit;

pub use config::LspConfig;
pub use config::LspConfigToml;
pub use config::LspDiagnosticsInPrompt;
pub use config::LspMode;
pub use config::LspServerConfigToml;
pub use diagnostics::DiagnosticEntry;
pub use diagnostics::DiagnosticStore;
pub use diagnostics::DiagnosticSummary;
pub use diagnostics::DiagnosticSummaryLine;
pub use diagnostics::SeverityFilter;
pub use manager::LspManager;
pub use manager::LspManagerStatus;
pub use manager::LspManagerStatusEntry;
pub use registry::LanguageServerId;
pub use registry::ServerRegistry;
pub use registry::ServerSpec;
pub use workspace_edit::WorkspaceEditError;
pub use workspace_edit::WorkspaceEditResult;
pub use manager::LocationInfo;
pub use manager::LspError;
