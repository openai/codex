use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    Diagnostic, PartialResultParams, StaticRegistrationOptions, TextDocumentIdentifier,
    TextDocumentRegistrationOptions, WorkDoneProgressOptions, WorkDoneProgressParams,
};

/// Client capabilities specific to diagnostic pull requests.
///
/// @since 3.17.0
#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticClientCapabilities {
    /// Whether implementation supports dynamic registration.
    ///
    /// If this is set to `true` the client supports the new `(TextDocumentRegistrationOptions &
    /// StaticRegistrationOptions)` return value for the corresponding server capability as well.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_registration: Option<bool>,

    /// Whether the clients supports related documents for document diagnostic pulls.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_document_support: Option<bool>,
}

/// Diagnostic options.
///
/// @since 3.17.0
#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticOptions {
    /// An optional identifier under which the diagnostics are
    /// managed by the client.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identifier: Option<String>,

    /// Whether the language has inter file dependencies, meaning that editing code in one file can
    /// result in a different diagnostic set in another file. Inter file dependencies are common
    /// for most programming languages and typically uncommon for linters.
    pub inter_file_dependencies: bool,

    /// The server provides support for workspace diagnostics as well.
    pub workspace_diagnostics: bool,

    #[serde(flatten)]
    pub work_done_progress_options: WorkDoneProgressOptions,
}

/// Diagnostic registration options.
///
/// @since 3.17.0
#[derive(Debug, Eq, PartialEq, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticRegistrationOptions {
    #[serde(flatten)]
    pub text_document_registration_options: TextDocumentRegistrationOptions,

    #[serde(flatten)]
    pub diagnostic_options: DiagnosticOptions,

    #[serde(flatten)]
    pub static_registration_options: StaticRegistrationOptions,
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum DiagnosticServerCapabilities {
    Options(DiagnosticOptions),
    RegistrationOptions(DiagnosticRegistrationOptions),
}

/// Parameters of the document diagnostic request.
///
/// @since 3.17.0
#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentDiagnosticParams {
    /// The text document.
    pub text_document: TextDocumentIdentifier,

    /// The additional identifier provided during registration.
    pub identifier: Option<String>,

    /// The result ID of a previous response if provided.
    pub previous_result_id: Option<String>,

    #[serde(flatten)]
    pub work_done_progress_params: WorkDoneProgressParams,

    #[serde(flatten)]
    pub partial_result_params: PartialResultParams,
}

/// A diagnostic report with a full set of problems.
///
/// @since 3.17.0
#[derive(Debug, PartialEq, Default, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FullDocumentDiagnosticReport {
    /// An optional result ID. If provided it will be sent on the next diagnostic request for the
    /// same document.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_id: Option<String>,

    /// The actual items.
    pub items: Vec<Diagnostic>,
}

/// A diagnostic report indicating that the last returned report is still accurate.
///
/// A server can only return `unchanged` if result ids are provided.
///
/// @since 3.17.0
#[derive(Debug, PartialEq, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UnchangedDocumentDiagnosticReport {
    /// A result ID which will be sent on the next diagnostic request for the same document.
    pub result_id: String,
}

/// The document diagnostic report kinds.
///
/// @since 3.17.0
#[derive(Debug, PartialEq, Deserialize, Serialize, Clone)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum DocumentDiagnosticReportKind {
    /// A diagnostic report with a full set of problems.
    Full(FullDocumentDiagnosticReport),
    /// A report indicating that the last returned report is still accurate.
    Unchanged(UnchangedDocumentDiagnosticReport),
}

impl From<FullDocumentDiagnosticReport> for DocumentDiagnosticReportKind {
    fn from(from: FullDocumentDiagnosticReport) -> Self {
        DocumentDiagnosticReportKind::Full(from)
    }
}

impl From<UnchangedDocumentDiagnosticReport> for DocumentDiagnosticReportKind {
    fn from(from: UnchangedDocumentDiagnosticReport) -> Self {
        DocumentDiagnosticReportKind::Unchanged(from)
    }
}

/// A full diagnostic report with a set of related documents.
///
/// @since 3.17.0
#[derive(Debug, PartialEq, Default, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RelatedFullDocumentDiagnosticReport {
    /// Diagnostics of related documents.
    ///
    /// This information is useful in programming languages where code in a file A can generate
    /// diagnostics in a file B which A depends on. An example of such a language is C/C++ where
    /// macro definitions in a file `a.cpp` result in errors in a header file `b.hpp`.
    ///
    /// @since 3.17.0
    #[serde(with = "crate::url_map")]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub related_documents: Option<HashMap<Url, DocumentDiagnosticReportKind>>,
    // relatedDocuments?: { [uri: string]: FullDocumentDiagnosticReport | UnchangedDocumentDiagnosticReport; };
    #[serde(flatten)]
    pub full_document_diagnostic_report: FullDocumentDiagnosticReport,
}

/// An unchanged diagnostic report with a set of related documents.
///
/// @since 3.17.0
#[derive(Debug, PartialEq, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RelatedUnchangedDocumentDiagnosticReport {
    /// Diagnostics of related documents.
    ///
    /// This information is useful in programming languages where code in a file A can generate
    /// diagnostics in a file B which A depends on. An example of such a language is C/C++ where
    /// macro definitions in a file `a.cpp` result in errors in a header file `b.hpp`.
    ///
    /// @since 3.17.0
    #[serde(with = "crate::url_map")]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub related_documents: Option<HashMap<Url, DocumentDiagnosticReportKind>>,
    // relatedDocuments?: { [uri: string]: FullDocumentDiagnosticReport | UnchangedDocumentDiagnosticReport; };
    #[serde(flatten)]
    pub unchanged_document_diagnostic_report: UnchangedDocumentDiagnosticReport,
}

/// The result of a document diagnostic pull request.
///
/// A report can either be a full report containing all diagnostics for the requested document or
/// an unchanged report indicating that nothing has changed in terms of diagnostics in comparison
/// to the last pull request.
///
/// @since 3.17.0
#[derive(Debug, PartialEq, Deserialize, Serialize, Clone)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum DocumentDiagnosticReport {
    /// A diagnostic report with a full set of problems.
    Full(RelatedFullDocumentDiagnosticReport),
    /// A report indicating that the last returned report is still accurate.
    Unchanged(RelatedUnchangedDocumentDiagnosticReport),
}

impl From<RelatedFullDocumentDiagnosticReport> for DocumentDiagnosticReport {
    fn from(from: RelatedFullDocumentDiagnosticReport) -> Self {
        DocumentDiagnosticReport::Full(from)
    }
}

impl From<RelatedUnchangedDocumentDiagnosticReport> for DocumentDiagnosticReport {
    fn from(from: RelatedUnchangedDocumentDiagnosticReport) -> Self {
        DocumentDiagnosticReport::Unchanged(from)
    }
}

/// A partial result for a document diagnostic report.
///
/// @since 3.17.0
#[derive(Debug, PartialEq, Default, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DocumentDiagnosticReportPartialResult {
    #[serde(with = "crate::url_map")]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub related_documents: Option<HashMap<Url, DocumentDiagnosticReportKind>>,
    // relatedDocuments?: { [uri: string]: FullDocumentDiagnosticReport | UnchangedDocumentDiagnosticReport; };
}

#[derive(Debug, PartialEq, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum DocumentDiagnosticReportResult {
    Report(DocumentDiagnosticReport),
    Partial(DocumentDiagnosticReportPartialResult),
}

impl From<DocumentDiagnosticReport> for DocumentDiagnosticReportResult {
    fn from(from: DocumentDiagnosticReport) -> Self {
        DocumentDiagnosticReportResult::Report(from)
    }
}

impl From<DocumentDiagnosticReportPartialResult> for DocumentDiagnosticReportResult {
    fn from(from: DocumentDiagnosticReportPartialResult) -> Self {
        DocumentDiagnosticReportResult::Partial(from)
    }
}

/// Cancellation data returned from a diagnostic request.
///
/// If no data is provided, it defaults to `{ retrigger_request: true }`.
///
/// @since 3.17.0
#[derive(Debug, PartialEq, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticServerCancellationData {
    pub retrigger_request: bool,
}

impl Default for DiagnosticServerCancellationData {
    fn default() -> Self {
        DiagnosticServerCancellationData {
            retrigger_request: true,
        }
    }
}
