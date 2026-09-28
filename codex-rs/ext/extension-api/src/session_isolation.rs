//! Host-supplied isolation for internal runtimes, independent of their attribution.
//! Isolation only removes inherited capabilities; it never grants review authority.
//! Isolated sessions may receive a separate registry of explicit extensions.

use std::sync::Arc;

use crate::ExtensionRegistry;

/// Runtime policy supplied through `ExtensionDataInit` before session startup.
/// The host captures this value once so later extension-state changes cannot alter it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SessionIsolation {
    /// Use the host's ordinary instruction providers, extensions and execution rules.
    #[default]
    Inherit,
    /// Start without inherited instruction providers or extensions, retain only managed
    /// execution rules, and omit implicit Apps and executor-discovered MCP servers. Explicitly supplied
    /// instructions and permissions remain the responsibility of the internal caller.
    Isolated,
}

/// Explicit extensions for an isolated session, supplied through `ExtensionDataInit`.
/// The host uses only this registry and never combines it with inherited extensions.
pub struct IsolatedSessionExtensions<C: Sync>(pub Arc<ExtensionRegistry<C>>);
