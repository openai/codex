//! Service-computed permissions and denials. Codex exposes these without evaluating ACLs.

use codex_protocol::error::CodexErr;
use codex_tools::FunctionCallError;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;
use std::fmt;

/// Effective board actions for the authenticated caller, not an ACL definition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardPermissions {
    pub create_channel: bool,
    pub edit_metadata: bool,
    pub manage_permissions: bool,
}

/// Effective channel actions; replying does not imply permission to start a thread.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelPermissions {
    pub read: bool,
    pub post: bool,
    pub reply: bool,
    pub edit_metadata: bool,
}

/// A service denial retained through the board's existing error interface.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "code", rename = "permission_denied")]
pub struct PermissionDenied {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<String>,
}

impl fmt::Display for PermissionDenied {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "permission_denied: {}", self.message)
    }
}

impl std::error::Error for PermissionDenied {}

impl From<PermissionDenied> for CodexErr {
    fn from(error: PermissionDenied) -> Self {
        Self::Io(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            error,
        ))
    }
}

impl PermissionDenied {
    pub(crate) fn model_response(&self, budget: usize) -> Result<Value, FunctionCallError> {
        // Keep the denial structured and below 1k tokens, even for oversized service text.
        let mut limit = 512;
        loop {
            let truncated = std::iter::once(self.message.as_str())
                .chain(self.action.as_deref())
                .chain(self.resource.as_deref())
                .any(|text| text.chars().count() > limit);
            let mut result = json!({"error": Self {
                message: self.message.chars().take(limit).collect(),
                action: self.action.as_ref().map(|text| text.chars().take(limit).collect()),
                resource: self.resource.as_ref().map(|text| text.chars().take(limit).collect()),
            }});
            if truncated {
                result["error"]["truncated"] = json!(true);
            }
            if result.to_string().len() <= budget.min(/*other*/ 768) {
                return Ok(result);
            }
            if limit == 0 {
                return Err(FunctionCallError::RespondToModel(
                    "permission_denied: output budget too small for the error".into(),
                ));
            }
            limit /= 2;
        }
    }
}
