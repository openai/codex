use crate::model::SkillDependencies;
use crate::model::SkillError;
use crate::model::SkillInterface;
use crate::model::SkillLoadOutcome;
use crate::model::SkillMetadata;
use crate::model::SkillPolicy;
use crate::model::SkillToolDependency;
use crate::system::system_cache_root_dir;
use codex_app_server_protocol::ConfigLayerSource;
use codex_config::ConfigLayerStack;
use codex_config::ConfigLayerStackOrdering;
use codex_config::default_project_root_markers;
use codex_config::merge_toml_values;
use codex_config::project_root_markers_from_config;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::ExecutorRpcBatchCall;
use codex_exec_server::ExecutorRpcBatchResult;
use codex_exec_server::FS_CANONICALIZE_METHOD;
use codex_exec_server::FS_GET_METADATA_METHOD;
use codex_exec_server::FS_READ_DIRECTORY_METHOD;
use codex_exec_server::FS_READ_FILE_METHOD;
use codex_exec_server::FileMetadata;
use codex_exec_server::FsCanonicalizeParams;
use codex_exec_server::FsCanonicalizeResponse;
use codex_exec_server::FsGetMetadataParams;
use codex_exec_server::FsGetMetadataResponse;
use codex_exec_server::FsReadDirectoryParams;
use codex_exec_server::FsReadDirectoryResponse;
use codex_exec_server::FsReadFileParams;
use codex_exec_server::FsReadFileResponse;
use codex_exec_server::LOCAL_FS;
use codex_protocol::protocol::Product;
use codex_protocol::protocol::SkillScope;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::AbsolutePathBufGuard;
use codex_utils_path_uri::PathUri;
use codex_utils_plugins::PluginNamespaceResolver;
use codex_utils_plugins::PluginSkillRoot;
use dirs::home_dir;
use serde::Deserialize;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::io;
use std::path::Component;
use std::path::Path;
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

struct PreloadedSkillFile {
    contents: io::Result<String>,
    metadata_contents: io::Result<String>,
    resolved_path: AbsolutePathBuf,
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
const MAX_QUALIFIED_NAME_LEN: usize = 128;
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
    pub path: AbsolutePathBuf,
    pub scope: SkillScope,
    pub file_system: Arc<dyn ExecutorFileSystem>,
    pub plugin_id: Option<String>,
    pub plugin_namespace: Option<String>,
    pub plugin_root: Option<AbsolutePathBuf>,
}

pub async fn load_skills_from_roots<I>(
    roots: I,
    plugin_skill_snapshots: Option<&crate::PluginSkillSnapshots>,
) -> SkillLoadOutcome
where
    I: IntoIterator<Item = SkillRoot>,
{
    crate::root_loader::load_and_merge_skill_roots(roots, plugin_skill_snapshots).await
}

#[derive(Clone)]
pub(crate) struct SkillRootSnapshot {
    pub(crate) root: AbsolutePathBuf,
    pub(crate) skills: Vec<SkillMetadata>,
    pub(crate) errors: Vec<SkillError>,
    pub(crate) file_system: Arc<dyn ExecutorFileSystem>,
}

pub(crate) async fn load_skill_root(root: SkillRoot) -> SkillRootSnapshot {
    let SkillRoot {
        path,
        scope,
        file_system,
        plugin_id,
        plugin_namespace,
        plugin_root,
    } = root;
    let root = canonicalize_for_skill_identity(file_system.as_ref(), &path).await;
    let mut outcome = SkillLoadOutcome::default();
    discover_skills_under_root(
        file_system.as_ref(),
        &root,
        scope,
        plugin_id.as_deref(),
        plugin_namespace.as_deref(),
        plugin_root.as_ref(),
        &mut outcome,
    )
    .await;
    SkillRootSnapshot {
        root,
        skills: outcome.skills,
        errors: outcome.errors,
        file_system,
    }
}

pub(crate) async fn skill_roots(
    fs: Option<Arc<dyn ExecutorFileSystem>>,
    config_layer_stack: &ConfigLayerStack,
    cwd: &AbsolutePathBuf,
    plugin_skill_roots: Vec<PluginSkillRoot>,
    extra_skill_roots: Vec<AbsolutePathBuf>,
) -> Vec<SkillRoot> {
    let home_dir =
        home_dir().and_then(|path| AbsolutePathBuf::from_absolute_path_checked(path).ok());
    skill_roots_with_home_dir(
        fs,
        config_layer_stack,
        cwd,
        home_dir.as_ref(),
        plugin_skill_roots,
        extra_skill_roots,
    )
    .await
}

async fn skill_roots_with_home_dir(
    fs: Option<Arc<dyn ExecutorFileSystem>>,
    config_layer_stack: &ConfigLayerStack,
    cwd: &AbsolutePathBuf,
    home_dir: Option<&AbsolutePathBuf>,
    plugin_skill_roots: Vec<PluginSkillRoot>,
    extra_skill_roots: Vec<AbsolutePathBuf>,
) -> Vec<SkillRoot> {
    let mut roots = skill_roots_from_layer_stack_inner(config_layer_stack, home_dir, fs.clone());
    roots.extend(plugin_skill_roots.into_iter().map(|root| SkillRoot {
        path: root.path,
        scope: SkillScope::User,
        file_system: Arc::clone(&LOCAL_FS),
        plugin_id: Some(root.plugin_id),
        plugin_namespace: Some(root.plugin_namespace),
        plugin_root: Some(root.plugin_root),
    }));
    roots.extend(extra_skill_roots.into_iter().map(|path| SkillRoot {
        path,
        scope: SkillScope::User,
        file_system: Arc::clone(&LOCAL_FS),
        plugin_id: None,
        plugin_namespace: None,
        plugin_root: None,
    }));
    roots.extend(repo_agents_skill_roots(fs, config_layer_stack, cwd).await);
    dedupe_skill_roots_by_path(&mut roots);
    roots
}

fn skill_roots_from_layer_stack_inner(
    config_layer_stack: &ConfigLayerStack,
    home_dir: Option<&AbsolutePathBuf>,
    repo_fs: Option<Arc<dyn ExecutorFileSystem>>,
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
                if let Some(repo_fs) = &repo_fs {
                    roots.push(SkillRoot {
                        path: config_folder.join(SKILLS_DIR_NAME),
                        scope: SkillScope::Repo,
                        file_system: Arc::clone(repo_fs),
                        plugin_id: None,
                        plugin_namespace: None,
                        plugin_root: None,
                    });
                }
            }
            ConfigLayerSource::User { .. } => {
                // Deprecated user skills location (`$CODEX_HOME/skills`), kept for backward
                // compatibility.
                roots.push(SkillRoot {
                    path: config_folder.join(SKILLS_DIR_NAME),
                    scope: SkillScope::User,
                    file_system: Arc::clone(&LOCAL_FS),
                    plugin_id: None,
                    plugin_namespace: None,
                    plugin_root: None,
                });

                // `$HOME/.agents/skills` (user-installed skills).
                if let Some(home_dir) = home_dir {
                    roots.push(SkillRoot {
                        path: home_dir.join(AGENTS_DIR_NAME).join(SKILLS_DIR_NAME),
                        scope: SkillScope::User,
                        file_system: Arc::clone(&LOCAL_FS),
                        plugin_id: None,
                        plugin_namespace: None,
                        plugin_root: None,
                    });
                }

                // Embedded system skills are cached under `$CODEX_HOME/skills/.system` and are a
                // special case (not a config layer).
                roots.push(SkillRoot {
                    path: system_cache_root_dir(&config_folder),
                    scope: SkillScope::System,
                    file_system: Arc::clone(&LOCAL_FS),
                    plugin_id: None,
                    plugin_namespace: None,
                    plugin_root: None,
                });
            }
            ConfigLayerSource::System { .. } => {
                // The system config layer lives under `/etc/codex/` on Unix, so treat
                // `/etc/codex/skills` as admin-scoped skills.
                roots.push(SkillRoot {
                    path: config_folder.join(SKILLS_DIR_NAME),
                    scope: SkillScope::Admin,
                    file_system: Arc::clone(&LOCAL_FS),
                    plugin_id: None,
                    plugin_namespace: None,
                    plugin_root: None,
                });
            }
            ConfigLayerSource::Mdm { .. }
            | ConfigLayerSource::EnterpriseManaged { .. }
            | ConfigLayerSource::SessionFlags
            | ConfigLayerSource::LegacyManagedConfigTomlFromFile { .. }
            | ConfigLayerSource::LegacyManagedConfigTomlFromMdm => {}
        }
    }

    roots
}

async fn repo_agents_skill_roots(
    fs: Option<Arc<dyn ExecutorFileSystem>>,
    config_layer_stack: &ConfigLayerStack,
    cwd: &AbsolutePathBuf,
) -> Vec<SkillRoot> {
    let Some(fs) = fs else {
        return Vec::new();
    };
    let project_root_markers = project_root_markers_from_stack(config_layer_stack);
    let project_root = find_project_root(fs.as_ref(), cwd, &project_root_markers).await;
    let dirs = dirs_between_project_root_and_cwd(cwd, &project_root);
    let mut roots = Vec::new();
    for dir in dirs {
        let agents_skills = dir.join(AGENTS_DIR_NAME).join(SKILLS_DIR_NAME);
        let agents_skills_uri = PathUri::from_abs_path(&agents_skills);
        match fs.get_metadata(&agents_skills_uri, /*sandbox*/ None).await {
            Ok(metadata) if metadata.is_directory => roots.push(SkillRoot {
                path: agents_skills,
                scope: SkillScope::Repo,
                file_system: Arc::clone(&fs),
                plugin_id: None,
                plugin_namespace: None,
                plugin_root: None,
            }),
            Ok(_) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
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

async fn find_project_root(
    fs: &dyn ExecutorFileSystem,
    cwd: &AbsolutePathBuf,
    project_root_markers: &[String],
) -> AbsolutePathBuf {
    if project_root_markers.is_empty() {
        return cwd.clone();
    }

    for ancestor in cwd.ancestors() {
        for marker in project_root_markers {
            let marker_path = ancestor.join(marker);
            let marker_path_uri = PathUri::from_abs_path(&marker_path);
            match fs.get_metadata(&marker_path_uri, /*sandbox*/ None).await {
                Ok(_) => return ancestor,
                Err(err) if err.kind() == io::ErrorKind::NotFound => {}
                Err(err) => {
                    tracing::warn!(
                        "failed to stat project root marker {}: {err:#}",
                        marker_path.display()
                    );
                }
            }
        }
    }

    cwd.clone()
}

fn dirs_between_project_root_and_cwd(
    cwd: &AbsolutePathBuf,
    project_root: &AbsolutePathBuf,
) -> Vec<AbsolutePathBuf> {
    let mut dirs = cwd
        .ancestors()
        .scan(false, |done, dir| {
            if *done {
                None
            } else {
                if &dir == project_root {
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
    roots.retain(|root| seen.insert(root.path.clone()));
}

async fn canonicalize_for_skill_identity(
    fs: &dyn ExecutorFileSystem,
    path: &AbsolutePathBuf,
) -> AbsolutePathBuf {
    let path_uri = PathUri::from_abs_path(path);
    fs.canonicalize(&path_uri, /*sandbox*/ None)
        .await
        .and_then(|path| path.to_abs_path())
        .unwrap_or_else(|_| path.clone())
}

async fn discover_skills_under_root(
    fs: &dyn ExecutorFileSystem,
    root: &AbsolutePathBuf,
    scope: SkillScope,
    plugin_id: Option<&str>,
    plugin_namespace: Option<&str>,
    plugin_root: Option<&AbsolutePathBuf>,
    outcome: &mut SkillLoadOutcome,
) {
    let root = root.clone();
    let plugin_root = match plugin_root {
        Some(plugin_root) => Some(canonicalize_for_skill_identity(fs, plugin_root).await),
        None => None,
    };

    let root_uri = PathUri::from_abs_path(&root);
    match fs.get_metadata(&root_uri, /*sandbox*/ None).await {
        Ok(metadata) if metadata.is_directory => {}
        Ok(_) => return,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return,
        Err(err) => {
            error!("failed to stat skills root {}: {err:#}", root.display());
            return;
        }
    }

    fn enqueue_dir(
        queue: &mut Vec<(AbsolutePathBuf, usize)>,
        visited_dirs: &mut HashSet<AbsolutePathBuf>,
        truncated_by_dir_limit: &mut bool,
        path: AbsolutePathBuf,
        depth: usize,
    ) {
        if depth > MAX_SCAN_DEPTH {
            return;
        }
        if visited_dirs.len() >= MAX_SKILLS_DIRS_PER_ROOT {
            *truncated_by_dir_limit = true;
            return;
        }
        if visited_dirs.insert(path.clone()) {
            queue.push((path, depth));
        }
    }

    // Follow symlinked directories for user, admin, and repo skills. System skills are written by Codex itself.
    let follow_symlinks = matches!(
        scope,
        SkillScope::Repo | SkillScope::User | SkillScope::Admin
    );

    let mut visited_dirs: HashSet<AbsolutePathBuf> = HashSet::new();
    visited_dirs.insert(root.clone());

    let mut queue = vec![(root.clone(), 0)];
    let mut truncated_by_dir_limit = false;
    let mut skill_paths = Vec::new();

    while !queue.is_empty() {
        let dirs = std::mem::take(&mut queue);
        let directory_calls = match dirs
            .iter()
            .map(|(dir, _)| {
                rpc_batch_call(
                    FS_READ_DIRECTORY_METHOD,
                    FsReadDirectoryParams {
                        path: PathUri::from_abs_path(dir),
                        sandbox: None,
                    },
                )
            })
            .collect::<io::Result<Vec<_>>>()
        {
            Ok(calls) => calls,
            Err(err) => {
                error!("failed to build batched skills directory reads: {err:#}");
                break;
            }
        };
        let directory_results = match fs.execute_rpc_batch(directory_calls).await {
            Ok(results) => results,
            Err(err) => {
                error!("failed to batch-read skills directories: {err:#}");
                break;
            }
        };

        let mut candidates = Vec::new();
        for ((dir, depth), result) in dirs.into_iter().zip(directory_results) {
            let response: FsReadDirectoryResponse =
                match decode_rpc_batch_result(Some(result), "read skills dir") {
                    Ok(response) => response,
                    Err(err) => {
                        error!("failed to read skills dir {}: {err:#}", dir.display());
                        continue;
                    }
                };
            candidates.extend(response.entries.into_iter().filter_map(|entry| {
                let file_name = entry.file_name;
                if file_name.starts_with('.') {
                    return None;
                }
                Some((dir.join(&file_name), depth, file_name))
            }));
        }

        let metadata_calls = match candidates
            .iter()
            .map(|(path, _, _)| {
                rpc_batch_call(
                    FS_GET_METADATA_METHOD,
                    FsGetMetadataParams {
                        path: PathUri::from_abs_path(path),
                        sandbox: None,
                    },
                )
            })
            .collect::<io::Result<Vec<_>>>()
        {
            Ok(calls) => calls,
            Err(err) => {
                error!("failed to build batched skills path stats: {err:#}");
                break;
            }
        };
        let metadata_results = match fs.execute_rpc_batch(metadata_calls).await {
            Ok(results) => results,
            Err(err) => {
                error!("failed to batch-stat skills paths: {err:#}");
                break;
            }
        };

        let mut directories = Vec::new();
        for ((path, depth, file_name), result) in candidates.into_iter().zip(metadata_results) {
            let response: FsGetMetadataResponse =
                match decode_rpc_batch_result(Some(result), "stat skills path") {
                    Ok(response) => response,
                    Err(err) => {
                        error!("failed to stat skills path {}: {err:#}", path.display());
                        continue;
                    }
                };
            let metadata = FileMetadata {
                is_directory: response.is_directory,
                is_file: response.is_file,
                is_symlink: response.is_symlink,
                size: response.size,
                created_at_ms: response.created_at_ms,
                modified_at_ms: response.modified_at_ms,
            };
            if metadata.is_symlink {
                if follow_symlinks && metadata.is_directory {
                    directories.push((path, depth + 1));
                }
            } else if metadata.is_directory {
                directories.push((path, depth + 1));
            } else if metadata.is_file && file_name == SKILLS_FILENAME {
                skill_paths.push(path);
            }
        }

        let canonical_calls = match directories
            .iter()
            .map(|(path, _)| {
                rpc_batch_call(
                    FS_CANONICALIZE_METHOD,
                    FsCanonicalizeParams {
                        path: PathUri::from_abs_path(path),
                        sandbox: None,
                    },
                )
            })
            .collect::<io::Result<Vec<_>>>()
        {
            Ok(calls) => calls,
            Err(err) => {
                error!("failed to build batched skills directory canonicalization: {err:#}");
                break;
            }
        };
        let canonical_results = match fs.execute_rpc_batch(canonical_calls).await {
            Ok(results) => results,
            Err(err) => {
                error!("failed to batch-canonicalize skills directories: {err:#}");
                break;
            }
        };
        for ((path, depth), result) in directories.into_iter().zip(canonical_results) {
            let resolved_dir = match decode_rpc_batch_result::<FsCanonicalizeResponse>(
                Some(result),
                "canonicalize skills dir",
            ) {
                Ok(response) => response.path.to_abs_path().unwrap_or_else(|_| path.clone()),
                Err(err) => {
                    error!(
                        "failed to canonicalize skills dir {}: {err:#}",
                        path.display()
                    );
                    path
                }
            };
            enqueue_dir(
                &mut queue,
                &mut visited_dirs,
                &mut truncated_by_dir_limit,
                resolved_dir,
                depth,
            );
        }
    }

    let plugin_namespace_resolver = if plugin_namespace.is_none() {
        Some(PluginNamespaceResolver::load(fs, &skill_paths).await)
    } else {
        None
    };
    let preloaded_skills = match preload_skill_files(fs, &skill_paths).await {
        Ok(preloaded_skills) => preloaded_skills,
        Err(err) => {
            error!("failed to batch-read skill files: {err:#}");
            Vec::new()
        }
    };
    for (path, preloaded) in skill_paths.into_iter().zip(preloaded_skills) {
        let plugin_namespace = plugin_namespace.or_else(|| {
            plugin_namespace_resolver
                .as_ref()
                .and_then(|resolver| resolver.resolve(&path))
        });
        match parse_skill_file(
            &path,
            scope,
            plugin_id,
            plugin_namespace,
            plugin_root.as_ref(),
            preloaded,
        ) {
            Ok(skill) => outcome.skills.push(skill),
            Err(err) if scope != SkillScope::System => outcome.errors.push(SkillError {
                path,
                message: err.to_string(),
            }),
            Err(_) => {}
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

async fn preload_skill_files(
    fs: &dyn ExecutorFileSystem,
    skill_paths: &[AbsolutePathBuf],
) -> io::Result<Vec<PreloadedSkillFile>> {
    let calls = skill_paths
        .iter()
        .flat_map(|path| {
            let metadata_path = path
                .parent()
                .map(|parent| {
                    parent
                        .join(SKILLS_METADATA_DIR)
                        .join(SKILLS_METADATA_FILENAME)
                })
                .unwrap_or_else(|| path.clone());
            [
                rpc_batch_call(
                    FS_READ_FILE_METHOD,
                    FsReadFileParams {
                        path: PathUri::from_abs_path(path),
                        sandbox: None,
                    },
                ),
                rpc_batch_call(
                    FS_READ_FILE_METHOD,
                    FsReadFileParams {
                        path: PathUri::from_abs_path(&metadata_path),
                        sandbox: None,
                    },
                ),
                rpc_batch_call(
                    FS_CANONICALIZE_METHOD,
                    FsCanonicalizeParams {
                        path: PathUri::from_abs_path(path),
                        sandbox: None,
                    },
                ),
            ]
        })
        .collect::<io::Result<Vec<_>>>()?;
    let mut results = fs.execute_rpc_batch(calls).await?.into_iter();

    Ok(skill_paths
        .iter()
        .map(|path| {
            let contents = decode_text_rpc_result(results.next(), "read skill file");
            let metadata_contents = decode_text_rpc_result(results.next(), "read skill metadata");
            let resolved_path = match decode_rpc_batch_result::<FsCanonicalizeResponse>(
                results.next(),
                "canonicalize skill path",
            ) {
                Ok(response) => response.path.to_abs_path().unwrap_or_else(|_| path.clone()),
                Err(_) => path.clone(),
            };
            PreloadedSkillFile {
                contents,
                metadata_contents,
                resolved_path,
            }
        })
        .collect())
}

fn rpc_batch_call<P: Serialize>(method: &str, params: P) -> io::Result<ExecutorRpcBatchCall> {
    Ok(ExecutorRpcBatchCall {
        method: method.to_string(),
        params: serde_json::to_value(params).map_err(io::Error::other)?,
    })
}

fn decode_rpc_batch_result<T: DeserializeOwned>(
    result: Option<ExecutorRpcBatchResult>,
    operation: &str,
) -> io::Result<T> {
    match result {
        Some(Ok(value)) => serde_json::from_value(value)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error)),
        Some(Err(error)) => Err(error),
        None => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("filesystem returned no result for {operation}"),
        )),
    }
}

fn decode_text_rpc_result(
    result: Option<ExecutorRpcBatchResult>,
    operation: &str,
) -> io::Result<String> {
    let response: FsReadFileResponse = decode_rpc_batch_result(result, operation)?;
    let contents = response
        .into_bytes()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    String::from_utf8(contents).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn parse_skill_file(
    path: &AbsolutePathBuf,
    scope: SkillScope,
    plugin_id: Option<&str>,
    plugin_namespace: Option<&str>,
    plugin_root: Option<&AbsolutePathBuf>,
    preloaded: PreloadedSkillFile,
) -> Result<SkillMetadata, SkillParseError> {
    let PreloadedSkillFile {
        contents,
        metadata_contents,
        resolved_path,
    } = preloaded;
    let contents = contents.map_err(SkillParseError::Read)?;

    let frontmatter = extract_frontmatter(&contents).ok_or(SkillParseError::MissingFrontmatter)?;

    let parsed: SkillFrontmatter = match serde_yaml::from_str(&frontmatter) {
        Ok(parsed) => Ok(parsed),
        Err(original_error) => match repair_frontmatter_scalar_fields(&frontmatter) {
            // Some third-party skills use prose like `description: Build for AWS: ECS`
            // or `argument-hint: <duration: e.g. 7d>`. Keep the repair line-oriented
            // so unrelated invalid YAML still surfaces.
            Some(repaired_frontmatter) => {
                serde_yaml::from_str(&repaired_frontmatter).map_err(|_| original_error)
            }
            None => Err(original_error),
        },
    }
    .map_err(SkillParseError::InvalidYaml)?;

    let base_name = parsed
        .name
        .as_deref()
        .map(sanitize_single_line)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default_skill_name(path));
    let name = plugin_namespace
        .map(|namespace| format!("{namespace}:{base_name}"))
        .unwrap_or_else(|| base_name.clone());
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
    } = load_skill_metadata(path, plugin_root, metadata_contents);

    validate_len(&base_name, MAX_NAME_LEN, "name")?;
    validate_len(&name, MAX_QUALIFIED_NAME_LEN, "qualified name")?;
    validate_len(&description, MAX_DESCRIPTION_LEN, "description")?;
    if let Some(short_description) = short_description.as_deref() {
        validate_len(
            short_description,
            MAX_SHORT_DESCRIPTION_LEN,
            "metadata.short-description",
        )?;
    }

    Ok(SkillMetadata {
        name,
        description,
        short_description,
        interface,
        dependencies,
        policy,
        path_to_skills_md: resolved_path,
        scope,
        plugin_id: plugin_id.map(str::to_string),
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

fn load_skill_metadata(
    skill_path: &AbsolutePathBuf,
    plugin_root: Option<&AbsolutePathBuf>,
    contents: io::Result<String>,
) -> LoadedSkillMetadata {
    // Fail open: optional metadata should not block loading SKILL.md.
    let Some(skill_dir) = skill_path.parent() else {
        return LoadedSkillMetadata::default();
    };
    let metadata_path = skill_dir
        .join(SKILLS_METADATA_DIR)
        .join(SKILLS_METADATA_FILENAME);
    let contents = match contents {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return LoadedSkillMetadata::default();
        }
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
        let _guard = AbsolutePathBufGuard::new(skill_dir.as_path());
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
        interface: resolve_interface(interface, &skill_dir, plugin_root),
        dependencies: resolve_dependencies(dependencies),
        policy: resolve_policy(policy),
    }
}

fn resolve_interface(
    interface: Option<Interface>,
    skill_dir: &AbsolutePathBuf,
    plugin_root: Option<&AbsolutePathBuf>,
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
        icon_small: resolve_asset_path(
            skill_dir,
            plugin_root,
            "interface.icon_small",
            interface.icon_small,
        ),
        icon_large: resolve_asset_path(
            skill_dir,
            plugin_root,
            "interface.icon_large",
            interface.icon_large,
        ),
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
    plugin_root: Option<&AbsolutePathBuf>,
    field: &'static str,
    path: Option<PathBuf>,
) -> Option<AbsolutePathBuf> {
    // Icons must stay under the skill's assets directory. Plugin skills may
    // also share icons from the plugin-level assets directory.
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
                return resolve_plugin_shared_asset_path(skill_dir, plugin_root, field, &path);
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

fn resolve_plugin_shared_asset_path(
    skill_dir: &AbsolutePathBuf,
    plugin_root: Option<&AbsolutePathBuf>,
    field: &'static str,
    path: &Path,
) -> Option<AbsolutePathBuf> {
    let Some(plugin_root) = plugin_root else {
        tracing::warn!("ignoring {field}: icon path must not contain '..'");
        return None;
    };

    let plugin_assets_dir = lexically_normalize(plugin_root.join("assets").as_path());
    let resolved = lexically_normalize(skill_dir.join(path).as_path());
    if !resolved.starts_with(&plugin_assets_dir) {
        tracing::warn!("ignoring {field}: icon path with '..' must resolve under plugin assets/");
        return None;
    }

    AbsolutePathBuf::try_from(resolved)
        .map_err(|err| {
            tracing::warn!("ignoring {field}: icon path must resolve to an absolute path: {err}");
            err
        })
        .ok()
}

fn lexically_normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    normalized
}

fn sanitize_single_line(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn repair_frontmatter_scalar_fields(frontmatter: &str) -> Option<String> {
    let mut changed = false;
    let mut block_scalar_indent: Option<usize> = None;
    let mut repaired_lines: Vec<String> = Vec::new();
    for line in frontmatter.lines() {
        let indent = line
            .chars()
            .take_while(|character| *character == ' ')
            .count();
        if let Some(block_indent) = block_scalar_indent {
            if line.trim().is_empty() || indent > block_indent {
                repaired_lines.push(line.to_string());
                continue;
            }
            block_scalar_indent = None;
        }

        let Some((key, value)) = line.split_once(':') else {
            repaired_lines.push(line.to_string());
            continue;
        };
        if key.trim().is_empty() || !value.chars().next().is_none_or(char::is_whitespace) {
            repaired_lines.push(line.to_string());
            continue;
        }

        let trimmed_start = value.trim_start();
        let leading_whitespace = &value[..value.len() - trimmed_start.len()];
        let mut scalar = trimmed_start;
        let mut comment = "";
        for (index, character) in trimmed_start.char_indices() {
            if character == '#'
                && (index == 0
                    || trimmed_start[..index]
                        .chars()
                        .next_back()
                        .is_some_and(char::is_whitespace))
            {
                let comment_start = trimmed_start[..index].trim_end().len();
                scalar = &trimmed_start[..comment_start];
                comment = &trimmed_start[comment_start..];
                break;
            }
        }

        let scalar = scalar.trim_end();
        let Some(first_char) = scalar.chars().next() else {
            repaired_lines.push(line.to_string());
            continue;
        };
        if matches!(first_char, '|' | '>') {
            block_scalar_indent = Some(indent);
            repaired_lines.push(line.to_string());
            continue;
        }
        if matches!(first_char, '\'' | '"') {
            repaired_lines.push(line.to_string());
            continue;
        }
        let mut has_colon_separator = false;
        let mut chars = scalar.chars().peekable();
        while let Some(character) = chars.next() {
            if character == ':'
                && matches!(chars.peek(), Some(next_character) if next_character.is_whitespace())
            {
                has_colon_separator = true;
                break;
            }
        }
        let invalid_flow_like_scalar = matches!(first_char, '[' | '{' | '@' | '`')
            && serde_yaml::from_str::<serde_yaml::Value>(scalar).is_err();
        if !has_colon_separator && !invalid_flow_like_scalar {
            repaired_lines.push(line.to_string());
            continue;
        }

        let quoted_scalar = format!("'{}'", scalar.replace('\'', "''"));
        repaired_lines.push(format!(
            "{key}:{leading_whitespace}{quoted_scalar}{comment}"
        ));
        changed = true;
    }
    changed.then(|| repaired_lines.join("\n"))
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
    skill_roots_with_home_dir(
        Some(fs),
        config_layer_stack,
        cwd,
        home_dir,
        Vec::new(),
        Vec::new(),
    )
    .await
}

#[cfg(test)]
#[path = "loader_tests.rs"]
mod tests;
