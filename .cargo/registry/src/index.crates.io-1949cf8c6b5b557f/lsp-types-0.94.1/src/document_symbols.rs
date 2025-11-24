use crate::{
    Location, PartialResultParams, Range, SymbolKind, SymbolKindCapability, TextDocumentIdentifier,
    WorkDoneProgressParams,
};

use crate::{SymbolTag, TagSupport};

use serde::{Deserialize, Serialize};

#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSymbolClientCapabilities {
    /// This capability supports dynamic registration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_registration: Option<bool>,

    /// Specific capabilities for the `SymbolKind`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_kind: Option<SymbolKindCapability>,

    /// The client support hierarchical document symbols.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hierarchical_document_symbol_support: Option<bool>,

    /// The client supports tags on `SymbolInformation`. Tags are supported on
    /// `DocumentSymbol` if `hierarchicalDocumentSymbolSupport` is set to true.
    /// Clients supporting tags have to handle unknown tags gracefully.
    ///
    /// @since 3.16.0
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "TagSupport::deserialize_compat"
    )]
    pub tag_support: Option<TagSupport<SymbolTag>>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DocumentSymbolResponse {
    Flat(Vec<SymbolInformation>),
    Nested(Vec<DocumentSymbol>),
}

impl From<Vec<SymbolInformation>> for DocumentSymbolResponse {
    fn from(info: Vec<SymbolInformation>) -> Self {
        DocumentSymbolResponse::Flat(info)
    }
}

impl From<Vec<DocumentSymbol>> for DocumentSymbolResponse {
    fn from(symbols: Vec<DocumentSymbol>) -> Self {
        DocumentSymbolResponse::Nested(symbols)
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSymbolParams {
    /// The text document.
    pub text_document: TextDocumentIdentifier,

    #[serde(flatten)]
    pub work_done_progress_params: WorkDoneProgressParams,

    #[serde(flatten)]
    pub partial_result_params: PartialResultParams,
}

/// Represents programming constructs like variables, classes, interfaces etc.
/// that appear in a document. Document symbols can be hierarchical and they have two ranges:
/// one that encloses its definition and one that points to its most interesting range,
/// e.g. the range of an identifier.
#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSymbol {
    /// The name of this symbol.
    pub name: String,
    /// More detail for this symbol, e.g the signature of a function. If not provided the
    /// name is used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// The kind of this symbol.
    pub kind: SymbolKind,
    /// Tags for this completion item.
    ///
    /// @since 3.15.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<SymbolTag>>,
    /// Indicates if this symbol is deprecated.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[deprecated(note = "Use tags instead")]
    pub deprecated: Option<bool>,
    /// The range enclosing this symbol not including leading/trailing whitespace but everything else
    /// like comments. This information is typically used to determine if the the clients cursor is
    /// inside the symbol to reveal in the symbol in the UI.
    pub range: Range,
    /// The range that should be selected and revealed when this symbol is being picked, e.g the name of a function.
    /// Must be contained by the the `range`.
    pub selection_range: Range,
    /// Children of this symbol, e.g. properties of a class.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<DocumentSymbol>>,
}

/// Represents information about programming constructs like variables, classes,
/// interfaces etc.
#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SymbolInformation {
    /// The name of this symbol.
    pub name: String,

    /// The kind of this symbol.
    pub kind: SymbolKind,

    /// Tags for this completion item.
    ///
    /// @since 3.16.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<SymbolTag>>,

    /// Indicates if this symbol is deprecated.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[deprecated(note = "Use tags instead")]
    pub deprecated: Option<bool>,

    /// The location of this symbol.
    pub location: Location,

    /// The name of the symbol containing this symbol.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_name: Option<String>,
}
