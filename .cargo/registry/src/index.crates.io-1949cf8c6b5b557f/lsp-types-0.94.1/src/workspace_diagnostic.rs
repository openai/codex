use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    FullDocumentDiagnosticReport, PartialResultParams, UnchangedDocumentDiagnosticReport,
    WorkDoneProgressParams,
};

/// Workspace client capabilities specific to diagnostic pull requests.
///
/// @since 3.17.0
#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticWorkspaceClientCapabilities {
    /// Whether the client implementation supports a refresh request sent from
    /// the server to the client.
    ///
    /// Note that this event is global and will force the client to refresh all
    /// pulled diagnostics currently shown. It should be used with absolute care
    /// and is useful for situation where a server for example detects a project
    /// wide change that requires such a calculation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_support: Option<bool>,
}

/// A previous result ID in a workspace pull request.
///
/// @since 3.17.0
#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub struct PreviousResultId {
    /// The URI for which the client knows a result ID.
    pub uri: Url,

    /// The value of the previous result ID.
    pub value: String,
}

/// Parameters of the workspace diagnostic request.
///
/// @since 3.17.0
#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDiagnosticParams {
    /// The additional identifier provided during registration.
    pub identifier: Option<String>,

    /// The currently known diagnostic reports with their
    /// previous result ids.
    pub previous_result_ids: Vec<PreviousResultId>,

    #[serde(flatten)]
    pub work_done_progress_params: WorkDoneProgressParams,

    #[serde(flatten)]
    pub partial_result_params: PartialResultParams,
}

/// A full document diagnostic report for a workspace diagnostic result.
///
/// @since 3.17.0
#[derive(Debug, PartialEq, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFullDocumentDiagnosticReport {
    /// The URI for which diagnostic information is reported.
    pub uri: Url,

    /// The version number for which the diagnostics are reported.
    ///
    /// If the document is not marked as open, `None` can be provided.
    pub version: Option<i64>,

    #[serde(flatten)]
    pub full_document_diagnostic_report: FullDocumentDiagnosticReport,
}

/// An unchanged document diagnostic report for a workspace diagnostic result.
///
/// @since 3.17.0
#[derive(Debug, PartialEq, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceUnchangedDocumentDiagnosticReport {
    /// The URI for which diagnostic information is reported.
    pub uri: Url,

    /// The version number for which the diagnostics are reported.
    ///
    /// If the document is not marked as open, `None` can be provided.
    pub version: Option<i64>,

    #[serde(flatten)]
    pub unchanged_document_diagnostic_report: UnchangedDocumentDiagnosticReport,
}

/// A workspace diagnostic document report.
///
/// @since 3.17.0
#[derive(Debug, PartialEq, Deserialize, Serialize, Clone)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum WorkspaceDocumentDiagnosticReport {
    Full(WorkspaceFullDocumentDiagnosticReport),
    Unchanged(WorkspaceUnchangedDocumentDiagnosticReport),
}

impl From<WorkspaceFullDocumentDiagnosticReport> for WorkspaceDocumentDiagnosticReport {
    fn from(from: WorkspaceFullDocumentDiagnosticReport) -> Self {
        WorkspaceDocumentDiagnosticReport::Full(from)
    }
}

impl From<WorkspaceUnchangedDocumentDiagnosticReport> for WorkspaceDocumentDiagnosticReport {
    fn from(from: WorkspaceUnchangedDocumentDiagnosticReport) -> Self {
        WorkspaceDocumentDiagnosticReport::Unchanged(from)
    }
}

/// A workspace diagnostic report.
///
/// @since 3.17.0
#[derive(Debug, PartialEq, Default, Deserialize, Serialize, Clone)]
pub struct WorkspaceDiagnosticReport {
    pub items: Vec<WorkspaceDocumentDiagnosticReport>,
}

/// A partial result for a workspace diagnostic report.
///
/// @since 3.17.0
#[derive(Debug, PartialEq, Default, Deserialize, Serialize, Clone)]
pub struct WorkspaceDiagnosticReportPartialResult {
    pub items: Vec<WorkspaceDocumentDiagnosticReport>,
}

#[derive(Debug, PartialEq, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum WorkspaceDiagnosticReportResult {
    Report(WorkspaceDiagnosticReport),
    Partial(WorkspaceDiagnosticReportPartialResult),
}

impl From<WorkspaceDiagnosticReport> for WorkspaceDiagnosticReportResult {
    fn from(from: WorkspaceDiagnosticReport) -> Self {
        WorkspaceDiagnosticReportResult::Report(from)
    }
}

impl From<WorkspaceDiagnosticReportPartialResult> for WorkspaceDiagnosticReportResult {
    fn from(from: WorkspaceDiagnosticReportPartialResult) -> Self {
        WorkspaceDiagnosticReportResult::Partial(from)
    }
}
