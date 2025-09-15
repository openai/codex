// Tests for slash_command.rs
use super::*;
use strum::IntoEnumIterator;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_command_exists() {
        // Verify that Context command exists in the enum
        let commands: Vec<SlashCommand> = SlashCommand::iter().collect();
        assert!(
            commands.contains(&SlashCommand::Context),
            "Context command should exist in SlashCommand enum"
        );
    }

    #[test]
    fn test_context_command_string_representation() {
        // Test that the Context command can be parsed from string
        let cmd: SlashCommand = "context".parse().expect("Should parse 'context' string");
        assert_eq!(cmd, SlashCommand::Context);
        
        // Test kebab-case serialization
        assert_eq!(cmd.as_ref(), "context");
        assert_eq!(cmd.command(), "context");
    }

    #[test]
    fn test_context_command_description() {
        // Verify the description is correct
        assert_eq!(
            SlashCommand::Context.description(),
            "show detailed context window usage"
        );
    }

    #[test]
    fn test_context_command_available_during_task() {
        // Context command should be available during task execution
        assert!(
            SlashCommand::Context.available_during_task(),
            "Context command should be available during task"
        );
    }

    #[test]
    fn test_all_commands_have_descriptions() {
        // Ensure all commands have non-empty descriptions
        for cmd in SlashCommand::iter() {
            let desc = cmd.description();
            assert!(!desc.is_empty(), "Command {:?} has empty description", cmd);
            assert!(
                desc.len() > 5,
                "Command {:?} has suspiciously short description: '{}'",
                cmd,
                desc
            );
        }
    }

    #[test]
    fn test_built_in_slash_commands_includes_context() {
        // Verify Context is included in built-in commands list
        let commands = built_in_slash_commands();
        let context_cmd = commands
            .iter()
            .find(|(name, cmd)| *cmd == SlashCommand::Context);
        
        assert!(
            context_cmd.is_some(),
            "Context command should be in built-in commands list"
        );
        
        if let Some((name, _)) = context_cmd {
            assert_eq!(*name, "context");
        }
    }

    #[test]
    fn test_command_ordering_preference() {
        // Verify that frequently used commands come before Context
        let commands: Vec<SlashCommand> = SlashCommand::iter().collect();
        let context_idx = commands
            .iter()
            .position(|&c| c == SlashCommand::Context)
            .expect("Context command should exist");
        
        // These commands should appear before Context
        let high_priority = vec![
            SlashCommand::Model,
            SlashCommand::Approvals,
            SlashCommand::New,
            SlashCommand::Status,
        ];
        
        for priority_cmd in high_priority {
            let priority_idx = commands
                .iter()
                .position(|&c| c == priority_cmd)
                .expect(&format!("{:?} should exist", priority_cmd));
            
            assert!(
                priority_idx < context_idx,
                "{:?} (idx {}) should come before Context (idx {})",
                priority_cmd,
                priority_idx,
                context_idx
            );
        }
    }

    #[test]
    fn test_parsing_with_slash_prefix() {
        // While the enum parses without slash, ensure we handle the command string correctly
        let without_slash: Result<SlashCommand, _> = "context".parse();
        assert!(without_slash.is_ok());
        assert_eq!(without_slash.unwrap(), SlashCommand::Context);
        
        // The slash prefix should be handled by the calling code, not the enum parser
        let with_slash: Result<SlashCommand, _> = "/context".parse();
        assert!(with_slash.is_err(), "Enum parser should not accept slash prefix");
    }

    #[test]
    fn test_case_insensitive_parsing() {
        // Strum should handle case-insensitive parsing with the kebab-case serialization
        let lowercase: Result<SlashCommand, _> = "context".parse();
        assert!(lowercase.is_ok());
        
        // Note: Strum's default parsing is case-sensitive unless configured otherwise
        // This test documents the actual behavior
        let uppercase: Result<SlashCommand, _> = "CONTEXT".parse();
        assert!(uppercase.is_err(), "Default Strum parsing is case-sensitive");
        
        let mixed: Result<SlashCommand, _> = "Context".parse();
        assert!(mixed.is_err(), "Default Strum parsing is case-sensitive");
    }

    #[test]
    fn test_unavailable_commands_during_task() {
        // Verify which commands are NOT available during task
        let unavailable = vec![
            SlashCommand::New,
            SlashCommand::Init,
            SlashCommand::Compact,
            SlashCommand::Model,
            SlashCommand::Approvals,
            SlashCommand::Logout,
        ];
        
        for cmd in unavailable {
            assert!(
                !cmd.available_during_task(),
                "{:?} should NOT be available during task",
                cmd
            );
        }
    }

    #[test]
    fn test_available_commands_during_task() {
        // Verify which commands ARE available during task
        let available = vec![
            SlashCommand::Diff,
            SlashCommand::Mention,
            SlashCommand::Status,
            SlashCommand::Context,  // Our new command
            SlashCommand::Mcp,
            SlashCommand::Quit,
        ];
        
        for cmd in available {
            assert!(
                cmd.available_during_task(),
                "{:?} should be available during task",
                cmd
            );
        }
    }

    #[test]
    fn test_context_command_similar_to_status() {
        // Context and Status commands should have similar properties
        // Both show information and are available during tasks
        assert_eq!(
            SlashCommand::Context.available_during_task(),
            SlashCommand::Status.available_during_task(),
            "Context and Status should have same availability"
        );
        
        // Both should be info commands (not action commands)
        let info_commands = vec![SlashCommand::Status, SlashCommand::Context, SlashCommand::Mcp];
        for cmd in info_commands {
            assert!(
                cmd.available_during_task(),
                "Info command {:?} should be available during task",
                cmd
            );
        }
    }

    #[test] 
    fn test_command_enum_derives() {
        // Test that required derives are present and working
        
        // Test Debug
        let cmd = SlashCommand::Context;
        let debug_str = format!("{:?}", cmd);
        assert_eq!(debug_str, "Context");
        
        // Test Clone
        let cmd2 = cmd.clone();
        assert_eq!(cmd, cmd2);
        
        // Test Copy (implicitly tested by using cmd after clone)
        let _cmd3 = cmd;  // This would fail if Copy wasn't implemented
        assert_eq!(cmd, SlashCommand::Context);  // Can still use cmd
        
        // Test PartialEq and Eq
        assert_eq!(SlashCommand::Context, SlashCommand::Context);
        assert_ne!(SlashCommand::Context, SlashCommand::Status);
        
        // Test Hash
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(SlashCommand::Context);
        assert!(set.contains(&SlashCommand::Context));
    }
}