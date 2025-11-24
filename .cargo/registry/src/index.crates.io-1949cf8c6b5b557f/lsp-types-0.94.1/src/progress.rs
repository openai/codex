use serde::{Deserialize, Serialize};

use crate::NumberOrString;

pub type ProgressToken = NumberOrString;

/// The progress notification is sent from the server to the client to ask
/// the client to indicate progress.
#[derive(Debug, PartialEq, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProgressParams {
    /// The progress token provided by the client.
    pub token: ProgressToken,

    /// The progress data.
    pub value: ProgressParamsValue,
}

#[derive(Debug, PartialEq, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum ProgressParamsValue {
    WorkDone(WorkDoneProgress),
}

/// The `window/workDoneProgress/create` request is sent
/// from the server to the client to ask the client to create a work done progress.
#[derive(Debug, PartialEq, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WorkDoneProgressCreateParams {
    /// The token to be used to report progress.
    pub token: ProgressToken,
}

/// The `window/workDoneProgress/cancel` notification is sent from the client
/// to the server to cancel a progress initiated on the server side using the `window/workDoneProgress/create`.
#[derive(Debug, PartialEq, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WorkDoneProgressCancelParams {
    /// The token to be used to report progress.
    pub token: ProgressToken,
}

/// Options to signal work done progress support in server capabilities.
#[derive(Debug, Eq, PartialEq, Default, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WorkDoneProgressOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub work_done_progress: Option<bool>,
}

/// An optional token that a server can use to report work done progress
#[derive(Debug, Eq, PartialEq, Default, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WorkDoneProgressParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub work_done_token: Option<ProgressToken>,
}

#[derive(Debug, PartialEq, Default, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WorkDoneProgressBegin {
    /// Mandatory title of the progress operation. Used to briefly inform
    /// about the kind of operation being performed.
    /// Examples: "Indexing" or "Linking dependencies".
    pub title: String,

    /// Controls if a cancel button should show to allow the user to cancel the
    /// long running operation. Clients that don't support cancellation are allowed
    /// to ignore the setting.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancellable: Option<bool>,

    /// Optional, more detailed associated progress message. Contains
    /// complementary information to the `title`.
    ///
    /// Examples: "3/25 files", "project/src/module2", "node_modules/some_dep".
    /// If unset, the previous progress message (if any) is still valid.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    /// Optional progress percentage to display (value 100 is considered 100%).
    /// If not provided infinite progress is assumed and clients are allowed
    /// to ignore the `percentage` value in subsequent in report notifications.
    ///
    /// The value should be steadily rising. Clients are free to ignore values
    /// that are not following this rule. The value range is [0, 100]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percentage: Option<u32>,
}

#[derive(Debug, PartialEq, Default, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WorkDoneProgressReport {
    /// Controls if a cancel button should show to allow the user to cancel the
    /// long running operation. Clients that don't support cancellation are allowed
    /// to ignore the setting.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancellable: Option<bool>,

    /// Optional, more detailed associated progress message. Contains
    /// complementary information to the `title`.
    /// Examples: "3/25 files", "project/src/module2", "node_modules/some_dep".
    /// If unset, the previous progress message (if any) is still valid.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    /// Optional progress percentage to display (value 100 is considered 100%).
    /// If not provided infinite progress is assumed and clients are allowed
    /// to ignore the `percentage` value in subsequent in report notifications.
    ///
    /// The value should be steadily rising. Clients are free to ignore values
    /// that are not following this rule. The value range is [0, 100]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percentage: Option<u32>,
}

#[derive(Debug, PartialEq, Default, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WorkDoneProgressEnd {
    /// Optional, more detailed associated progress message. Contains
    /// complementary information to the `title`.
    /// Examples: "3/25 files", "project/src/module2", "node_modules/some_dep".
    /// If unset, the previous progress message (if any) is still valid.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, PartialEq, Deserialize, Serialize, Clone)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum WorkDoneProgress {
    Begin(WorkDoneProgressBegin),
    Report(WorkDoneProgressReport),
    End(WorkDoneProgressEnd),
}
