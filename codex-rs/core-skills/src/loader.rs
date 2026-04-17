use crate::model::SkillDependencies;
use crate::model::SkillError;
use crate::model::SkillInterface;
use crate::model::SkillLoadOutcome;
use crate::model::SkillMetadata;
use crate::model::SkillPolicy;
use crate::model::SkillSourcesByPath;
use crate::model::SkillToolDependency;
use crate::system::system_cache_root_dir;
use codex_app_server_protocol::ConfigLayerSource;
use codex_config::ConfigLayerStack;
use codex_config::ConfigLayerStackOrdering;
use codex_config::default_project_root_markers;
use codex_config::merge_toml_values;
use codex_config::project_root_markers_from_config;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::ExecutorPath;
use codex_exec_server::LOCAL_FS;
use codex_protocol::protocol::Product;
use codex_protocol::protocol::SkillScope;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::AbsolutePathBufGuard;
use codex_utils_plugins::plugin_namespace_for_skill_path;
use dirs::home_dir;
use serde::Deserialize;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::error::Error;
use std::fmt;
use std::io;
use std::path::Component;
use std::path::PathBuf;
use std::sync::Arc;
use toml::Value as TomlValue;
use tracing::error;

#[derive(Debug, Deserialize)]
struct SkillFrontmatter {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    metadata: SkillFrontmatterMetadata,
}

#[derive(Debug, Default, Deserialize)]
struct SkillFrontmatterMetadata {
    #[serde(default, rename = "short-description")]
    short_description: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct SkillMetadataFile {
    #[serde(default)]
    interface: Option<Interface>,
    #[serde(default)]
    dependencies: Option<Dependencies>,
    #[serde(default)]
    policy: Option<Policy>,
}

#[derive(Default)]
struct LoadedSkillMetadata {
    interface: Option<SkillInterface>,
    dependencies: Option<SkillDependencies>,
    policy: Option<SkillPolicy>,
}

#[derive(Debug, Default, Deserialize)]
struct Interface {
    display_name: Option<String>,
    short_description: Option<String>,
    icon_small: Option<PathBuf>,
    icon_large: Option<PathBuf>,
    brand_color: Option<String>,
    default_prompt: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct Dependencies {
    #[serde(default)]
    tools: Vec<DependencyTool>,
}

#[derive(Debug, Deserialize)]
struct Policy {
    #[serde(default)]
    allow_implicit_invocation: Option<bool>,
    #[serde(default)]
    products: Vec<Product>,
}

#[derive(Debug, Default, Deserialize)]
struct DependencyTool {
    #[serde(rename = "type")]
    kind: Option<String>,
    value: Option<String>,
    description: Option<String>,
    transport: Option<String>,
    command: Option<String>,
    url: Option<String>,
}

const SKILLS_FILENAME: &str = "SKILL.md";
const AGENTS_DIR_NAME: &str = ".agents";
const SKILLS_METADATA_DIR: &str = "agents";
const SKILLS_METADATA_FILENAME: &str = "openai.yaml";
const SKILLS_DIR_NAME: &str = "skills";
const MAX_NAME_LEN: usize = 64;
const MAX_DESCRIPTION_LEN: usize = 1024;
const MAX_SHORT_DESCRIPTION_LEN: usize = MAX_DESCRIPTION_LEN;
const MAX_DEFAULT_PROMPT_LEN: usize = MAX_DESCRIPTION_LEN;
const MAX_DEPENDENCY_TYPE_LEN: usize = MAX_NAME_LEN;
const MAX_DEPENDENCY_TRANSPORT_LEN: usize = MAX_NAME_LEN;
const MAX_DEPENDENCY_VALUE_LEN: usize = MAX_DESCRIPTION_LEN;
const MAX_DEPENDENCY_DESCRIPTION_LEN: usize = MAX_DESCRIPTION_LEN;
const MAX_DEPENDENCY_COMMAND_LEN: usize = MAX_DESCRIPTION_LEN;
const MAX_DEPENDENCY_URL_LEN: usize = MAX_DESCRIPTION_LEN;
// Traversal depth from the skills root.
const MAX_SCAN_DEPTH: usize = 6;
const MAX_SKILLS_DIRS_PER_ROOT: usize = 2000;

#[derive(Debug)]
enum SkillParseError {
    Read(std::io::Error),
    MissingFrontmatter,
    InvalidYaml(serde_yaml::Error),
    MissingField(&'static str),
    InvalidField { field: &'static str, reason: String },
}

impl fmt::Display for SkillParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SkillParseError::Read(e) => write!(f, "failed to read file: {e}"),
            SkillParseError::MissingFrontmatter => {
                write!(f, "missing YAML frontmatter delimited by ---")
            }
            SkillParseError::InvalidYaml(e) => write!(f, "invalid YAML: {e}"),
            SkillParseError::MissingField(field) => write!(f, "missing field `{field}`"),
            SkillParseError::InvalidField { field, reason } => {
                write!(f, "invalid {field}: {reason}")
            }
        }
    }
}

impl Error for SkillParseError {}

pub struct SkillRoot {
    pub path: ExecutorPath,
    pub scope: SkillScope,
}

pub async fn load_skills_from_roots<I>(roots: I) -> SkillLoadOutcome
where
    I: IntoIterator<Item = SkillRoot>,
{
    let mut outcome = SkillLoadOutcome::default();
    let mut sources_by_skill_path: HashMap<AbsolutePathBuf, ExecutorPath> = HashMap::new();
    for root in roots {
        let skills_before_root = outcome.skills.len();
        discover_skills_under_root(&root.path, root.scope, &mut outcome).await;
        for skill in &outcome.skills[skills_before_root..] {
            sources_by_skill_path
                .entry(skill.path_to_skills_md.clone())
                .or_insert_with(|| root.path.with_path(skill.path_to_skills_md.clone()));
        }
    }

    let mut seen: HashSet<AbsolutePathBuf> = HashSet::new();
    outcome
        .skills
        .retain(|skill| seen.insert(skill.path_to_skills_md.clone()));
    let retained_skill_paths: HashSet<AbsolutePathBuf> = outcome
        .skills
        .iter()
        .map(|skill| skill.path_to_skills_md.clone())
        .collect();
    sources_by_skill_path.retain(|path, _| retained_skill_paths.contains(path));
    outcome.sources_by_skill_path = SkillSourcesByPath::new(sources_by_skill_path);

    fn scope_rank(scope: SkillScope) -> u8 {
        // Higher-priority scopes first (matches root scan order for dedupe).
        match scope {
            SkillScope::Repo => 0,
            SkillScope::User => 1,
            SkillScope::System => 2,
            SkillScope::Admin => 3,
        }
    }

    outcome.skills.sort_by(|a, b| {
        scope_rank(a.scope)
            .cmp(&scope_rank(b.scope))
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.path_to_skills_md.cmp(&b.path_to_skills_md))
    });

    outcome
}

pub(crate) async fn skill_roots(
    fs: Option<Arc<dyn ExecutorFileSystem>>,
    config_layer_stack: &ConfigLayerStack,
    cwd: &AbsolutePathBuf,
    plugin_skill_roots: Vec<AbsolutePathBuf>,
) -> Vec<SkillRoot> {
    let home_dir =
        home_dir().and_then(|path| AbsolutePathBuf::from_absolute_path_checked(path).ok());
    let fs = fs.unwrap_or_else(|| Arc::clone(&LOCAL_FS));
    let cwd = ExecutorPath::new(Arc::clone(&fs), cwd.clone());
    skill_roots_with_home_dir(
        &fs,
        Some(&cwd),
        config_layer_stack,
        home_dir.as_ref(),
        plugin_skill_roots,
    )
    .await
}

async fn skill_roots_with_home_dir(
    fs: &Arc<dyn ExecutorFileSystem>,
    cwd: Option<&ExecutorPath>,
    config_layer_stack: &ConfigLayerStack,
    home_dir: Option<&AbsolutePathBuf>,
    plugin_skill_roots: Vec<AbsolutePathBuf>,
) -> Vec<SkillRoot> {
    let mut roots = skill_roots_from_layer_stack_inner(fs, config_layer_stack, home_dir, cwd);
    roots.extend(plugin_skill_roots.into_iter().map(|path| SkillRoot {
        path: ExecutorPath::new(Arc::clone(fs), path),
        scope: SkillScope::User,
    }));
    roots.extend(repo_agents_skill_roots(cwd, config_layer_stack).await);
    dedupe_skill_roots_by_path(&mut roots);
    roots
}

fn skill_roots_from_layer_stack_inner(
    fs: &Arc<dyn ExecutorFileSystem>,
    config_layer_stack: &ConfigLayerStack,
    home_dir: Option<&AbsolutePathBuf>,
    repo_cwd: Option<&ExecutorPath>,
) -> Vec<SkillRoot> {
    let mut roots = Vec::new();

    for layer in config_layer_stack.get_layers(
        ConfigLayerStackOrdering::HighestPrecedenceFirst,
        /*include_disabled*/ true,
    ) {
        let Some(config_folder) = layer.config_folder() else {
            continue;
        };

        match &layer.name {
            ConfigLayerSource::Project { .. } => {
                if let Some(repo_cwd) = repo_cwd {
                    roots.push(SkillRoot {
                        path: repo_cwd.with_path(config_folder.join(SKILLS_DIR_NAME)),
                        scope: SkillScope::Repo,
                    });
                }
            }
            ConfigLayerSource::User { .. } => {
                // Deprecated user skills location (`$CODEX_HOME/skills`), kept for backward
                // compatibility.
                roots.push(SkillRoot {
                    path: ExecutorPath::new(Arc::clone(fs), config_folder.join(SKILLS_DIR_NAME)),
                    scope: SkillScope::User,
                });

                // `$HOME/.agents/skills` (user-installed skills).
                if let Some(home_dir) = home_dir {
                    roots.push(SkillRoot {
                        path: ExecutorPath::new(
                            Arc::clone(fs),
                            home_dir.join(AGENTS_DIR_NAME).join(SKILLS_DIR_NAME),
                        ),
                        scope: SkillScope::User,
                    });
                }

                // Embedded system skills are cached under `$CODEX_HOME/skills/.system` and are a
                // special case (not a config layer).
                roots.push(SkillRoot {
                    path: ExecutorPath::new(Arc::clone(fs), system_cache_root_dir(&config_folder)),
                    scope: SkillScope::System,
                });
            }
            ConfigLayerSource::System { .. } => {
                // The system config layer lives under `/etc/codex/` on Unix, so treat
                // `/etc/codex/skills` as admin-scoped skills.
                roots.push(SkillRoot {
                    path: ExecutorPath::new(Arc::clone(fs), config_folder.join(SKILLS_DIR_NAME)),
                    scope: SkillScope::Admin,
                });
            }
            ConfigLayerSource::Mdm { .. }
            | ConfigLayerSource::SessionFlags
            | ConfigLayerSource::LegacyManagedConfigTomlFromFile { .. }
            | ConfigLayerSource::LegacyManagedConfigTomlFromMdm => {}
        }
    }

    roots
}

async fn repo_agents_skill_roots(
    cwd: Option<&ExecutorPath>,
    config_layer_stack: &ConfigLayerStack,
) -> Vec<SkillRoot> {
    let Some(cwd) = cwd else {
        return Vec::new();
    };
    let project_root_markers = project_root_markers_from_stack(config_layer_stack);
    let project_root = find_project_root(cwd, &project_root_markers).await;
    let dirs = dirs_between_project_root_and_cwd(cwd, &project_root);
    let mut roots = Vec::new();
    for dir in dirs {
        let agents_skills = dir.join(AGENTS_DIR_NAME).join(SKILLS_DIR_NAME);
        match agents_skills.unsandboxed().is_dir().await {
            Ok(true) => roots.push(SkillRoot {
                path: agents_skills,
                scope: SkillScope::Repo,
            }),
            Ok(false) => {}
            Err(err) => {
                tracing::warn!(
                    "failed to stat repo skills root {}: {err:#}",
                    agents_skills.display()
                );
            }
        }
    }
    roots
}

fn project_root_markers_from_stack(config_layer_stack: &ConfigLayerStack) -> Vec<String> {
    let mut merged = TomlValue::Table(toml::map::Map::new());
    for layer in config_layer_stack.get_layers(
        ConfigLayerStackOrdering::LowestPrecedenceFirst,
        /*include_disabled*/ false,
    ) {
        if matches!(layer.name, ConfigLayerSource::Project { .. }) {
            continue;
        }
        merge_toml_values(&mut merged, &layer.config);
    }

    match project_root_markers_from_config(&merged) {
        Ok(Some(markers)) => markers,
        Ok(None) => default_project_root_markers(),
        Err(err) => {
            tracing::warn!("invalid project_root_markers: {err}");
            default_project_root_markers()
        }
    }
}

async fn find_project_root(cwd: &ExecutorPath, project_root_markers: &[String]) -> AbsolutePathBuf {
    if project_root_markers.is_empty() {
        return cwd.path().clone();
    }

    for ancestor in cwd.ancestors() {
        for marker in project_root_markers {
            let marker_path = ancestor.join(marker);
            match marker_path.unsandboxed().exists().await {
                Ok(true) => return ancestor.into_path(),
                Ok(false) => {}
                Err(err) => {
                    tracing::warn!(
                        "failed to stat project root marker {}: {err:#}",
                        marker_path.display()
                    );
                }
            }
        }
    }

    cwd.path().clone()
}

fn dirs_between_project_root_and_cwd(
    cwd: &ExecutorPath,
    project_root: &AbsolutePathBuf,
) -> Vec<ExecutorPath> {
    let mut dirs = cwd
        .ancestors()
        .scan(false, |done, dir| {
            if *done {
                None
            } else {
                if dir.path() == project_root {
                    *done = true;
                }
                Some(dir)
            }
        })
        .collect::<Vec<_>>();
    dirs.reverse();
    dirs
}

fn dedupe_skill_roots_by_path(roots: &mut Vec<SkillRoot>) {
    let mut seen: HashSet<AbsolutePathBuf> = HashSet::new();
    roots.retain(|root| seen.insert(root.path.path().clone()));
}

fn canonicalize_for_skill_identity(path: &ExecutorPath) -> ExecutorPath {
    if !path.is_same_file_system(&LOCAL_FS) {
        return path.clone();
    }

    path.with_path(
        path.path()
            .canonicalize()
            .unwrap_or_else(|_| path.path().clone()),
    )
}

async fn discover_skills_under_root(
    root: &ExecutorPath,
    scope: SkillScope,
    outcome: &mut SkillLoadOutcome,
) {
    let root = canonicalize_for_skill_identity(root);

    match root.unsandboxed().is_dir().await {
        Ok(true) => {}
        Ok(false) => return,
        Err(err) => {
            error!("failed to stat skills root {}: {err:#}", root.display());
            return;
        }
    }

    fn enqueue_dir(
        queue: &mut VecDeque<(ExecutorPath, usize)>,
        visited_dirs: &mut HashSet<AbsolutePathBuf>,
        truncated_by_dir_limit: &mut bool,
        path: ExecutorPath,
        depth: usize,
    ) {
        if depth > MAX_SCAN_DEPTH {
            return;
        }
        if visited_dirs.len() >= MAX_SKILLS_DIRS_PER_ROOT {
            *truncated_by_dir_limit = true;
            return;
        }
        if visited_dirs.insert(path.path().clone()) {
            queue.push_back((path, depth));
        }
    }

    // Follow symlinked directories for user, admin, and repo skills. System skills are written by Codex itself.
    let follow_symlinks = matches!(
        scope,
        SkillScope::Repo | SkillScope::User | SkillScope::Admin
    );

    let mut visited_dirs: HashSet<AbsolutePathBuf> = HashSet::new();
    visited_dirs.insert(root.path().clone());

    let mut queue: VecDeque<(ExecutorPath, usize)> = VecDeque::from([(root.clone(), 0)]);
    let mut truncated_by_dir_limit = false;

    while let Some((dir, depth)) = queue.pop_front() {
        let entries = match dir.unsandboxed().read_directory().await {
            Ok(entries) => entries,
            Err(e) => {
                error!("failed to read skills dir {}: {e:#}", dir.display());
                continue;
            }
        };

        for entry in entries {
            let file_name = entry.file_name;
            if file_name.starts_with('.') {
                continue;
            }

            let path = dir.join(&file_name);
            let metadata = match path.unsandboxed().get_metadata().await {
                Ok(metadata) => metadata,
                Err(e) => {
                    error!("failed to stat skills path {}: {e:#}", path.display());
                    continue;
                }
            };

            if metadata.is_symlink {
                if !follow_symlinks {
                    continue;
                }
                match path.unsandboxed().is_dir().await {
                    Ok(true) => {
                        let resolved_dir = canonicalize_for_skill_identity(&path);
                        enqueue_dir(
                            &mut queue,
                            &mut visited_dirs,
                            &mut truncated_by_dir_limit,
                            resolved_dir,
                            depth + 1,
                        );
                    }
                    Ok(false) => {}
                    Err(err) => {
                        error!(
                            "failed to read skills symlink dir {}: {err:#}",
                            path.display()
                        );
                    }
                }
                continue;
            }

            if metadata.is_directory {
                let resolved_dir = canonicalize_for_skill_identity(&path);
                enqueue_dir(
                    &mut queue,
                    &mut visited_dirs,
                    &mut truncated_by_dir_limit,
                    resolved_dir,
                    depth + 1,
                );
                continue;
            }

            if metadata.is_file && file_name == SKILLS_FILENAME {
                match parse_skill_file(&path, scope).await {
                    Ok(skill) => {
                        outcome.skills.push(skill);
                    }
                    Err(err) => {
                        if scope != SkillScope::System {
                            outcome.errors.push(SkillError {
                                path: path.path().clone(),
                                message: err.to_string(),
                            });
                        }
                    }
                }
            }
        }
    }

    if truncated_by_dir_limit {
        tracing::warn!(
            "skills scan truncated after {} directories (root: {})",
            MAX_SKILLS_DIRS_PER_ROOT,
            root.display()
        );
    }
}

async fn parse_skill_file(
    path: &ExecutorPath,
    scope: SkillScope,
) -> Result<SkillMetadata, SkillParseError> {
    let contents = path
        .unsandboxed()
        .read_file_text()
        .await
        .map_err(SkillParseError::Read)?;

    let frontmatter = extract_frontmatter(&contents).ok_or(SkillParseError::MissingFrontmatter)?;

    let parsed: SkillFrontmatter =
        serde_yaml::from_str(&frontmatter).map_err(SkillParseError::InvalidYaml)?;

    let base_name = parsed
        .name
        .as_deref()
        .map(sanitize_single_line)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default_skill_name(path.path()));
    let name = namespaced_skill_name(path, &base_name).await;
    let description = parsed
        .description
        .as_deref()
        .map(sanitize_single_line)
        .unwrap_or_default();
    let short_description = parsed
        .metadata
        .short_description
        .as_deref()
        .map(sanitize_single_line)
        .filter(|value| !value.is_empty());
    let LoadedSkillMetadata {
        interface,
        dependencies,
        policy,
    } = load_skill_metadata(path).await;

    validate_len(&name, MAX_NAME_LEN, "name")?;
    validate_len(&description, MAX_DESCRIPTION_LEN, "description")?;
    if let Some(short_description) = short_description.as_deref() {
        validate_len(
            short_description,
            MAX_SHORT_DESCRIPTION_LEN,
            "metadata.short-description",
        )?;
    }

    let resolved_path = canonicalize_for_skill_identity(path);

    Ok(SkillMetadata {
        name,
        description,
        short_description,
        interface,
        dependencies,
        policy,
        path_to_skills_md: resolved_path.into_path(),
        scope,
    })
}

fn default_skill_name(path: &AbsolutePathBuf) -> String {
    path.parent()
        .and_then(|parent| {
            parent
                .file_name()
                .and_then(|name| name.to_str())
                .map(sanitize_single_line)
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "skill".to_string())
}

async fn namespaced_skill_name(path: &ExecutorPath, base_name: &str) -> String {
    plugin_namespace_for_skill_path(&path.as_ref())
        .await
        .map(|namespace| format!("{namespace}:{base_name}"))
        .unwrap_or_else(|| base_name.to_string())
}

async fn load_skill_metadata(skill_path: &ExecutorPath) -> LoadedSkillMetadata {
    // Fail open: optional metadata should not block loading SKILL.md.
    let Some(skill_dir) = skill_path.parent() else {
        return LoadedSkillMetadata::default();
    };
    let metadata_path = skill_dir
        .join(SKILLS_METADATA_DIR)
        .join(SKILLS_METADATA_FILENAME);
    match metadata_path.unsandboxed().is_file().await {
        Ok(true) => {}
        Ok(false) => return LoadedSkillMetadata::default(),
        Err(error) => {
            tracing::warn!(
                "ignoring {path}: failed to stat {label}: {error}",
                path = metadata_path.display(),
                label = SKILLS_METADATA_FILENAME
            );
            return LoadedSkillMetadata::default();
        }
    }

    let contents = match metadata_path.unsandboxed().read_file_text().await {
        Ok(contents) => contents,
        Err(error) => {
            tracing::warn!(
                "ignoring {path}: failed to read {label}: {error}",
                path = metadata_path.display(),
                label = SKILLS_METADATA_FILENAME
            );
            return LoadedSkillMetadata::default();
        }
    };

    let parsed: SkillMetadataFile = {
        let _guard = AbsolutePathBufGuard::new(skill_dir.path().as_path());
        match serde_yaml::from_str(&contents) {
            Ok(parsed) => parsed,
            Err(error) => {
                tracing::warn!(
                    "ignoring {path}: invalid {label}: {error}",
                    path = metadata_path.display(),
                    label = SKILLS_METADATA_FILENAME
                );
                return LoadedSkillMetadata::default();
            }
        }
    };

    let SkillMetadataFile {
        interface,
        dependencies,
        policy,
    } = parsed;
    LoadedSkillMetadata {
        interface: resolve_interface(interface, skill_dir.path()),
        dependencies: resolve_dependencies(dependencies),
        policy: resolve_policy(policy),
    }
}

fn resolve_interface(
    interface: Option<Interface>,
    skill_dir: &AbsolutePathBuf,
) -> Option<SkillInterface> {
    let interface = interface?;
    let interface = SkillInterface {
        display_name: resolve_str(
            interface.display_name,
            MAX_NAME_LEN,
            "interface.display_name",
        ),
        short_description: resolve_str(
            interface.short_description,
            MAX_SHORT_DESCRIPTION_LEN,
            "interface.short_description",
        ),
        icon_small: resolve_asset_path(skill_dir, "interface.icon_small", interface.icon_small),
        icon_large: resolve_asset_path(skill_dir, "interface.icon_large", interface.icon_large),
        brand_color: resolve_color_str(interface.brand_color, "interface.brand_color"),
        default_prompt: resolve_str(
            interface.default_prompt,
            MAX_DEFAULT_PROMPT_LEN,
            "interface.default_prompt",
        ),
    };
    let has_fields = interface.display_name.is_some()
        || interface.short_description.is_some()
        || interface.icon_small.is_some()
        || interface.icon_large.is_some()
        || interface.brand_color.is_some()
        || interface.default_prompt.is_some();
    if has_fields { Some(interface) } else { None }
}

fn resolve_dependencies(dependencies: Option<Dependencies>) -> Option<SkillDependencies> {
    let dependencies = dependencies?;
    let tools: Vec<SkillToolDependency> = dependencies
        .tools
        .into_iter()
        .filter_map(resolve_dependency_tool)
        .collect();
    if tools.is_empty() {
        None
    } else {
        Some(SkillDependencies { tools })
    }
}

fn resolve_policy(policy: Option<Policy>) -> Option<SkillPolicy> {
    policy.map(|policy| SkillPolicy {
        allow_implicit_invocation: policy.allow_implicit_invocation,
        products: policy.products,
    })
}

fn resolve_dependency_tool(tool: DependencyTool) -> Option<SkillToolDependency> {
    let r#type = resolve_required_str(
        tool.kind,
        MAX_DEPENDENCY_TYPE_LEN,
        "dependencies.tools.type",
    )?;
    let value = resolve_required_str(
        tool.value,
        MAX_DEPENDENCY_VALUE_LEN,
        "dependencies.tools.value",
    )?;
    let description = resolve_str(
        tool.description,
        MAX_DEPENDENCY_DESCRIPTION_LEN,
        "dependencies.tools.description",
    );
    let transport = resolve_str(
        tool.transport,
        MAX_DEPENDENCY_TRANSPORT_LEN,
        "dependencies.tools.transport",
    );
    let command = resolve_str(
        tool.command,
        MAX_DEPENDENCY_COMMAND_LEN,
        "dependencies.tools.command",
    );
    let url = resolve_str(tool.url, MAX_DEPENDENCY_URL_LEN, "dependencies.tools.url");

    Some(SkillToolDependency {
        r#type,
        value,
        description,
        transport,
        command,
        url,
    })
}

fn resolve_asset_path(
    skill_dir: &AbsolutePathBuf,
    field: &'static str,
    path: Option<PathBuf>,
) -> Option<AbsolutePathBuf> {
    // Icons must be relative paths under the skill's assets/ directory; otherwise return None.
    let path = path?;
    if path.as_os_str().is_empty() {
        return None;
    }

    let assets_dir = skill_dir.join("assets");
    if path.is_absolute() {
        tracing::warn!(
            "ignoring {field}: icon must be a relative assets path (not {})",
            assets_dir.display()
        );
        return None;
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(component) => normalized.push(component),
            Component::ParentDir => {
                tracing::warn!("ignoring {field}: icon path must not contain '..'");
                return None;
            }
            _ => {
                tracing::warn!("ignoring {field}: icon path must be under assets/");
                return None;
            }
        }
    }

    let mut components = normalized.components();
    match components.next() {
        Some(Component::Normal(component)) if component == "assets" => {}
        _ => {
            tracing::warn!("ignoring {field}: icon path must be under assets/");
            return None;
        }
    }

    Some(skill_dir.join(normalized))
}

fn sanitize_single_line(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn validate_len(
    value: &str,
    max_len: usize,
    field_name: &'static str,
) -> Result<(), SkillParseError> {
    if value.is_empty() {
        return Err(SkillParseError::MissingField(field_name));
    }
    if value.chars().count() > max_len {
        return Err(SkillParseError::InvalidField {
            field: field_name,
            reason: format!("exceeds maximum length of {max_len} characters"),
        });
    }
    Ok(())
}

fn resolve_str(value: Option<String>, max_len: usize, field: &'static str) -> Option<String> {
    let value = value?;
    let value = sanitize_single_line(&value);
    if value.is_empty() {
        tracing::warn!("ignoring {field}: value is empty");
        return None;
    }
    if value.chars().count() > max_len {
        tracing::warn!("ignoring {field}: exceeds maximum length of {max_len} characters");
        return None;
    }
    Some(value)
}

fn resolve_required_str(
    value: Option<String>,
    max_len: usize,
    field: &'static str,
) -> Option<String> {
    let Some(value) = value else {
        tracing::warn!("ignoring {field}: value is missing");
        return None;
    };
    resolve_str(Some(value), max_len, field)
}

fn resolve_color_str(value: Option<String>, field: &'static str) -> Option<String> {
    let value = value?;
    let value = value.trim();
    if value.is_empty() {
        tracing::warn!("ignoring {field}: value is empty");
        return None;
    }
    let mut chars = value.chars();
    if value.len() == 7 && chars.next() == Some('#') && chars.all(|c| c.is_ascii_hexdigit()) {
        Some(value.to_string())
    } else {
        tracing::warn!("ignoring {field}: expected #RRGGBB, got {value}");
        None
    }
}

fn extract_frontmatter(contents: &str) -> Option<String> {
    let mut lines = contents.lines();
    if !matches!(lines.next(), Some(line) if line.trim() == "---") {
        return None;
    }

    let mut frontmatter_lines: Vec<&str> = Vec::new();
    let mut found_closing = false;
    for line in lines.by_ref() {
        if line.trim() == "---" {
            found_closing = true;
            break;
        }
        frontmatter_lines.push(line);
    }

    if frontmatter_lines.is_empty() || !found_closing {
        return None;
    }

    Some(frontmatter_lines.join("\n"))
}
#[cfg(test)]
pub(crate) async fn skill_roots_from_layer_stack(
    fs: Arc<dyn ExecutorFileSystem>,
    config_layer_stack: &ConfigLayerStack,
    cwd: &AbsolutePathBuf,
    home_dir: Option<&AbsolutePathBuf>,
) -> Vec<SkillRoot> {
    let cwd = ExecutorPath::new(fs, cwd.clone());
    skill_roots_with_home_dir(Some(&cwd), config_layer_stack, home_dir, Vec::new()).await
}

#[cfg(test)]
#[path = "loader_tests.rs"]
mod tests;
