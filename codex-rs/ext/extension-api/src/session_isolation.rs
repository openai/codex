//! Host-supplied isolation for internal runtimes, independent of their attribution.
//! Isolation only removes inherited capabilities; it never grants review authority.

/// Runtime policy supplied through `ExtensionDataInit` before session startup.
/// The host captures this value once so later extension-state changes cannot alter it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SessionIsolation {
    /// Use the host's ordinary instruction providers, extensions and execution rules.
    #[default]
    Inherit,
    /// Start without inherited instruction providers or extensions, retain only managed
    /// execution rules, and omit executor-discovered MCP servers. Explicitly supplied
    /// instructions and permissions remain the responsibility of the internal caller.
    Isolated,
}
