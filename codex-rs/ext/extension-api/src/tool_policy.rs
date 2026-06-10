/// Per-turn policy that suppresses the request_user_input tool.
///
/// Extensions insert this marker into turn-scoped [`ExtensionData`](crate::ExtensionData)
/// when they own context that makes blocking user input inappropriate for the
/// current turn.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestUserInputSuppression {
    /// Suppress request_user_input while Default mode is working on an active goal.
    ActiveDefaultModeGoal,
}

impl RequestUserInputSuppression {
    /// Returns the model-facing message to use if the suppressed tool is invoked.
    pub fn unavailable_message(self) -> &'static str {
        match self {
            Self::ActiveDefaultModeGoal => {
                "request_user_input is unavailable while the current Default mode turn is working on an active goal"
            }
        }
    }
}
