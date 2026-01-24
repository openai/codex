//! Plugin custom command registry for TUI.
//!
//! Stores and manages custom slash commands from plugins.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::LazyLock;
use tokio::sync::RwLock;

/// Entry representing a plugin custom command.
#[derive(Debug, Clone)]
pub struct PluginCommandEntry {
    /// Command name (e.g., "my-plugin:review").
    pub name: String,
    /// Description shown in popup.
    pub description: String,
    /// Path to the prompt file.
    pub path: PathBuf,
    /// Source plugin ID.
    #[allow(dead_code)] // Reserved for future UI display
    pub source_plugin: String,
    /// Optional argument hint (e.g., "$FILE").
    #[allow(dead_code)] // Reserved for future hint display
    pub argument_hint: Option<String>,
}

impl PluginCommandEntry {
    /// Create from InjectedCommand.
    pub fn from_injected(cmd: &codex_plugin::injection::InjectedCommand) -> Self {
        Self {
            name: cmd.name.clone(),
            description: cmd.description.clone(),
            path: cmd.path.clone(),
            source_plugin: cmd.source_plugin.clone(),
            argument_hint: None,
        }
    }
}

/// Global registry for plugin commands.
pub struct PluginCommandRegistry {
    commands: RwLock<HashMap<String, PluginCommandEntry>>,
}

impl PluginCommandRegistry {
    /// Create new empty registry.
    pub fn new() -> Self {
        Self {
            commands: RwLock::new(HashMap::new()),
        }
    }

    /// Register a plugin command.
    #[allow(dead_code)]
    pub async fn register(&self, entry: PluginCommandEntry) {
        let mut commands = self.commands.write().await;
        commands.insert(entry.name.clone(), entry);
    }

    /// Register multiple plugin commands.
    pub async fn register_all(&self, entries: Vec<PluginCommandEntry>) -> i32 {
        let mut commands = self.commands.write().await;
        let mut count = 0;
        for entry in entries {
            commands.insert(entry.name.clone(), entry);
            count += 1;
        }
        count
    }

    /// Get a command by name.
    pub async fn get(&self, name: &str) -> Option<PluginCommandEntry> {
        let commands = self.commands.read().await;
        commands.get(name).cloned()
    }

    /// List all registered commands.
    pub async fn list(&self) -> Vec<PluginCommandEntry> {
        let commands = self.commands.read().await;
        commands.values().cloned().collect()
    }

    /// Clear all commands.
    #[allow(dead_code)]
    pub async fn clear(&self) {
        let mut commands = self.commands.write().await;
        commands.clear();
    }

    /// Check if a command is registered.
    #[allow(dead_code)]
    pub async fn contains(&self, name: &str) -> bool {
        let commands = self.commands.read().await;
        commands.contains_key(name)
    }
}

impl Default for PluginCommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Global plugin command registry instance.
static PLUGIN_COMMANDS: LazyLock<PluginCommandRegistry> = LazyLock::new(PluginCommandRegistry::new);

/// Get the global plugin command registry.
pub fn plugin_commands() -> &'static PluginCommandRegistry {
    &PLUGIN_COMMANDS
}

/// Initialize plugin commands from the plugin service.
///
/// This should be called during TUI startup to load all plugin commands.
pub async fn init_plugin_commands(codex_home: &std::path::Path) -> i32 {
    let service = match codex_plugin::get_plugin_service().await {
        Some(s) => s,
        None => {
            // Try to initialize the service
            match codex_plugin::get_or_init_plugin_service(codex_home).await {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("Failed to initialize plugin service: {e}");
                    return 0;
                }
            }
        }
    };

    let commands = service.get_commands().await;
    let entries: Vec<PluginCommandEntry> = commands
        .iter()
        .map(PluginCommandEntry::from_injected)
        .collect();

    let count = entries.len() as i32;
    PLUGIN_COMMANDS.register_all(entries).await;
    tracing::debug!("Registered {count} plugin commands");
    count
}

/// Expand a plugin command prompt, replacing $ARGUMENTS with the provided args.
pub async fn expand_plugin_command(name: &str, args: &str) -> Option<String> {
    let entry = PLUGIN_COMMANDS.get(name).await?;
    let content = match tokio::fs::read_to_string(&entry.path).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(
                "Failed to read plugin command prompt at {:?}: {e}",
                entry.path
            );
            return None;
        }
    };
    Some(content.replace("$ARGUMENTS", args).replace("$FILE", args))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_registry_operations() {
        let registry = PluginCommandRegistry::new();

        // Register a command
        registry
            .register(PluginCommandEntry {
                name: "test:cmd".to_string(),
                description: "Test command".to_string(),
                path: PathBuf::from("/tmp/test.md"),
                source_plugin: "test-plugin".to_string(),
                argument_hint: None,
            })
            .await;

        // Verify registration
        assert!(registry.contains("test:cmd").await);
        assert!(!registry.contains("nonexistent").await);

        // Get command
        let cmd = registry.get("test:cmd").await;
        assert!(cmd.is_some());
        assert_eq!(cmd.unwrap().description, "Test command");

        // List commands
        let list = registry.list().await;
        assert_eq!(list.len(), 1);

        // Clear
        registry.clear().await;
        assert!(!registry.contains("test:cmd").await);
    }
}
