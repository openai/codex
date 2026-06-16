use std::fs;

use codex_utils_absolute_path::AbsolutePathBuf;

/// A revision marker materialized alongside the catalog source it identifies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PluginCatalogRevision {
    marker_path: AbsolutePathBuf,
    marker_contents: String,
}

impl PluginCatalogRevision {
    pub(crate) fn new(marker_path: AbsolutePathBuf, marker_contents: String) -> Self {
        Self {
            marker_path,
            marker_contents,
        }
    }

    pub(crate) fn read(marker_path: AbsolutePathBuf) -> Option<Self> {
        let marker_contents = fs::read_to_string(marker_path.as_path()).ok()?;
        Some(Self::new(marker_path, marker_contents))
    }

    pub(crate) fn is_current(&self) -> bool {
        fs::read_to_string(self.marker_path.as_path())
            .is_ok_and(|contents| contents == self.marker_contents)
    }
}
