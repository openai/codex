//! Multi-agent orchestration system with customizable system prompts
//!
//! This module provides a lightweight agent system where agents are primarily
//! specialized through custom system prompts while inheriting tools and permissions
//! from the current workspace context.

use crate::error::Result;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;

/// Configuration for a single agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// The system prompt that defines the agent's behavior
    pub prompt: String,

    /// Optional: Load prompt from file instead of inline
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_file: Option<String>,

    /// Optional: Override tools (usually inherits from context)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<String>>,

    /// Optional: Override permissions (usually inherits from context)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<String>,
}

/// Registry of available agents and their configurations
pub struct AgentRegistry {
    agents: HashMap<String, AgentConfig>,
    #[allow(dead_code)]
    agents_dir: Option<PathBuf>,
}

impl AgentRegistry {
    /// Create a new agent registry, loading user configurations if available
    pub fn new() -> Result<Self> {
        let mut agents = HashMap::new();

        // Add the single default "general" agent
        agents.insert(
            "general".to_string(),
            AgentConfig {
                prompt: "You are a helpful AI assistant. Complete the given task efficiently and accurately.".to_string(),
                prompt_file: None,
                tools: None,
                permissions: None,
            }
        );

        // Try to load user agents from ~/.codex/agents.toml
        let agents_dir = Self::get_agents_directory();
        if let Some(ref dir) = agents_dir {
            let config_path = dir.join("agents.toml");
            if config_path.exists() {
                match std::fs::read_to_string(&config_path) {
                    Ok(content) => {
                        match toml::from_str::<HashMap<String, AgentConfig>>(&content) {
                            Ok(user_agents) => {
                                // Process each agent config
                                for (name, mut config) in user_agents {
                                    // If prompt_file is specified, load the prompt from file
                                    if let Some(ref prompt_file) = config.prompt_file {
                                        let prompt_path = if prompt_file.starts_with('/') {
                                            PathBuf::from(prompt_file)
                                        } else {
                                            dir.join(prompt_file)
                                        };

                                        if let Ok(prompt_content) =
                                            std::fs::read_to_string(prompt_path)
                                        {
                                            config.prompt = prompt_content;
                                        } else {
                                            tracing::warn!(
                                                "Could not load prompt file for agent '{}': {}",
                                                name,
                                                prompt_file
                                            );
                                            continue;
                                        }
                                    }

                                    agents.insert(name, config);
                                }
                                tracing::info!("Loaded {} user-defined agents", agents.len() - 1);
                            }
                            Err(e) => {
                                tracing::warn!("Failed to parse agents.toml: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::debug!("Could not read agents.toml: {}", e);
                    }
                }
            }
        }

        Ok(Self { agents, agents_dir })
    }

    /// Get the agents directory path (~/.codex/agents)
    fn get_agents_directory() -> Option<PathBuf> {
        std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .ok()
            .map(|home| PathBuf::from(home).join(".codex"))
    }

    /// Get an agent configuration by name
    #[allow(dead_code)]
    pub fn get_agent(&self, name: &str) -> Option<&AgentConfig> {
        self.agents.get(name)
    }

    /// Get the system prompt for an agent (falls back to "general" if not found)
    pub fn get_system_prompt(&self, agent_name: &str) -> String {
        self.agents
            .get(agent_name)
            .or_else(|| self.agents.get("general"))
            .map(|config| config.prompt.clone())
            .unwrap_or_else(|| "You are a helpful AI assistant.".to_string())
    }

    /// List all available agents
    #[allow(dead_code)]
    pub fn list_agents(&self) -> Vec<String> {
        self.agents.keys().cloned().collect()
    }

    /// Get detailed information about all agents
    pub fn list_agent_details(&self) -> Vec<crate::protocol::AgentInfo> {
        let mut agents = Vec::new();

        for (name, config) in &self.agents {
            let description = self.extract_description(&config.prompt);
            agents.push(crate::protocol::AgentInfo {
                name: name.clone(),
                description,
                is_builtin: name == "general",
            });
        }

        agents.sort_by(|a, b| {
            // Built-in agents first, then alphabetical
            match (a.is_builtin, b.is_builtin) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.cmp(&b.name),
            }
        });

        agents
    }

    /// Extract brief description from prompt
    fn extract_description(&self, prompt: &str) -> String {
        // Take first line or first sentence as description
        let first_line = prompt.lines().next().unwrap_or("");
        let desc = if let Some(pos) = first_line.find('.') {
            &first_line[..=pos]
        } else {
            first_line
        };

        // Clean up common prefixes
        desc.trim_start_matches("You are a ")
            .trim_start_matches("You are an ")
            .trim_start_matches("You are ")
            .trim()
            .to_string()
    }

    /// Check if agents can spawn other agents (always false to prevent recursion)
    #[allow(dead_code)]
    pub fn can_spawn_agents(metadata: &HashMap<String, String>) -> bool {
        !metadata.contains_key("is_agent")
    }

    /// Mark a context as being an agent context
    #[allow(dead_code)]
    pub fn mark_as_agent_context(metadata: &mut HashMap<String, String>) {
        metadata.insert("is_agent".to_string(), "true".to_string());
    }
}

/// Execute an agent with a specific task
#[allow(dead_code)]
pub async fn execute_agent_task(
    agent_name: &str,
    task: String,
    registry: &AgentRegistry,
) -> Result<String> {
    // Get the agent's system prompt
    let system_prompt = registry.get_system_prompt(agent_name);

    // Build the specialized prompt for this agent
    let full_prompt = format!("{system_prompt}\n\nTask: {task}");

    // Note: The actual execution will be handled by the parent context
    // using the existing conversation infrastructure
    Ok(full_prompt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_agent_exists() {
        let registry = AgentRegistry::new().unwrap();
        assert!(registry.get_agent("general").is_some());
    }

    #[test]
    fn test_agent_recursion_prevention() {
        let mut metadata = HashMap::new();
        assert!(AgentRegistry::can_spawn_agents(&metadata));

        AgentRegistry::mark_as_agent_context(&mut metadata);
        assert!(!AgentRegistry::can_spawn_agents(&metadata));
    }
}
