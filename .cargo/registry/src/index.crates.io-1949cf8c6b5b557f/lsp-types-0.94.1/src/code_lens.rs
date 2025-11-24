use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    Command, DynamicRegistrationClientCapabilities, PartialResultParams, Range,
    TextDocumentIdentifier, WorkDoneProgressParams,
};

pub type CodeLensClientCapabilities = DynamicRegistrationClientCapabilities;

/// Code Lens options.
#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize, Copy)]
#[serde(rename_all = "camelCase")]
pub struct CodeLensOptions {
    /// Code lens has a resolve provider as well.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolve_provider: Option<bool>,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeLensParams {
    /// The document to request code lens for.
    pub text_document: TextDocumentIdentifier,

    #[serde(flatten)]
    pub work_done_progress_params: WorkDoneProgressParams,

    #[serde(flatten)]
    pub partial_result_params: PartialResultParams,
}

/// A code lens represents a command that should be shown along with
/// source text, like the number of references, a way to run tests, etc.
///
/// A code lens is _unresolved_ when no command is associated to it. For performance
/// reasons the creation of a code lens and resolving should be done in two stages.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeLens {
    /// The range in which this code lens is valid. Should only span a single line.
    pub range: Range,

    /// The command this code lens represents.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<Command>,

    /// A data entry field that is preserved on a code lens item between
    /// a code lens and a code lens resolve request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeLensWorkspaceClientCapabilities {
    /// Whether the client implementation supports a refresh request sent from the
    /// server to the client.
    ///
    /// Note that this event is global and will force the client to refresh all
    /// code lenses currently shown. It should be used with absolute care and is
    /// useful for situation where a server for example detect a project wide
    /// change that requires such a calculation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_support: Option<bool>,
}
