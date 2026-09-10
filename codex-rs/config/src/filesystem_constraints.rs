//! Convert resolved filesystem denials into portable policy entries.

use crate::FilesystemConstraints;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_utils_path_uri::LegacyAppPathString;
use codex_utils_path_uri::LegacyAppPathStringError;
use codex_utils_path_uri::PathConvention;
use codex_utils_path_uri::PathUri;
use std::io;

impl FilesystemConstraints {
    /// Adds denials without changing existing entries or other policy settings.
    /// `convention` must match the executor facts used to resolve these requirements.
    /// Literal paths become URIs; glob spellings and entry order are preserved.
    /// Invalid denials return an error before the policy is changed.
    pub fn apply_to_policy(
        &self,
        policy: &mut FileSystemSandboxPolicy,
        convention: PathConvention,
    ) -> io::Result<()> {
        let entries = self
            .deny_read
            .iter()
            .map(|deny_read| {
                PathUri::validate_config_path_text(deny_read.as_str(), convention)?;
                let path = if deny_read.contains_glob() {
                    FileSystemPath::GlobPattern {
                        pattern: deny_read.as_str().to_string(),
                    }
                } else {
                    let mut path = LegacyAppPathString::from_string(deny_read.as_str())
                        .to_path_uri(convention)?;
                    path.validate_config_path(convention)?;
                    // Match Core's existing URI spelling for UNC share roots.
                    if path.to_url().host_str().is_some() {
                        path = path.join(".")?;
                    }
                    path.into()
                };
                Ok(FileSystemSandboxEntry::new(
                    path,
                    FileSystemAccessMode::Deny,
                ))
            })
            .collect::<Result<Vec<_>, LegacyAppPathStringError>>()
            .map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("invalid permissions.filesystem.deny_read path: {error}"),
                )
            })?;
        for entry in entries {
            if !policy.entries.contains(&entry) {
                policy.entries.push(entry);
            }
        }
        Ok(())
    }
}
