//! LSP client library for codex-rs
//!
//! Provides AI-friendly LSP operations with symbol name resolution
//! instead of exact line/column positions.
//!
//! # Supported Languages
//!
//! - Rust (rust-analyzer)
//! - Go (gopls)
//! - Python (pyright)
//! - TypeScript/JavaScript (typescript-language-server)
//!
//! # Example
//!
//! ```ignore
//! use codex_lsp::{LspServersConfig, LspServerManager, DiagnosticsStore, SymbolKind};
//! use std::sync::Arc;
//!
//! let diagnostics = Arc::new(DiagnosticsStore::new());
//! let config = LspServersConfig::default();
//! let manager = LspServerManager::new(config, diagnostics);
//!
//! // Get client for a Rust file
//! let client = manager.get_client(Path::new("src/lib.rs")).await?;
//!
//! // Find definition using symbol name (AI-friendly)
//! let locations = client.definition(
//!     Path::new("src/lib.rs"),
//!     "Config",
//!     Some(SymbolKind::Struct)
//! ).await?;
//! ```

mod client;
mod server;

pub mod client_ext;
pub mod config;
pub mod diagnostics;
pub mod error;
pub mod lifecycle;
pub mod protocol;
pub mod symbols;

// Public exports
pub use client::LspClient;
pub use config::BUILTIN_SERVERS;
pub use config::BuiltinServer;
pub use config::LSP_SERVERS_CONFIG_FILE;
pub use config::LifecycleConfig;
pub use config::LspServerConfig;
pub use config::LspServersConfig;
pub use diagnostics::DiagnosticEntry;
pub use diagnostics::DiagnosticSeverityLevel;
pub use diagnostics::DiagnosticsStore;
pub use error::LspErr;
pub use error::Result;
pub use lifecycle::ServerHealth;
pub use lifecycle::ServerLifecycle;
pub use lifecycle::ServerStats;
pub use protocol::TimeoutConfig;
pub use server::LspServerManager;
pub use symbols::ResolvedSymbol;
pub use symbols::SymbolKind;
pub use symbols::SymbolMatch;
pub use symbols::find_matching_symbols;
pub use symbols::flatten_symbols;

// Re-export lsp_types for handler use
pub use lsp_types::CallHierarchyIncomingCall;
pub use lsp_types::CallHierarchyItem;
pub use lsp_types::CallHierarchyOutgoingCall;
pub use lsp_types::Location;
pub use lsp_types::SymbolInformation;

/// Re-export lsp_types for test use in other crates
pub mod lsp_types_reexport {
    pub use lsp_types::Position;
    pub use lsp_types::Range;
    pub use lsp_types::SymbolKind as LspSymbolKind;
    pub use lsp_types::Url;
}
