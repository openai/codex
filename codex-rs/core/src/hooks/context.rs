//! Hook execution context and data management.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use tokio::fs;

use crate::hooks::types::{HookError, LifecycleEvent};

/// Context provided to hooks during execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookContext {
    /// The lifecycle event that triggered this hook.
    pub event: LifecycleEvent,
    /// Environment variables to be passed to the hook.
    pub environment: HashMap<String, String>,
    /// Temporary files created for hook data.
    pub temp_files: HashMap<String, PathBuf>,
    /// Additional metadata for the hook execution.
    pub metadata: HashMap<String, serde_json::Value>,
    /// Working directory for hook execution.
    pub working_directory: PathBuf,
    /// Hook execution timestamp.
    pub execution_timestamp: SystemTime,
}

impl HookContext {
    /// Create a new hook context for the given event.
    pub fn new(event: LifecycleEvent, working_directory: PathBuf) -> Self {
        Self {
            event,
            environment: HashMap::new(),
            temp_files: HashMap::new(),
            metadata: HashMap::new(),
            working_directory,
            execution_timestamp: SystemTime::now(),
        }
    }

    /// Add an environment variable to the context.
    pub fn with_env(mut self, key: String, value: String) -> Self {
        self.environment.insert(key, value);
        self
    }

    /// Add multiple environment variables to the context.
    pub fn with_env_vars(mut self, vars: HashMap<String, String>) -> Self {
        self.environment.extend(vars);
        self
    }

    /// Add metadata to the context.
    pub fn with_metadata(mut self, key: String, value: serde_json::Value) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Get an environment variable from the context.
    pub fn get_env(&self, key: &str) -> Option<&String> {
        self.environment.get(key)
    }

    /// Get metadata from the context.
    pub fn get_metadata(&self, key: &str) -> Option<&serde_json::Value> {
        self.metadata.get(key)
    }

    /// Create a temporary file with the given content and register it in the context.
    pub async fn create_temp_file(
        &mut self,
        name: &str,
        content: &str,
    ) -> Result<PathBuf, HookError> {
        let temp_file = NamedTempFile::new()
            .map_err(|e| HookError::Context(format!("Failed to create temp file: {}", e)))?;
        
        let temp_path = temp_file.path().to_path_buf();
        
        // Write content to the temporary file
        fs::write(&temp_path, content)
            .await
            .map_err(|e| HookError::Context(format!("Failed to write temp file: {}", e)))?;
        
        // Keep the temp file alive by storing it
        self.temp_files.insert(name.to_string(), temp_path.clone());
        
        // Prevent the temp file from being deleted when NamedTempFile is dropped
        let _ = temp_file.into_temp_path().keep()
            .map_err(|e| HookError::Context(format!("Failed to persist temp file: {}", e)))?;
        
        Ok(temp_path)
    }

    /// Get the path to a temporary file by name.
    pub fn get_temp_file(&self, name: &str) -> Option<&PathBuf> {
        self.temp_files.get(name)
    }

    /// Serialize the event data to JSON.
    pub fn event_as_json(&self) -> Result<String, HookError> {
        serde_json::to_string_pretty(&self.event)
            .map_err(|e| HookError::Context(format!("Failed to serialize event: {}", e)))
    }

    /// Create a JSON file with the event data.
    pub async fn create_event_json_file(&mut self) -> Result<PathBuf, HookError> {
        let json_content = self.event_as_json()?;
        self.create_temp_file("event.json", &json_content).await
    }

    /// Get all environment variables as a HashMap suitable for process execution.
    pub fn get_all_env_vars(&self) -> HashMap<String, String> {
        let mut env_vars = self.environment.clone();
        
        // Add standard Codex environment variables
        env_vars.insert("CODEX_EVENT_TYPE".to_string(), format!("{:?}", self.event.event_type()));
        env_vars.insert("CODEX_TIMESTAMP".to_string(), 
                       self.execution_timestamp.duration_since(SystemTime::UNIX_EPOCH)
                           .unwrap_or_default().as_secs().to_string());
        
        // Add task ID if available
        if let Some(task_id) = self.event.task_id() {
            env_vars.insert("CODEX_TASK_ID".to_string(), task_id.to_string());
        }
        
        // Add event-specific environment variables
        match &self.event {
            LifecycleEvent::SessionStart { session_id, model, .. } => {
                env_vars.insert("CODEX_SESSION_ID".to_string(), session_id.clone());
                env_vars.insert("CODEX_MODEL".to_string(), model.clone());
            }
            LifecycleEvent::SessionEnd { session_id, duration, .. } => {
                env_vars.insert("CODEX_SESSION_ID".to_string(), session_id.clone());
                env_vars.insert("CODEX_DURATION_SECS".to_string(), duration.as_secs().to_string());
            }
            LifecycleEvent::TaskStart { prompt, .. } => {
                env_vars.insert("CODEX_PROMPT".to_string(), prompt.clone());
            }
            LifecycleEvent::TaskComplete { success, .. } => {
                env_vars.insert("CODEX_SUCCESS".to_string(), success.to_string());
            }
            LifecycleEvent::ExecBefore { command, .. } => {
                env_vars.insert("CODEX_COMMAND".to_string(), command.join(" "));
            }
            LifecycleEvent::ExecAfter { command, exit_code, .. } => {
                env_vars.insert("CODEX_COMMAND".to_string(), command.join(" "));
                env_vars.insert("CODEX_EXIT_CODE".to_string(), exit_code.to_string());
            }
            LifecycleEvent::McpToolBefore { server, tool, .. } => {
                env_vars.insert("CODEX_MCP_SERVER".to_string(), server.clone());
                env_vars.insert("CODEX_MCP_TOOL".to_string(), tool.clone());
            }
            LifecycleEvent::McpToolAfter { server, tool, success, .. } => {
                env_vars.insert("CODEX_MCP_SERVER".to_string(), server.clone());
                env_vars.insert("CODEX_MCP_TOOL".to_string(), tool.clone());
                env_vars.insert("CODEX_SUCCESS".to_string(), success.to_string());
            }
            _ => {}
        }
        
        env_vars
    }
}

/// Builder for creating hook execution contexts with fluent API.
pub struct HookExecutionContext {
    event: LifecycleEvent,
    working_directory: PathBuf,
    environment: HashMap<String, String>,
    metadata: HashMap<String, serde_json::Value>,
}

impl HookExecutionContext {
    /// Create a new hook execution context builder.
    pub fn new(event: LifecycleEvent, working_directory: PathBuf) -> Self {
        Self {
            event,
            working_directory,
            environment: HashMap::new(),
            metadata: HashMap::new(),
        }
    }

    /// Add an environment variable.
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.environment.insert(key.into(), value.into());
        self
    }

    /// Add multiple environment variables.
    pub fn env_vars(mut self, vars: HashMap<String, String>) -> Self {
        self.environment.extend(vars);
        self
    }

    /// Add metadata.
    pub fn metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Build the hook context.
    pub fn build(self) -> HookContext {
        HookContext::new(self.event, self.working_directory)
            .with_env_vars(self.environment)
            .with_metadata("builder_metadata".to_string(), serde_json::json!(self.metadata))
    }
}

/// Template variable substitution for hook configurations.
pub struct TemplateSubstitution {
    variables: HashMap<String, String>,
}

impl TemplateSubstitution {
    /// Create a new template substitution from a hook context.
    pub fn from_context(context: &HookContext) -> Self {
        let mut variables = HashMap::new();
        
        // Add environment variables
        for (key, value) in &context.environment {
            variables.insert(format!("env.{}", key), value.clone());
        }
        
        // Add event-specific variables
        match &context.event {
            LifecycleEvent::SessionStart { session_id, model, .. } => {
                variables.insert("session_id".to_string(), session_id.clone());
                variables.insert("model".to_string(), model.clone());
            }
            LifecycleEvent::TaskStart { task_id, prompt, .. } => {
                variables.insert("task_id".to_string(), task_id.clone());
                variables.insert("prompt".to_string(), prompt.clone());
            }
            LifecycleEvent::ExecBefore { call_id, command, .. } => {
                variables.insert("call_id".to_string(), call_id.clone());
                variables.insert("command".to_string(), command.join(" "));
            }
            LifecycleEvent::ExecAfter { call_id, command, exit_code, .. } => {
                variables.insert("call_id".to_string(), call_id.clone());
                variables.insert("command".to_string(), command.join(" "));
                variables.insert("exit_code".to_string(), exit_code.to_string());
            }
            _ => {}
        }
        
        // Add temp file paths
        for (name, path) in &context.temp_files {
            variables.insert(format!("temp.{}", name), path.to_string_lossy().to_string());
        }
        
        Self { variables }
    }

    /// Substitute template variables in a string.
    pub fn substitute(&self, template: &str) -> String {
        let mut result = template.to_string();
        
        for (key, value) in &self.variables {
            let placeholder = format!("{{{}}}", key);
            result = result.replace(&placeholder, value);
        }
        
        result
    }

    /// Substitute template variables in a vector of strings.
    pub fn substitute_vec(&self, templates: &[String]) -> Vec<String> {
        templates.iter().map(|t| self.substitute(t)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_hook_context_creation() {
        let event = LifecycleEvent::TaskStart {
            task_id: "test".to_string(),
            session_id: "session".to_string(),
            prompt: "test prompt".to_string(),
            timestamp: Utc::now(),
        };
        
        let context = HookContext::new(event, PathBuf::from("/tmp"))
            .with_env("TEST_VAR".to_string(), "test_value".to_string());
        
        assert_eq!(context.get_env("TEST_VAR"), Some(&"test_value".to_string()));
        assert_eq!(context.working_directory, PathBuf::from("/tmp"));
    }

    #[test]
    fn test_hook_execution_context_builder() {
        let event = LifecycleEvent::TaskStart {
            task_id: "test".to_string(),
            session_id: "session".to_string(),
            prompt: "test prompt".to_string(),
            timestamp: Utc::now(),
        };
        
        let context = HookExecutionContext::new(event, PathBuf::from("/tmp"))
            .env("TEST_VAR", "test_value")
            .metadata("test_key", serde_json::json!("test_value"))
            .build();
        
        assert_eq!(context.get_env("TEST_VAR"), Some(&"test_value".to_string()));
    }

    #[test]
    fn test_template_substitution() {
        let event = LifecycleEvent::TaskStart {
            task_id: "test_task".to_string(),
            session_id: "session".to_string(),
            prompt: "test prompt".to_string(),
            timestamp: Utc::now(),
        };
        
        let context = HookContext::new(event, PathBuf::from("/tmp"));
        let substitution = TemplateSubstitution::from_context(&context);
        
        let template = "Task {task_id} started";
        let result = substitution.substitute(template);
        assert_eq!(result, "Task test_task started");
    }
}
