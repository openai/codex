//! Plugin command types.
//!
//! Commands are plugin-provided actions that can be invoked via slash commands
//! or other interfaces. They support shell commands, skill invocations, and
//! agent spawning.

use serde::Deserialize;
use serde::Serialize;

/// Default function for visible field.
fn default_true() -> bool {
    true
}

/// A command contributed by a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCommand {
    /// Command name (used as the slash command).
    pub name: String,

    /// Human-readable description.
    pub description: String,

    /// How the command is executed.
    pub handler: CommandHandler,

    /// Whether the command is visible in help/completions.
    #[serde(default = "default_true")]
    pub visible: bool,
}

/// Handler type for plugin commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommandHandler {
    /// Execute a shell command.
    Shell {
        /// The command to execute.
        command: String,
        /// Optional timeout in seconds.
        #[serde(default)]
        timeout_sec: Option<i32>,
    },

    /// Invoke a skill.
    Skill {
        /// Name of the skill to invoke.
        skill_name: String,
    },

    /// Spawn an agent.
    Agent {
        /// Agent type to spawn.
        agent_type: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_shell() {
        let cmd = PluginCommand {
            name: "build".to_string(),
            description: "Build the project".to_string(),
            handler: CommandHandler::Shell {
                command: "cargo build".to_string(),
                timeout_sec: Some(300),
            },
            visible: true,
        };

        assert_eq!(cmd.name, "build");
        assert!(cmd.visible);

        if let CommandHandler::Shell {
            command,
            timeout_sec,
        } = &cmd.handler
        {
            assert_eq!(command, "cargo build");
            assert_eq!(*timeout_sec, Some(300));
        } else {
            panic!("Expected Shell handler");
        }
    }

    #[test]
    fn test_command_skill() {
        let cmd = PluginCommand {
            name: "review".to_string(),
            description: "Review code".to_string(),
            handler: CommandHandler::Skill {
                skill_name: "code-review".to_string(),
            },
            visible: true,
        };

        if let CommandHandler::Skill { skill_name } = &cmd.handler {
            assert_eq!(skill_name, "code-review");
        } else {
            panic!("Expected Skill handler");
        }
    }

    #[test]
    fn test_command_agent() {
        let cmd = PluginCommand {
            name: "explore".to_string(),
            description: "Explore codebase".to_string(),
            handler: CommandHandler::Agent {
                agent_type: "explore".to_string(),
            },
            visible: true,
        };

        if let CommandHandler::Agent { agent_type } = &cmd.handler {
            assert_eq!(agent_type, "explore");
        } else {
            panic!("Expected Agent handler");
        }
    }

    #[test]
    fn test_command_serialize_deserialize() {
        let cmd = PluginCommand {
            name: "test".to_string(),
            description: "Run tests".to_string(),
            handler: CommandHandler::Shell {
                command: "cargo test".to_string(),
                timeout_sec: None,
            },
            visible: true,
        };

        let toml_str = toml::to_string(&cmd).expect("serialize");
        let back: PluginCommand = toml::from_str(&toml_str).expect("deserialize");
        assert_eq!(back.name, "test");
        assert!(back.visible);
    }

    #[test]
    fn test_command_default_visible() {
        let toml_str = r#"
name = "hidden"
description = "A command"
visible = false

[handler]
type = "shell"
command = "echo hi"
"#;
        let cmd: PluginCommand = toml::from_str(toml_str).expect("deserialize");
        assert!(!cmd.visible);

        // Without explicit visible, should default to true
        let toml_str2 = r#"
name = "visible"
description = "Another command"

[handler]
type = "shell"
command = "echo hi"
"#;
        let cmd2: PluginCommand = toml::from_str(toml_str2).expect("deserialize");
        assert!(cmd2.visible);
    }
}
