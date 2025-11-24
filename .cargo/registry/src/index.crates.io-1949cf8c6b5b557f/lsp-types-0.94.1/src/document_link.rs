use crate::{
    PartialResultParams, Range, TextDocumentIdentifier, WorkDoneProgressOptions,
    WorkDoneProgressParams,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentLinkClientCapabilities {
    /// Whether document link supports dynamic registration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_registration: Option<bool>,

    /// Whether the client support the `tooltip` property on `DocumentLink`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tooltip_support: Option<bool>,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentLinkOptions {
    /// Document links have a resolve provider as well.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolve_provider: Option<bool>,

    #[serde(flatten)]
    pub work_done_progress_options: WorkDoneProgressOptions,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentLinkParams {
    /// The document to provide document links for.
    pub text_document: TextDocumentIdentifier,

    #[serde(flatten)]
    pub work_done_progress_params: WorkDoneProgressParams,

    #[serde(flatten)]
    pub partial_result_params: PartialResultParams,
}

/// A document link is a range in a text document that links to an internal or external resource, like another
/// text document or a web site.
#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub struct DocumentLink {
    /// The range this link applies to.
    pub range: Range,
    /// The uri this link points to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<Url>,

    /// The tooltip text when you hover over this link.
    ///
    /// If a tooltip is provided, is will be displayed in a string that includes instructions on how to
    /// trigger the link, such as `{0} (ctrl + click)`. The specific instructions vary depending on OS,
    /// user settings, and localization.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tooltip: Option<String>,

    /// A data entry field that is preserved on a document link between a DocumentLinkRequest
    /// and a DocumentLinkResolveRequest.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}
