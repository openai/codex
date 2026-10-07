//! Shared findings for policy-based containment integrity checks.

use codex_utils_absolute_path::AbsolutePathBuf;

/// A containment dependency and the effective write grants responsible for a finding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntegrityFinding {
    pub target: AbsolutePathBuf,
    pub write_roots: Vec<AbsolutePathBuf>,
    pub details: IntegrityFindingDetails,
}

/// Evidence specific to the kind of containment weakness detected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IntegrityFindingDetails {
    /// The policy permits modifying an existing file's contents.
    Contents { resolved_target: AbsolutePathBuf },
}
