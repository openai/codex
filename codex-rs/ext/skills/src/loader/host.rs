use std::collections::HashMap;
use std::sync::Arc;

use codex_exec_server::ExecutorFileSystem;
use codex_protocol::protocol::SkillScope;
use codex_skills::ParsedSkillFrontmatter;
use codex_skills::SkillError;
use codex_skills::SkillMetadata;
use codex_skills::parse_skill_frontmatter_metadata;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use codex_utils_plugins::PluginIdentity;
use codex_utils_plugins::PluginSkillRoot;
use codex_utils_plugins::SkillDiscoveryMode;
use futures::StreamExt;
use tracing::error;

use super::MAX_QUALIFIED_NAME_LEN;
use super::SKILLS_METADATA_DIR;
use super::SKILLS_METADATA_FILENAME;
use super::discovery::DirectorySymlinkPolicy;
use super::discovery::DiscoveredSkill;
use super::discovery::HiddenDirectoryPolicy;
use super::discovery::MAX_CONCURRENT_SKILL_LOADS;
use super::discovery::SkillDiscovery;
use super::discovery::SkillDiscoveryOptions;
use super::discovery::SkillMetadataDiscovery;
use super::discovery::discover_skills;
use super::metadata::LoadedSkillMetadata;
use super::metadata::load_host_skill_metadata;
use super::metadata::sanitize_single_line;
use super::metadata::validate_len;
use super::namespace::SkillNamespaceResolver;

/// A resolved host skill root ready for filesystem discovery.
pub struct HostSkillRoot {
    pub path: AbsolutePathBuf,
    pub scope: SkillScope,
    pub file_system: Arc<dyn ExecutorFileSystem>,
    plugin: Option<PluginSkillRootContext>,
}

struct PluginSkillRootContext {
    identity: PluginIdentity,
    namespace: String,
    root: AbsolutePathBuf,
    discovery_mode: SkillDiscoveryMode,
}

impl HostSkillRoot {
    pub(crate) fn host(
        path: AbsolutePathBuf,
        scope: SkillScope,
        file_system: Arc<dyn ExecutorFileSystem>,
    ) -> Self {
        Self {
            path,
            scope,
            file_system,
            plugin: None,
        }
    }

    pub(crate) fn plugin(root: PluginSkillRoot, file_system: Arc<dyn ExecutorFileSystem>) -> Self {
        Self {
            path: root.path,
            scope: SkillScope::User,
            file_system,
            plugin: Some(PluginSkillRootContext {
                identity: root.plugin_identity,
                namespace: root.plugin_namespace,
                root: root.plugin_root,
                discovery_mode: root.discovery_mode,
            }),
        }
    }

    /// Returns the owning plugin identity when this root belongs to a plugin.
    pub fn plugin_identity(&self) -> Option<&PluginIdentity> {
        self.plugin.as_ref().map(|plugin| &plugin.identity)
    }

    pub(crate) fn plugin_namespace(&self) -> Option<&str> {
        self.plugin.as_ref().map(|plugin| plugin.namespace.as_str())
    }

    pub(crate) fn plugin_root(&self) -> Option<&AbsolutePathBuf> {
        self.plugin.as_ref().map(|plugin| &plugin.root)
    }

    pub(crate) fn plugin_skill_root(&self) -> Option<PluginSkillRoot> {
        self.plugin.as_ref().map(|plugin| PluginSkillRoot {
            path: self.path.clone(),
            plugin_identity: plugin.identity.clone(),
            plugin_namespace: plugin.namespace.clone(),
            plugin_root: plugin.root.clone(),
            discovery_mode: plugin.discovery_mode,
        })
    }

    pub(crate) fn discovery_mode(&self) -> SkillDiscoveryMode {
        self.plugin
            .as_ref()
            .map_or(SkillDiscoveryMode::Recursive, |plugin| {
                plugin.discovery_mode
            })
    }
}

/// Skills and errors loaded from one canonical host root.
#[derive(Clone)]
pub struct HostSkillRootSnapshot {
    pub root: AbsolutePathBuf,
    pub skills: Vec<SkillMetadata>,
    pub skill_discovery_path_by_path: Arc<HashMap<AbsolutePathBuf, AbsolutePathBuf>>,
    pub errors: Vec<SkillError>,
    pub file_system: Arc<dyn ExecutorFileSystem>,
    pub is_agent_plugin: bool,
}

struct ResolvedDiscoveredSkill {
    skill: DiscoveredSkill,
    path: AbsolutePathBuf,
    path_uri: PathUri,
}

pub async fn load_host_skill_root(root: HostSkillRoot) -> HostSkillRootSnapshot {
    let is_agent_plugin = root.discovery_mode() == SkillDiscoveryMode::DirectChildren;
    let canonical_root =
        canonicalize_for_skill_identity(root.file_system.as_ref(), &root.path).await;
    let (skills, skill_discovery_path_by_path, errors) =
        load_skills_under_root(&root, &canonical_root).await;
    HostSkillRootSnapshot {
        root: canonical_root,
        skills,
        skill_discovery_path_by_path,
        errors,
        file_system: root.file_system,
        is_agent_plugin,
    }
}

async fn load_skills_under_root(
    skill_root: &HostSkillRoot,
    root: &AbsolutePathBuf,
) -> (
    Vec<SkillMetadata>,
    Arc<HashMap<AbsolutePathBuf, AbsolutePathBuf>>,
    Vec<SkillError>,
) {
    let file_system = skill_root.file_system.as_ref();
    let plugin_identity = skill_root.plugin_identity();
    let plugin_root = match skill_root.plugin_root() {
        Some(plugin_root) => Some(canonicalize_for_skill_identity(file_system, plugin_root).await),
        None => None,
    };
    let discovery_mode = skill_root.discovery_mode();
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
        file_system,
        &PathUri::from_abs_path(root),
        SkillDiscoveryOptions {
            directory_symlinks,
            hidden_directories: HiddenDirectoryPolicy::Skip,
            mode: discovery_mode,
        },
    )
    .await;
    for warning in warnings {
        error!("{warning}");
    }
    if skills.is_empty() {
        return (Vec::new(), Arc::default(), Vec::new());
    }

    let root_uri = PathUri::from_abs_path(root);
    let resolved_plugin_root = plugin_root.as_ref();
    let resolved_skills = futures::stream::iter(skills)
        .map(|skill| async move {
            let path_uri = match file_system
                .canonicalize(&skill.path, /*sandbox*/ None)
                .await
            {
                Ok(path) => path,
                Err(error) if discovery_mode == SkillDiscoveryMode::DirectChildren => {
                    error!(
                        "failed to resolve Agent Plugin skill path {}: {error}",
                        skill.path
                    );
                    return None;
                }
                Err(_) => skill.path.clone(),
            };
            let path = match path_uri.to_abs_path() {
                Ok(path) => path,
                Err(error) => {
                    error!("failed to convert discovered skill path {path_uri}: {error}");
                    return None;
                }
            };
            if discovery_mode == SkillDiscoveryMode::DirectChildren {
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
                match file_system.get_metadata(&path_uri, /*sandbox*/ None).await {
                    Ok(metadata) if metadata.is_file => {}
                    Ok(_) => {
                        error!(
                            "Agent Plugin skill path {} is not a regular file",
                            path.display()
                        );
                        return None;
                    }
                    Err(error) => {
                        error!(
                            "failed to inspect Agent Plugin skill path {}: {error}",
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
        match skill_root.plugin_namespace() {
            Some(namespace) => SkillNamespaceResolver::with_provided_namespace(namespace),
            None => {
                SkillNamespaceResolver::discover(
                    file_system,
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
                    file_system,
                    &skill.skill,
                    &skill.path,
                    &skill.path_uri,
                    skill_root.scope,
                    plugin_identity,
                    plugin_root,
                )
                .await;
                (skill.path, skill.path_uri, discovery_path, result)
            }
        })
        .buffered(MAX_CONCURRENT_SKILL_LOADS)
        .collect::<Vec<_>>();
    let (namespace_resolver, skill_results) = tokio::join!(namespace_resolver, skill_results);

    let mut loaded_skills = Vec::new();
    let mut skill_discovery_path_by_path = HashMap::new();
    let mut errors = Vec::new();
    for (path, path_uri, discovery_path, result) in skill_results {
        let result = result.and_then(|mut skill| {
            skill.name = namespace_resolver
                .for_skill(&root_uri, &path_uri)
                .qualify(&skill.name);
            validate_len(&skill.name, MAX_QUALIFIED_NAME_LEN, "qualified name")
                .map_err(|error| error.to_string())?;
            Ok(skill)
        });
        match result {
            Ok(skill) => {
                skill_discovery_path_by_path
                    .insert(skill.path_to_skills_md.clone(), discovery_path);
                loaded_skills.push(skill);
            }
            Err(message) if skill_root.scope != SkillScope::System => {
                errors.push(SkillError { path, message });
            }
            Err(_) => {}
        }
    }
    (
        loaded_skills,
        Arc::new(skill_discovery_path_by_path),
        errors,
    )
}

async fn parse_skill_file(
    file_system: &dyn ExecutorFileSystem,
    skill: &DiscoveredSkill,
    path: &AbsolutePathBuf,
    path_uri: &PathUri,
    scope: SkillScope,
    plugin_identity: Option<&PluginIdentity>,
    plugin_root: Option<&AbsolutePathBuf>,
) -> Result<SkillMetadata, String> {
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
        file_system.read_file_text(path_uri, /*sandbox*/ None),
        load_host_skill_metadata(file_system, path, &metadata, plugin_root),
    );
    let contents = contents.map_err(|error| format!("failed to read file: {error}"))?;
    let ParsedSkillFrontmatter {
        name,
        description,
        short_description,
    } = parse_skill_frontmatter_metadata(&contents, || default_skill_name(path))
        .map_err(|error| error.to_string())?;
    let LoadedSkillMetadata {
        interface,
        dependencies,
        policy,
    } = loaded_metadata;

    Ok(SkillMetadata {
        name,
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

async fn canonicalize_for_skill_identity(
    file_system: &dyn ExecutorFileSystem,
    path: &AbsolutePathBuf,
) -> AbsolutePathBuf {
    let path_uri = PathUri::from_abs_path(path);
    file_system
        .canonicalize(&path_uri, /*sandbox*/ None)
        .await
        .and_then(|path| path.to_abs_path())
        .unwrap_or_else(|_| path.clone())
}

#[cfg(test)]
#[path = "host_tests.rs"]
mod tests;

#[cfg(all(test, unix))]
#[path = "host_io_tests.rs"]
mod io_tests;
