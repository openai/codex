use crate::{
    LSPAny, Location, OneOf, PartialResultParams, SymbolInformation, SymbolKind,
    SymbolKindCapability, SymbolTag, TagSupport, Url, WorkDoneProgressParams,
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSymbolClientCapabilities {
    /// This capability supports dynamic registration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_registration: Option<bool>,

    /// Specific capabilities for the `SymbolKind` in the `workspace/symbol` request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_kind: Option<SymbolKindCapability>,

    /// The client supports tags on `SymbolInformation`.
    /// Clients supporting tags have to handle unknown tags gracefully.
    ///
    /// @since 3.16.0
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "TagSupport::deserialize_compat"
    )]
    pub tag_support: Option<TagSupport<SymbolTag>>,

    /// The client support partial workspace symbols. The client will send the
    /// request `workspaceSymbol/resolve` to the server to resolve additional
    /// properties.
    ///
    /// @since 3.17.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolve_support: Option<WorkspaceSymbolResolveSupportCapability>,
}

/// The parameters of a Workspace Symbol Request.
#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
pub struct WorkspaceSymbolParams {
    #[serde(flatten)]
    pub partial_result_params: PartialResultParams,

    #[serde(flatten)]
    pub work_done_progress_params: WorkDoneProgressParams,

    /// A non-empty query string
    pub query: String,
}

#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
pub struct WorkspaceSymbolResolveSupportCapability {
    /// The properties that a client can resolve lazily. Usually
    /// `location.range`
    pub properties: Vec<String>,
}

/// A special workspace symbol that supports locations without a range
///
/// @since 3.17.0
#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSymbol {
    /// The name of this symbol.
    pub name: String,

    /// The kind of this symbol.
    pub kind: SymbolKind,

    /// Tags for this completion item.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<SymbolTag>>,

    /// The name of the symbol containing this symbol. This information is for
    /// user interface purposes (e.g. to render a qualifier in the user interface
    /// if necessary). It can't be used to re-infer a hierarchy for the document
    /// symbols.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_name: Option<String>,

    /// The location of this symbol. Whether a server is allowed to
    /// return a location without a range depends on the client
    /// capability `workspace.symbol.resolveSupport`.
    ///
    /// See also `SymbolInformation.location`.
    pub location: OneOf<Location, WorkspaceLocation>,

    /// A data entry field that is preserved on a workspace symbol between a
    /// workspace symbol request and a workspace symbol resolve request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<LSPAny>,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub struct WorkspaceLocation {
    pub uri: Url,
}

#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WorkspaceSymbolResponse {
    Flat(Vec<SymbolInformation>),
    Nested(Vec<WorkspaceSymbol>),
}
