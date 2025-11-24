use serde::{Deserialize, Serialize};

use crate::{
    DynamicRegistrationClientCapabilities, Range, StaticRegistrationOptions,
    TextDocumentPositionParams, TextDocumentRegistrationOptions, WorkDoneProgressOptions,
    WorkDoneProgressParams,
};

pub type LinkedEditingRangeClientCapabilities = DynamicRegistrationClientCapabilities;

#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedEditingRangeOptions {
    #[serde(flatten)]
    pub work_done_progress_options: WorkDoneProgressOptions,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedEditingRangeRegistrationOptions {
    #[serde(flatten)]
    pub text_document_registration_options: TextDocumentRegistrationOptions,

    #[serde(flatten)]
    pub linked_editing_range_options: LinkedEditingRangeOptions,

    #[serde(flatten)]
    pub static_registration_options: StaticRegistrationOptions,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum LinkedEditingRangeServerCapabilities {
    Simple(bool),
    Options(LinkedEditingRangeOptions),
    RegistrationOptions(LinkedEditingRangeRegistrationOptions),
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedEditingRangeParams {
    #[serde(flatten)]
    pub text_document_position_params: TextDocumentPositionParams,

    #[serde(flatten)]
    pub work_done_progress_params: WorkDoneProgressParams,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedEditingRanges {
    /// A list of ranges that can be renamed together. The ranges must have
    /// identical length and contain identical text content. The ranges cannot overlap.
    pub ranges: Vec<Range>,

    /// An optional word pattern (regular expression) that describes valid contents for
    /// the given ranges. If no pattern is provided, the client configuration's word
    /// pattern will be used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub word_pattern: Option<String>,
}
