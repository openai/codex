//! Agent injection.

use crate::error::Result;
use crate::loader::PluginAgent;
use std::path::PathBuf;

/// Injected agent ready for AgentRegistry.
#[derive(Debug, Clone)]
pub struct InjectedAgent {
    /// Agent type identifier.
    pub agent_type: String,
    /// Path to agent definition file.
    pub definition_path: PathBuf,
    /// Source plugin ID.
    pub source_plugin: String,
    /// Display name (optional).
    pub display_name: Option<String>,
    /// When to use hint (optional).
    pub when_to_use: Option<String>,
}

/// Convert a plugin agent to injectable format.
pub fn convert_agent(agent: &PluginAgent) -> Result<InjectedAgent> {
    Ok(InjectedAgent {
        agent_type: agent.agent_type.clone(),
        definition_path: agent.path.clone(),
        source_plugin: agent.source_plugin.clone(),
        display_name: None,
        when_to_use: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_agent() {
        let plugin_agent = PluginAgent {
            agent_type: "test-agent".to_string(),
            path: PathBuf::from("/path/to/agent.md"),
            source_plugin: "test-plugin".to_string(),
        };

        let injected = convert_agent(&plugin_agent).unwrap();
        assert_eq!(injected.agent_type, "test-agent");
        assert_eq!(injected.source_plugin, "test-plugin");
    }
}
