//! Agent registry for discovering and loading agent definitions.

use super::SubagentErr;
use super::definition::AgentDefinition;
use super::definition::get_builtin_agents;
use super::definition::parse_agent_definition;
use dashmap::DashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

/// Registry for agent definitions.
#[derive(Debug)]
pub struct AgentRegistry {
    /// Map of agent type to definition.
    agents: DashMap<String, Arc<AgentDefinition>>,
    /// Directories to search for user-defined agents.
    search_paths: Vec<PathBuf>,
}

impl AgentRegistry {
    /// Create a new registry with built-in agents.
    pub fn new() -> Self {
        let registry = Self {
            agents: DashMap::new(),
            search_paths: Vec::new(),
        };

        // Register built-in agents
        for agent in get_builtin_agents() {
            registry.register(agent.clone());
        }

        registry
    }

    /// Create registry with custom search paths.
    pub fn with_search_paths(search_paths: Vec<PathBuf>) -> Self {
        let mut registry = Self::new();
        registry.search_paths = search_paths;
        registry
    }

    /// Register an agent definition.
    pub fn register(&self, definition: AgentDefinition) {
        self.agents
            .insert(definition.agent_type.clone(), Arc::new(definition));
    }

    /// Get an agent definition by type.
    ///
    /// Uses or_insert pattern to handle race conditions when multiple
    /// callers try to load the same agent concurrently.
    pub async fn get(&self, agent_type: &str) -> Option<Arc<AgentDefinition>> {
        // Fast path: already cached
        if let Some(agent) = self.agents.get(agent_type) {
            return Some(agent.clone());
        }

        // Slow path: load from search paths
        // Note: Multiple callers may load concurrently, but or_insert
        // ensures only the first insert wins (no duplicates in cache).
        if let Some(definition) = self.load_from_paths(agent_type).await {
            let arc = Arc::new(definition);
            // Use or_insert to handle race - first writer wins
            let entry = self
                .agents
                .entry(agent_type.to_string())
                .or_insert(arc.clone());
            Some(entry.clone())
        } else {
            None
        }
    }

    /// List all registered agent types.
    pub async fn list_types(&self) -> Vec<String> {
        self.agents.iter().map(|r| r.key().clone()).collect()
    }

    /// Load agent definition from search paths.
    async fn load_from_paths(&self, agent_type: &str) -> Option<AgentDefinition> {
        for path in &self.search_paths {
            // Try .yaml and .md extensions
            for ext in &["yaml", "yml", "md"] {
                let file_path = path.join(format!("{agent_type}.{ext}"));
                if let Some(def) = self.load_from_file(&file_path).await {
                    return Some(def);
                }
            }
        }
        None
    }

    /// Load agent definition from a file.
    async fn load_from_file(&self, path: &Path) -> Option<AgentDefinition> {
        let content = tokio::fs::read_to_string(path).await.ok()?;
        parse_agent_definition(&content).ok()
    }

    /// Scan search paths and load all agent definitions.
    pub async fn scan_and_load(&self) -> Result<i32, SubagentErr> {
        let mut loaded = 0;

        for search_path in &self.search_paths {
            if !search_path.exists() {
                continue;
            }

            let mut entries = tokio::fs::read_dir(search_path)
                .await
                .map_err(|e| SubagentErr::Internal(format!("Failed to read directory: {e}")))?;

            while let Some(entry) = entries
                .next_entry()
                .await
                .map_err(|e| SubagentErr::Internal(format!("Failed to read entry: {e}")))?
            {
                let path = entry.path();
                let ext = path.extension().and_then(|e| e.to_str());

                if matches!(ext, Some("yaml") | Some("yml") | Some("md")) {
                    if let Some(def) = self.load_from_file(&path).await {
                        self.register(def);
                        loaded += 1;
                    }
                }
            }
        }

        Ok(loaded)
    }

    /// Register agents from plugins.
    ///
    /// Each plugin agent definition file is loaded and the agent is registered
    /// with `AgentSource::Plugin(plugin_id)`.
    ///
    /// # Arguments
    ///
    /// * `agents` - List of injected agents from plugins
    pub async fn register_plugin_agents(
        &self,
        agents: Vec<codex_plugin::injection::InjectedAgent>,
    ) -> i32 {
        use super::definition::AgentSource;

        let mut registered = 0;

        for agent in agents {
            // Load definition from file
            if let Some(mut definition) = self.load_from_file(&agent.definition_path).await {
                // Override source to indicate plugin origin
                definition.source = AgentSource::Plugin(agent.source_plugin.clone());

                // Override display_name if provided
                if let Some(name) = agent.display_name {
                    definition.display_name = Some(name);
                }

                // Override when_to_use if provided
                if let Some(when) = agent.when_to_use {
                    definition.when_to_use = Some(when);
                }

                tracing::debug!(
                    agent_type = %definition.agent_type,
                    plugin = %agent.source_plugin,
                    "Registering plugin agent"
                );

                self.register(definition);
                registered += 1;
            } else {
                tracing::warn!(
                    path = %agent.definition_path.display(),
                    plugin = %agent.source_plugin,
                    "Failed to load plugin agent definition"
                );
            }
        }

        tracing::info!("Registered {} plugin agents", registered);
        registered
    }
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_builtin_agents_registered() {
        let registry = AgentRegistry::new();

        let explore = registry.get("Explore").await;
        assert!(explore.is_some());
        assert_eq!(explore.unwrap().agent_type, "Explore");

        let plan = registry.get("Plan").await;
        assert!(plan.is_some());
    }

    #[tokio::test]
    async fn test_list_types() {
        let registry = AgentRegistry::new();
        let types = registry.list_types().await;

        assert!(types.contains(&"Explore".to_string()));
        assert!(types.contains(&"Plan".to_string()));
    }

    #[tokio::test]
    async fn test_unknown_agent() {
        let registry = AgentRegistry::new();
        let unknown = registry.get("NonExistent").await;
        assert!(unknown.is_none());
    }
}
