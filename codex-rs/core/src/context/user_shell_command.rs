use std::time::Duration;

use super::ContextualUserFragment;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct UserShellCommand {
    pub(crate) command: String,
    pub(crate) exit_code: i32,
    pub(crate) duration_seconds: f64,
    pub(crate) output: String,
}

impl UserShellCommand {
    pub(crate) fn new(
        command: impl Into<String>,
        exit_code: i32,
        duration: Duration,
        output: impl Into<String>,
    ) -> Self {
        Self {
            command: command.into(),
            exit_code,
            duration_seconds: duration.as_secs_f64(),
            output: output.into(),
        }
    }
}

impl ContextualUserFragment for UserShellCommand {
    const ROLE: &'static str = "user";
    const START_MARKER: &'static str = "<user_shell_command>";
    const END_MARKER: &'static str = "</user_shell_command>";

    fn body(&self) -> String {
        format!(
            "\n<command>\n{}\n</command>\n<result>\nExit code: {}\nDuration: {:.4} seconds\nOutput:\n{}\n</result>",
            self.command, self.exit_code, self.duration_seconds, self.output,
        )
    }
}
