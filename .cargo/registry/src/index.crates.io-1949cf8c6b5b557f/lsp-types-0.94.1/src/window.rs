use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use serde_json::Value;

use url::Url;

use crate::Range;

#[derive(Eq, PartialEq, Clone, Copy, Deserialize, Serialize)]
#[serde(transparent)]
pub struct MessageType(i32);
lsp_enum! {
impl MessageType {
    /// An error message.
    pub const ERROR: MessageType = MessageType(1);
    /// A warning message.
    pub const WARNING: MessageType = MessageType(2);
    /// An information message;
    pub const INFO: MessageType = MessageType(3);
    /// A log message.
    pub const LOG: MessageType = MessageType(4);
}
}

/// Window specific client capabilities.
#[derive(Debug, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowClientCapabilities {
    /// Whether client supports handling progress notifications. If set
    /// servers are allowed to report in `workDoneProgress` property in the
    /// request specific server capabilities.
    ///
    /// @since 3.15.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub work_done_progress: Option<bool>,

    /// Capabilities specific to the showMessage request.
    ///
    /// @since 3.16.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_message: Option<ShowMessageRequestClientCapabilities>,

    /// Client capabilities for the show document request.
    ///
    /// @since 3.16.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_document: Option<ShowDocumentClientCapabilities>,
}

/// Show message request client capabilities
#[derive(Debug, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShowMessageRequestClientCapabilities {
    /// Capabilities specific to the `MessageActionItem` type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_action_item: Option<MessageActionItemCapabilities>,
}

#[derive(Debug, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageActionItemCapabilities {
    /// Whether the client supports additional attributes which
    /// are preserved and send back to the server in the
    /// request's response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_properties_support: Option<bool>,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageActionItem {
    /// A short title like 'Retry', 'Open Log' etc.
    pub title: String,

    /// Additional attributes that the client preserves and
    /// sends back to the server. This depends on the client
    /// capability window.messageActionItem.additionalPropertiesSupport
    #[serde(flatten)]
    pub properties: HashMap<String, MessageActionItemProperty>,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum MessageActionItemProperty {
    String(String),
    Boolean(bool),
    Integer(i32),
    Object(Value),
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub struct LogMessageParams {
    /// The message type. See {@link MessageType}
    #[serde(rename = "type")]
    pub typ: MessageType,

    /// The actual message
    pub message: String,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub struct ShowMessageParams {
    /// The message type. See {@link MessageType}.
    #[serde(rename = "type")]
    pub typ: MessageType,

    /// The actual message.
    pub message: String,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub struct ShowMessageRequestParams {
    /// The message type. See {@link MessageType}
    #[serde(rename = "type")]
    pub typ: MessageType,

    /// The actual message
    pub message: String,

    /// The message action items to present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actions: Option<Vec<MessageActionItem>>,
}

/// Client capabilities for the show document request.
#[derive(Debug, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShowDocumentClientCapabilities {
    /// The client has support for the show document request.
    pub support: bool,
}

/// Params to show a document.
///
/// @since 3.16.0
#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShowDocumentParams {
    /// The document uri to show.
    pub uri: Url,

    /// Indicates to show the resource in an external program.
    /// To show for example `https://code.visualstudio.com/`
    /// in the default WEB browser set `external` to `true`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external: Option<bool>,

    /// An optional property to indicate whether the editor
    /// showing the document should take focus or not.
    /// Clients might ignore this property if an external
    /// program in started.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub take_focus: Option<bool>,

    /// An optional selection range if the document is a text
    /// document. Clients might ignore the property if an
    /// external program is started or the file is not a text
    /// file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selection: Option<Range>,
}

/// The result of an show document request.
///
/// @since 3.16.0
#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShowDocumentResult {
    /// A boolean indicating if the show was successful.
    pub success: bool,
}
