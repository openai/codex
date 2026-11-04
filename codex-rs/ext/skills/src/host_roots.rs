use std::collections::HashSet;
use std::io;
use std::sync::Mutex;
use tokio::sync::oneshot;

use codex_config::ConfigLayerSource;
use codex_config::ConfigLayerStack;
use codex_config::default_project_root_markers;
use codex_config::merge_toml_values;
use codex_config::project_root_markers_from_config;
use codex_exec_server::EnvironmentAccess;
use codex_exec_server::EnvironmentAccessKey;
use codex_exec_server::FileSystemEnvironmentAccessor;
use codex_exec_server::GetMetadataOptions;
use codex_exec_server::LOCAL_FS;
use codex_protocol::protocol::SkillScope;
use codex_skills::system_cache_root_dir;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use codex_utils_plugins::PluginSkillRoot;
use dirs::home_dir;
use futures::StreamExt;
use toml::Value as TomlValue;

use crate::loader::HostSkillRoot;

const AGENTS_DIR_NAME: &str = ".agents";
const SKILLS_DIR_NAME: &str = "skills";
const MAX_CONCURRENT_ANCESTOR_PROBES: usize = 8;

#[derive(Default)]
pub(crate) struct EmptyRemoteRootsCache {
    generation: u64,
    entry: Option<EmptyRemoteRoots>,
}

struct EmptyRemoteRoots {
    fs: EnvironmentAccessKey,
    cwd: AbsolutePathBuf,
    markers: Vec<String>,
    watch: EmptyRootsWatch,
}

enum EmptyRootsWatch {
    Registering(oneshot::Receiver<oneshot::Receiver<()>>),
    Watching(oneshot::Receiver<()>),
}

impl EmptyRemoteRootsCache {
    pub(crate) fn clear(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.entry = None;
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "explicit discovery inputs and cache"
)]
pub(crate) async fn resolve_skill_roots<'a>(
    repository_file_system: Option<&'a dyn EnvironmentAccess>,
    host: &'a dyn EnvironmentAccess,
    config_layer_stack: &ConfigLayerStack,
    cwd: &AbsolutePathBuf,
    project_markers: Option<&dyn EnvironmentAccess>,
    plugin_skill_roots: Vec<PluginSkillRoot>,
    extra_skill_roots: Vec<AbsolutePathBuf>,
    empty_remote_roots: Option<&Mutex<EmptyRemoteRootsCache>>,
) -> Vec<HostSkillRoot<'a>> {
    let home_dir =
        home_dir().and_then(|path| AbsolutePathBuf::from_absolute_path_checked(path).ok());
    resolve_skill_roots_with_home_dir(
        repository_file_system,
        host,
        config_layer_stack,
        cwd,
        project_markers,
        home_dir.as_ref(),
        plugin_skill_roots,
        extra_skill_roots,
        empty_remote_roots,
    )
    .await
}

#[expect(
    clippy::too_many_arguments,
    reason = "explicit filesystem authorities and a home override keep root-order tests deterministic"
)]
async fn resolve_skill_roots_with_home_dir<'a>(
    repository_file_system: Option<&'a dyn EnvironmentAccess>,
    host: &'a dyn EnvironmentAccess,
    config_layer_stack: &ConfigLayerStack,
    cwd: &AbsolutePathBuf,
    project_markers: Option<&dyn EnvironmentAccess>,
    home_dir: Option<&AbsolutePathBuf>,
    plugin_skill_roots: Vec<PluginSkillRoot>,
    extra_skill_roots: Vec<AbsolutePathBuf>,
    empty_remote_roots: Option<&Mutex<EmptyRemoteRootsCache>>,
) -> Vec<HostSkillRoot<'a>> {
    let mut roots =
        roots_from_layer_stack(config_layer_stack, home_dir, repository_file_system, host);
    roots.extend(
        plugin_skill_roots
            .into_iter()
            .map(|root| HostSkillRoot::plugin(root, host)),
    );
    roots.extend(
        extra_skill_roots
            .into_iter()
            .map(|path| HostSkillRoot::host(path, SkillScope::User, host)),
    );
    roots.extend(
        repo_agents_skill_roots(
            repository_file_system,
            config_layer_stack,
            cwd,
            project_markers,
            empty_remote_roots,
        )
        .await,
    );
    dedupe_skill_roots_by_path(&mut roots);
    roots
}

fn roots_from_layer_stack<'a>(
    config_layer_stack: &ConfigLayerStack,
    home_dir: Option<&AbsolutePathBuf>,
    repository_file_system: Option<&'a dyn EnvironmentAccess>,
    host: &'a dyn EnvironmentAccess,
) -> Vec<HostSkillRoot<'a>> {
    let mut roots = Vec::new();

    for layer in config_layer_stack.all_layers_high_to_low() {
        let Some(config_folder) = layer.config_folder() else {
            continue;
        };

        match &layer.name {
            ConfigLayerSource::Project { .. } => {
                if let Some(repository_file_system) = repository_file_system {
                    roots.push(HostSkillRoot::host(
                        config_folder.join(SKILLS_DIR_NAME),
                        SkillScope::Repo,
                        repository_file_system,
                    ));
                }
            }
            ConfigLayerSource::User { .. } => {
                // Deprecated user skills location (`$CODEX_HOME/skills`), kept for backward
                // compatibility.
                roots.push(HostSkillRoot::host(
                    config_folder.join(SKILLS_DIR_NAME),
                    SkillScope::User,
                    host,
                ));

                if let Some(home_dir) = home_dir {
                    roots.push(HostSkillRoot::host(
                        home_dir.join(AGENTS_DIR_NAME).join(SKILLS_DIR_NAME),
                        SkillScope::User,
                        host,
                    ));
                }

                roots.push(HostSkillRoot::host(
                    system_cache_root_dir(&config_folder),
                    SkillScope::System,
                    host,
                ));
            }
            ConfigLayerSource::System { .. } => {
                roots.push(HostSkillRoot::host(
                    config_folder.join(SKILLS_DIR_NAME),
                    SkillScope::Admin,
                    host,
                ));
            }
            ConfigLayerSource::PackagedDefaults { .. }
            | ConfigLayerSource::Mdm { .. }
            | ConfigLayerSource::EnterpriseManaged { .. }
            | ConfigLayerSource::SessionFlags
            | ConfigLayerSource::LegacyManagedConfigTomlFromFile { .. }
            | ConfigLayerSource::LegacyManagedConfigTomlFromMdm => {}
        }
    }

    roots
}

async fn repo_agents_skill_roots<'a>(
    repository_fs_accessor: Option<&'a dyn EnvironmentAccess>,
    config_layer_stack: &ConfigLayerStack,
    cwd: &AbsolutePathBuf,
    project_markers: Option<&dyn EnvironmentAccess>,
    empty_remote_roots: Option<&Mutex<EmptyRemoteRootsCache>>,
) -> Vec<HostSkillRoot<'a>> {
    let Some(repository_fs_accessor) = repository_fs_accessor else {
        return Vec::new();
    };
    let project_root_markers = project_root_markers_from_stack(config_layer_stack);
    let file_system = repository_fs_accessor.cache_key();
    let host = FileSystemEnvironmentAccessor::unrestricted(&LOCAL_FS);
    let cache = empty_remote_roots.filter(|_| !file_system.same_file_system(&host.cache_key()));
    let generation = if let Some(cache) = cache {
        let mut cache = cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut register = true;
        if let Some(entry) = cache.entry.as_mut().filter(|entry| {
            entry.fs == file_system && entry.cwd == *cwd && entry.markers == project_root_markers
        }) {
            if let EmptyRootsWatch::Registering(registered) = &mut entry.watch {
                match registered.try_recv() {
                    Ok(invalidated) => entry.watch = EmptyRootsWatch::Watching(invalidated),
                    Err(oneshot::error::TryRecvError::Empty) => register = false,
                    Err(oneshot::error::TryRecvError::Closed) => {}
                }
            }
            if let EmptyRootsWatch::Watching(invalidated) = &mut entry.watch
                && invalidated.try_recv() == Err(oneshot::error::TryRecvError::Empty)
            {
                tracing::debug!("reused watched empty remote repository skills discovery");
                return Vec::new();
            }
        }
        register.then_some(cache.generation)
    } else {
        None
    };
    let ResolvedProjectRoot {
        path: project_root,
        marker,
        mut complete,
    } = find_project_root(
        project_markers.unwrap_or(repository_fs_accessor),
        config_layer_stack,
        cwd,
    )
    .await;
    let directories = dirs_between_project_root_and_cwd(cwd, &project_root);
    let mut roots = Vec::new();
    let mut missing = Vec::new();
    let mut results = futures::stream::iter(directories)
        .map(|directory| async move {
            let agents_skills = directory.join(AGENTS_DIR_NAME).join(SKILLS_DIR_NAME);
            let agents_skills_uri = PathUri::from_abs_path(&agents_skills);
            let result = repository_fs_accessor
                .get_metadata(&agents_skills_uri, GetMetadataOptions::default())
                .await;
            (agents_skills, result)
        })
        .buffered(MAX_CONCURRENT_ANCESTOR_PROBES);
    while let Some((agents_skills, result)) = results.next().await {
        match result {
            Ok(metadata) if metadata.is_directory => roots.push(HostSkillRoot::host(
                agents_skills,
                SkillScope::Repo,
                repository_fs_accessor,
            )),
            Ok(_) => complete = false,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                if generation.is_some() {
                    missing.push(PathUri::from_abs_path(&agents_skills));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::Unsupported => {
                complete = false;
                // Let loading report an unsupported sandbox backend instead of hiding this root.
                roots.push(HostSkillRoot::host(
                    agents_skills,
                    SkillScope::Repo,
                    repository_fs_accessor,
                ));
            }
            Err(error) => {
                complete = false;
                tracing::warn!(
                    "failed to stat repo skills root {}: {error:#}",
                    agents_skills.display()
                );
            }
        }
    }
    if complete
        && roots.is_empty()
        && let (Some(cache), Some(generation)) = (cache, generation)
    {
        // A nearer marker can only narrow an empty scan. Losing the selected marker
        // can widen it. With no marker, a new marker above cwd can widen it too.
        if marker.is_none() {
            for ancestor in cwd.ancestors() {
                missing.extend(
                    project_root_markers
                        .iter()
                        .map(|name| PathUri::from_abs_path(&ancestor.join(name))),
                );
            }
        }
        let mut cache = cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if cache.generation == generation {
            let (mut ready, registered) = oneshot::channel();
            cache.entry = Some(EmptyRemoteRoots {
                fs: file_system,
                cwd: cwd.clone(),
                markers: project_root_markers,
                watch: EmptyRootsWatch::Registering(registered),
            });
            // A cold question never waits for an extra round trip. Until the executor
            // acknowledges coverage, other questions take the ordinary live scan.
            let present = marker.as_ref().map(PathUri::from_abs_path);
            let watch = repository_fs_accessor.watch_missing(missing, present);
            tokio::spawn(async move {
                tokio::select! {
                    biased;
                    _ = ready.closed() => {}
                    watch = watch => match watch {
                        Ok(Some(invalidated)) => {
                            tracing::debug!("watch established for empty remote repository skills");
                            let _ = ready.send(invalidated);
                        }
                        Ok(None) => {}
                        Err(error) => tracing::debug!(%error, "remote skills watch failed; retaining live discovery"),
                    }
                }
            });
        }
    }
    roots
}

fn project_root_markers_from_stack(config_layer_stack: &ConfigLayerStack) -> Vec<String> {
    let mut merged = TomlValue::Table(toml::map::Map::new());
    for layer in config_layer_stack.layers_low_to_high() {
        if matches!(layer.name, ConfigLayerSource::Project { .. }) {
            continue;
        }
        merge_toml_values(&mut merged, &layer.config);
    }

    match project_root_markers_from_config(&merged) {
        Ok(Some(markers)) => markers,
        Ok(None) => default_project_root_markers(),
        Err(error) => {
            tracing::warn!("invalid project_root_markers: {error}");
            default_project_root_markers()
        }
    }
}

/// Repository boundary and the observations required to watch an empty discovery result.
struct ResolvedProjectRoot {
    path: AbsolutePathBuf,
    marker: Option<AbsolutePathBuf>,
    complete: bool,
}

async fn find_project_root(
    repository_file_system: &dyn EnvironmentAccess,
    config_layer_stack: &ConfigLayerStack,
    cwd: &AbsolutePathBuf,
) -> ResolvedProjectRoot {
    let project_root_markers = project_root_markers_from_stack(config_layer_stack);
    if project_root_markers.is_empty() {
        return ResolvedProjectRoot {
            path: cwd.clone(),
            marker: None,
            complete: true,
        };
    }

    let mut probes = Vec::new();
    for ancestor in cwd.ancestors() {
        for marker in &project_root_markers {
            probes.push((ancestor.clone(), ancestor.join(marker)));
        }
    }
    let mut results = futures::stream::iter(probes)
        .map(|(ancestor, marker_path)| async move {
            let marker_path_uri = PathUri::from_abs_path(&marker_path);
            let result = repository_file_system
                .get_metadata(&marker_path_uri, GetMetadataOptions::default())
                .await;
            (ancestor, marker_path, result)
        })
        .buffered(MAX_CONCURRENT_ANCESTOR_PROBES);
    while let Some((ancestor, marker_path, result)) = results.next().await {
        match result {
            Ok(_) => {
                return ResolvedProjectRoot {
                    path: ancestor,
                    marker: Some(marker_path),
                    complete: true,
                };
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                tracing::warn!(
                    "failed to stat project root marker {}: {error:#}",
                    marker_path.display()
                );
                // An inconclusive nearer marker must not expose a parent repository's skills.
                return ResolvedProjectRoot {
                    path: ancestor,
                    marker: None,
                    complete: false,
                };
            }
        }
    }

    ResolvedProjectRoot {
        path: cwd.clone(),
        marker: None,
        complete: true,
    }
}

fn dirs_between_project_root_and_cwd(
    cwd: &AbsolutePathBuf,
    project_root: &AbsolutePathBuf,
) -> Vec<AbsolutePathBuf> {
    let mut directories = cwd
        .ancestors()
        .scan(false, |done, directory| {
            if *done {
                None
            } else {
                if &directory == project_root {
                    *done = true;
                }
                Some(directory)
            }
        })
        .collect::<Vec<_>>();
    directories.reverse();
    directories
}

fn dedupe_skill_roots_by_path(roots: &mut Vec<HostSkillRoot>) {
    let mut seen = HashSet::new();
    roots.retain(|root| seen.insert(root.path.clone()));
}

#[cfg(test)]
#[path = "host_roots_tests.rs"]
mod tests;
