use crate::{Range, TextDocumentPositionParams, WorkDoneProgressOptions, WorkDoneProgressParams};
use serde::{Deserialize, Serialize};

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameParams {
    /// Text Document and Position fields
    #[serde(flatten)]
    pub text_document_position: TextDocumentPositionParams,

    /// The new name of the symbol. If the given name is not valid the
    /// request must return a [ResponseError](#ResponseError) with an
    /// appropriate message set.
    pub new_name: String,

    #[serde(flatten)]
    pub work_done_progress_params: WorkDoneProgressParams,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameOptions {
    /// Renames should be checked and tested before being executed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prepare_provider: Option<bool>,

    #[serde(flatten)]
    pub work_done_progress_options: WorkDoneProgressOptions,
}

#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameClientCapabilities {
    /// Whether rename supports dynamic registration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_registration: Option<bool>,

    /// Client supports testing for validity of rename operations before execution.
    ///
    /// @since 3.12.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prepare_support: Option<bool>,

    /// Client supports the default behavior result.
    ///
    /// The value indicates the default behavior used by the
    /// client.
    ///
    /// @since 3.16.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prepare_support_default_behavior: Option<PrepareSupportDefaultBehavior>,

    /// Whether the client honors the change annotations in
    /// text edits and resource operations returned via the
    /// rename request's workspace edit by for example presenting
    /// the workspace edit in the user interface and asking
    /// for confirmation.
    ///
    /// @since 3.16.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub honors_change_annotations: Option<bool>,
}

#[derive(Eq, PartialEq, Copy, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PrepareSupportDefaultBehavior(i32);
lsp_enum! {
impl PrepareSupportDefaultBehavior {
    /// The client's default behavior is to select the identifier
    /// according the to language's syntax rule
    pub const IDENTIFIER: PrepareSupportDefaultBehavior = PrepareSupportDefaultBehavior(1);
}
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(untagged)]
#[serde(rename_all = "camelCase")]
pub enum PrepareRenameResponse {
    Range(Range),
    RangeWithPlaceholder {
        range: Range,
        placeholder: String,
    },
    #[serde(rename_all = "camelCase")]
    DefaultBehavior {
        default_behavior: bool,
    },
}
