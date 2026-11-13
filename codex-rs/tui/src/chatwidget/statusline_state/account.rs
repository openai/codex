//! Projects the account values displayed by the built-in status line.

use super::ChatWidget;
use serde::Serialize;

#[derive(Serialize)]
pub(super) struct Account {
    #[serde(skip_serializing_if = "Option::is_none")]
    workspace: Option<Workspace>,
}

#[derive(Serialize)]
struct Workspace {
    headline: String,
}

impl From<&ChatWidget> for Account {
    fn from(chat: &ChatWidget) -> Self {
        Self {
            workspace: chat
                .status_line_workspace_headline
                .clone()
                .map(|headline| Workspace { headline }),
        }
    }
}
