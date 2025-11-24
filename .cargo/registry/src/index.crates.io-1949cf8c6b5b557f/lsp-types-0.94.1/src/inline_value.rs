use crate::{
    DynamicRegistrationClientCapabilities, Range, StaticRegistrationOptions,
    TextDocumentIdentifier, TextDocumentRegistrationOptions, WorkDoneProgressOptions,
    WorkDoneProgressParams,
};
use serde::{Deserialize, Serialize};

pub type InlineValueClientCapabilities = DynamicRegistrationClientCapabilities;

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum InlineValueServerCapabilities {
    Options(InlineValueOptions),
    RegistrationOptions(InlineValueRegistrationOptions),
}

/// Inline value options used during static registration.
///
/// @since 3.17.0
#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
pub struct InlineValueOptions {
    #[serde(flatten)]
    pub work_done_progress_options: WorkDoneProgressOptions,
}

/// Inline value options used during static or dynamic registration.
///
/// @since 3.17.0
#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
pub struct InlineValueRegistrationOptions {
    #[serde(flatten)]
    pub inline_value_options: InlineValueOptions,

    #[serde(flatten)]
    pub text_document_registration_options: TextDocumentRegistrationOptions,

    #[serde(flatten)]
    pub static_registration_options: StaticRegistrationOptions,
}

/// A parameter literal used in inline value requests.
///
/// @since 3.17.0
#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InlineValueParams {
    #[serde(flatten)]
    pub work_done_progress_params: WorkDoneProgressParams,

    /// The text document.
    pub text_document: TextDocumentIdentifier,

    /// The document range for which inline values should be computed.
    pub range: Range,

    /// Additional information about the context in which inline values were
    /// requested.
    pub context: InlineValueContext,
}

/// @since 3.17.0
#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InlineValueContext {
    /// The stack frame (as a DAP Id) where the execution has stopped.
    pub frame_id: i32,

    /// The document range where execution has stopped.
    /// Typically the end position of the range denotes the line where the
    /// inline values are shown.
    pub stopped_location: Range,
}

/// Provide inline value as text.
///
/// @since 3.17.0
#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
pub struct InlineValueText {
    /// The document range for which the inline value applies.
    pub range: Range,

    /// The text of the inline value.
    pub text: String,
}

/// Provide inline value through a variable lookup.
///
/// If only a range is specified, the variable name will be extracted from
/// the underlying document.
///
/// An optional variable name can be used to override the extracted name.
///
/// @since 3.17.0
#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InlineValueVariableLookup {
    /// The document range for which the inline value applies.
    /// The range is used to extract the variable name from the underlying
    /// document.
    pub range: Range,

    /// If specified the name of the variable to look up.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variable_name: Option<String>,

    /// How to perform the lookup.
    pub case_sensitive_lookup: bool,
}

/// Provide an inline value through an expression evaluation.
///
/// If only a range is specified, the expression will be extracted from the
/// underlying document.
///
/// An optional expression can be used to override the extracted expression.
///
/// @since 3.17.0
#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InlineValueEvaluatableExpression {
    /// The document range for which the inline value applies.
    /// The range is used to extract the evaluatable expression from the
    /// underlying document.
    pub range: Range,

    /// If specified the expression overrides the extracted expression.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expression: Option<String>,
}

/// Inline value information can be provided by different means:
/// - directly as a text value (class InlineValueText).
/// - as a name to use for a variable lookup (class InlineValueVariableLookup)
/// - as an evaluatable expression (class InlineValueEvaluatableExpression)
/// The InlineValue types combines all inline value types into one type.
///
/// @since 3.17.0
#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum InlineValue {
    Text(InlineValueText),
    VariableLookup(InlineValueVariableLookup),
    EvaluatableExpression(InlineValueEvaluatableExpression),
}

impl From<InlineValueText> for InlineValue {
    #[inline]
    fn from(from: InlineValueText) -> Self {
        Self::Text(from)
    }
}

impl From<InlineValueVariableLookup> for InlineValue {
    #[inline]
    fn from(from: InlineValueVariableLookup) -> Self {
        Self::VariableLookup(from)
    }
}

impl From<InlineValueEvaluatableExpression> for InlineValue {
    #[inline]
    fn from(from: InlineValueEvaluatableExpression) -> Self {
        Self::EvaluatableExpression(from)
    }
}

/// Client workspace capabilities specific to inline values.
#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
///
/// @since 3.17.0
#[serde(rename_all = "camelCase")]
pub struct InlineValueWorkspaceClientCapabilities {
    /// Whether the client implementation supports a refresh request sent from
    /// the server to the client.
    ///
    /// Note that this event is global and will force the client to refresh all
    /// inline values currently shown. It should be used with absolute care and
    /// is useful for situation where a server for example detect a project wide
    /// change that requires such a calculation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_support: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::test_serialization;
    use crate::Position;

    #[test]
    fn inline_values() {
        test_serialization(
            &InlineValueText {
                range: Range::new(Position::new(0, 0), Position::new(0, 4)),
                text: "one".to_owned(),
            },
            r#"{"range":{"start":{"line":0,"character":0},"end":{"line":0,"character":4}},"text":"one"}"#,
        );

        test_serialization(
            &InlineValue::VariableLookup(InlineValueVariableLookup {
                range: Range::new(Position::new(1, 0), Position::new(1, 4)),
                variable_name: None,
                case_sensitive_lookup: false,
            }),
            r#"{"range":{"start":{"line":1,"character":0},"end":{"line":1,"character":4}},"caseSensitiveLookup":false}"#,
        );

        test_serialization(
            &InlineValue::EvaluatableExpression(InlineValueEvaluatableExpression {
                range: Range::new(Position::new(2, 0), Position::new(2, 4)),
                expression: None,
            }),
            r#"{"range":{"start":{"line":2,"character":0},"end":{"line":2,"character":4}}}"#,
        );
    }
}
