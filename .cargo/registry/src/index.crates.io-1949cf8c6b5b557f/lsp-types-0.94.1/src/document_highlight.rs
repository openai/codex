use serde::{Deserialize, Serialize};

use crate::{
    DynamicRegistrationClientCapabilities, PartialResultParams, Range, TextDocumentPositionParams,
    WorkDoneProgressParams,
};

pub type DocumentHighlightClientCapabilities = DynamicRegistrationClientCapabilities;

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentHighlightParams {
    #[serde(flatten)]
    pub text_document_position_params: TextDocumentPositionParams,

    #[serde(flatten)]
    pub work_done_progress_params: WorkDoneProgressParams,

    #[serde(flatten)]
    pub partial_result_params: PartialResultParams,
}

/// A document highlight is a range inside a text document which deserves
/// special attention. Usually a document highlight is visualized by changing
/// the background color of its range.
#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub struct DocumentHighlight {
    /// The range this highlight applies to.
    pub range: Range,

    /// The highlight kind, default is DocumentHighlightKind.Text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<DocumentHighlightKind>,
}

/// A document highlight kind.
#[derive(Eq, PartialEq, Copy, Clone, Deserialize, Serialize)]
#[serde(transparent)]
pub struct DocumentHighlightKind(i32);
lsp_enum! {
impl DocumentHighlightKind {
    /// A textual occurrence.
    pub const TEXT: DocumentHighlightKind = DocumentHighlightKind(1);

    /// Read-access of a symbol, like reading a variable.
    pub const READ: DocumentHighlightKind = DocumentHighlightKind(2);

    /// Write-access of a symbol, like writing to a variable.
    pub const WRITE: DocumentHighlightKind = DocumentHighlightKind(3);
}
}
