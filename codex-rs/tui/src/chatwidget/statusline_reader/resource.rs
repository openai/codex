//! Classify provider failures without carrying raw RPC errors or resource content.

use super::UiTree;
use codex_app_server_client::TypedRequestError;
use codex_app_server_protocol::McpResourceContent;

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub(super) enum ReadIssue {
    #[error("timed out after 1 second")]
    TimedOut,
    #[error("failed in transport")]
    Transport,
    #[error("was rejected with code {0}")]
    Rejected(i64),
    #[error("returned an invalid app-server response")]
    Response,
    #[error("expected exactly one text resource")]
    Envelope,
    #[error("resource URI differs from the advertised URI")]
    Uri,
    #[error("expected application/json on the resource")]
    Mime,
    #[error("resource text is not a valid, bounded UI tree")]
    Tree,
}

impl From<TypedRequestError> for ReadIssue {
    fn from(error: TypedRequestError) -> Self {
        match error {
            TypedRequestError::Transport { .. } => Self::Transport,
            TypedRequestError::Server { source, .. } => Self::Rejected(source.code),
            TypedRequestError::Deserialize { .. } => Self::Response,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub(super) enum Failure {
    #[error("The selected provider is unavailable.")]
    LookupUnavailable,
    #[error("The selected provider lookup timed out.")]
    LookupTimedOut,
    #[error("The displayed thread could not supply its current status line state.")]
    Snapshot,
    #[error("Resource read: {0}.")]
    Read(#[from] ReadIssue),
}

pub(super) fn decode_tree(uri: &str, contents: &[McpResourceContent]) -> Result<UiTree, ReadIssue> {
    let [
        McpResourceContent::Text {
            uri: returned_uri,
            mime_type,
            text,
            ..
        },
    ] = contents
    else {
        return Err(ReadIssue::Envelope);
    };
    if uri != returned_uri {
        return Err(ReadIssue::Uri);
    }
    let media_type = mime_type
        .as_deref()
        .unwrap_or_default()
        .split(';')
        .next()
        .unwrap_or_default();
    if !media_type.trim().eq_ignore_ascii_case("application/json") {
        return Err(ReadIssue::Mime);
    }
    text.parse().map_err(|_| ReadIssue::Tree)
}
