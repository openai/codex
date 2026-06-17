use std::fs;
use std::path::Path;
use std::path::PathBuf;

/// A revision marker materialized inside the plugin catalog source it identifies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginCatalogRevision {
    source_root: PathBuf,
    marker_path: PathBuf,
    marker_contents: String,
}

impl PluginCatalogRevision {
    pub(crate) fn read(source_root: &Path, marker_path: PathBuf) -> Option<Self> {
        let marker_contents = fs::read_to_string(&marker_path).ok()?;
        Some(Self {
            source_root: source_root.to_path_buf(),
            marker_path,
            marker_contents,
        })
    }

    pub fn source_root(&self) -> &Path {
        &self.source_root
    }

    pub fn marker_contents(&self) -> &str {
        &self.marker_contents
    }

    pub(crate) fn is_current(&self) -> bool {
        fs::read_to_string(&self.marker_path).is_ok_and(|contents| contents == self.marker_contents)
    }
}
