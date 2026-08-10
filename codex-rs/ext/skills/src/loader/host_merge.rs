use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

use codex_exec_server::ExecutorFileSystem;
use codex_protocol::protocol::Product;
use codex_protocol::protocol::SkillScope;
use codex_skills::LoadedSkillRoot;
use codex_skills::SkillRootSnapshots;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use codex_utils_plugins::PluginSkillRoot;
use codex_utils_plugins::migrated_command_skills_root;
use futures::FutureExt;
use futures::StreamExt;
use tokio::sync::OnceCell;
use tokio::sync::Semaphore;

use super::HostSkillRoot;
use super::MAX_CONCURRENT_ROOT_SCANS;
use super::host::HostSkillRootSnapshot;
use super::load_host_skill_root;
use crate::SkillLoadOutcome;
use crate::host_service::RequestSkillRootSnapshots;
use crate::host_service::config_skill_root_cache_key;

pub(crate) async fn load_and_merge_host_skill_roots(
    roots: Vec<HostSkillRoot>,
    root_scan_slots: &Semaphore,
    restriction_product: Option<Product>,
    plugin_skill_snapshots: Option<&SkillRootSnapshots<PluginSkillRoot>>,
) -> SkillLoadOutcome {
    load_and_merge_host_skill_roots_with_request_snapshots(
        roots,
        root_scan_slots,
        restriction_product,
        plugin_skill_snapshots,
        /*request_root_snapshots*/ None,
    )
    .await
}

pub(crate) async fn load_and_merge_host_skill_roots_with_request_snapshots(
    roots: Vec<HostSkillRoot>,
    root_scan_slots: &Semaphore,
    restriction_product: Option<Product>,
    plugin_skill_snapshots: Option<&SkillRootSnapshots<PluginSkillRoot>>,
    request_root_snapshots: Option<&RequestSkillRootSnapshots>,
) -> SkillLoadOutcome {
    let migrated_command_roots_by_plugin = roots
        .iter()
        .filter_map(HostSkillRoot::plugin_root)
        .map(|plugin_root| (plugin_root.clone(), Arc::new(OnceCell::new())))
        .collect::<HashMap<_, _>>();
    let migrated_command_roots = &migrated_command_roots_by_plugin;
    let mut indexed_snapshots = futures::stream::iter(roots.into_iter().enumerate())
        .map(|(root_index, root)| async move {
            let plugin_root = root.plugin_root().cloned();
            let migrated_command_root = plugin_root
                .as_ref()
                .and_then(|plugin_root| migrated_command_roots.get(plugin_root))
                .cloned();
            let plugin_skill_root = root.plugin_skill_root();
            let file_system = Arc::clone(&root.file_system);
            let request_snapshot = if plugin_skill_root.is_none() {
                request_root_snapshots.map(|snapshots| {
                    Arc::clone(
                        snapshots
                            .write()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .entry(config_skill_root_cache_key(&root))
                            .or_insert_with(|| Arc::new(OnceCell::new())),
                    )
                })
            } else {
                None
            };
            let load = async {
                let _root_scan_slot = root_scan_slots
                    .acquire()
                    .await
                    .unwrap_or_else(|_| unreachable!());
                let cached_snapshot = plugin_skill_root
                    .as_ref()
                    .and_then(|plugin_skill_root| plugin_skill_snapshots?.get(plugin_skill_root));
                match cached_snapshot {
                    Some(snapshot) => HostSkillRootSnapshot {
                        root: snapshot.root,
                        skills: snapshot.skills,
                        skill_discovery_path_by_path: snapshot.skill_discovery_path_by_path,
                        errors: snapshot.errors,
                        file_system: Arc::clone(&file_system),
                        is_agent_plugin: snapshot.is_agent_plugin,
                    },
                    None => {
                        let snapshot = load_host_skill_root(root).await;
                        if let Some(plugin_skill_snapshots) = plugin_skill_snapshots
                            && let Some(plugin_skill_root) = plugin_skill_root
                        {
                            plugin_skill_snapshots.insert(
                                plugin_skill_root,
                                LoadedSkillRoot {
                                    root: snapshot.root.clone(),
                                    skills: snapshot.skills.clone(),
                                    skill_discovery_path_by_path: Arc::clone(
                                        &snapshot.skill_discovery_path_by_path,
                                    ),
                                    errors: snapshot.errors.clone(),
                                    is_agent_plugin: snapshot.is_agent_plugin,
                                },
                            );
                        }
                        snapshot
                    }
                }
            }
            .boxed();
            let snapshot = if let Some(snapshot) = request_snapshot {
                snapshot.get_or_init(|| load).await.clone()
            } else {
                load.await
            };
            let migrated_command_root = match (plugin_root.as_ref(), migrated_command_root) {
                (Some(plugin_root), Some(migrated_command_root)) => Some(
                    migrated_command_root
                        .get_or_init(|| async {
                            let migrated_root = migrated_command_skills_root(plugin_root);
                            file_system
                                .canonicalize(
                                    &PathUri::from_abs_path(&migrated_root),
                                    /*sandbox*/ None,
                                )
                                .await
                                .and_then(|path| path.to_abs_path())
                                .unwrap_or(migrated_root)
                        })
                        .await
                        .clone(),
                ),
                _ => None,
            };
            (root_index, migrated_command_root, snapshot)
        })
        .buffer_unordered(MAX_CONCURRENT_ROOT_SCANS)
        .collect::<Vec<_>>()
        .await;
    indexed_snapshots.sort_unstable_by_key(|(root_index, _, _)| *root_index);

    for (_, _, snapshot) in &mut indexed_snapshots {
        snapshot
            .skills
            .retain(|skill| skill.matches_product_restriction_for_product(restriction_product));
    }

    let mut native_plugin_skill_names = HashMap::<String, HashSet<String>>::new();
    for skill in indexed_snapshots
        .iter()
        .flat_map(|(_, migrated_command_root, snapshot)| {
            snapshot.skills.iter().filter(|skill| {
                !migrated_command_root.as_ref().is_some_and(|migrated_root| {
                    skill
                        .path_to_skills_md
                        .as_path()
                        .starts_with(migrated_root.as_path())
                })
            })
        })
    {
        if let Some(plugin_id) = &skill.plugin_id {
            native_plugin_skill_names
                .entry(plugin_id.clone())
                .or_default()
                .insert(skill.name.clone());
        }
    }
    for (_, migrated_command_root, snapshot) in &mut indexed_snapshots {
        let Some(migrated_command_root) = migrated_command_root else {
            continue;
        };
        snapshot.skills.retain(|skill| {
            !skill
                .path_to_skills_md
                .as_path()
                .starts_with(migrated_command_root.as_path())
                || skill.plugin_id.as_ref().is_none_or(|plugin_id| {
                    native_plugin_skill_names
                        .get(plugin_id)
                        .is_none_or(|names| !names.contains(&skill.name))
                })
        });
    }

    merge_host_skill_root_snapshots(
        indexed_snapshots
            .into_iter()
            .map(|(_, _, snapshot)| snapshot)
            .collect(),
    )
}

fn merge_host_skill_root_snapshots(snapshots: Vec<HostSkillRootSnapshot>) -> SkillLoadOutcome {
    let mut skills = Vec::new();
    let mut errors = Vec::new();
    let mut skill_roots = Vec::new();
    let mut skill_root_by_path = HashMap::new();
    let mut skill_discovery_path_by_path = HashMap::new();
    let mut agent_plugin_skill_paths = HashSet::new();
    let mut file_systems_by_skill_path =
        HashMap::<AbsolutePathBuf, Arc<dyn ExecutorFileSystem>>::new();

    for snapshot in snapshots {
        if !snapshot.skills.is_empty() && !skill_roots.contains(&snapshot.root) {
            skill_roots.push(snapshot.root.clone());
        }
        for skill in &snapshot.skills {
            let path = skill.path_to_skills_md.clone();
            if !skill_root_by_path.contains_key(&path) {
                skill_root_by_path.insert(path.clone(), snapshot.root.clone());
                if let Some(discovery_path) = snapshot.skill_discovery_path_by_path.get(&path) {
                    skill_discovery_path_by_path.insert(path.clone(), discovery_path.clone());
                }
                file_systems_by_skill_path.insert(path.clone(), Arc::clone(&snapshot.file_system));
                if snapshot.is_agent_plugin {
                    agent_plugin_skill_paths.insert(path);
                }
            }
        }
        skills.extend(snapshot.skills);
        errors.extend(snapshot.errors);
    }

    let mut seen_paths = HashSet::new();
    skills.retain(|skill| seen_paths.insert(skill.path_to_skills_md.clone()));
    let retained_paths = skills
        .iter()
        .map(|skill| skill.path_to_skills_md.clone())
        .collect::<HashSet<_>>();
    skill_root_by_path.retain(|path, _| retained_paths.contains(path));
    skill_discovery_path_by_path.retain(|path, _| retained_paths.contains(path));
    let retained_roots = skill_root_by_path.values().collect::<HashSet<_>>();
    skill_roots.retain(|root| retained_roots.contains(root));
    file_systems_by_skill_path.retain(|path, _| retained_paths.contains(path));
    agent_plugin_skill_paths.retain(|path| retained_paths.contains(path));
    skills.sort_by(|left, right| {
        scope_rank(left.scope)
            .cmp(&scope_rank(right.scope))
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.path_to_skills_md.cmp(&right.path_to_skills_md))
    });

    SkillLoadOutcome::from_parts(
        skills,
        errors,
        skill_roots,
        skill_root_by_path,
        skill_discovery_path_by_path,
        agent_plugin_skill_paths,
        file_systems_by_skill_path,
    )
}

fn scope_rank(scope: SkillScope) -> u8 {
    match scope {
        SkillScope::Repo => 0,
        SkillScope::User => 1,
        SkillScope::System => 2,
        SkillScope::Admin => 3,
    }
}

#[cfg(test)]
#[path = "host_merge_tests.rs"]
mod tests;
