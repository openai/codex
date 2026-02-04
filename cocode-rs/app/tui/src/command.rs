//! Commands sent from TUI to core agent.
//!
//! These commands represent user actions that need to be communicated
//! to the core agent loop for processing.

use cocode_protocol::SubmissionId;
use cocode_protocol::ThinkingLevel;

/// Commands sent from the TUI to the core agent.
///
/// These commands allow the TUI to communicate user intentions
/// to the core agent loop, which will process them accordingly.
#[derive(Debug, Clone)]
pub enum UserCommand {
    /// Submit user input to the agent.
    SubmitInput {
        /// The message to send to the agent.
        message: String,
    },

    /// Interrupt the current operation.
    ///
    /// This is typically triggered by Ctrl+C.
    Interrupt,

    /// Set plan mode state.
    SetPlanMode {
        /// Whether plan mode should be active.
        active: bool,
    },

    /// Set the thinking level.
    SetThinkingLevel {
        /// The new thinking level.
        level: ThinkingLevel,
    },

    /// Set the model to use.
    SetModel {
        /// The model identifier.
        model: String,
    },

    /// Respond to a permission/approval request.
    ApprovalResponse {
        /// The request ID being responded to.
        request_id: String,
        /// Whether the user approved the request.
        approved: bool,
        /// Whether to remember this decision for similar operations.
        remember: bool,
    },

    /// Execute a skill command.
    ExecuteSkill {
        /// The skill name (e.g., "commit").
        name: String,
        /// Arguments passed to the skill.
        args: String,
    },

    /// Queue a command for later processing (Enter during streaming).
    ///
    /// The command will be processed as a new user turn after the
    /// current turn completes. Also serves as real-time steering:
    /// queued commands are injected into the current turn as
    /// <system-reminder>User sent: {message}</system-reminder>
    QueueCommand {
        /// The prompt to queue.
        prompt: String,
    },

    /// Clear all queued commands.
    ClearQueues,

    /// Request graceful shutdown.
    Shutdown,
}

impl UserCommand {
    /// Create a submission with a correlation ID.
    ///
    /// Returns a tuple of (SubmissionId, UserCommand) where the SubmissionId
    /// can be used to correlate events back to this command.
    ///
    /// # Example
    ///
    /// ```
    /// use cocode_tui::UserCommand;
    ///
    /// let cmd = UserCommand::SubmitInput { message: "Hello".to_string() };
    /// let (id, cmd) = cmd.with_correlation_id();
    /// // `id` can now be used to track events related to this command
    /// ```
    pub fn with_correlation_id(self) -> (SubmissionId, Self) {
        (SubmissionId::new(), self)
    }

    /// Check if this command triggers a turn (requires correlation tracking).
    ///
    /// Commands that trigger turns should have their events correlated.
    pub fn triggers_turn(&self) -> bool {
        matches!(
            self,
            UserCommand::SubmitInput { .. }
                | UserCommand::ExecuteSkill { .. }
                | UserCommand::QueueCommand { .. }
        )
    }
}

impl std::fmt::Display for UserCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UserCommand::SubmitInput { message } => {
                let preview = if message.len() > 20 {
                    format!("{}...", &message[..20])
                } else {
                    message.clone()
                };
                write!(f, "SubmitInput({preview:?})")
            }
            UserCommand::Interrupt => write!(f, "Interrupt"),
            UserCommand::SetPlanMode { active } => write!(f, "SetPlanMode({active})"),
            UserCommand::SetThinkingLevel { level } => {
                write!(f, "SetThinkingLevel({:?})", level.effort)
            }
            UserCommand::SetModel { model } => write!(f, "SetModel({model})"),
            UserCommand::ApprovalResponse {
                request_id,
                approved,
                remember,
            } => {
                write!(
                    f,
                    "ApprovalResponse({request_id}, approved={approved}, remember={remember})"
                )
            }
            UserCommand::ExecuteSkill { name, args } => {
                if args.is_empty() {
                    write!(f, "ExecuteSkill({name})")
                } else {
                    write!(f, "ExecuteSkill({name}, args={args})")
                }
            }
            UserCommand::QueueCommand { prompt } => {
                let preview = if prompt.len() > 20 {
                    format!("{}...", &prompt[..20])
                } else {
                    prompt.clone()
                };
                write!(f, "QueueCommand({preview:?})")
            }
            UserCommand::ClearQueues => write!(f, "ClearQueues"),
            UserCommand::Shutdown => write!(f, "Shutdown"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cocode_protocol::ReasoningEffort;

    #[test]
    fn test_user_command_display() {
        let cmd = UserCommand::SubmitInput {
            message: "Hello, world!".to_string(),
        };
        assert!(cmd.to_string().contains("SubmitInput"));

        let cmd = UserCommand::Interrupt;
        assert_eq!(cmd.to_string(), "Interrupt");

        let cmd = UserCommand::SetPlanMode { active: true };
        assert!(cmd.to_string().contains("true"));

        let cmd = UserCommand::SetThinkingLevel {
            level: ThinkingLevel::new(ReasoningEffort::High),
        };
        assert!(cmd.to_string().contains("High"));

        let cmd = UserCommand::SetModel {
            model: "claude-sonnet-4".to_string(),
        };
        assert!(cmd.to_string().contains("claude-sonnet-4"));

        let cmd = UserCommand::ApprovalResponse {
            request_id: "req-1".to_string(),
            approved: true,
            remember: false,
        };
        assert!(cmd.to_string().contains("approved=true"));

        let cmd = UserCommand::Shutdown;
        assert_eq!(cmd.to_string(), "Shutdown");
    }

    #[test]
    fn test_long_message_truncation() {
        let long_message = "This is a very long message that should be truncated in display";
        let cmd = UserCommand::SubmitInput {
            message: long_message.to_string(),
        };
        let display = cmd.to_string();
        assert!(display.contains("..."));
        assert!(display.len() < long_message.len() + 30);
    }

    #[test]
    fn test_with_correlation_id() {
        let cmd = UserCommand::SubmitInput {
            message: "Hello".to_string(),
        };
        let (id1, cmd1) = cmd.with_correlation_id();

        // ID should be a valid UUID (36 chars with hyphens)
        assert_eq!(id1.as_str().len(), 36);

        // Command should be preserved
        if let UserCommand::SubmitInput { message } = cmd1 {
            assert_eq!(message, "Hello");
        } else {
            panic!("Expected SubmitInput command");
        }

        // Each call should generate unique IDs
        let cmd = UserCommand::Interrupt;
        let (id2, _) = cmd.with_correlation_id();
        assert_ne!(id1.as_str(), id2.as_str());
    }

    #[test]
    fn test_triggers_turn() {
        // Commands that trigger turns
        assert!(
            UserCommand::SubmitInput {
                message: "test".to_string()
            }
            .triggers_turn()
        );
        assert!(
            UserCommand::ExecuteSkill {
                name: "commit".to_string(),
                args: String::new()
            }
            .triggers_turn()
        );
        assert!(
            UserCommand::QueueCommand {
                prompt: "test".to_string()
            }
            .triggers_turn()
        );

        // Commands that don't trigger turns
        assert!(!UserCommand::Interrupt.triggers_turn());
        assert!(!UserCommand::SetPlanMode { active: true }.triggers_turn());
        assert!(
            !UserCommand::SetModel {
                model: "test".to_string()
            }
            .triggers_turn()
        );
        assert!(!UserCommand::Shutdown.triggers_turn());
        assert!(!UserCommand::ClearQueues.triggers_turn());
    }
}
