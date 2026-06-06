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
use codex_prompts::HIERARCHICAL_AGENTS_MESSAGE;
use codex_protocol::protocol::InstructionSnapshot;
use codex_protocol::protocol::UserInstructionsSnapshot;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_string::approx_bytes_for_tokens;
use std::io;
use toml::Value as TomlValue;
use tracing::error;

/// Default filename scanned for AGENTS.md instructions.
pub const DEFAULT_AGENTS_MD_FILENAME: &str = "AGENTS.md";
/// Preferred local override for AGENTS.md instructions.
pub const LOCAL_AGENTS_MD_FILENAME: &str = "AGENTS.override.md";

/// When both user and project AGENTS.md docs are present, they will be
/// concatenated with the following separator.
const AGENTS_MD_SEPARATOR: &str = "\n\n--- project-doc ---\n\n";
const MAX_USER_INSTRUCTIONS_TOKENS: usize = 8_000;
const USER_INSTRUCTIONS_TRUNCATION_MARKER: &str = "\n[instructions truncated]";

/// Resolves AGENTS.md files into model-visible user instructions and source
/// paths.
pub struct AgentsMdManager<'a> {
    config: &'a Config,
}

impl<'a> AgentsMdManager<'a> {
    pub fn new(config: &'a Config) -> Self {
        Self { config }
    }

    pub(crate) async fn load_global_instructions(
        fs: &dyn ExecutorFileSystem,
        codex_dir: Option<&AbsolutePathBuf>,
        startup_warnings: &mut Vec<String>,
    ) -> Option<LoadedAgentsMd> {
        let base = codex_dir?;
        for candidate in [LOCAL_AGENTS_MD_FILENAME, DEFAULT_AGENTS_MD_FILENAME] {
            let path = base.join(candidate);
            let data = match fs.read_file(&path, /*sandbox*/ None).await {
                Ok(data) => data,
                Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
                Err(err) if err.kind() == io::ErrorKind::IsADirectory => continue,
                Err(err) => {
                    startup_warnings.push(format!(
                        "Failed to read global AGENTS.md instructions from `{}`: {err}",
                        path.display()
                    ));
                    continue;
                }
            };
            warn_invalid_utf8(&path, &data, "Global", startup_warnings);
            let contents = String::from_utf8_lossy(&data);
            let trimmed = contents.trim();
            if !trimmed.is_empty() {
                return Some(LoadedAgentsMd::new_user(trimmed.to_string(), path));
            }
        }
        None
    }

    /// Combines global instructions and project AGENTS.md content into a
    /// single model-visible instruction snapshot.
    pub(crate) async fn user_instructions(
        &self,
        environment: Option<&Environment>,
        loaded: LoadedAgentsMd,
        startup_warnings: &mut Vec<String>,
    ) -> Option<LoadedAgentsMd> {
        if let Some(environment) = environment {
            let fs = environment.get_filesystem();
            return self
                .user_instructions_with_loaded_fs(fs.as_ref(), loaded, startup_warnings)
                .await;
        }

        self.finish_instructions(loaded)
    }

    async fn user_instructions_with_loaded_fs(
        &self,
        fs: &dyn ExecutorFileSystem,
        mut loaded: LoadedAgentsMd,
        startup_warnings: &mut Vec<String>,
    ) -> Option<LoadedAgentsMd> {
        match self.read_agents_md(fs, startup_warnings).await {
            Ok(Some(docs)) => loaded.entries.extend(docs.entries),
            Ok(None) => {}
            Err(e) => {
                error!("error trying to find AGENTS.md docs: {e:#}");
            }
        }

        self.finish_instructions(loaded)
    }

    fn finish_instructions(&self, mut loaded: LoadedAgentsMd) -> Option<LoadedAgentsMd> {
        if self.config.features.enabled(Feature::ChildAgentsMd) {
            loaded.entries.push(InstructionEntry {
                contents: HIERARCHICAL_AGENTS_MESSAGE.to_string(),
                provenance: InstructionProvenance::Internal,
            });
        }
        loaded.enforce_model_context_limit();

        (!loaded.is_empty()).then_some(loaded)
    }

    /// Attempt to locate and load AGENTS.md documentation.
    ///
    /// On success returns `Ok(Some(loaded))` where `loaded` contains every
    /// discovered doc. If no documentation file is found the function returns
    /// `Ok(None)`. Unexpected I/O failures bubble up as `Err` so callers can
    /// decide how to handle them.
    async fn read_agents_md(
        &self,
        fs: &dyn ExecutorFileSystem,
        startup_warnings: &mut Vec<String>,
    ) -> io::Result<Option<LoadedAgentsMd>> {
        let max_total = self.config.project_doc_max_bytes;

        if max_total == 0 {
            return Ok(None);
        }

        let paths = self.agents_md_paths(fs).await?;
        if paths.is_empty() {
            return Ok(None);
        }

        let mut remaining: u64 = max_total as u64;
        let mut loaded = LoadedAgentsMd::default();

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
            warn_invalid_utf8(&p, &data, "Project", startup_warnings);

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
                loaded.entries.push(InstructionEntry {
                    contents: text,
                    provenance: InstructionProvenance::Project(Some(p)),
                });
                remaining = remaining.saturating_sub(data.len() as u64);
            }
        }

        if loaded.is_empty() {
            Ok(None)
        } else {
            Ok(Some(loaded))
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

        let dir = self.config.cwd.clone();

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

/// Model-visible instructions loaded from AGENTS.md files and internal
/// guidance.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoadedAgentsMd {
    /// Ordered instructions and their provenance.
    entries: Vec<InstructionEntry>,
}

impl LoadedAgentsMd {
    pub fn new_user(contents: String, path: AbsolutePathBuf) -> Self {
        if contents.trim().is_empty() {
            return Self::default();
        }
        let mut loaded = Self {
            entries: vec![InstructionEntry {
                contents,
                provenance: InstructionProvenance::Global(Some(path)),
            }],
        };
        loaded.enforce_model_context_limit();
        loaded
    }

    pub fn from_text_for_testing(contents: impl Into<String>) -> Self {
        let contents = contents.into();
        if contents.trim().is_empty() {
            return Self::default();
        }
        Self {
            entries: vec![InstructionEntry {
                contents,
                provenance: InstructionProvenance::Internal,
            }],
        }
    }

    pub(crate) fn from_snapshot(snapshot: UserInstructionsSnapshot) -> Self {
        let mut loaded = Self {
            entries: snapshot
                .instructions
                .into_iter()
                .map(|instruction| InstructionEntry {
                    contents: instruction.contents,
                    provenance: match instruction.provenance {
                        codex_protocol::protocol::InstructionProvenance::Global => {
                            InstructionProvenance::Global(instruction.source)
                        }
                        codex_protocol::protocol::InstructionProvenance::Project => {
                            InstructionProvenance::Project(instruction.source)
                        }
                        codex_protocol::protocol::InstructionProvenance::Internal => {
                            InstructionProvenance::Internal
                        }
                    },
                })
                .collect(),
        };
        loaded.enforce_model_context_limit();
        loaded
    }

    pub(crate) fn snapshot(&self) -> UserInstructionsSnapshot {
        UserInstructionsSnapshot {
            instructions: self
                .entries
                .iter()
                .map(|entry| InstructionSnapshot {
                    contents: entry.contents.clone(),
                    provenance: match entry.provenance {
                        InstructionProvenance::Global(_) => {
                            codex_protocol::protocol::InstructionProvenance::Global
                        }
                        InstructionProvenance::Project(_) => {
                            codex_protocol::protocol::InstructionProvenance::Project
                        }
                        InstructionProvenance::Internal => {
                            codex_protocol::protocol::InstructionProvenance::Internal
                        }
                    },
                    source: entry.provenance.path().cloned(),
                })
                .collect(),
        }
    }

    pub(crate) fn replace_global(&mut self, instructions: LoadedAgentsMd) {
        let first_non_global = self
            .entries
            .iter()
            .position(|entry| !matches!(entry.provenance, InstructionProvenance::Global(_)))
            .unwrap_or(self.entries.len());
        self.entries
            .splice(..first_non_global, instructions.entries);
        self.enforce_model_context_limit();
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries
            .iter()
            .all(|entry| entry.contents.trim().is_empty())
    }

    /// Returns the concatenated model-visible instruction text.
    pub fn text(&self) -> String {
        let mut output = String::new();
        let mut previous_provenance: Option<&InstructionProvenance> = None;
        for entry in &self.entries {
            if let Some(previous_provenance) = previous_provenance {
                // The project-doc marker tells the model where workspace-scoped
                // instructions begin, so it is only needed on the transition
                // from user or internal instructions to project instructions.
                let separator = match (previous_provenance, &entry.provenance) {
                    (
                        InstructionProvenance::Global(_) | InstructionProvenance::Internal,
                        InstructionProvenance::Project(_),
                    ) => AGENTS_MD_SEPARATOR,
                    _ => "\n\n",
                };
                output.push_str(separator);
            }
            output.push_str(&entry.contents);
            previous_provenance = Some(&entry.provenance);
        }
        output
    }

    /// Returns the AGENTS.md files that supplied instruction entries.
    pub fn sources(&self) -> impl Iterator<Item = &AbsolutePathBuf> {
        self.entries
            .iter()
            .filter_map(|entry| entry.provenance.path())
    }

    fn enforce_model_context_limit(&mut self) {
        let max_bytes = approx_bytes_for_tokens(MAX_USER_INSTRUCTIONS_TOKENS);
        let mut remaining_bytes = max_bytes;
        let mut bounded_entries = Vec::with_capacity(self.entries.len());

        for mut entry in std::mem::take(&mut self.entries) {
            let separator = bounded_entries
                .last()
                .map(|previous_entry: &InstructionEntry| {
                    let previous_provenance = &previous_entry.provenance;
                    match (previous_provenance, &entry.provenance) {
                        (
                            InstructionProvenance::Global(_) | InstructionProvenance::Internal,
                            InstructionProvenance::Project(_),
                        ) => AGENTS_MD_SEPARATOR,
                        _ => "\n\n",
                    }
                });
            if let Some(separator) = separator {
                if separator.len() >= remaining_bytes {
                    break;
                }
                remaining_bytes = remaining_bytes.saturating_sub(separator.len());
            }

            if entry.contents.len() > remaining_bytes {
                entry.contents = truncate_instruction_contents(&entry.contents, remaining_bytes);
                if !entry.contents.is_empty() {
                    bounded_entries.push(entry);
                }
                break;
            }

            remaining_bytes = remaining_bytes.saturating_sub(entry.contents.len());
            bounded_entries.push(entry);
        }

        self.entries = bounded_entries;
    }
}

fn truncate_instruction_contents(contents: &str, max_bytes: usize) -> String {
    if contents.len() <= max_bytes {
        return contents.to_string();
    }
    if max_bytes <= USER_INSTRUCTIONS_TRUNCATION_MARKER.len() {
        return USER_INSTRUCTIONS_TRUNCATION_MARKER[..max_bytes].to_string();
    }

    let prefix_budget = max_bytes.saturating_sub(USER_INSTRUCTIONS_TRUNCATION_MARKER.len());
    let mut prefix_end = prefix_budget.min(contents.len());
    while !contents.is_char_boundary(prefix_end) {
        prefix_end = prefix_end.saturating_sub(1);
    }
    format!(
        "{}{USER_INSTRUCTIONS_TRUNCATION_MARKER}",
        &contents[..prefix_end]
    )
}

/// One model-visible instruction and its provenance.
#[derive(Clone, Debug, PartialEq, Eq)]
struct InstructionEntry {
    /// Model-visible instruction text.
    contents: String,

    /// Origin of the instruction.
    provenance: InstructionProvenance,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum InstructionProvenance {
    /// Global instructions, normally loaded from CODEX_HOME.
    Global(Option<AbsolutePathBuf>),

    /// Workspace instructions discovered from project AGENTS.md files.
    Project(Option<AbsolutePathBuf>),

    /// Instructions without a file source, including internally defined guidance.
    Internal,
}

impl InstructionProvenance {
    fn path(&self) -> Option<&AbsolutePathBuf> {
        match self {
            Self::Global(path) | Self::Project(path) => path.as_ref(),
            Self::Internal => None,
        }
    }
}

fn warn_invalid_utf8(
    path: &AbsolutePathBuf,
    data: &[u8],
    source: &str,
    startup_warnings: &mut Vec<String>,
) {
    if let Err(err) = std::str::from_utf8(data) {
        startup_warnings.push(format!(
            "{source} AGENTS.md instructions from `{}` contain invalid UTF-8: {err}. Invalid byte sequences were replaced.",
            path.display()
        ));
    }
}

#[cfg(test)]
#[path = "agents_md_tests.rs"]
mod tests;
