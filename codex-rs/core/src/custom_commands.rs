// Custom Command System for calling subagents and tools
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;

/// Custom command definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomCommand {
    /// Command name (e.g., "analyze_code", "security_review")
    pub name: String,

    /// Description of what the command does
    pub description: String,

    /// Target subagent type (if applicable)
    pub subagent: Option<String>,

    /// Custom parameters
    pub parameters: HashMap<String, String>,

    /// Pre-hook commands to run before execution
    pub pre_hooks: Vec<String>,

    /// Post-hook commands to run after execution
    pub post_hooks: Vec<String>,
}

impl CustomCommand {
    pub fn new(name: String, description: String) -> Self {
        Self {
            name,
            description,
            subagent: None,
            parameters: HashMap::new(),
            pre_hooks: Vec::new(),
            post_hooks: Vec::new(),
        }
    }

    pub fn with_subagent(mut self, subagent: String) -> Self {
        self.subagent = Some(subagent);
        self
    }

    pub fn with_parameter(mut self, key: String, value: String) -> Self {
        self.parameters.insert(key, value);
        self
    }

    pub fn with_pre_hook(mut self, command: String) -> Self {
        self.pre_hooks.push(command);
        self
    }

    pub fn with_post_hook(mut self, command: String) -> Self {
        self.post_hooks.push(command);
        self
    }
}

/// Custom command registry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomCommandRegistry {
    commands: HashMap<String, CustomCommand>,
}

impl CustomCommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
        }
    }

    /// Register a custom command
    pub fn register(&mut self, command: CustomCommand) {
        self.commands.insert(command.name.clone(), command);
    }

    /// Get a command by name
    pub fn get(&self, name: &str) -> Option<&CustomCommand> {
        self.commands.get(name)
    }

    /// Remove a command
    pub fn remove(&mut self, name: &str) -> Option<CustomCommand> {
        self.commands.remove(name)
    }

    /// List all command names
    pub fn list_names(&self) -> Vec<String> {
        self.commands.keys().cloned().collect()
    }

    /// Get all commands
    pub fn get_all(&self) -> Vec<&CustomCommand> {
        self.commands.values().collect()
    }

    /// Check if a command exists
    pub fn has_command(&self, name: &str) -> bool {
        self.commands.contains_key(name)
    }

    /// Register default commands
    pub fn register_defaults(&mut self) {
        // Code analysis command
        self.register(
            CustomCommand::new(
                "analyze_code".to_string(),
                "Analyze code for bugs and improvements".to_string(),
            )
            .with_subagent("CodeExpert".to_string())
            .with_parameter("depth".to_string(), "detailed".to_string()),
        );

        // Security review command
        self.register(
            CustomCommand::new(
                "security_review".to_string(),
                "Perform comprehensive security review".to_string(),
            )
            .with_subagent("SecurityExpert".to_string())
            .with_parameter("check_vulnerabilities".to_string(), "true".to_string()),
        );

        // Generate tests command
        self.register(
            CustomCommand::new(
                "generate_tests".to_string(),
                "Generate comprehensive test suite".to_string(),
            )
            .with_subagent("TestingExpert".to_string())
            .with_parameter("coverage_target".to_string(), "80".to_string()),
        );

        // Deep research command
        self.register(
            CustomCommand::new(
                "deep_research".to_string(),
                "Conduct deep research on a topic".to_string(),
            )
            .with_subagent("DeepResearcher".to_string())
            .with_parameter("depth".to_string(), "5".to_string())
            .with_parameter("sources".to_string(), "20".to_string()),
        );

        // Debug command
        self.register(
            CustomCommand::new(
                "debug_issue".to_string(),
                "Debug and fix issues".to_string(),
            )
            .with_subagent("DebugExpert".to_string())
            .with_parameter("verbose".to_string(), "true".to_string()),
        );

        // Performance optimization command
        self.register(
            CustomCommand::new(
                "optimize_performance".to_string(),
                "Optimize code for better performance".to_string(),
            )
            .with_subagent("PerformanceExpert".to_string())
            .with_parameter("profile".to_string(), "true".to_string()),
        );

        // Documentation command
        self.register(
            CustomCommand::new(
                "generate_docs".to_string(),
                "Generate comprehensive documentation".to_string(),
            )
            .with_subagent("DocsExpert".to_string())
            .with_parameter("format".to_string(), "markdown".to_string()),
        );
    }
}

impl Default for CustomCommandRegistry {
    fn default() -> Self {
        let mut registry = Self::new();
        registry.register_defaults();
        registry
    }
}

/// Custom command executor
#[derive(Debug)]
pub struct CustomCommandExecutor {
    registry: CustomCommandRegistry,
}

impl CustomCommandExecutor {
    pub fn new(registry: CustomCommandRegistry) -> Self {
        Self { registry }
    }

    /// Execute a custom command
    pub async fn execute(&self, command_name: &str, context: &str) -> Result<CustomCommandResult> {
        let command = self
            .registry
            .get(command_name)
            .ok_or_else(|| anyhow::anyhow!("Custom command not found: {}", command_name))?;

        // Build execution plan
        let mut result = CustomCommandResult {
            command_name: command_name.to_string(),
            success: true,
            subagent_used: command.subagent.clone(),
            output: String::new(),
            pre_hook_results: Vec::new(),
            post_hook_results: Vec::new(),
        };

        // Execute pre-hooks (if any)
        for pre_hook in &command.pre_hooks {
            result
                .output
                .push_str(&format!("[Pre-hook] Running: {}\n", pre_hook));
            // In actual implementation, execute the hook
            result
                .pre_hook_results
                .push(format!("Executed: {}", pre_hook));
        }

        // Execute main command (dispatch to subagent if specified)
        if let Some(subagent) = &command.subagent {
            result.output.push_str(&format!(
                "[CustomCommand] Dispatching to subagent: {}\n",
                subagent
            ));
            result.output.push_str(&format!("Context: {}\n", context));
            result
                .output
                .push_str(&format!("Parameters: {:?}\n", command.parameters));
        } else {
            result
                .output
                .push_str("[CustomCommand] No subagent specified, executing directly\n");
        }

        // Execute post-hooks (if any)
        for post_hook in &command.post_hooks {
            result
                .output
                .push_str(&format!("[Post-hook] Running: {}\n", post_hook));
            result
                .post_hook_results
                .push(format!("Executed: {}", post_hook));
        }

        Ok(result)
    }

    /// List available commands
    pub fn list_commands(&self) -> Vec<String> {
        self.registry.list_names()
    }

    /// Get command info
    pub fn get_command_info(&self, name: &str) -> Option<&CustomCommand> {
        self.registry.get(name)
    }
}

impl Default for CustomCommandExecutor {
    fn default() -> Self {
        Self::new(CustomCommandRegistry::default())
    }
}

/// Custom command execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomCommandResult {
    pub command_name: String,
    pub success: bool,
    pub subagent_used: Option<String>,
    pub output: String,
    pub pre_hook_results: Vec<String>,
    pub post_hook_results: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_custom_command() {
        let command = CustomCommand::new("test_cmd".to_string(), "Test command".to_string())
            .with_subagent("CodeExpert".to_string())
            .with_parameter("key1".to_string(), "value1".to_string())
            .with_pre_hook("echo pre".to_string())
            .with_post_hook("echo post".to_string());

        assert_eq!(command.name, "test_cmd");
        assert_eq!(command.subagent, Some("CodeExpert".to_string()));
        assert_eq!(command.parameters.get("key1"), Some(&"value1".to_string()));
        assert_eq!(command.pre_hooks.len(), 1);
        assert_eq!(command.post_hooks.len(), 1);
    }

    #[test]
    fn test_custom_command_registry() {
        let mut registry = CustomCommandRegistry::new();

        let cmd1 = CustomCommand::new("cmd1".to_string(), "Command 1".to_string());
        let cmd2 = CustomCommand::new("cmd2".to_string(), "Command 2".to_string());

        registry.register(cmd1);
        registry.register(cmd2);

        assert_eq!(registry.list_names().len(), 2);
        assert!(registry.has_command("cmd1"));
        assert!(!registry.has_command("cmd3"));

        let retrieved = registry.get("cmd1");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "cmd1");

        registry.remove("cmd1");
        assert_eq!(registry.list_names().len(), 1);
    }

    #[test]
    fn test_default_commands() {
        let registry = CustomCommandRegistry::default();

        assert!(registry.has_command("analyze_code"));
        assert!(registry.has_command("security_review"));
        assert!(registry.has_command("generate_tests"));
        assert!(registry.has_command("deep_research"));
        assert!(registry.has_command("debug_issue"));
        assert!(registry.has_command("optimize_performance"));
        assert!(registry.has_command("generate_docs"));

        assert_eq!(registry.list_names().len(), 7);
    }

    #[tokio::test]
    async fn test_custom_command_executor() {
        let executor = CustomCommandExecutor::default();

        let result = executor
            .execute("analyze_code", "fn main() {}")
            .await
            .unwrap();

        assert_eq!(result.command_name, "analyze_code");
        assert!(result.success);
        assert_eq!(result.subagent_used, Some("CodeExpert".to_string()));
    }

    #[test]
    fn test_list_commands() {
        let executor = CustomCommandExecutor::default();
        let commands = executor.list_commands();

        assert!(commands.len() >= 7);
        assert!(commands.contains(&"analyze_code".to_string()));
        assert!(commands.contains(&"security_review".to_string()));
    }

    #[test]
    fn test_get_command_info() {
        let executor = CustomCommandExecutor::default();
        let info = executor.get_command_info("analyze_code");

        assert!(info.is_some());
        let cmd = info.unwrap();
        assert_eq!(cmd.name, "analyze_code");
        assert_eq!(cmd.subagent, Some("CodeExpert".to_string()));
    }
}
