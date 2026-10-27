use std::collections::HashSet;
use std::io;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::Weak;
use tokio::sync::oneshot;

use codex_config::ConfigLayerSource;
use codex_config::ConfigLayerStack;
use codex_config::default_project_root_markers;
use codex_config::merge_toml_values;
use codex_config::project_root_markers_from_config;
use codex_exec_server::ExecutorFileSystem;
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
const MAX_CONCURRENT_ANCESTOR_PROBES: usize = 256;

#[derive(Default)]
pub(crate) struct EmptyRemoteRootsCache {
    generation: u64,
    entry: Option<EmptyRemoteRoots>,
}

struct EmptyRemoteRoots {
    fs: Weak<dyn ExecutorFileSystem>,
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

pub(crate) async fn resolve_skill_roots(
    repository_file_system: Option<Arc<dyn ExecutorFileSystem>>,
    config_layer_stack: &ConfigLayerStack,
    cwd: &AbsolutePathBuf,
    plugin_skill_roots: Vec<PluginSkillRoot>,
    extra_skill_roots: Vec<AbsolutePathBuf>,
    empty_remote_roots: Option<&Mutex<EmptyRemoteRootsCache>>,
) -> Vec<HostSkillRoot> {
    let home_dir =
        home_dir().and_then(|path| AbsolutePathBuf::from_absolute_path_checked(path).ok());
    resolve_skill_roots_with_home_dir(
        repository_file_system,
        config_layer_stack,
        cwd,
        home_dir.as_ref(),
        plugin_skill_roots,
        extra_skill_roots,
        empty_remote_roots,
    )
    .await
}

async fn resolve_skill_roots_with_home_dir(
    repository_file_system: Option<Arc<dyn ExecutorFileSystem>>,
    config_layer_stack: &ConfigLayerStack,
    cwd: &AbsolutePathBuf,
    home_dir: Option<&AbsolutePathBuf>,
    plugin_skill_roots: Vec<PluginSkillRoot>,
    extra_skill_roots: Vec<AbsolutePathBuf>,
    empty_remote_roots: Option<&Mutex<EmptyRemoteRootsCache>>,
) -> Vec<HostSkillRoot> {
    let mut roots =
        roots_from_layer_stack(config_layer_stack, home_dir, repository_file_system.clone());
    roots.extend(
        plugin_skill_roots
            .into_iter()
            .map(|root| HostSkillRoot::plugin(root, Arc::clone(&LOCAL_FS))),
    );
    roots.extend(
        extra_skill_roots
            .into_iter()
            .map(|path| local_root(path, SkillScope::User)),
    );
    roots.extend(
        repo_agents_skill_roots(
            repository_file_system,
            config_layer_stack,
            cwd,
            empty_remote_roots,
        )
        .await,
    );
    dedupe_skill_roots_by_path(&mut roots);
    roots
}

fn roots_from_layer_stack(
    config_layer_stack: &ConfigLayerStack,
    home_dir: Option<&AbsolutePathBuf>,
    repository_file_system: Option<Arc<dyn ExecutorFileSystem>>,
) -> Vec<HostSkillRoot> {
    let mut roots = Vec::new();

    for layer in config_layer_stack.all_layers_high_to_low() {
        let Some(config_folder) = layer.config_folder() else {
            continue;
        };

        match &layer.name {
            ConfigLayerSource::Project { .. } => {
                if let Some(repository_file_system) = &repository_file_system {
                    roots.push(HostSkillRoot::host(
                        config_folder.join(SKILLS_DIR_NAME),
                        SkillScope::Repo,
                        Arc::clone(repository_file_system),
                    ));
                }
            }
            ConfigLayerSource::User { .. } => {
                // Deprecated user skills location (`$CODEX_HOME/skills`), kept for backward
                // compatibility.
                roots.push(local_root(
                    config_folder.join(SKILLS_DIR_NAME),
                    SkillScope::User,
                ));

                if let Some(home_dir) = home_dir {
                    roots.push(local_root(
                        home_dir.join(AGENTS_DIR_NAME).join(SKILLS_DIR_NAME),
                        SkillScope::User,
                    ));
                }

                roots.push(local_root(
                    system_cache_root_dir(&config_folder),
                    SkillScope::System,
                ));
            }
            ConfigLayerSource::System { .. } => {
                roots.push(local_root(
                    config_folder.join(SKILLS_DIR_NAME),
                    SkillScope::Admin,
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

fn local_root(path: AbsolutePathBuf, scope: SkillScope) -> HostSkillRoot {
    HostSkillRoot::host(path, scope, Arc::clone(&LOCAL_FS))
}

async fn repo_agents_skill_roots(
    repository_file_system: Option<Arc<dyn ExecutorFileSystem>>,
    config_layer_stack: &ConfigLayerStack,
    cwd: &AbsolutePathBuf,
    empty_remote_roots: Option<&Mutex<EmptyRemoteRootsCache>>,
) -> Vec<HostSkillRoot> {
    let Some(repository_file_system) = repository_file_system else {
        return Vec::new();
    };
    let project_root_markers = project_root_markers_from_stack(config_layer_stack);
    let cache = empty_remote_roots.filter(|_| !Arc::ptr_eq(&repository_file_system, &LOCAL_FS));
    let file_system = Arc::downgrade(&repository_file_system);
    let generation = if let Some(cache) = cache {
        let mut cache = cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut register = true;
        if let Some(entry) = cache.entry.as_mut().filter(|entry| {
            entry.fs.ptr_eq(&file_system)
                && entry.cwd == *cwd
                && entry.markers == project_root_markers
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
    let (project_root, marker, mut complete) =
        find_project_root(repository_file_system.as_ref(), cwd, &project_root_markers).await;
    let directories = dirs_between_project_root_and_cwd(cwd, &project_root);
    let mut roots = Vec::new();
    let mut missing = Vec::new();
    let mut results = futures::stream::iter(directories)
        .map(|directory| {
            let repository_file_system = Arc::clone(&repository_file_system);
            async move {
                let agents_skills = directory.join(AGENTS_DIR_NAME).join(SKILLS_DIR_NAME);
                let agents_skills_uri = PathUri::from_abs_path(&agents_skills);
                let result = repository_file_system
                    .get_metadata(
                        &agents_skills_uri,
                        GetMetadataOptions::default(),
                        /*sandbox*/ None,
                    )
                    .await;
                (agents_skills, result)
            }
        })
        .buffered(MAX_CONCURRENT_ANCESTOR_PROBES);
    while let Some((agents_skills, result)) = results.next().await {
        match result {
            Ok(metadata) if metadata.is_directory => roots.push(HostSkillRoot::host(
                agents_skills,
                SkillScope::Repo,
                Arc::clone(&repository_file_system),
            )),
            Ok(_) => complete = false,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                if generation.is_some() {
                    missing.push(PathUri::from_abs_path(&agents_skills));
                }
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
            let repository_file_system = Arc::clone(&repository_file_system);
            tokio::spawn(async move {
                let present = marker.as_ref().map(PathUri::from_abs_path);
                tokio::select! {
                    biased;
                    _ = ready.closed() => {}
                    watch = repository_file_system.watch_missing(missing, present) => match watch {
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

async fn find_project_root(
    repository_file_system: &dyn ExecutorFileSystem,
    cwd: &AbsolutePathBuf,
    project_root_markers: &[String],
) -> (AbsolutePathBuf, Option<AbsolutePathBuf>, bool) {
    if project_root_markers.is_empty() {
        return (cwd.clone(), None, true);
    }

    let mut complete = true;
    let mut probes = Vec::new();
    for ancestor in cwd.ancestors() {
        for marker in project_root_markers {
            probes.push((ancestor.clone(), ancestor.join(marker)));
        }
    }
    let mut results = futures::stream::iter(probes)
        .map(|(ancestor, marker_path)| async move {
            let marker_path_uri = PathUri::from_abs_path(&marker_path);
            let result = repository_file_system
                .get_metadata(
                    &marker_path_uri,
                    GetMetadataOptions::default(),
                    /*sandbox*/ None,
                )
                .await;
            (ancestor, marker_path, result)
        })
        .buffered(MAX_CONCURRENT_ANCESTOR_PROBES);
    while let Some((ancestor, marker_path, result)) = results.next().await {
        match result {
            Ok(_) => return (ancestor, Some(marker_path), complete),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                complete = false;
                tracing::warn!(
                    "failed to stat project root marker {}: {error:#}",
                    marker_path.display()
                );
            }
        }
    }

    (cwd.clone(), None, complete)
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
