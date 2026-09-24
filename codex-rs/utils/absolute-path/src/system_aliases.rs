//! Normalize aliases rooted directly in the trusted filesystem root.
//! Never resolve mutable components below that root, including missing descendants.

use crate::AbsolutePathBuf;
use std::io;
use std::path::Path;

impl AbsolutePathBuf {
    /// Resolve a top-level system alias, such as macOS `/tmp -> /private/tmp`.
    /// All remaining components keep their logical spelling, even if they are
    /// symlinks or do not exist. Only call this on the host that owns the path.
    pub fn normalize_system_aliases(&self) -> io::Result<Self> {
        let Some(top_level) = self.as_path().ancestors().find(|ancestor| {
            ancestor.parent().is_some() && ancestor.parent().and_then(Path::parent).is_none()
        }) else {
            return Ok(self.clone());
        };
        match std::fs::symlink_metadata(top_level) {
            Ok(metadata) if metadata.file_type().is_symlink() => {}
            Ok(_) => return Ok(self.clone()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(self.clone()),
            Err(error) => return Err(error),
        }
        let canonical_top_level = dunce::canonicalize(top_level)?;
        let suffix = self
            .as_path()
            .strip_prefix(top_level)
            .map_err(io::Error::other)?;
        Self::from_absolute_path(canonical_top_level.join(suffix))
    }
}

#[cfg(test)]
#[path = "system_aliases_tests.rs"]
mod tests;
