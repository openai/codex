use serde::{Deserialize, Serialize};
use url::Url;

use crate::OneOf;

#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFoldersServerCapabilities {
    /// The server has support for workspace folders
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supported: Option<bool>,

    /// Whether the server wants to receive workspace folder
    /// change notifications.
    ///
    /// If a string is provided, the string is treated as an ID
    /// under which the notification is registered on the client
    /// side. The ID can be used to unregister for these events
    /// using the `client/unregisterCapability` request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_notifications: Option<OneOf<bool, String>>,
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFolder {
    /// The associated URI for this workspace folder.
    pub uri: Url,
    /// The name of the workspace folder. Defaults to the uri's basename.
    pub name: String,
}

#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DidChangeWorkspaceFoldersParams {
    /// The actual workspace folder change event.
    pub event: WorkspaceFoldersChangeEvent,
}

/// The workspace folder change event.
#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFoldersChangeEvent {
    /// The array of added workspace folders
    pub added: Vec<WorkspaceFolder>,

    /// The array of the removed workspace folders
    pub removed: Vec<WorkspaceFolder>,
}
