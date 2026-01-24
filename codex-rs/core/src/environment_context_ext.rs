//! Extension for EnvironmentContext - adds platform/git info to XML output.
//!
//! This module provides additional environment context fields that are
//! computed at serialization time to avoid modifying the core EnvironmentContext
//! struct and its constructor signatures.

use std::path::Path;

use crate::git_info::get_git_repo_root;

/// Extended environment context fields (computed at serialization time).
pub(crate) struct EnvironmentContextExt {
    pub is_git_repo: bool,
    pub platform: &'static str,
    pub cpu_arch: &'static str,
}

impl EnvironmentContextExt {
    /// Create from cwd - computes all fields internally.
    pub fn from_cwd(cwd: Option<&Path>) -> Self {
        Self {
            is_git_repo: cwd.map(|p| get_git_repo_root(p).is_some()).unwrap_or(false),
            platform: std::env::consts::OS,
            cpu_arch: std::env::consts::ARCH,
        }
    }

    /// Serialize to XML lines (to be appended before closing tag).
    pub fn serialize_to_xml_lines(&self) -> Vec<String> {
        vec![
            format!("  <is_git_repo>{}</is_git_repo>", self.is_git_repo),
            format!("  <platform>{}</platform>", self.platform),
            format!("  <cpu_arch>{}</cpu_arch>", self.cpu_arch),
        ]
    }
}
