use crate::{
    DynamicRegistrationClientCapabilities, LSPAny, PartialResultParams, Range,
    StaticRegistrationOptions, SymbolKind, SymbolTag, TextDocumentPositionParams,
    TextDocumentRegistrationOptions, Url, WorkDoneProgressOptions, WorkDoneProgressParams,
};

use serde::{Deserialize, Serialize};

pub type TypeHierarchyClientCapabilities = DynamicRegistrationClientCapabilities;

#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
pub struct TypeHierarchyOptions {
    #[serde(flatten)]
    pub work_done_progress_options: WorkDoneProgressOptions,
}

#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
pub struct TypeHierarchyRegistrationOptions {
    #[serde(flatten)]
    pub text_document_registration_options: TextDocumentRegistrationOptions,
    #[serde(flatten)]
    pub type_hierarchy_options: TypeHierarchyOptions,
    #[serde(flatten)]
    pub static_registration_options: StaticRegistrationOptions,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub struct TypeHierarchyPrepareParams {
    #[serde(flatten)]
    pub text_document_position_params: TextDocumentPositionParams,
    #[serde(flatten)]
    pub work_done_progress_params: WorkDoneProgressParams,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub struct TypeHierarchySupertypesParams {
    pub item: TypeHierarchyItem,

    #[serde(flatten)]
    pub work_done_progress_params: WorkDoneProgressParams,
    #[serde(flatten)]
    pub partial_result_params: PartialResultParams,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub struct TypeHierarchySubtypesParams {
    pub item: TypeHierarchyItem,

    #[serde(flatten)]
    pub work_done_progress_params: WorkDoneProgressParams,
    #[serde(flatten)]
    pub partial_result_params: PartialResultParams,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeHierarchyItem {
    /// The name of this item.
    pub name: String,

    /// The kind of this item.
    pub kind: SymbolKind,

    /// Tags for this item.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<SymbolTag>,

    /// More detail for this item, e.g. the signature of a function.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,

    /// The resource identifier of this item.
    pub uri: Url,

    /// The range enclosing this symbol not including leading/trailing whitespace
    /// but everything else, e.g. comments and code.
    pub range: Range,

    /// The range that should be selected and revealed when this symbol is being
    /// picked, e.g. the name of a function. Must be contained by the
    /// [`range`](#TypeHierarchyItem.range).
    pub selection_range: Range,

    /// A data entry field that is preserved between a type hierarchy prepare and
    /// supertypes or subtypes requests. It could also be used to identify the
    /// type hierarchy in the server, helping improve the performance on
    /// resolving supertypes and subtypes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<LSPAny>,
}
