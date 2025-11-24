use crate::{
    PartialResultParams, StaticTextDocumentColorProviderOptions, TextDocumentIdentifier,
    WorkDoneProgressParams,
};
use serde::{Deserialize, Serialize};
#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FoldingRangeParams {
    /// The text document.
    pub text_document: TextDocumentIdentifier,

    #[serde(flatten)]
    pub work_done_progress_params: WorkDoneProgressParams,

    #[serde(flatten)]
    pub partial_result_params: PartialResultParams,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum FoldingRangeProviderCapability {
    Simple(bool),
    FoldingProvider(FoldingProviderOptions),
    Options(StaticTextDocumentColorProviderOptions),
}

impl From<StaticTextDocumentColorProviderOptions> for FoldingRangeProviderCapability {
    fn from(from: StaticTextDocumentColorProviderOptions) -> Self {
        Self::Options(from)
    }
}

impl From<FoldingProviderOptions> for FoldingRangeProviderCapability {
    fn from(from: FoldingProviderOptions) -> Self {
        Self::FoldingProvider(from)
    }
}

impl From<bool> for FoldingRangeProviderCapability {
    fn from(from: bool) -> Self {
        Self::Simple(from)
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub struct FoldingProviderOptions {}

#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FoldingRangeKindCapability {
    /// The folding range kind values the client supports. When this
    /// property exists the client also guarantees that it will
    /// handle values outside its set gracefully and falls back
    /// to a default value when unknown.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_set: Option<Vec<FoldingRangeKind>>,
}

#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FoldingRangeCapability {
    /// If set, the client signals that it supports setting collapsedText on
    /// folding ranges to display custom labels instead of the default text.
    ///
    /// @since 3.17.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collapsed_text: Option<bool>,
}

#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FoldingRangeClientCapabilities {
    /// Whether implementation supports dynamic registration for folding range providers. If this is set to `true`
    /// the client supports the new `(FoldingRangeProviderOptions & TextDocumentRegistrationOptions & StaticRegistrationOptions)`
    /// return value for the corresponding server capability as well.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_registration: Option<bool>,

    /// The maximum number of folding ranges that the client prefers to receive per document. The value serves as a
    /// hint, servers are free to follow the limit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range_limit: Option<u32>,

    /// If set, the client signals that it only supports folding complete lines. If set, client will
    /// ignore specified `startCharacter` and `endCharacter` properties in a FoldingRange.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_folding_only: Option<bool>,

    /// Specific options for the folding range kind.
    ///
    /// @since 3.17.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folding_range_kind: Option<FoldingRangeKindCapability>,

    /// Specific options for the folding range.
    ///
    /// @since 3.17.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folding_range: Option<FoldingRangeCapability>,
}

/// Enum of known range kinds
#[derive(Debug, Eq, PartialEq, Deserialize, Serialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum FoldingRangeKind {
    /// Folding range for a comment
    Comment,
    /// Folding range for a imports or includes
    Imports,
    /// Folding range for a region (e.g. `#region`)
    Region,
}

/// Represents a folding range.
#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FoldingRange {
    /// The zero-based line number from where the folded range starts.
    pub start_line: u32,

    /// The zero-based character offset from where the folded range starts. If not defined, defaults to the length of the start line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_character: Option<u32>,

    /// The zero-based line number where the folded range ends.
    pub end_line: u32,

    /// The zero-based character offset before the folded range ends. If not defined, defaults to the length of the end line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_character: Option<u32>,

    /// Describes the kind of the folding range such as `comment' or 'region'. The kind
    /// is used to categorize folding ranges and used by commands like 'Fold all comments'. See
    /// [FoldingRangeKind](#FoldingRangeKind) for an enumeration of standardized kinds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<FoldingRangeKind>,

    /// The text that the client should show when the specified range is
    /// collapsed. If not defined or not supported by the client, a default
    /// will be chosen by the client.
    ///
    /// @since 3.17.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collapsed_text: Option<String>,
}
