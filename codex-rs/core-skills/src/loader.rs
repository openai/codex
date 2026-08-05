mod discovery;
mod environment;
mod namespace;

pub use crate::root_loader::load_skill_root_snapshot;
pub use environment::EnvironmentSkillLoadOutcome;
pub use environment::EnvironmentSkillMetadata;
pub use environment::load_environment_skills_from_root;

use crate::model::SkillDependencies;
use crate::model::SkillError;
use crate::model::SkillInterface;
use crate::model::SkillLoadOutcome;
use crate::model::SkillMetadata;
use crate::model::SkillPolicy;
use crate::model::SkillToolDependency;
use codex_exec_server::ExecutorFileSystem;
use codex_protocol::protocol::Product;
use codex_protocol::protocol::SkillScope;
use codex_skills::ParsedSkillFrontmatter;
use codex_skills::SkillInterfaceAssetPolicy;
use codex_skills::SkillInterfaceFile;
use codex_skills::SkillParseError;
use codex_skills::parse_skill_frontmatter_metadata;
use codex_skills::resolve_skill_interface;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::AbsolutePathBufGuard;
use codex_utils_path_uri::PathUri;
use codex_utils_plugins::PluginIdentity;
use codex_utils_plugins::SkillDiscoveryMode;
use discovery::DirectorySymlinkPolicy;
use discovery::DiscoveredSkill;
use discovery::HiddenDirectoryPolicy;
use discovery::MAX_CONCURRENT_SKILL_LOADS;
use discovery::SkillDiscovery;
use discovery::SkillDiscoveryOptions;
use discovery::SkillMetadataDiscovery;
use discovery::discover_skills;
use futures::FutureExt;
use futures::StreamExt;
use namespace::SkillNamespaceResolver;
use serde::Deserialize;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::io;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::error;

// TODO(anp): Tune this eight-scan limit after revisiting byte-based backpressure.
pub const MAX_CONCURRENT_ROOT_SCANS: usize = 8;

#[derive(Debug, Default, Deserialize)]
struct SkillMetadataFile {
    #[serde(default)]
    interface: Option<SkillInterfaceFile>,
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
const SKILLS_METADATA_DIR: &str = "agents";
const SKILLS_METADATA_FILENAME: &str = "openai.yaml";
const MAX_NAME_LEN: usize = 64;
const MAX_QUALIFIED_NAME_LEN: usize = MAX_NAME_LEN * 2 + 1;
const MAX_DESCRIPTION_LEN: usize = 1024;
const MAX_DEPENDENCY_TYPE_LEN: usize = MAX_NAME_LEN;
const MAX_DEPENDENCY_TRANSPORT_LEN: usize = MAX_NAME_LEN;
const MAX_DEPENDENCY_VALUE_LEN: usize = MAX_DESCRIPTION_LEN;
const MAX_DEPENDENCY_DESCRIPTION_LEN: usize = MAX_DESCRIPTION_LEN;
const MAX_DEPENDENCY_COMMAND_LEN: usize = MAX_DESCRIPTION_LEN;
const MAX_DEPENDENCY_URL_LEN: usize = MAX_DESCRIPTION_LEN;
// Traversal depth from the skills root.
const MAX_SCAN_DEPTH: usize = 6;
const MAX_SKILLS_DIRS_PER_ROOT: usize = 2000;

struct ResolvedDiscoveredSkill {
    skill: DiscoveredSkill,
    path: AbsolutePathBuf,
    path_uri: PathUri,
}

#[derive(Debug)]
enum SkillLoadError {
    Read(std::io::Error),
    Parse(SkillParseError),
}

impl fmt::Display for SkillLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => write!(f, "failed to read file: {error}"),
            Self::Parse(error) => write!(f, "{error}"),
        }
    }
}

impl Error for SkillLoadError {}

impl From<SkillParseError> for SkillLoadError {
    fn from(error: SkillParseError) -> Self {
        Self::Parse(error)
    }
}

pub struct SkillRoot {
    pub path: AbsolutePathBuf,
    pub scope: SkillScope,
    pub file_system: Arc<dyn ExecutorFileSystem>,
    pub plugin_identity: Option<PluginIdentity>,
    pub plugin_namespace: Option<String>,
    pub plugin_root: Option<AbsolutePathBuf>,
    pub discovery_mode: SkillDiscoveryMode,
}

pub async fn load_skills_from_roots<I>(
    roots: I,
    plugin_skill_snapshots: Option<&crate::PluginSkillSnapshots>,
    root_scan_slots: Arc<Semaphore>,
) -> SkillLoadOutcome
where
    I: IntoIterator<Item = SkillRoot> + Send,
    I::IntoIter: Send,
{
    crate::root_loader::load_and_merge_skill_roots(roots, plugin_skill_snapshots, &root_scan_slots)
        .boxed()
        .await
}

/// One loaded skill root and the filesystem that supplied it.
///
/// This is exposed temporarily while host discovery moves out of `core-skills`; callers should
/// combine snapshots through [`SkillLoadOutcome::from_root_snapshots`].
#[derive(Clone)]
pub struct SkillRootSnapshot {
    pub(crate) root: AbsolutePathBuf,
    pub(crate) is_agent_plugin: bool,
    pub(crate) skills: Vec<SkillMetadata>,
    pub(crate) skill_discovery_path_by_path: Arc<HashMap<AbsolutePathBuf, AbsolutePathBuf>>,
    pub(crate) errors: Vec<SkillError>,
    pub(crate) file_system: Arc<dyn ExecutorFileSystem>,
}

impl SkillRootSnapshot {
    pub fn new(
        root: AbsolutePathBuf,
        skills: Vec<SkillMetadata>,
        skill_discovery_path_by_path: Arc<HashMap<AbsolutePathBuf, AbsolutePathBuf>>,
        errors: Vec<SkillError>,
        file_system: Arc<dyn ExecutorFileSystem>,
    ) -> Self {
        Self {
            root,
            is_agent_plugin: false,
            skills,
            skill_discovery_path_by_path,
            errors,
            file_system,
        }
    }
}

pub(crate) async fn load_skill_root(root: SkillRoot) -> SkillRootSnapshot {
    let canonical_root =
        canonicalize_for_skill_identity(root.file_system.as_ref(), &root.path).await;
    let mut outcome = SkillLoadOutcome::default();
    load_skills_under_root(&root, &canonical_root, &mut outcome).await;
    SkillRootSnapshot {
        root: canonical_root,
        is_agent_plugin: root.discovery_mode == SkillDiscoveryMode::DirectChildren,
        skills: outcome.skills,
        skill_discovery_path_by_path: outcome.skill_discovery_path_by_path,
        errors: outcome.errors,
        file_system: root.file_system,
    }
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

async fn load_skills_under_root(
    skill_root: &SkillRoot,
    root: &AbsolutePathBuf,
    outcome: &mut SkillLoadOutcome,
) {
    let fs = skill_root.file_system.as_ref();
    let plugin_identity = skill_root.plugin_identity.as_ref();
    let plugin_root = match skill_root.plugin_root.as_ref() {
        Some(plugin_root) => Some(canonicalize_for_skill_identity(fs, plugin_root).await),
        None => None,
    };
    let directory_symlinks = match skill_root.scope {
        SkillScope::User | SkillScope::Repo | SkillScope::Admin => DirectorySymlinkPolicy::Follow,
        SkillScope::System => DirectorySymlinkPolicy::Ignore,
    };
    let SkillDiscovery {
        skills,
        plugin_roots,
        mut namespace_roots,
        warnings,
    } = discover_skills(
        fs,
        &PathUri::from_abs_path(root),
        // Preserve host discovery behavior: directory aliases are scope-dependent, while hidden
        // directories are skipped unless reached through a visible alias.
        SkillDiscoveryOptions {
            directory_symlinks,
            hidden_directories: HiddenDirectoryPolicy::Skip,
            mode: skill_root.discovery_mode,
        },
    )
    .await;
    for warning in warnings {
        error!("{warning}");
    }
    // With no skills, there is nothing to canonicalize, parse, or namespace-qualify.
    if skills.is_empty() {
        return;
    }
    let root_uri = PathUri::from_abs_path(root);
    let resolved_plugin_root = plugin_root.as_ref();
    let resolved_skills = futures::stream::iter(skills)
        .map(|skill| async move {
            let path_uri = match fs.canonicalize(&skill.path, /*sandbox*/ None).await {
                Ok(path) => path,
                Err(err) if skill_root.discovery_mode == SkillDiscoveryMode::DirectChildren => {
                    error!(
                        "failed to resolve Agent Plugin skill path {}: {err}",
                        skill.path
                    );
                    return None;
                }
                Err(_) => skill.path.clone(),
            };
            let path = match path_uri.to_abs_path() {
                Ok(path) => path,
                Err(err) => {
                    error!("failed to convert discovered skill path {path_uri}: {err}");
                    return None;
                }
            };
            if skill_root.discovery_mode == SkillDiscoveryMode::DirectChildren {
                let Some(plugin_root) = resolved_plugin_root else {
                    error!("Agent Plugin skill root is missing its plugin root");
                    return None;
                };
                if !path.as_path().starts_with(plugin_root.as_path()) {
                    error!(
                        "Agent Plugin skill path {} resolves outside plugin root {}",
                        path.display(),
                        plugin_root.display()
                    );
                    return None;
                }
                match fs.get_metadata(&path_uri, /*sandbox*/ None).await {
                    Ok(metadata) if metadata.is_file => {}
                    Ok(_) => {
                        error!(
                            "Agent Plugin skill path {} is not a regular file",
                            path.display()
                        );
                        return None;
                    }
                    Err(err) => {
                        error!(
                            "failed to inspect Agent Plugin skill path {}: {err}",
                            path.display()
                        );
                        return None;
                    }
                }
            }
            Some(ResolvedDiscoveredSkill {
                skill,
                path,
                path_uri,
            })
        })
        .buffered(MAX_CONCURRENT_SKILL_LOADS)
        .filter_map(futures::future::ready)
        .collect::<Vec<_>>()
        .await;
    namespace_roots.extend(resolved_skills.iter().filter_map(|skill| {
        (skill.path_uri != skill.skill.path)
            .then(|| skill.path_uri.parent())
            .flatten()
    }));
    let skill_paths = resolved_skills
        .iter()
        .map(|skill| skill.path_uri.clone())
        .collect::<Vec<_>>();
    let namespace_resolver = async {
        match skill_root.plugin_namespace.as_deref() {
            Some(namespace) => SkillNamespaceResolver::with_provided_namespace(namespace),
            None => {
                SkillNamespaceResolver::discover(
                    fs,
                    &root_uri,
                    &skill_paths,
                    plugin_roots,
                    namespace_roots,
                )
                .await
            }
        }
    };
    let skill_results = futures::stream::iter(resolved_skills)
        .map(|skill| {
            let plugin_root = plugin_root.as_ref();
            async move {
                let discovery_path = skill
                    .skill
                    .path
                    .to_abs_path()
                    .unwrap_or_else(|_| skill.path.clone());
                let result = parse_skill_file(
                    fs,
                    &skill.skill,
                    &skill.path,
                    &skill.path_uri,
                    skill_root.scope,
                    plugin_identity,
                    plugin_root,
                )
                .await
                .map_err(|err| err.to_string());
                (skill.path, skill.path_uri, discovery_path, result)
            }
        })
        .buffered(MAX_CONCURRENT_SKILL_LOADS)
        .collect::<Vec<_>>()
        .boxed();
    let (namespace_resolver, skill_results) = tokio::join!(namespace_resolver, skill_results);
    for (path, path_uri, discovery_path, result) in skill_results {
        let result = result.and_then(|mut skill| {
            skill.name = namespace_resolver
                .for_skill(&root_uri, &path_uri)
                .qualify(&skill.name);
            validate_len(&skill.name, MAX_QUALIFIED_NAME_LEN, "qualified name")
                .map_err(|err| err.to_string())?;
            Ok(skill)
        });
        match result {
            Ok(skill) => {
                Arc::make_mut(&mut outcome.skill_discovery_path_by_path)
                    .insert(skill.path_to_skills_md.clone(), discovery_path);
                outcome.skills.push(skill);
            }
            Err(err) if skill_root.scope != SkillScope::System => {
                outcome.errors.push(SkillError { path, message: err })
            }
            Err(_) => {}
        }
    }
}

async fn parse_skill_file(
    fs: &dyn ExecutorFileSystem,
    skill: &DiscoveredSkill,
    path: &AbsolutePathBuf,
    path_uri: &PathUri,
    scope: SkillScope,
    plugin_identity: Option<&PluginIdentity>,
    plugin_root: Option<&AbsolutePathBuf>,
) -> Result<SkillMetadata, SkillLoadError> {
    let metadata_path = path_uri
        .parent()
        .and_then(|parent| parent.join(SKILLS_METADATA_DIR).ok())
        .and_then(|directory| directory.join(SKILLS_METADATA_FILENAME).ok());
    let metadata = match &skill.metadata {
        SkillMetadataDiscovery::Present(_) => metadata_path.map(SkillMetadataDiscovery::Present),
        SkillMetadataDiscovery::Probe(_) => metadata_path.map(SkillMetadataDiscovery::Probe),
        SkillMetadataDiscovery::Absent => None,
    }
    .unwrap_or(SkillMetadataDiscovery::Absent);
    let (contents, loaded_metadata) = tokio::join!(
        fs.read_file_text(path_uri, /*sandbox*/ None),
        load_skill_metadata(fs, path, &metadata, plugin_root),
    );
    let contents = contents.map_err(SkillLoadError::Read)?;
    let ParsedSkillFrontmatter {
        name: base_name,
        description,
        short_description,
    } = parse_skill_frontmatter_metadata(&contents, || default_skill_name(path))?;
    let LoadedSkillMetadata {
        interface,
        dependencies,
        policy,
    } = loaded_metadata;

    Ok(SkillMetadata {
        name: base_name,
        description,
        short_description,
        interface,
        dependencies,
        policy,
        path_to_skills_md: path.clone(),
        scope,
        plugin_id: plugin_identity.map(|identity| identity.plugin_id.clone()),
        remote_plugin_id: plugin_identity.and_then(|identity| identity.remote_plugin_id.clone()),
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

async fn load_skill_metadata(
    fs: &dyn ExecutorFileSystem,
    skill_path: &AbsolutePathBuf,
    metadata: &SkillMetadataDiscovery,
    plugin_root: Option<&AbsolutePathBuf>,
) -> LoadedSkillMetadata {
    // Fail open: optional metadata should not block loading SKILL.md.
    let Some(skill_dir) = skill_path.parent() else {
        return LoadedSkillMetadata::default();
    };
    let metadata_path_uri = match metadata {
        SkillMetadataDiscovery::Present(path) => path,
        SkillMetadataDiscovery::Absent => return LoadedSkillMetadata::default(),
        SkillMetadataDiscovery::Probe(path) => {
            match fs.get_metadata(path, /*sandbox*/ None).await {
                Ok(metadata) if metadata.is_file => {}
                Ok(_) => return LoadedSkillMetadata::default(),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    return LoadedSkillMetadata::default();
                }
                Err(error) => {
                    tracing::warn!(
                        "ignoring {path}: failed to stat {label}: {error}",
                        path = path,
                        label = SKILLS_METADATA_FILENAME
                    );
                    return LoadedSkillMetadata::default();
                }
            }
            path
        }
    };

    let contents = match fs.read_file_text(metadata_path_uri, /*sandbox*/ None).await {
        Ok(contents) => contents,
        Err(error) => {
            tracing::warn!(
                "ignoring {path}: failed to read {label}: {error}",
                path = metadata_path_uri,
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
                    path = metadata_path_uri,
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
    let asset_policy = match plugin_root {
        Some(plugin_root) => SkillInterfaceAssetPolicy::PluginShared { plugin_root },
        None => SkillInterfaceAssetPolicy::LocalOnly,
    };
    LoadedSkillMetadata {
        interface: resolve_skill_interface(interface, &skill_dir, asset_policy),
        dependencies: resolve_dependencies(dependencies),
        policy: resolve_policy(policy),
    }
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

#[cfg(test)]
#[path = "loader_tests.rs"]
mod tests;
