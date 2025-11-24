use std::borrow::Cow;

use serde::ser::SerializeSeq;
use serde::{Deserialize, Serialize};

use crate::{
    PartialResultParams, Range, StaticRegistrationOptions, TextDocumentIdentifier,
    TextDocumentRegistrationOptions, WorkDoneProgressOptions, WorkDoneProgressParams,
};
/// A set of predefined token types. This set is not fixed
/// and clients can specify additional token types via the
/// corresponding client capabilities.
///
/// @since 3.16.0
#[derive(Debug, Eq, PartialEq, Hash, PartialOrd, Clone, Deserialize, Serialize)]
pub struct SemanticTokenType(Cow<'static, str>);

impl SemanticTokenType {
    pub const NAMESPACE: SemanticTokenType = SemanticTokenType::new("namespace");
    pub const TYPE: SemanticTokenType = SemanticTokenType::new("type");
    pub const CLASS: SemanticTokenType = SemanticTokenType::new("class");
    pub const ENUM: SemanticTokenType = SemanticTokenType::new("enum");
    pub const INTERFACE: SemanticTokenType = SemanticTokenType::new("interface");
    pub const STRUCT: SemanticTokenType = SemanticTokenType::new("struct");
    pub const TYPE_PARAMETER: SemanticTokenType = SemanticTokenType::new("typeParameter");
    pub const PARAMETER: SemanticTokenType = SemanticTokenType::new("parameter");
    pub const VARIABLE: SemanticTokenType = SemanticTokenType::new("variable");
    pub const PROPERTY: SemanticTokenType = SemanticTokenType::new("property");
    pub const ENUM_MEMBER: SemanticTokenType = SemanticTokenType::new("enumMember");
    pub const EVENT: SemanticTokenType = SemanticTokenType::new("event");
    pub const FUNCTION: SemanticTokenType = SemanticTokenType::new("function");
    pub const METHOD: SemanticTokenType = SemanticTokenType::new("method");
    pub const MACRO: SemanticTokenType = SemanticTokenType::new("macro");
    pub const KEYWORD: SemanticTokenType = SemanticTokenType::new("keyword");
    pub const MODIFIER: SemanticTokenType = SemanticTokenType::new("modifier");
    pub const COMMENT: SemanticTokenType = SemanticTokenType::new("comment");
    pub const STRING: SemanticTokenType = SemanticTokenType::new("string");
    pub const NUMBER: SemanticTokenType = SemanticTokenType::new("number");
    pub const REGEXP: SemanticTokenType = SemanticTokenType::new("regexp");
    pub const OPERATOR: SemanticTokenType = SemanticTokenType::new("operator");

    /// @since 3.17.0
    pub const DECORATOR: SemanticTokenType = SemanticTokenType::new("decorator");

    pub const fn new(tag: &'static str) -> Self {
        SemanticTokenType(Cow::Borrowed(tag))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for SemanticTokenType {
    fn from(from: String) -> Self {
        SemanticTokenType(Cow::from(from))
    }
}

impl From<&'static str> for SemanticTokenType {
    fn from(from: &'static str) -> Self {
        SemanticTokenType::new(from)
    }
}

/// A set of predefined token modifiers. This set is not fixed
/// and clients can specify additional token types via the
/// corresponding client capabilities.
///
/// @since 3.16.0
#[derive(Debug, Eq, PartialEq, Hash, PartialOrd, Clone, Deserialize, Serialize)]
pub struct SemanticTokenModifier(Cow<'static, str>);

impl SemanticTokenModifier {
    pub const DECLARATION: SemanticTokenModifier = SemanticTokenModifier::new("declaration");
    pub const DEFINITION: SemanticTokenModifier = SemanticTokenModifier::new("definition");
    pub const READONLY: SemanticTokenModifier = SemanticTokenModifier::new("readonly");
    pub const STATIC: SemanticTokenModifier = SemanticTokenModifier::new("static");
    pub const DEPRECATED: SemanticTokenModifier = SemanticTokenModifier::new("deprecated");
    pub const ABSTRACT: SemanticTokenModifier = SemanticTokenModifier::new("abstract");
    pub const ASYNC: SemanticTokenModifier = SemanticTokenModifier::new("async");
    pub const MODIFICATION: SemanticTokenModifier = SemanticTokenModifier::new("modification");
    pub const DOCUMENTATION: SemanticTokenModifier = SemanticTokenModifier::new("documentation");
    pub const DEFAULT_LIBRARY: SemanticTokenModifier = SemanticTokenModifier::new("defaultLibrary");

    pub const fn new(tag: &'static str) -> Self {
        SemanticTokenModifier(Cow::Borrowed(tag))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for SemanticTokenModifier {
    fn from(from: String) -> Self {
        SemanticTokenModifier(Cow::from(from))
    }
}

impl From<&'static str> for SemanticTokenModifier {
    fn from(from: &'static str) -> Self {
        SemanticTokenModifier::new(from)
    }
}

#[derive(Debug, Eq, PartialEq, Hash, PartialOrd, Clone, Deserialize, Serialize)]
pub struct TokenFormat(Cow<'static, str>);

impl TokenFormat {
    pub const RELATIVE: TokenFormat = TokenFormat::new("relative");

    pub const fn new(tag: &'static str) -> Self {
        TokenFormat(Cow::Borrowed(tag))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for TokenFormat {
    fn from(from: String) -> Self {
        TokenFormat(Cow::from(from))
    }
}

impl From<&'static str> for TokenFormat {
    fn from(from: &'static str) -> Self {
        TokenFormat::new(from)
    }
}

/// @since 3.16.0
#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticTokensLegend {
    /// The token types a server uses.
    pub token_types: Vec<SemanticTokenType>,

    /// The token modifiers a server uses.
    pub token_modifiers: Vec<SemanticTokenModifier>,
}

/// The actual tokens.
#[derive(Debug, Eq, PartialEq, Copy, Clone, Default)]
pub struct SemanticToken {
    pub delta_line: u32,
    pub delta_start: u32,
    pub length: u32,
    pub token_type: u32,
    pub token_modifiers_bitset: u32,
}

impl SemanticToken {
    fn deserialize_tokens<'de, D>(deserializer: D) -> Result<Vec<SemanticToken>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let data = Vec::<u32>::deserialize(deserializer)?;
        let chunks = data.chunks_exact(5);

        if !chunks.remainder().is_empty() {
            return Result::Err(serde::de::Error::custom("Length is not divisible by 5"));
        }

        Result::Ok(
            chunks
                .map(|chunk| SemanticToken {
                    delta_line: chunk[0],
                    delta_start: chunk[1],
                    length: chunk[2],
                    token_type: chunk[3],
                    token_modifiers_bitset: chunk[4],
                })
                .collect(),
        )
    }

    fn serialize_tokens<S>(tokens: &[SemanticToken], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(tokens.len() * 5))?;
        for token in tokens.iter() {
            seq.serialize_element(&token.delta_line)?;
            seq.serialize_element(&token.delta_start)?;
            seq.serialize_element(&token.length)?;
            seq.serialize_element(&token.token_type)?;
            seq.serialize_element(&token.token_modifiers_bitset)?;
        }
        seq.end()
    }

    fn deserialize_tokens_opt<'de, D>(
        deserializer: D,
    ) -> Result<Option<Vec<SemanticToken>>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(transparent)]
        struct Wrapper {
            #[serde(deserialize_with = "SemanticToken::deserialize_tokens")]
            tokens: Vec<SemanticToken>,
        }

        Ok(Option::<Wrapper>::deserialize(deserializer)?.map(|wrapper| wrapper.tokens))
    }

    fn serialize_tokens_opt<S>(
        data: &Option<Vec<SemanticToken>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        #[serde(transparent)]
        struct Wrapper {
            #[serde(serialize_with = "SemanticToken::serialize_tokens")]
            tokens: Vec<SemanticToken>,
        }

        let opt = data.as_ref().map(|t| Wrapper { tokens: t.to_vec() });

        opt.serialize(serializer)
    }
}

/// @since 3.16.0
#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticTokens {
    /// An optional result id. If provided and clients support delta updating
    /// the client will include the result id in the next semantic token request.
    /// A server can then instead of computing all semantic tokens again simply
    /// send a delta.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_id: Option<String>,

    /// The actual tokens. For a detailed description about how the data is
    /// structured please see
    /// <https://github.com/microsoft/vscode-extension-samples/blob/5ae1f7787122812dcc84e37427ca90af5ee09f14/semantic-tokens-sample/vscode.proposed.d.ts#L71>
    #[serde(
        deserialize_with = "SemanticToken::deserialize_tokens",
        serialize_with = "SemanticToken::serialize_tokens"
    )]
    pub data: Vec<SemanticToken>,
}

/// @since 3.16.0
#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticTokensPartialResult {
    #[serde(
        deserialize_with = "SemanticToken::deserialize_tokens",
        serialize_with = "SemanticToken::serialize_tokens"
    )]
    pub data: Vec<SemanticToken>,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum SemanticTokensResult {
    Tokens(SemanticTokens),
    Partial(SemanticTokensPartialResult),
}

impl From<SemanticTokens> for SemanticTokensResult {
    fn from(from: SemanticTokens) -> Self {
        SemanticTokensResult::Tokens(from)
    }
}

impl From<SemanticTokensPartialResult> for SemanticTokensResult {
    fn from(from: SemanticTokensPartialResult) -> Self {
        SemanticTokensResult::Partial(from)
    }
}

/// @since 3.16.0
#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticTokensEdit {
    pub start: u32,
    pub delete_count: u32,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "SemanticToken::deserialize_tokens_opt",
        serialize_with = "SemanticToken::serialize_tokens_opt"
    )]
    pub data: Option<Vec<SemanticToken>>,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum SemanticTokensFullDeltaResult {
    Tokens(SemanticTokens),
    TokensDelta(SemanticTokensDelta),
    PartialTokensDelta { edits: Vec<SemanticTokensEdit> },
}

impl From<SemanticTokens> for SemanticTokensFullDeltaResult {
    fn from(from: SemanticTokens) -> Self {
        SemanticTokensFullDeltaResult::Tokens(from)
    }
}

impl From<SemanticTokensDelta> for SemanticTokensFullDeltaResult {
    fn from(from: SemanticTokensDelta) -> Self {
        SemanticTokensFullDeltaResult::TokensDelta(from)
    }
}

/// @since 3.16.0
#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticTokensDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_id: Option<String>,
    /// For a detailed description how these edits are structured please see
    /// <https://github.com/microsoft/vscode-extension-samples/blob/5ae1f7787122812dcc84e37427ca90af5ee09f14/semantic-tokens-sample/vscode.proposed.d.ts#L131>
    pub edits: Vec<SemanticTokensEdit>,
}

/// Capabilities specific to the `textDocument/semanticTokens/*` requests.
///
/// @since 3.16.0
#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticTokensClientCapabilities {
    /// Whether implementation supports dynamic registration. If this is set to `true`
    /// the client supports the new `(TextDocumentRegistrationOptions & StaticRegistrationOptions)`
    /// return value for the corresponding server capability as well.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_registration: Option<bool>,

    /// Which requests the client supports and might send to the server
    /// depending on the server's capability. Please note that clients might not
    /// show semantic tokens or degrade some of the user experience if a range
    /// or full request is advertised by the client but not provided by the
    /// server. If for example the client capability `requests.full` and
    /// `request.range` are both set to true but the server only provides a
    /// range provider the client might not render a minimap correctly or might
    /// even decide to not show any semantic tokens at all.
    pub requests: SemanticTokensClientCapabilitiesRequests,

    /// The token types that the client supports.
    pub token_types: Vec<SemanticTokenType>,

    /// The token modifiers that the client supports.
    pub token_modifiers: Vec<SemanticTokenModifier>,

    /// The token formats the clients supports.
    pub formats: Vec<TokenFormat>,

    /// Whether the client supports tokens that can overlap each other.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overlapping_token_support: Option<bool>,

    /// Whether the client supports tokens that can span multiple lines.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multiline_token_support: Option<bool>,

    /// Whether the client allows the server to actively cancel a
    /// semantic token request, e.g. supports returning
    /// ErrorCodes.ServerCancelled. If a server does the client
    /// needs to retrigger the request.
    ///
    /// @since 3.17.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_cancel_support: Option<bool>,

    /// Whether the client uses semantic tokens to augment existing
    /// syntax tokens. If set to `true` client side created syntax
    /// tokens and semantic tokens are both used for colorization. If
    /// set to `false` the client only uses the returned semantic tokens
    /// for colorization.
    ///
    /// If the value is `undefined` then the client behavior is not
    /// specified.
    ///
    /// @since 3.17.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub augments_syntax_tokens: Option<bool>,
}

#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticTokensClientCapabilitiesRequests {
    /// The client will send the `textDocument/semanticTokens/range` request if the server provides a corresponding handler.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<bool>,

    /// The client will send the `textDocument/semanticTokens/full` request if the server provides a corresponding handler.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full: Option<SemanticTokensFullOptions>,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum SemanticTokensFullOptions {
    Bool(bool),
    Delta {
        /// The client will send the `textDocument/semanticTokens/full/delta` request if the server provides a corresponding handler.
        /// The server supports deltas for full documents.
        #[serde(skip_serializing_if = "Option::is_none")]
        delta: Option<bool>,
    },
}

/// @since 3.16.0
#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticTokensOptions {
    #[serde(flatten)]
    pub work_done_progress_options: WorkDoneProgressOptions,

    /// The legend used by the server
    pub legend: SemanticTokensLegend,

    /// Server supports providing semantic tokens for a specific range
    /// of a document.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<bool>,

    /// Server supports providing semantic tokens for a full document.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full: Option<SemanticTokensFullOptions>,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticTokensRegistrationOptions {
    #[serde(flatten)]
    pub text_document_registration_options: TextDocumentRegistrationOptions,

    #[serde(flatten)]
    pub semantic_tokens_options: SemanticTokensOptions,

    #[serde(flatten)]
    pub static_registration_options: StaticRegistrationOptions,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum SemanticTokensServerCapabilities {
    SemanticTokensOptions(SemanticTokensOptions),
    SemanticTokensRegistrationOptions(SemanticTokensRegistrationOptions),
}

impl From<SemanticTokensOptions> for SemanticTokensServerCapabilities {
    fn from(from: SemanticTokensOptions) -> Self {
        SemanticTokensServerCapabilities::SemanticTokensOptions(from)
    }
}

impl From<SemanticTokensRegistrationOptions> for SemanticTokensServerCapabilities {
    fn from(from: SemanticTokensRegistrationOptions) -> Self {
        SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(from)
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticTokensWorkspaceClientCapabilities {
    /// Whether the client implementation supports a refresh request sent from
    /// the server to the client.
    ///
    /// Note that this event is global and will force the client to refresh all
    /// semantic tokens currently shown. It should be used with absolute care
    /// and is useful for situation where a server for example detect a project
    /// wide change that requires such a calculation.
    pub refresh_support: Option<bool>,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticTokensParams {
    #[serde(flatten)]
    pub work_done_progress_params: WorkDoneProgressParams,

    #[serde(flatten)]
    pub partial_result_params: PartialResultParams,

    /// The text document.
    pub text_document: TextDocumentIdentifier,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticTokensDeltaParams {
    #[serde(flatten)]
    pub work_done_progress_params: WorkDoneProgressParams,

    #[serde(flatten)]
    pub partial_result_params: PartialResultParams,

    /// The text document.
    pub text_document: TextDocumentIdentifier,

    /// The result id of a previous response. The result Id can either point to a full response
    /// or a delta response depending on what was received last.
    pub previous_result_id: String,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticTokensRangeParams {
    #[serde(flatten)]
    pub work_done_progress_params: WorkDoneProgressParams,

    #[serde(flatten)]
    pub partial_result_params: PartialResultParams,

    /// The text document.
    pub text_document: TextDocumentIdentifier,

    /// The range the semantic tokens are requested for.
    pub range: Range,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum SemanticTokensRangeResult {
    Tokens(SemanticTokens),
    Partial(SemanticTokensPartialResult),
}

impl From<SemanticTokens> for SemanticTokensRangeResult {
    fn from(tokens: SemanticTokens) -> Self {
        SemanticTokensRangeResult::Tokens(tokens)
    }
}

impl From<SemanticTokensPartialResult> for SemanticTokensRangeResult {
    fn from(partial: SemanticTokensPartialResult) -> Self {
        SemanticTokensRangeResult::Partial(partial)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{test_deserialization, test_serialization};

    #[test]
    fn test_semantic_tokens_support_serialization() {
        test_serialization(
            &SemanticTokens {
                result_id: None,
                data: vec![],
            },
            r#"{"data":[]}"#,
        );

        test_serialization(
            &SemanticTokens {
                result_id: None,
                data: vec![SemanticToken {
                    delta_line: 2,
                    delta_start: 5,
                    length: 3,
                    token_type: 0,
                    token_modifiers_bitset: 3,
                }],
            },
            r#"{"data":[2,5,3,0,3]}"#,
        );

        test_serialization(
            &SemanticTokens {
                result_id: None,
                data: vec![
                    SemanticToken {
                        delta_line: 2,
                        delta_start: 5,
                        length: 3,
                        token_type: 0,
                        token_modifiers_bitset: 3,
                    },
                    SemanticToken {
                        delta_line: 0,
                        delta_start: 5,
                        length: 4,
                        token_type: 1,
                        token_modifiers_bitset: 0,
                    },
                ],
            },
            r#"{"data":[2,5,3,0,3,0,5,4,1,0]}"#,
        );
    }

    #[test]
    fn test_semantic_tokens_support_deserialization() {
        test_deserialization(
            r#"{"data":[]}"#,
            &SemanticTokens {
                result_id: None,
                data: vec![],
            },
        );

        test_deserialization(
            r#"{"data":[2,5,3,0,3]}"#,
            &SemanticTokens {
                result_id: None,
                data: vec![SemanticToken {
                    delta_line: 2,
                    delta_start: 5,
                    length: 3,
                    token_type: 0,
                    token_modifiers_bitset: 3,
                }],
            },
        );

        test_deserialization(
            r#"{"data":[2,5,3,0,3,0,5,4,1,0]}"#,
            &SemanticTokens {
                result_id: None,
                data: vec![
                    SemanticToken {
                        delta_line: 2,
                        delta_start: 5,
                        length: 3,
                        token_type: 0,
                        token_modifiers_bitset: 3,
                    },
                    SemanticToken {
                        delta_line: 0,
                        delta_start: 5,
                        length: 4,
                        token_type: 1,
                        token_modifiers_bitset: 0,
                    },
                ],
            },
        );
    }

    #[test]
    #[should_panic]
    fn test_semantic_tokens_support_deserialization_err() {
        test_deserialization(
            r#"{"data":[1]}"#,
            &SemanticTokens {
                result_id: None,
                data: vec![],
            },
        );
    }

    #[test]
    fn test_semantic_tokens_edit_support_deserialization() {
        test_deserialization(
            r#"{"start":0,"deleteCount":1,"data":[2,5,3,0,3,0,5,4,1,0]}"#,
            &SemanticTokensEdit {
                start: 0,
                delete_count: 1,
                data: Some(vec![
                    SemanticToken {
                        delta_line: 2,
                        delta_start: 5,
                        length: 3,
                        token_type: 0,
                        token_modifiers_bitset: 3,
                    },
                    SemanticToken {
                        delta_line: 0,
                        delta_start: 5,
                        length: 4,
                        token_type: 1,
                        token_modifiers_bitset: 0,
                    },
                ]),
            },
        );

        test_deserialization(
            r#"{"start":0,"deleteCount":1}"#,
            &SemanticTokensEdit {
                start: 0,
                delete_count: 1,
                data: None,
            },
        );
    }

    #[test]
    fn test_semantic_tokens_edit_support_serialization() {
        test_serialization(
            &SemanticTokensEdit {
                start: 0,
                delete_count: 1,
                data: Some(vec![
                    SemanticToken {
                        delta_line: 2,
                        delta_start: 5,
                        length: 3,
                        token_type: 0,
                        token_modifiers_bitset: 3,
                    },
                    SemanticToken {
                        delta_line: 0,
                        delta_start: 5,
                        length: 4,
                        token_type: 1,
                        token_modifiers_bitset: 0,
                    },
                ]),
            },
            r#"{"start":0,"deleteCount":1,"data":[2,5,3,0,3,0,5,4,1,0]}"#,
        );

        test_serialization(
            &SemanticTokensEdit {
                start: 0,
                delete_count: 1,
                data: None,
            },
            r#"{"start":0,"deleteCount":1}"#,
        );
    }
}
