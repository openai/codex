//! AGENTS.md discovery and user instruction assembly.
//!
//! Project-level documentation is primarily stored in files named `AGENTS.md`.
//! Additional fallback filenames can be configured via `project_doc_fallback_filenames`.
//! We include the concatenation of all files found along the path from the
//! project root to the current working directory as follows:
//!
//! 1.  Determine the project root by walking upwards from the current working
//!     directory until a configured `project_root_markers` entry is found.
//!     When `project_root_markers` is unset, the default marker list is used
//!     (`.git`). If no marker is found, only the current working directory is
//!     considered. An empty marker list disables parent traversal.
//! 2.  Collect every `AGENTS.md` found from the project root down to the
//!     current working directory (inclusive) and concatenate their contents in
//!     that order.
//! 3.  We do **not** walk past the project root.

use crate::config::Config;
use codex_app_server_protocol::ConfigLayerSource;
use codex_config::ConfigLayerStackOrdering;
use codex_config::default_project_root_markers;
use codex_config::merge_toml_values;
use codex_config::project_root_markers_from_config;
use codex_exec_server::Environment;
use codex_exec_server::ExecutorFileSystem;
use codex_features::Feature;
use codex_utils_absolute_path::AbsolutePathBuf;
use dunce::canonicalize as normalize_path;
use std::io;
use toml::Value as TomlValue;
use tracing::error;

pub(crate) const HIERARCHICAL_AGENTS_MESSAGE: &str =
    include_str!("../hierarchical_agents_message.md");

/// Default filename scanned for AGENTS.md instructions.
pub const DEFAULT_AGENTS_MD_FILENAME: &str = "AGENTS.md";
/// Preferred local override for AGENTS.md instructions.
pub const LOCAL_AGENTS_MD_FILENAME: &str = "AGENTS.override.md";

/// When both `Config::instructions` and AGENTS.md docs are present, they will
/// be concatenated with the following separator.
const AGENTS_MD_SEPARATOR: &str = "\n\n--- project-doc ---\n\n";

/// Resolves AGENTS.md files into model-visible user instructions and source
/// paths.
pub struct AgentsMdManager<'a> {
    config: &'a Config,
}

pub(crate) struct LoadedAgentsMd {
    pub(crate) contents: String,
    pub(crate) path: AbsolutePathBuf,
}

impl<'a> AgentsMdManager<'a> {
    pub fn new(config: &'a Config) -> Self {
        Self { config }
    }

    pub(crate) fn load_global_instructions(
        codex_dir: Option<&AbsolutePathBuf>,
    ) -> Option<LoadedAgentsMd> {
        let base = codex_dir?;
        for candidate in [LOCAL_AGENTS_MD_FILENAME, DEFAULT_AGENTS_MD_FILENAME] {
            let path = base.join(candidate);
            if let Ok(contents) = std::fs::read_to_string(&path) {
                let trimmed = contents.trim();
                if !trimmed.is_empty() {
                    return Some(LoadedAgentsMd {
                        contents: trimmed.to_string(),
                        path,
                    });
                }
            }
        }
        None
    }

    /// Combines configured user instructions and AGENTS.md content into a
    /// single model-visible instruction string.
    pub(crate) async fn user_instructions(
        &self,
        environment: Option<&Environment>,
    ) -> Option<String> {
        let fs = environment?.get_filesystem();
        self.user_instructions_with_fs(fs.as_ref()).await
    }

    pub(crate) async fn user_instructions_with_fs(
        &self,
        fs: &dyn ExecutorFileSystem,
    ) -> Option<String> {
        let agents_md_docs = self.read_agents_md(fs).await;

        let mut output = String::new();

        if let Some(instructions) = self.config.user_instructions.clone() {
            output.push_str(&instructions);
        }

        match agents_md_docs {
            Ok(Some(docs)) => {
                if !output.is_empty() {
                    output.push_str(AGENTS_MD_SEPARATOR);
                }
                output.push_str(&docs);
            }
            Ok(None) => {}
            Err(e) => {
                error!("error trying to find AGENTS.md docs: {e:#}");
            }
        };

        if self.config.features.enabled(Feature::ChildAgentsMd) {
            if !output.is_empty() {
                output.push_str("\n\n");
            }
            output.push_str(HIERARCHICAL_AGENTS_MESSAGE);
        }

        if !output.is_empty() {
            Some(output)
        } else {
            None
        }
    }

    /// Returns all instruction source files included in the current config.
    pub async fn instruction_sources(&self, fs: &dyn ExecutorFileSystem) -> Vec<AbsolutePathBuf> {
        let mut paths = Self::load_global_instructions(Some(&self.config.codex_home))
            .map(|loaded| vec![loaded.path])
            .unwrap_or_default();
        match self.agents_md_paths(fs).await {
            Ok(agents_md_paths) => paths.extend(agents_md_paths),
            Err(err) => {
                tracing::warn!(error = %err, "failed to discover AGENTS.md docs for instruction sources");
            }
        }
        paths
    }

    /// Attempt to locate and load AGENTS.md documentation.
    ///
    /// On success returns `Ok(Some(contents))` where `contents` is the
    /// concatenation of all discovered docs. If no documentation file is found
    /// the function returns `Ok(None)`. Unexpected I/O failures bubble up as
    /// `Err` so callers can decide how to handle them.
    async fn read_agents_md(&self, fs: &dyn ExecutorFileSystem) -> io::Result<Option<String>> {
        let max_total = self.config.project_doc_max_bytes;

        if max_total == 0 {
            return Ok(None);
        }

        let paths = self.agents_md_paths(fs).await?;
        if paths.is_empty() {
            return Ok(None);
        }

        let mut remaining: u64 = max_total as u64;
        let mut parts: Vec<String> = Vec::new();

        for p in paths {
            if remaining == 0 {
                break;
            }

            match fs.get_metadata(&p, /*sandbox*/ None).await {
                Ok(metadata) if !metadata.is_file => continue,
                Ok(_) => {}
                Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
                Err(err) => return Err(err),
            }

            let mut data = match fs.read_file(&p, /*sandbox*/ None).await {
                Ok(data) => data,
                Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
                Err(err) => return Err(err),
            };
            let size = data.len() as u64;
            if size > remaining {
                data.truncate(remaining as usize);
            }

            if size > remaining {
                tracing::warn!(
                    "Project doc `{}` exceeds remaining budget ({} bytes) - truncating.",
                    p.display(),
                    remaining,
                );
            }

            let text = String::from_utf8_lossy(&data).to_string();
            if !text.trim().is_empty() {
                parts.push(text);
                remaining = remaining.saturating_sub(data.len() as u64);
            }
        }

        if parts.is_empty() {
            Ok(None)
        } else {
            Ok(Some(parts.join("\n\n")))
        }
    }

    /// Discover the list of AGENTS.md files using the same search rules as
    /// `read_agents_md`, but return the file paths instead of concatenated
    /// contents. The list is ordered from project root to the current working
    /// directory (inclusive). Symlinks are allowed. When `project_doc_max_bytes`
    /// is zero, returns an empty list.
    async fn agents_md_paths(
        &self,
        fs: &dyn ExecutorFileSystem,
    ) -> io::Result<Vec<AbsolutePathBuf>> {
        if self.config.project_doc_max_bytes == 0 {
            return Ok(Vec::new());
        }

        let mut dir = self.config.cwd.clone();
        if let Ok(canon) = normalize_path(&dir) {
            dir = AbsolutePathBuf::try_from(canon)?;
        }

        let mut merged = TomlValue::Table(toml::map::Map::new());
        for layer in self.config.config_layer_stack.get_layers(
            ConfigLayerStackOrdering::LowestPrecedenceFirst,
            /*include_disabled*/ false,
        ) {
            if matches!(layer.name, ConfigLayerSource::Project { .. }) {
                continue;
            }
            merge_toml_values(&mut merged, &layer.config);
        }
        let project_root_markers = match project_root_markers_from_config(&merged) {
            Ok(Some(markers)) => markers,
            Ok(None) => default_project_root_markers(),
            Err(err) => {
                tracing::warn!("invalid project_root_markers: {err}");
                default_project_root_markers()
            }
        };
        let mut project_root = None;
        if !project_root_markers.is_empty() {
            for ancestor in dir.ancestors() {
                for marker in &project_root_markers {
                    let marker_path = ancestor.join(marker);
                    let marker_exists = match fs.get_metadata(&marker_path, /*sandbox*/ None).await
                    {
                        Ok(_) => true,
                        Err(err) if err.kind() == io::ErrorKind::NotFound => false,
                        Err(err) => return Err(err),
                    };
                    if marker_exists {
                        project_root = Some(ancestor.clone());
                        break;
                    }
                }
                if project_root.is_some() {
                    break;
                }
            }
        }

        let search_dirs: Vec<AbsolutePathBuf> = if let Some(root) = project_root {
            let mut dirs = Vec::new();
            let mut cursor = dir.clone();
            loop {
                dirs.push(cursor.clone());
                if cursor == root {
                    break;
                }
                let Some(parent) = cursor.parent() else {
                    break;
                };
                cursor = parent;
            }
            dirs.reverse();
            dirs
        } else {
            vec![dir]
        };

        let mut found: Vec<AbsolutePathBuf> = Vec::new();
        let candidate_filenames = self.candidate_filenames();
        for d in search_dirs {
            for name in &candidate_filenames {
                let candidate = d.join(name);
                match fs.get_metadata(&candidate, /*sandbox*/ None).await {
                    Ok(md) if md.is_file => {
                        found.push(candidate);
                        break;
                    }
                    Ok(_) => {}
                    Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
                    Err(err) => return Err(err),
                }
            }
        }

        Ok(found)
    }

    fn candidate_filenames(&self) -> Vec<&str> {
        let mut names: Vec<&str> =
            Vec::with_capacity(2 + self.config.project_doc_fallback_filenames.len());
        names.push(LOCAL_AGENTS_MD_FILENAME);
        names.push(DEFAULT_AGENTS_MD_FILENAME);
        for candidate in &self.config.project_doc_fallback_filenames {
            let candidate = candidate.as_str();
            if candidate.is_empty() {
                continue;
            }
            if !names.contains(&candidate) {
                names.push(candidate);
            }
        }
        names
    }
}

#[cfg(test)]
#[path = "agents_md_tests.rs"]
mod tests;
