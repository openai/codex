//! Shared Guardian transcript encoding selected by configuration and used by evidence consumers.

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

/// Transcript encoding shared by rendering, provenance instructions and budget recovery.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptFormat {
    /// Numbered entries with a role prefix and unescaped text.
    #[default]
    Line,
    /// Host-owned metadata with message text escaped as a JSON string.
    Json,
}
