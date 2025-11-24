use serde::{Deserialize, Serialize};

use crate::{
    DocumentSelector, DynamicRegistrationClientCapabilities, Range, TextDocumentIdentifier,
    TextDocumentPositionParams, WorkDoneProgressParams,
};

use std::collections::HashMap;

pub type DocumentFormattingClientCapabilities = DynamicRegistrationClientCapabilities;
pub type DocumentRangeFormattingClientCapabilities = DynamicRegistrationClientCapabilities;
pub type DocumentOnTypeFormattingClientCapabilities = DynamicRegistrationClientCapabilities;

/// Format document on type options
#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentOnTypeFormattingOptions {
    /// A character on which formatting should be triggered, like `}`.
    pub first_trigger_character: String,

    /// More trigger characters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub more_trigger_character: Option<Vec<String>>,
}

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentFormattingParams {
    /// The document to format.
    pub text_document: TextDocumentIdentifier,

    /// The format options.
    pub options: FormattingOptions,

    #[serde(flatten)]
    pub work_done_progress_params: WorkDoneProgressParams,
}

/// Value-object describing what options formatting should use.
#[derive(Debug, PartialEq, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormattingOptions {
    /// Size of a tab in spaces.
    pub tab_size: u32,

    /// Prefer spaces over tabs.
    pub insert_spaces: bool,

    /// Signature for further properties.
    #[serde(flatten)]
    pub properties: HashMap<String, FormattingProperty>,

    /// Trim trailing whitespace on a line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trim_trailing_whitespace: Option<bool>,

    /// Insert a newline character at the end of the file if one does not exist.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insert_final_newline: Option<bool>,

    /// Trim all newlines after the final newline at the end of the file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trim_final_newlines: Option<bool>,
}

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum FormattingProperty {
    Bool(bool),
    Number(i32),
    String(String),
}

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentRangeFormattingParams {
    /// The document to format.
    pub text_document: TextDocumentIdentifier,

    /// The range to format
    pub range: Range,

    /// The format options
    pub options: FormattingOptions,

    #[serde(flatten)]
    pub work_done_progress_params: WorkDoneProgressParams,
}

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentOnTypeFormattingParams {
    /// Text Document and Position fields.
    #[serde(flatten)]
    pub text_document_position: TextDocumentPositionParams,

    /// The character that has been typed.
    pub ch: String,

    /// The format options.
    pub options: FormattingOptions,
}

/// Extends TextDocumentRegistrationOptions
#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentOnTypeFormattingRegistrationOptions {
    /// A document selector to identify the scope of the registration. If set to null
    /// the document selector provided on the client side will be used.
    pub document_selector: Option<DocumentSelector>,

    /// A character on which formatting should be triggered, like `}`.
    pub first_trigger_character: String,

    /// More trigger characters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub more_trigger_character: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::test_serialization;

    #[test]
    fn formatting_options() {
        test_serialization(
            &FormattingOptions {
                tab_size: 123,
                insert_spaces: true,
                properties: HashMap::new(),
                trim_trailing_whitespace: None,
                insert_final_newline: None,
                trim_final_newlines: None,
            },
            r#"{"tabSize":123,"insertSpaces":true}"#,
        );

        test_serialization(
            &FormattingOptions {
                tab_size: 123,
                insert_spaces: true,
                properties: vec![("prop".to_string(), FormattingProperty::Number(1))]
                    .into_iter()
                    .collect(),
                trim_trailing_whitespace: None,
                insert_final_newline: None,
                trim_final_newlines: None,
            },
            r#"{"tabSize":123,"insertSpaces":true,"prop":1}"#,
        );
    }
}
