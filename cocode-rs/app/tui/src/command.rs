//! Commands sent from TUI to core agent.
//!
//! These commands represent user actions that need to be communicated
//! to the core agent loop for processing.

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

    /// Request graceful shutdown.
    Shutdown,
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
}
