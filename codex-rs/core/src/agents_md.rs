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
use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecutorFileSystem;
use codex_features::Feature;
use codex_prompts::HIERARCHICAL_AGENTS_MESSAGE;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_utils_absolute_path::AbsolutePathBuf;
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

/// Preloads AGENTS.md instructions for thread construction from the primary
/// selected environment.
///
/// Call this after the selected environments have been materialized and before
/// passing `config` to a thread start, resume, or fork operation. When no
/// environment is selected, this clears [`Config::user_instructions`].
pub async fn load_thread_user_instructions(
    config: &mut Config,
    environment_manager: &EnvironmentManager,
    environments: &[TurnEnvironmentSelection],
) -> CodexResult<()> {
    let Some(primary_selection) = environments.first() else {
        config.user_instructions = None;
        return Ok(());
    };
    let environment = environment_manager
        .get_environment(&primary_selection.environment_id)
        .ok_or_else(|| {
            CodexErr::InvalidRequest(format!(
                "unknown turn environment id `{}`",
                primary_selection.environment_id
            ))
        })?;
    let mut warnings = Vec::new();
    let user_instructions = AgentsMdManager::new(config)
        .load_user_instructions(environment.as_ref(), &mut warnings)
        .await;
    config.startup_warnings.extend(warnings);
    config.user_instructions = user_instructions;
    Ok(())
}

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

    /// Loads project instructions from `environment` and combines them with
    /// the configured user instructions.
    ///
    /// Callers should invoke this before constructing a thread and provide the
    /// returned value through [`Config::user_instructions`].
    pub async fn load_user_instructions(
        &self,
        environment: &Environment,
        startup_warnings: &mut Vec<String>,
    ) -> Option<LoadedAgentsMd> {
        let fs = environment.get_filesystem();
        self.user_instructions_with_fs(fs.as_ref(), startup_warnings)
            .await
    }

    async fn user_instructions_with_fs(
        &self,
        fs: &dyn ExecutorFileSystem,
        startup_warnings: &mut Vec<String>,
    ) -> Option<LoadedAgentsMd> {
        let agents_md_docs = self.read_agents_md(fs, startup_warnings).await;

        let mut loaded = self
            .config
            .user_instructions
            .as_ref()
            .map(LoadedAgentsMd::user_instructions)
            .unwrap_or_default();

        match agents_md_docs {
            Ok(Some(docs)) => loaded.project_entries = docs.project_entries,
            Ok(None) => {}
            Err(e) => {
                error!("error trying to find AGENTS.md docs: {e:#}");
            }
        };

        if self.config.features.enabled(Feature::ChildAgentsMd) {
            loaded.internal_entries.push(InternalInstructionEntry {
                contents: HIERARCHICAL_AGENTS_MESSAGE.to_string(),
            });
        }

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
                loaded.project_entries.push(ProjectInstructionEntry {
                    contents: text,
                    source: p,
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
    user_entries: Vec<UserInstructionEntry>,
    project_entries: Vec<ProjectInstructionEntry>,
    internal_entries: Vec<InternalInstructionEntry>,
}

impl LoadedAgentsMd {
    /// Creates loaded instructions containing one user-level AGENTS.md entry.
    pub fn new_user(contents: String, path: AbsolutePathBuf) -> Self {
        if contents.trim().is_empty() {
            return Self::default();
        }
        Self {
            user_entries: vec![UserInstructionEntry {
                contents,
                source: Some(path),
            }],
            ..Default::default()
        }
    }

    /// Creates source-less user instructions for tests.
    ///
    /// This cannot be gated with `#[cfg(test)]` because integration tests
    /// compile `codex-core` as a normal dependency without that configuration.
    pub fn from_text_for_testing(contents: impl Into<String>) -> Self {
        let contents = contents.into();
        if contents.trim().is_empty() {
            return Self::default();
        }
        Self {
            user_entries: vec![UserInstructionEntry {
                contents,
                source: None,
            }],
            ..Default::default()
        }
    }

    fn is_empty(&self) -> bool {
        self.user_entries
            .iter()
            .all(|entry| entry.contents.trim().is_empty())
            && self
                .project_entries
                .iter()
                .all(|entry| entry.contents.trim().is_empty())
            && self
                .internal_entries
                .iter()
                .all(|entry| entry.contents.trim().is_empty())
    }

    fn user_instructions(&self) -> Self {
        Self {
            user_entries: self.user_entries.clone(),
            ..Default::default()
        }
    }

    /// Returns the concatenated model-visible instruction text.
    pub fn text(&self) -> String {
        let mut output = String::new();
        for entry in &self.user_entries {
            append_instruction(&mut output, &entry.contents, "\n\n");
        }
        let mut appended_project_entry = false;
        for entry in &self.project_entries {
            let separator = if appended_project_entry || output.is_empty() {
                "\n\n"
            } else {
                AGENTS_MD_SEPARATOR
            };
            if append_instruction(&mut output, &entry.contents, separator) {
                appended_project_entry = true;
            }
        }
        for entry in &self.internal_entries {
            append_instruction(&mut output, &entry.contents, "\n\n");
        }
        output
    }

    /// Returns the AGENTS.md files that supplied instruction entries.
    pub fn sources(&self) -> impl Iterator<Item = &AbsolutePathBuf> {
        self.user_entries
            .iter()
            .filter_map(|entry| entry.source.as_ref())
            .chain(self.project_entries.iter().map(|entry| &entry.source))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct UserInstructionEntry {
    contents: String,
    source: Option<AbsolutePathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ProjectInstructionEntry {
    contents: String,
    source: AbsolutePathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct InternalInstructionEntry {
    contents: String,
}

fn append_instruction(output: &mut String, contents: &str, separator: &str) -> bool {
    if contents.trim().is_empty() {
        return false;
    }
    if !output.is_empty() {
        output.push_str(separator);
    }
    output.push_str(contents);
    true
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
