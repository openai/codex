use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::PoisonError;

use codex_tools::ToolName;

/// Turn-scoped availability policy for tools.
///
/// Extensions can attach this to turn-scoped [`ExtensionData`](crate::ExtensionData)
/// when they own context that makes one or more tools inappropriate for the
/// current turn. Core tool planning can omit unavailable tools, while handlers
/// can use the same policy for defensive rejection if an unavailable tool is
/// invoked anyway.
#[derive(Debug, Default)]
pub struct ToolAvailability {
    unavailable: Mutex<HashMap<ToolName, ToolUnavailable>>,
}

impl ToolAvailability {
    /// Marks a tool unavailable for the current turn.
    pub fn mark_unavailable(
        &self,
        tool_name: ToolName,
        unavailable: ToolUnavailable,
    ) -> Option<ToolUnavailable> {
        self.unavailable().insert(tool_name, unavailable)
    }

    /// Returns the unavailable reason for `tool_name`, if one exists.
    pub fn unavailable_reason(&self, tool_name: &ToolName) -> Option<ToolUnavailable> {
        self.unavailable().get(tool_name).cloned()
    }

    /// Returns true when `tool_name` is marked unavailable for the current turn.
    pub fn is_unavailable(&self, tool_name: &ToolName) -> bool {
        self.unavailable().contains_key(tool_name)
    }

    /// Returns true when `tool_name` has not been marked unavailable.
    pub fn is_available(&self, tool_name: &ToolName) -> bool {
        !self.is_unavailable(tool_name)
    }

    fn unavailable(&self) -> std::sync::MutexGuard<'_, HashMap<ToolName, ToolUnavailable>> {
        self.unavailable
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

/// Explains why a tool is unavailable for a turn.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolUnavailable {
    model_message: String,
}

impl ToolUnavailable {
    /// Creates an unavailable reason whose message is returned to the model if
    /// the unavailable tool is invoked anyway.
    pub fn model_message(message: impl Into<String>) -> Self {
        Self {
            model_message: message.into(),
        }
    }

    /// Returns the model-facing unavailable message.
    pub fn model_message_text(&self) -> &str {
        &self.model_message
    }

    /// Consumes this reason and returns the model-facing unavailable message.
    pub fn into_model_message(self) -> String {
        self.model_message
    }
}
