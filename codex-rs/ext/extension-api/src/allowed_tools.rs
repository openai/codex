//! A startup ceiling on the tools a thread may advertise or execute.

use crate::ToolName;

/// Supply through `ExtensionDataInit` before starting a thread. The host captures
/// this value once; changing extension state later cannot widen the tool set.
/// Callers must supply it again when resuming a thread.
///
/// An absent value keeps ordinary tool setup. An empty list permits no tools.
/// Names include their namespace; a plain name uses the default namespace.
/// This only removes tools: configuration and permission checks still apply.
/// Generated tools such as Code Mode's `exec` and `wait` must also be listed.
#[derive(Clone, Debug, Default)]
pub struct AllowedTools(pub Vec<ToolName>);

impl AllowedTools {
    pub fn contains(&self, tool: &ToolName) -> bool {
        self.0.iter().any(|allowed| {
            allowed.name == tool.name
                && (allowed.namespace == tool.namespace
                    || (allowed.is_default_namespace() && tool.is_default_namespace()))
        })
    }
}
