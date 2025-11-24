use crate::{
    DocumentSelector, DynamicRegistrationClientCapabilities, PartialResultParams, Range,
    TextDocumentIdentifier, TextEdit, WorkDoneProgressParams,
};
use serde::{Deserialize, Serialize};

pub type DocumentColorClientCapabilities = DynamicRegistrationClientCapabilities;

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorProviderOptions {}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StaticTextDocumentColorProviderOptions {
    /// A document selector to identify the scope of the registration. If set to null
    /// the document selector provided on the client side will be used.
    pub document_selector: Option<DocumentSelector>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ColorProviderCapability {
    Simple(bool),
    ColorProvider(ColorProviderOptions),
    Options(StaticTextDocumentColorProviderOptions),
}

impl From<ColorProviderOptions> for ColorProviderCapability {
    fn from(from: ColorProviderOptions) -> Self {
        Self::ColorProvider(from)
    }
}

impl From<StaticTextDocumentColorProviderOptions> for ColorProviderCapability {
    fn from(from: StaticTextDocumentColorProviderOptions) -> Self {
        Self::Options(from)
    }
}

impl From<bool> for ColorProviderCapability {
    fn from(from: bool) -> Self {
        Self::Simple(from)
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentColorParams {
    /// The text document
    pub text_document: TextDocumentIdentifier,

    #[serde(flatten)]
    pub work_done_progress_params: WorkDoneProgressParams,

    #[serde(flatten)]
    pub partial_result_params: PartialResultParams,
}

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorInformation {
    /// The range in the document where this color appears.
    pub range: Range,
    /// The actual color value for this color range.
    pub color: Color,
}

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize, Copy)]
#[serde(rename_all = "camelCase")]
pub struct Color {
    /// The red component of this color in the range [0-1].
    pub red: f32,
    /// The green component of this color in the range [0-1].
    pub green: f32,
    /// The blue component of this color in the range [0-1].
    pub blue: f32,
    /// The alpha component of this color in the range [0-1].
    pub alpha: f32,
}

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorPresentationParams {
    /// The text document.
    pub text_document: TextDocumentIdentifier,

    /// The color information to request presentations for.
    pub color: Color,

    /// The range where the color would be inserted. Serves as a context.
    pub range: Range,

    #[serde(flatten)]
    pub work_done_progress_params: WorkDoneProgressParams,

    #[serde(flatten)]
    pub partial_result_params: PartialResultParams,
}

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ColorPresentation {
    /// The label of this color presentation. It will be shown on the color
    /// picker header. By default this is also the text that is inserted when selecting
    /// this color presentation.
    pub label: String,

    /// An [edit](#TextEdit) which is applied to a document when selecting
    /// this presentation for the color.  When `falsy` the [label](#ColorPresentation.label)
    /// is used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_edit: Option<TextEdit>,

    /// An optional array of additional [text edits](#TextEdit) that are applied when
    /// selecting this color presentation. Edits must not overlap with the main [edit](#ColorPresentation.textEdit) nor with themselves.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_text_edits: Option<Vec<TextEdit>>,
}
