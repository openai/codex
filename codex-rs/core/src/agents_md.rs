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
use crate::environment_selection::TurnEnvironmentSnapshot;
use codex_config::ConfigLayerSource;
use codex_config::ConfigLayerStackOrdering;
use codex_config::default_project_root_markers;
use codex_config::merge_toml_values;
use codex_config::project_root_markers_from_config;
use codex_exec_server::ExecutorFileSystem;
use codex_extension_api::UserInstructions;
use codex_file_system::FindUpErrorPolicy;
use codex_file_system::find_nearest_ancestor_with_markers;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use indexmap::IndexMap;
use std::io;
use toml::Value as TomlValue;
use tracing::error;

/// Default filename scanned for AGENTS.md instructions.
pub const DEFAULT_AGENTS_MD_FILENAME: &str = "AGENTS.md";
/// Preferred local override for AGENTS.md instructions.
pub const LOCAL_AGENTS_MD_FILENAME: &str = "AGENTS.override.md";

/// When both user and project AGENTS.md docs are present, they will be
/// concatenated with the following separator.
pub(crate) const AGENTS_MD_SEPARATOR: &str = "\n\n--- project-doc ---\n\n";

/// Loads project AGENTS.md content and combines it with host-provided user
/// instructions.
pub(crate) async fn load_project_instructions(
    config: &Config,
    user_instructions: Option<UserInstructions>,
    environments: &TurnEnvironmentSnapshot,
) -> Option<LoadedAgentsMd> {
    let mut loaded = LoadedAgentsMd::from_user_instructions(user_instructions);
    for turn_environment in &environments.turn_environments {
        let filesystem = turn_environment.environment.get_filesystem();
        let environment_id = &turn_environment.environment_id;
        match read_agents_md(config, filesystem.as_ref(), turn_environment.cwd()).await {
            Ok(instructions) => {
                loaded
                    .environments
                    .insert(environment_id.clone(), instructions);
            }
            Err(err) => {
                error!(
                    environment_id,
                    "error trying to find AGENTS.md docs: {err:#}"
                );
            }
        }
    }
    (!loaded.is_empty()).then_some(loaded)
}

/// Attempt to locate and load AGENTS.md documentation.
///
/// On success, the returned environment value contains every discovered doc.
/// Missing docs produce an empty value so the structured result can retain the
/// environment cwd. Unexpected I/O failures bubble up so callers can decide
/// how to handle them.
async fn read_agents_md(
    config: &Config,
    fs: &dyn ExecutorFileSystem,
    cwd: &PathUri,
) -> io::Result<EnvironmentInstructions> {
    let max_total = config.project_doc_max_bytes;

    if max_total == 0 {
        return Ok(EnvironmentInstructions::new(cwd.clone()));
    }

    let paths = agents_md_paths(config, cwd, fs).await?;
    if paths.is_empty() {
        return Ok(EnvironmentInstructions::new(cwd.clone()));
    }

    let mut remaining: u64 = max_total as u64;
    let mut entries = Vec::with_capacity(paths.len());

    for p in paths {
        if remaining == 0 {
            break;
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
                path = %p,
                remaining_bytes = remaining,
                "project doc exceeds remaining budget; truncating"
            );
        }

        let text = String::from_utf8_lossy(&data).to_string();
        if !text.trim().is_empty() {
            entries.push(InstructionEntry {
                contents: text,
                source_path: p,
            });
            remaining = remaining.saturating_sub(data.len() as u64);
        }
    }

    Ok(EnvironmentInstructions {
        cwd: cwd.clone(),
        entries,
    })
}

/// Discovers AGENTS.md files from the project root to the current working
/// directory, inclusive. Symlinks are allowed.
async fn agents_md_paths(
    config: &Config,
    cwd: &PathUri,
    fs: &dyn ExecutorFileSystem,
) -> io::Result<Vec<PathUri>> {
    let dir = cwd.clone();

    let mut merged = TomlValue::Table(toml::map::Map::new());
    for layer in config.config_layer_stack.get_layers(
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
    let project_root = find_nearest_ancestor_with_markers(
        fs,
        &dir,
        project_root_markers,
        FindUpErrorPolicy::Propagate,
        /*sandbox*/ None,
    )
    .await?;
    let search_dirs = if let Some(root) = project_root {
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

    let mut found = Vec::new();
    let candidate_filenames = candidate_filenames(config);
    for directory in search_dirs {
        for name in &candidate_filenames {
            let candidate = directory
                .join(name)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
            match fs.get_metadata(&candidate, /*sandbox*/ None).await {
                Ok(metadata) if metadata.is_file => {
                    found.push(candidate);
                    break;
                }
                Ok(_) => {}
                Err(err) if err.kind() == io::ErrorKind::NotFound => {}
                Err(err) => return Err(err),
            }
        }
    }
    Ok(found)
}

fn candidate_filenames(config: &Config) -> Vec<&str> {
    let mut names: Vec<&str> = Vec::with_capacity(2 + config.project_doc_fallback_filenames.len());
    names.push(LOCAL_AGENTS_MD_FILENAME);
    names.push(DEFAULT_AGENTS_MD_FILENAME);
    for candidate in &config.project_doc_fallback_filenames {
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

/// Model-visible instructions loaded from AGENTS.md files and internal
/// guidance.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoadedAgentsMd {
    /// Host-provided user instructions.
    pub(crate) user_instructions: Option<UserInstructions>,

    /// Instructions without a file source, including internally defined guidance.
    pub(crate) internal_instructions: Vec<String>,

    /// Project instructions keyed and ordered by environment.
    pub(crate) environments: IndexMap<String, EnvironmentInstructions>,
}

impl LoadedAgentsMd {
    /// Creates loaded instructions containing one user-level AGENTS.md entry.
    pub fn new_user(contents: String, path: AbsolutePathBuf) -> Self {
        if contents.trim().is_empty() {
            return Self::default();
        }
        Self {
            user_instructions: Some(UserInstructions {
                text: contents,
                source: path,
            }),
            internal_instructions: Vec::new(),
            environments: IndexMap::new(),
        }
    }

    fn from_user_instructions(user_instructions: Option<UserInstructions>) -> Self {
        Self {
            user_instructions: user_instructions
                .filter(|instructions| !instructions.text.trim().is_empty()),
            internal_instructions: Vec::new(),
            environments: IndexMap::new(),
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
            user_instructions: None,
            internal_instructions: vec![contents],
            environments: IndexMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EnvironmentInstructions {
    pub(crate) cwd: PathUri,
    pub(crate) entries: Vec<InstructionEntry>,
}

impl EnvironmentInstructions {
    fn new(cwd: PathUri) -> Self {
        Self {
            cwd,
            entries: Vec::new(),
        }
    }
}

/// One model-visible project instruction and its source file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct InstructionEntry {
    pub(crate) contents: String,
    pub(crate) source_path: PathUri,
}

#[cfg(test)]
#[path = "agents_md_tests.rs"]
mod tests;
