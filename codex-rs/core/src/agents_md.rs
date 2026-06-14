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
use crate::context::ContextualUserFragment;
use crate::context::UserInstructions as ContextUserInstructions;
use crate::environment_selection::ResolvedTurnEnvironments;
use codex_app_server_protocol::ConfigLayerSource;
use codex_config::ConfigLayerStackOrdering;
use codex_config::default_project_root_markers;
use codex_config::merge_toml_values;
use codex_config::project_root_markers_from_config;
use codex_exec_server::ExecutorFileSystem;
use codex_extension_api::UserInstructions;
use codex_features::Feature;
use codex_prompts::HIERARCHICAL_AGENTS_MESSAGE;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
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

/// Loads project AGENTS.md content and combines it with host-provided user
/// instructions.
pub(crate) async fn load_project_instructions(
    config: &mut Config,
    user_instructions: Option<UserInstructions>,
    environments: &ResolvedTurnEnvironments,
) -> Option<LoadedAgentsMd> {
    let mut loaded = LoadedAgentsMd::from_user_instructions(user_instructions);
    for turn_environment in &environments.turn_environments {
        let filesystem = turn_environment.environment.get_filesystem();
        match read_agents_md(
            config,
            filesystem.as_ref(),
            &turn_environment.environment_id,
            turn_environment.cwd(),
        )
        .await
        {
            Ok(Some(docs)) => loaded.entries.extend(docs.entries),
            Ok(None) => {}
            Err(e) => {
                error!(
                    environment_id = turn_environment.environment_id,
                    "error trying to find AGENTS.md docs: {e:#}"
                );
            }
        }
    }

    if config.features.enabled(Feature::ChildAgentsMd) {
        loaded.entries.push(InstructionEntry {
            contents: HIERARCHICAL_AGENTS_MESSAGE.to_string(),
            provenance: InstructionProvenance::Internal,
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
    config: &mut Config,
    fs: &dyn ExecutorFileSystem,
    environment_id: &str,
    cwd: &AbsolutePathBuf,
) -> io::Result<Option<LoadedAgentsMd>> {
    let max_total = config.project_doc_max_bytes;

    if max_total == 0 {
        return Ok(None);
    }

    let paths = agents_md_paths(config, cwd, fs).await?;
    if paths.is_empty() {
        return Ok(None);
    }

    let mut remaining: u64 = max_total as u64;
    let mut loaded = LoadedAgentsMd::default();

    for p in paths {
        if remaining == 0 {
            break;
        }

        let path_uri = PathUri::from_abs_path(&p);
        match fs.get_metadata(&path_uri, /*sandbox*/ None).await {
            Ok(metadata) if !metadata.is_file => continue,
            Ok(_) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) => return Err(err),
        }

        let mut data = match fs.read_file(&path_uri, /*sandbox*/ None).await {
            Ok(data) => data,
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) => return Err(err),
        };
        warn_invalid_utf8(&p, &data, "Project", &mut config.startup_warnings);

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
                provenance: InstructionProvenance::Project {
                    source_path: p,
                    environment_id: environment_id.to_string(),
                    cwd: cwd.clone(),
                },
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

/// Discovers AGENTS.md files from the project root to the current working
/// directory, inclusive. Symlinks are allowed.
async fn agents_md_paths(
    config: &Config,
    cwd: &AbsolutePathBuf,
    fs: &dyn ExecutorFileSystem,
) -> io::Result<Vec<AbsolutePathBuf>> {
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
    let mut project_root = None;
    if !project_root_markers.is_empty() {
        for ancestor in dir.ancestors() {
            for marker in &project_root_markers {
                let marker_path = ancestor.join(marker);
                let marker_path_uri = PathUri::from_abs_path(&marker_path);
                let marker_exists = match fs.get_metadata(&marker_path_uri, /*sandbox*/ None).await
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
    let candidate_filenames = candidate_filenames(config);
    for d in search_dirs {
        for name in &candidate_filenames {
            let candidate = d.join(name);
            let candidate_uri = PathUri::from_abs_path(&candidate);
            match fs.get_metadata(&candidate_uri, /*sandbox*/ None).await {
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
    user_instructions: Option<UserInstructions>,

    /// Ordered instructions and their provenance.
    entries: Vec<InstructionEntry>,
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
            entries: Vec::new(),
        }
    }

    fn from_user_instructions(user_instructions: Option<UserInstructions>) -> Self {
        Self {
            user_instructions: user_instructions
                .filter(|instructions| !instructions.text.trim().is_empty()),
            entries: Vec::new(),
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
            entries: vec![InstructionEntry {
                contents,
                provenance: InstructionProvenance::Internal,
            }],
        }
    }

    fn is_empty(&self) -> bool {
        self.user_instructions.is_none()
            && self
                .entries
                .iter()
                .all(|entry| entry.contents.trim().is_empty())
    }

    /// Returns the concatenated model-visible instruction text.
    pub fn text(&self) -> String {
        if self.has_multiple_project_environments() {
            self.environment_labeled_text()
        } else {
            self.legacy_text()
        }
    }

    fn legacy_text(&self) -> String {
        let mut output = String::new();
        let mut has_previous = false;
        let mut previous_was_project = false;
        if let Some(instructions) = &self.user_instructions {
            output.push_str(&instructions.text);
            has_previous = true;
        }
        for entry in &self.entries {
            let is_project = matches!(&entry.provenance, InstructionProvenance::Project { .. });
            if has_previous {
                // The project-doc marker tells the model where workspace-scoped
                // instructions begin, so it is only needed on the transition
                // from user or internal instructions to project instructions.
                let separator = if is_project && !previous_was_project {
                    AGENTS_MD_SEPARATOR
                } else {
                    "\n\n"
                };
                output.push_str(separator);
            }
            output.push_str(&entry.contents);
            has_previous = true;
            previous_was_project = is_project;
        }
        output
    }

    fn environment_labeled_text(&self) -> String {
        let mut output = String::new();
        let mut has_previous = false;
        let mut previous_environment: Option<(&str, &AbsolutePathBuf)> = None;
        if let Some(instructions) = &self.user_instructions {
            output.push_str(&instructions.text);
            has_previous = true;
        }
        for entry in &self.entries {
            match &entry.provenance {
                InstructionProvenance::Project {
                    environment_id,
                    cwd,
                    ..
                } => {
                    if has_previous {
                        output.push_str("\n\n");
                    }
                    // One environment can contribute several hierarchical AGENTS.md files from
                    // its project root through its cwd. Label that environment once for the
                    // complete group rather than repeating the label before every file.
                    let environment = (environment_id.as_str(), cwd);
                    if previous_environment != Some(environment) {
                        output.push_str(&format!(
                            "for `{}` with root {}\n\n",
                            environment_id,
                            cwd.display()
                        ));
                    }
                    output.push_str(&entry.contents);
                    previous_environment = Some(environment);
                }
                InstructionProvenance::Internal => {
                    if has_previous {
                        output.push_str("\n\n");
                    }
                    output.push_str(&entry.contents);
                    previous_environment = None;
                }
            }
            has_previous = true;
        }
        output
    }

    /// Returns the complete model-visible contextual user fragment.
    pub(crate) fn render(&self) -> String {
        // One contributing project environment retains the legacy cwd wrapper. With two or more,
        // the body labels every contributing environment itself, so the outer cwd is omitted.
        let directory = if self.has_multiple_project_environments() {
            None
        } else {
            self.single_project_cwd()
                .map(|cwd| cwd.to_string_lossy().into_owned())
        };
        ContextUserInstructions {
            directory,
            text: self.text(),
        }
        .render()
    }

    /// Returns the host-provided user instructions.
    pub(crate) fn user_instructions(&self) -> Option<&UserInstructions> {
        self.user_instructions.as_ref()
    }

    /// Returns the AGENTS.md files that supplied instruction entries.
    pub fn sources(&self) -> impl Iterator<Item = &AbsolutePathBuf> {
        self.user_instructions
            .iter()
            .map(|instructions| &instructions.source)
            .chain(
                self.entries
                    .iter()
                    .filter_map(|entry| entry.provenance.path()),
            )
    }

    fn has_multiple_project_environments(&self) -> bool {
        let mut first_environment_id = None;
        self.entries.iter().any(|entry| {
            let InstructionProvenance::Project { environment_id, .. } = &entry.provenance else {
                return false;
            };
            match first_environment_id {
                Some(first_environment_id) => first_environment_id != environment_id,
                None => {
                    first_environment_id = Some(environment_id);
                    false
                }
            }
        })
    }

    fn single_project_cwd(&self) -> Option<&AbsolutePathBuf> {
        self.entries
            .iter()
            .find_map(|entry| match &entry.provenance {
                InstructionProvenance::Project { cwd, .. } => Some(cwd),
                InstructionProvenance::Internal => None,
            })
    }
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
    /// Workspace instructions discovered from project AGENTS.md files.
    Project {
        /// Exact AGENTS.md file, distinct from the environment's selected cwd.
        source_path: AbsolutePathBuf,
        environment_id: String,
        cwd: AbsolutePathBuf,
    },

    /// Instructions without a file source, including internally defined guidance.
    Internal,
}

impl InstructionProvenance {
    fn path(&self) -> Option<&AbsolutePathBuf> {
        match self {
            Self::Project { source_path, .. } => Some(source_path),
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
