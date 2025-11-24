use crate::{
    DynamicRegistrationClientCapabilities, PartialResultParams, TextDocumentPositionParams,
    WorkDoneProgressParams,
};
use serde::{Deserialize, Serialize};

pub type ReferenceClientCapabilities = DynamicRegistrationClientCapabilities;
#[derive(Debug, Eq, PartialEq, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceContext {
    /// Include the declaration of the current symbol.
    pub include_declaration: bool,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceParams {
    // Text Document and Position fields
    #[serde(flatten)]
    pub text_document_position: TextDocumentPositionParams,

    #[serde(flatten)]
    pub work_done_progress_params: WorkDoneProgressParams,

    #[serde(flatten)]
    pub partial_result_params: PartialResultParams,

    // ReferenceParams properties:
    pub context: ReferenceContext,
}
