use serde::{Deserialize, Serialize};

use crate::{
    Documentation, MarkupKind, TextDocumentPositionParams, TextDocumentRegistrationOptions,
    WorkDoneProgressOptions, WorkDoneProgressParams,
};

#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureInformationSettings {
    /// Client supports the follow content formats for the documentation
    /// property. The order describes the preferred format of the client.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation_format: Option<Vec<MarkupKind>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameter_information: Option<ParameterInformationSettings>,

    /// The client support the `activeParameter` property on `SignatureInformation`
    /// literal.
    ///
    /// @since 3.16.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_parameter_support: Option<bool>,
}

#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParameterInformationSettings {
    /// The client supports processing label offsets instead of a
    /// simple label string.
    ///
    /// @since 3.14.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_offset_support: Option<bool>,
}

#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureHelpClientCapabilities {
    /// Whether completion supports dynamic registration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_registration: Option<bool>,

    /// The client supports the following `SignatureInformation`
    /// specific properties.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature_information: Option<SignatureInformationSettings>,

    /// The client supports to send additional context information for a
    /// `textDocument/signatureHelp` request. A client that opts into
    /// contextSupport will also support the `retriggerCharacters` on
    /// `SignatureHelpOptions`.
    ///
    /// @since 3.15.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_support: Option<bool>,
}

/// Signature help options.
#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureHelpOptions {
    /// The characters that trigger signature help automatically.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger_characters: Option<Vec<String>>,

    /// List of characters that re-trigger signature help.
    /// These trigger characters are only active when signature help is already showing. All trigger characters
    /// are also counted as re-trigger characters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retrigger_characters: Option<Vec<String>>,

    #[serde(flatten)]
    pub work_done_progress_options: WorkDoneProgressOptions,
}

/// Signature help options.
#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub struct SignatureHelpRegistrationOptions {
    #[serde(flatten)]
    pub text_document_registration_options: TextDocumentRegistrationOptions,
}

/// Signature help options.
#[derive(Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(transparent)]
pub struct SignatureHelpTriggerKind(i32);
lsp_enum! {
impl SignatureHelpTriggerKind {
    /// Signature help was invoked manually by the user or by a command.
    pub const INVOKED: SignatureHelpTriggerKind = SignatureHelpTriggerKind(1);
    /// Signature help was triggered by a trigger character.
    pub const TRIGGER_CHARACTER: SignatureHelpTriggerKind = SignatureHelpTriggerKind(2);
    /// Signature help was triggered by the cursor moving or by the document content changing.
    pub const CONTENT_CHANGE: SignatureHelpTriggerKind = SignatureHelpTriggerKind(3);
}
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureHelpParams {
    /// The signature help context. This is only available if the client specifies
    /// to send this using the client capability  `textDocument.signatureHelp.contextSupport === true`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<SignatureHelpContext>,

    #[serde(flatten)]
    pub text_document_position_params: TextDocumentPositionParams,

    #[serde(flatten)]
    pub work_done_progress_params: WorkDoneProgressParams,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureHelpContext {
    /// Action that caused signature help to be triggered.
    pub trigger_kind: SignatureHelpTriggerKind,

    /// Character that caused signature help to be triggered.
    /// This is undefined when `triggerKind !== SignatureHelpTriggerKind.TriggerCharacter`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger_character: Option<String>,

    /// `true` if signature help was already showing when it was triggered.
    /// Retriggers occur when the signature help is already active and can be caused by actions such as
    /// typing a trigger character, a cursor move, or document content changes.
    pub is_retrigger: bool,

    /// The currently active `SignatureHelp`.
    /// The `activeSignatureHelp` has its `SignatureHelp.activeSignature` field updated based on
    /// the user navigating through available signatures.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_signature_help: Option<SignatureHelp>,
}

/// Signature help represents the signature of something
/// callable. There can be multiple signature but only one
/// active and only one active parameter.
#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureHelp {
    /// One or more signatures.
    pub signatures: Vec<SignatureInformation>,

    /// The active signature.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_signature: Option<u32>,

    /// The active parameter of the active signature.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_parameter: Option<u32>,
}

/// Represents the signature of something callable. A signature
/// can have a label, like a function-name, a doc-comment, and
/// a set of parameters.
#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureInformation {
    /// The label of this signature. Will be shown in
    /// the UI.
    pub label: String,

    /// The human-readable doc-comment of this signature. Will be shown
    /// in the UI but can be omitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation: Option<Documentation>,

    /// The parameters of this signature.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<Vec<ParameterInformation>>,

    /// The index of the active parameter.
    ///
    /// If provided, this is used in place of `SignatureHelp.activeParameter`.
    ///
    /// @since 3.16.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_parameter: Option<u32>,
}

/// Represents a parameter of a callable-signature. A parameter can
/// have a label and a doc-comment.
#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParameterInformation {
    /// The label of this parameter information.
    ///
    /// Either a string or an inclusive start and exclusive end offsets within its containing
    /// signature label. (see SignatureInformation.label). *Note*: A label of type string must be
    /// a substring of its containing signature label.
    pub label: ParameterLabel,

    /// The human-readable doc-comment of this parameter. Will be shown
    /// in the UI but can be omitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation: Option<Documentation>,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ParameterLabel {
    Simple(String),
    LabelOffsets([u32; 2]),
}
