use serde::{Deserialize, Serialize};

use crate::{
    Command, Documentation, MarkupKind, PartialResultParams, TagSupport,
    TextDocumentPositionParams, TextDocumentRegistrationOptions, TextEdit, WorkDoneProgressOptions,
    WorkDoneProgressParams,
};

use crate::Range;
use serde_json::Value;
use std::fmt::Debug;

/// Defines how to interpret the insert text in a completion item
#[derive(Eq, PartialEq, Clone, Copy, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InsertTextFormat(i32);
lsp_enum! {
impl InsertTextFormat {
    pub const PLAIN_TEXT: InsertTextFormat = InsertTextFormat(1);
    pub const SNIPPET: InsertTextFormat = InsertTextFormat(2);
}
}

/// The kind of a completion entry.
#[derive(Eq, PartialEq, Clone, Copy, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CompletionItemKind(i32);
lsp_enum! {
impl CompletionItemKind {
    pub const TEXT: CompletionItemKind = CompletionItemKind(1);
    pub const METHOD: CompletionItemKind = CompletionItemKind(2);
    pub const FUNCTION: CompletionItemKind = CompletionItemKind(3);
    pub const CONSTRUCTOR: CompletionItemKind = CompletionItemKind(4);
    pub const FIELD: CompletionItemKind = CompletionItemKind(5);
    pub const VARIABLE: CompletionItemKind = CompletionItemKind(6);
    pub const CLASS: CompletionItemKind = CompletionItemKind(7);
    pub const INTERFACE: CompletionItemKind = CompletionItemKind(8);
    pub const MODULE: CompletionItemKind = CompletionItemKind(9);
    pub const PROPERTY: CompletionItemKind = CompletionItemKind(10);
    pub const UNIT: CompletionItemKind = CompletionItemKind(11);
    pub const VALUE: CompletionItemKind = CompletionItemKind(12);
    pub const ENUM: CompletionItemKind = CompletionItemKind(13);
    pub const KEYWORD: CompletionItemKind = CompletionItemKind(14);
    pub const SNIPPET: CompletionItemKind = CompletionItemKind(15);
    pub const COLOR: CompletionItemKind = CompletionItemKind(16);
    pub const FILE: CompletionItemKind = CompletionItemKind(17);
    pub const REFERENCE: CompletionItemKind = CompletionItemKind(18);
    pub const FOLDER: CompletionItemKind = CompletionItemKind(19);
    pub const ENUM_MEMBER: CompletionItemKind = CompletionItemKind(20);
    pub const CONSTANT: CompletionItemKind = CompletionItemKind(21);
    pub const STRUCT: CompletionItemKind = CompletionItemKind(22);
    pub const EVENT: CompletionItemKind = CompletionItemKind(23);
    pub const OPERATOR: CompletionItemKind = CompletionItemKind(24);
    pub const TYPE_PARAMETER: CompletionItemKind = CompletionItemKind(25);
}
}

#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionItemCapability {
    /// Client supports snippets as insert text.
    ///
    /// A snippet can define tab stops and placeholders with `$1`, `$2`
    /// and `${3:foo}`. `$0` defines the final tab stop, it defaults to
    /// the end of the snippet. Placeholders with equal identifiers are linked,
    /// that is typing in one will update others too.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet_support: Option<bool>,

    /// Client supports commit characters on a completion item.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_characters_support: Option<bool>,

    /// Client supports the follow content formats for the documentation
    /// property. The order describes the preferred format of the client.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation_format: Option<Vec<MarkupKind>>,

    /// Client supports the deprecated property on a completion item.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated_support: Option<bool>,

    /// Client supports the preselect property on a completion item.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preselect_support: Option<bool>,

    /// Client supports the tag property on a completion item. Clients supporting
    /// tags have to handle unknown tags gracefully. Clients especially need to
    /// preserve unknown tags when sending a completion item back to the server in
    /// a resolve call.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "TagSupport::deserialize_compat"
    )]
    pub tag_support: Option<TagSupport<CompletionItemTag>>,

    /// Client support insert replace edit to control different behavior if a
    /// completion item is inserted in the text or should replace text.
    ///
    /// @since 3.16.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insert_replace_support: Option<bool>,

    /// Indicates which properties a client can resolve lazily on a completion
    /// item. Before version 3.16.0 only the predefined properties `documentation`
    /// and `details` could be resolved lazily.
    ///
    /// @since 3.16.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolve_support: Option<CompletionItemCapabilityResolveSupport>,

    /// The client supports the `insertTextMode` property on
    /// a completion item to override the whitespace handling mode
    /// as defined by the client.
    ///
    /// @since 3.16.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insert_text_mode_support: Option<InsertTextModeSupport>,

    /// The client has support for completion item label
    /// details (see also `CompletionItemLabelDetails`).
    ///
    /// @since 3.17.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_details_support: Option<bool>,
}

#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionItemCapabilityResolveSupport {
    /// The properties that a client can resolve lazily.
    pub properties: Vec<String>,
}

#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InsertTextModeSupport {
    pub value_set: Vec<InsertTextMode>,
}

/// How whitespace and indentation is handled during completion
/// item insertion.
///
/// @since 3.16.0
#[derive(Eq, PartialEq, Clone, Copy, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InsertTextMode(i32);
lsp_enum! {
impl InsertTextMode {
    /// The insertion or replace strings is taken as it is. If the
    /// value is multi line the lines below the cursor will be
    /// inserted using the indentation defined in the string value.
    /// The client will not apply any kind of adjustments to the
    /// string.
    pub const AS_IS: InsertTextMode = InsertTextMode(1);

    /// The editor adjusts leading whitespace of new lines so that
    /// they match the indentation up to the cursor of the line for
    /// which the item is accepted.
    ///
    /// Consider a line like this: <2tabs><cursor><3tabs>foo. Accepting a
    /// multi line completion item is indented using 2 tabs all
    /// following lines inserted will be indented using 2 tabs as well.
    pub const ADJUST_INDENTATION: InsertTextMode = InsertTextMode(2);
}
}

#[derive(Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(transparent)]
pub struct CompletionItemTag(i32);
lsp_enum! {
impl CompletionItemTag {
    pub const DEPRECATED: CompletionItemTag = CompletionItemTag(1);
}
}

#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionItemKindCapability {
    /// The completion item kind values the client supports. When this
    /// property exists the client also guarantees that it will
    /// handle values outside its set gracefully and falls back
    /// to a default value when unknown.
    ///
    /// If this property is not present the client only supports
    /// the completion items kinds from `Text` to `Reference` as defined in
    /// the initial version of the protocol.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_set: Option<Vec<CompletionItemKind>>,
}

#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionListCapability {
    /// The client supports the following itemDefaults on
    /// a completion list.
    ///
    /// The value lists the supported property names of the
    /// `CompletionList.itemDefaults` object. If omitted
    /// no properties are supported.
    ///
    /// @since 3.17.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_defaults: Option<Vec<String>>,
}

#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionClientCapabilities {
    /// Whether completion supports dynamic registration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_registration: Option<bool>,

    /// The client supports the following `CompletionItem` specific
    /// capabilities.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_item: Option<CompletionItemCapability>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_item_kind: Option<CompletionItemKindCapability>,

    /// The client supports to send additional context information for a
    /// `textDocument/completion` request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_support: Option<bool>,

    /// The client's default when the completion item doesn't provide a
    /// `insertTextMode` property.
    ///
    /// @since 3.17.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insert_text_mode: Option<InsertTextMode>,

    /// The client supports the following `CompletionList` specific
    /// capabilities.
    ///
    /// @since 3.17.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_list: Option<CompletionListCapability>,
}

/// A special text edit to provide an insert and a replace operation.
///
/// @since 3.16.0
#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InsertReplaceEdit {
    /// The string to be inserted.
    pub new_text: String,

    /// The range if the insert is requested
    pub insert: Range,

    /// The range if the replace is requested.
    pub replace: Range,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum CompletionTextEdit {
    Edit(TextEdit),
    InsertAndReplace(InsertReplaceEdit),
}

impl From<TextEdit> for CompletionTextEdit {
    fn from(edit: TextEdit) -> Self {
        CompletionTextEdit::Edit(edit)
    }
}

impl From<InsertReplaceEdit> for CompletionTextEdit {
    fn from(edit: InsertReplaceEdit) -> Self {
        CompletionTextEdit::InsertAndReplace(edit)
    }
}

/// Completion options.
#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionOptions {
    /// The server provides support to resolve additional information for a completion item.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolve_provider: Option<bool>,

    /// Most tools trigger completion request automatically without explicitly
    /// requesting it using a keyboard shortcut (e.g. Ctrl+Space). Typically they
    /// do so when the user starts to type an identifier. For example if the user
    /// types `c` in a JavaScript file code complete will automatically pop up
    /// present `console` besides others as a completion item. Characters that
    /// make up identifiers don't need to be listed here.
    ///
    /// If code complete should automatically be trigger on characters not being
    /// valid inside an identifier (for example `.` in JavaScript) list them in
    /// `triggerCharacters`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger_characters: Option<Vec<String>>,

    /// The list of all possible characters that commit a completion. This field
    /// can be used if clients don't support individual commit characters per
    /// completion item. See client capability
    /// `completion.completionItem.commitCharactersSupport`.
    ///
    /// If a server provides both `allCommitCharacters` and commit characters on
    /// an individual completion item the ones on the completion item win.
    ///
    /// @since 3.2.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub all_commit_characters: Option<Vec<String>>,

    #[serde(flatten)]
    pub work_done_progress_options: WorkDoneProgressOptions,

    /// The server supports the following `CompletionItem` specific
    /// capabilities.
    ///
    /// @since 3.17.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_item: Option<CompletionOptionsCompletionItem>,
}

#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionOptionsCompletionItem {
    /// The server has support for completion item label
    /// details (see also `CompletionItemLabelDetails`) when receiving
    /// a completion item in a resolve call.
    ///
    /// @since 3.17.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_details_support: Option<bool>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct CompletionRegistrationOptions {
    #[serde(flatten)]
    pub text_document_registration_options: TextDocumentRegistrationOptions,

    #[serde(flatten)]
    pub completion_options: CompletionOptions,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CompletionResponse {
    Array(Vec<CompletionItem>),
    List(CompletionList),
}

impl From<Vec<CompletionItem>> for CompletionResponse {
    fn from(items: Vec<CompletionItem>) -> Self {
        CompletionResponse::Array(items)
    }
}

impl From<CompletionList> for CompletionResponse {
    fn from(list: CompletionList) -> Self {
        CompletionResponse::List(list)
    }
}

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionParams {
    // This field was "mixed-in" from TextDocumentPositionParams
    #[serde(flatten)]
    pub text_document_position: TextDocumentPositionParams,

    #[serde(flatten)]
    pub work_done_progress_params: WorkDoneProgressParams,

    #[serde(flatten)]
    pub partial_result_params: PartialResultParams,

    // CompletionParams properties:
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<CompletionContext>,
}

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionContext {
    /// How the completion was triggered.
    pub trigger_kind: CompletionTriggerKind,

    /// The trigger character (a single character) that has trigger code complete.
    /// Is undefined if `triggerKind !== CompletionTriggerKind.TriggerCharacter`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger_character: Option<String>,
}

/// How a completion was triggered.
#[derive(Eq, PartialEq, Clone, Copy, Deserialize, Serialize)]
#[serde(transparent)]
pub struct CompletionTriggerKind(i32);
lsp_enum! {
impl CompletionTriggerKind {
    pub const INVOKED: CompletionTriggerKind = CompletionTriggerKind(1);
    pub const TRIGGER_CHARACTER: CompletionTriggerKind = CompletionTriggerKind(2);
    pub const TRIGGER_FOR_INCOMPLETE_COMPLETIONS: CompletionTriggerKind = CompletionTriggerKind(3);
}
}

/// Represents a collection of [completion items](#CompletionItem) to be presented
/// in the editor.
#[derive(Debug, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionList {
    /// This list it not complete. Further typing should result in recomputing
    /// this list.
    pub is_incomplete: bool,

    /// The completion items.
    pub items: Vec<CompletionItem>,
}

#[derive(Debug, PartialEq, Default, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CompletionItem {
    /// The label of this completion item. By default
    /// also the text that is inserted when selecting
    /// this completion.
    pub label: String,

    /// Additional details for the label
    ///
    /// @since 3.17.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_details: Option<CompletionItemLabelDetails>,

    /// The kind of this completion item. Based of the kind
    /// an icon is chosen by the editor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<CompletionItemKind>,

    /// A human-readable string with additional information
    /// about this item, like type or symbol information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,

    /// A human-readable string that represents a doc-comment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation: Option<Documentation>,

    /// Indicates if this item is deprecated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<bool>,

    /// Select this item when showing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preselect: Option<bool>,

    /// A string that should be used when comparing this item
    /// with other items. When `falsy` the label is used
    /// as the sort text for this item.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_text: Option<String>,

    /// A string that should be used when filtering a set of
    /// completion items. When `falsy` the label is used as the
    /// filter text for this item.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter_text: Option<String>,

    /// A string that should be inserted into a document when selecting
    /// this completion. When `falsy` the label is used as the insert text
    /// for this item.
    ///
    /// The `insertText` is subject to interpretation by the client side.
    /// Some tools might not take the string literally. For example
    /// VS Code when code complete is requested in this example
    /// `con<cursor position>` and a completion item with an `insertText` of
    /// `console` is provided it will only insert `sole`. Therefore it is
    /// recommended to use `textEdit` instead since it avoids additional client
    /// side interpretation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insert_text: Option<String>,

    /// The format of the insert text. The format applies to both the `insertText` property
    /// and the `newText` property of a provided `textEdit`. If omitted defaults to `InsertTextFormat.PlainText`.
    ///
    /// @since 3.16.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insert_text_format: Option<InsertTextFormat>,

    /// How whitespace and indentation is handled during completion
    /// item insertion. If not provided the client's default value depends on
    /// the `textDocument.completion.insertTextMode` client capability.
    ///
    /// @since 3.16.0
    /// @since 3.17.0 - support for `textDocument.completion.insertTextMode`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insert_text_mode: Option<InsertTextMode>,

    /// An edit which is applied to a document when selecting
    /// this completion. When an edit is provided the value of
    /// insertText is ignored.
    ///
    /// Most editors support two different operation when accepting a completion item. One is to insert a

    /// completion text and the other is to replace an existing text with a completion text. Since this can
    /// usually not predetermined by a server it can report both ranges. Clients need to signal support for
    /// `InsertReplaceEdits` via the `textDocument.completion.insertReplaceSupport` client capability
    /// property.
    ///
    /// *Note 1:* The text edit's range as well as both ranges from a insert replace edit must be a
    /// [single line] and they must contain the position at which completion has been requested.
    /// *Note 2:* If an `InsertReplaceEdit` is returned the edit's insert range must be a prefix of
    /// the edit's replace range, that means it must be contained and starting at the same position.
    ///
    /// @since 3.16.0 additional type `InsertReplaceEdit`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_edit: Option<CompletionTextEdit>,

    /// An optional array of additional text edits that are applied when
    /// selecting this completion. Edits must not overlap with the main edit
    /// nor with themselves.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_text_edits: Option<Vec<TextEdit>>,

    /// An optional command that is executed *after* inserting this completion. *Note* that
    /// additional modifications to the current document should be described with the
    /// additionalTextEdits-property.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<Command>,

    /// An optional set of characters that when pressed while this completion is
    /// active will accept it first and then type that character. *Note* that all
    /// commit characters should have `length=1` and that superfluous characters
    /// will be ignored.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_characters: Option<Vec<String>>,

    /// An data entry field that is preserved on a completion item between
    /// a completion and a completion resolve request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,

    /// Tags for this completion item.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<CompletionItemTag>>,
}

impl CompletionItem {
    /// Create a CompletionItem with the minimum possible info (label and detail).
    pub fn new_simple(label: String, detail: String) -> CompletionItem {
        CompletionItem {
            label,
            detail: Some(detail),
            ..Self::default()
        }
    }
}

/// Additional details for a completion item label.
///
/// @since 3.17.0
#[derive(Debug, PartialEq, Default, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CompletionItemLabelDetails {
    /// An optional string which is rendered less prominently directly after
    /// {@link CompletionItemLabel.label label}, without any spacing. Should be
    /// used for function signatures or type annotations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,

    /// An optional string which is rendered less prominently after
    /// {@link CompletionItemLabel.detail}. Should be used for fully qualified
    /// names or file path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::test_deserialization;

    #[test]
    fn test_tag_support_deserialization() {
        let mut empty = CompletionItemCapability::default();
        empty.tag_support = None;

        test_deserialization(r#"{}"#, &empty);
        test_deserialization(r#"{"tagSupport": false}"#, &empty);

        let mut t = CompletionItemCapability::default();
        t.tag_support = Some(TagSupport { value_set: vec![] });
        test_deserialization(r#"{"tagSupport": true}"#, &t);

        let mut t = CompletionItemCapability::default();
        t.tag_support = Some(TagSupport {
            value_set: vec![CompletionItemTag::DEPRECATED],
        });
        test_deserialization(r#"{"tagSupport": {"valueSet": [1]}}"#, &t);
    }

    #[test]
    fn test_debug_enum() {
        assert_eq!(format!("{:?}", CompletionItemKind::TEXT), "Text");
        assert_eq!(
            format!("{:?}", CompletionItemKind::TYPE_PARAMETER),
            "TypeParameter"
        );
    }

    #[test]
    fn test_try_from_enum() {
        use std::convert::TryInto;
        assert_eq!("Text".try_into(), Ok(CompletionItemKind::TEXT));
        assert_eq!(
            "TypeParameter".try_into(),
            Ok(CompletionItemKind::TYPE_PARAMETER)
        );
    }
}
