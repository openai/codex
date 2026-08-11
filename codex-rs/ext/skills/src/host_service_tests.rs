use super::*;
use codex_config::CONFIG_TOML_FILE;
use codex_config::ConfigLayerEntry;
use codex_config::ConfigLayerSource;
use codex_config::ConfigLayerStack;
use codex_config::ConfigRequirementsToml;
use codex_exec_server::LOCAL_FS;
use codex_skills::ImplicitSkillLookup;
use codex_skills::LoadedSkillRoot;
use codex_skills::SkillRootSnapshotCache;
use codex_skills::SkillRootSnapshots;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::test_support::PathBufExt;
use codex_utils_absolute_path::test_support::PathExt;
use codex_utils_plugins::PluginIdentity;
use codex_utils_plugins::PluginSkillRoot;
use codex_utils_plugins::SkillDiscoveryMode;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use tempfile::TempDir;

#[derive(Default)]
struct TestPluginSkillSnapshotCache {
    snapshots: Mutex<HashMap<PluginSkillRoot, LoadedSkillRoot>>,
}

impl SkillRootSnapshotCache<PluginSkillRoot> for TestPluginSkillSnapshotCache {
    fn get(&self, root: &PluginSkillRoot) -> Option<LoadedSkillRoot> {
        self.snapshots.lock().unwrap().get(root).cloned()
    }

    fn insert(&self, root: PluginSkillRoot, snapshot: LoadedSkillRoot) {
        self.snapshots.lock().unwrap().insert(root, snapshot);
    }
}

fn test_plugin_skill_snapshots() -> SkillRootSnapshots<PluginSkillRoot> {
    SkillRootSnapshots::new(Arc::new(TestPluginSkillSnapshotCache::default()))
}

fn write_user_skill(codex_home: &TempDir, dir: &str, name: &str, description: &str) {
    let skill_dir = codex_home.path().join("skills").join(dir);
    fs::create_dir_all(&skill_dir).unwrap();
    let content = format!("---\nname: {name}\ndescription: {description}\n---\n\n# Body\n");
    fs::write(skill_dir.join("SKILL.md"), content).unwrap();
}

fn write_plugin_skill(
    codex_home: &TempDir,
    marketplace: &str,
    plugin_name: &str,
    dir: &str,
    name: &str,
    description: &str,
) -> PathBuf {
    let plugin_root = codex_home
        .path()
        .join("plugins/cache")
        .join(marketplace)
        .join(plugin_name)
        .join("local");
    let skill_dir = plugin_root.join("skills").join(dir);
    fs::create_dir_all(plugin_root.join(".codex-plugin")).unwrap();
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        format!(r#"{{"name":"{plugin_name}"}}"#),
    )
    .unwrap();
    let content = format!("---\nname: {name}\ndescription: {description}\n---\n\n# Body\n");
    let skill_path = skill_dir.join("SKILL.md");
    fs::write(&skill_path, content).unwrap();
    skill_path
}

fn plugin_skill_root_for_skill_path(
    skill_path: &Path,
    plugin_id: &str,
    plugin_namespace: &str,
) -> PluginSkillRoot {
    let skills_root = skill_path
        .parent()
        .and_then(Path::parent)
        .expect("plugin skill should live under a skills root");
    let plugin_root = skills_root
        .parent()
        .expect("plugin skills root should live under a plugin root");
    PluginSkillRoot {
        path: skills_root.abs(),
        plugin_identity: PluginIdentity {
            plugin_id: plugin_id.to_string(),
            remote_plugin_id: None,
        },
        plugin_namespace: plugin_namespace.to_string(),
        plugin_root: plugin_root.abs(),
        discovery_mode: SkillDiscoveryMode::Recursive,
    }
}

fn user_config_layer(codex_home: &TempDir, config_toml: &str) -> ConfigLayerEntry {
    let config_path = AbsolutePathBuf::try_from(codex_home.path().join(CONFIG_TOML_FILE))
        .expect("user config path should be absolute");
    ConfigLayerEntry::new(
        ConfigLayerSource::User {
            file: config_path,
            profile: None,
        },
        toml::from_str(config_toml).expect("user layer toml"),
    )
}

fn config_stack(codex_home: &TempDir, user_config_toml: &str) -> ConfigLayerStack {
    ConfigLayerStack::new(
        vec![user_config_layer(codex_home, user_config_toml)],
        Default::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("valid config layer stack")
}

fn config_stack_with_session_flags(
    codex_home: &TempDir,
    user_config_toml: &str,
    session_flags_toml: &str,
) -> ConfigLayerStack {
    ConfigLayerStack::new(
        vec![
            user_config_layer(codex_home, user_config_toml),
            ConfigLayerEntry::new(
                ConfigLayerSource::SessionFlags,
                toml::from_str(session_flags_toml).expect("session layer toml"),
            ),
        ],
        Default::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("valid config layer stack")
}

fn path_toggle_config(path: &std::path::Path, enabled: bool) -> String {
    let path = toml::Value::String(path.display().to_string());
    format!(
        r#"[[skills.config]]
path = {path}
enabled = {enabled}
"#
    )
}

fn name_toggle_config(name: &str, enabled: bool) -> String {
    format!(
        r#"[[skills.config]]
name = "{name}"
enabled = {enabled}
"#
    )
}

async fn skills_for_config_with_stack(
    skills_service: &HostSkillsService,
    cwd: &TempDir,
    config_layer_stack: &ConfigLayerStack,
    effective_skill_roots: &[PluginSkillRoot],
) -> SkillLoadOutcome {
    let skills_input = HostSkillsLoadInput::new(
        cwd.path().abs(),
        effective_skill_roots.to_vec(),
        config_layer_stack.clone(),
    );
    skills_service
        .snapshot_for_config(&skills_input, Some(Arc::clone(&LOCAL_FS)))
        .await
        .outcome()
        .clone()
}

#[tokio::test]
async fn skills_for_config_reuses_cache_for_same_effective_config() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let cwd = tempfile::tempdir().expect("tempdir");
    let config_layer_stack = config_stack(&codex_home, "");
    let skills_service = HostSkillsService::new(
        codex_home.path().abs(),
        /*bundled_skills_enabled*/ true,
    );

    write_user_skill(&codex_home, "a", "skill-a", "from a");
    let outcome1 =
        skills_for_config_with_stack(&skills_service, &cwd, &config_layer_stack, &[]).await;
    assert!(
        outcome1.skills.iter().any(|s| s.name == "skill-a"),
        "expected skill-a to be discovered"
    );

    // Write a new skill after the first call; the second call should reuse the config-aware cache
    // entry because the effective skill config is unchanged.
    write_user_skill(&codex_home, "b", "skill-b", "from b");
    let outcome2 =
        skills_for_config_with_stack(&skills_service, &cwd, &config_layer_stack, &[]).await;
    assert_eq!(outcome2.errors, outcome1.errors);
    assert_eq!(outcome2.skills, outcome1.skills);
}

#[tokio::test]
async fn watchable_skill_root_paths_exclude_plugin_and_system_roots() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let cwd = tempfile::tempdir().expect("tempdir");
    let skill_path = write_plugin_skill(
        &codex_home,
        "test",
        "sample",
        "search",
        "search",
        "plugin skill",
    );
    let plugin_skill_root = plugin_skill_root_for_skill_path(&skill_path, "sample@test", "sample");
    let config_layer_stack = config_stack(&codex_home, "");
    let input = HostSkillsLoadInput::new(
        cwd.path().abs(),
        vec![plugin_skill_root.clone()],
        config_layer_stack,
    );
    let skills_service = HostSkillsService::new(
        codex_home.path().abs(),
        /*bundled_skills_enabled*/ true,
    );

    let watchable_paths = skills_service
        .watchable_skill_root_paths(&input, Arc::clone(&LOCAL_FS))
        .await;

    assert!(watchable_paths.contains(&codex_home.path().join("skills").abs()));
    assert!(!watchable_paths.contains(&plugin_skill_root.path));
    assert!(!watchable_paths.contains(&codex_home.path().join("skills/.system").abs()));
}

#[tokio::test]
async fn snapshot_for_config_merges_extension_host_and_legacy_plugin_roots() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let cwd = tempfile::tempdir().expect("tempdir");
    write_user_skill(&codex_home, "user", "user-skill", "from the host loader");
    let plugin_skill_path = write_plugin_skill(
        &codex_home,
        "test",
        "sample",
        "search",
        "search",
        "from the plugin loader",
    );
    let plugin_skill_root =
        plugin_skill_root_for_skill_path(&plugin_skill_path, "sample@test", "sample");
    let config_layer_stack = config_stack(&codex_home, "[skills.bundled]\nenabled = false\n");
    let input = HostSkillsLoadInput::new(
        cwd.path().abs(),
        vec![plugin_skill_root],
        config_layer_stack,
    );
    let skills_service = HostSkillsService::new(
        codex_home.path().abs(),
        /*bundled_skills_enabled*/ false,
    );

    let snapshot = skills_service
        .snapshot_for_config(&input, Some(Arc::clone(&LOCAL_FS)))
        .await;
    let skills = snapshot
        .outcome()
        .skills
        .iter()
        .map(|skill| (skill.name.as_str(), skill.plugin_id.as_deref()))
        .collect::<Vec<_>>();

    assert_eq!(
        skills,
        vec![("sample:search", Some("sample@test")), ("user-skill", None)]
    );
}

#[cfg(unix)]
#[tokio::test]
async fn snapshot_for_config_preserves_host_precedence_for_symlinked_plugin_root() {
    use std::os::unix::fs::symlink;

    let codex_home = tempfile::tempdir().expect("tempdir");
    let cwd = tempfile::tempdir().expect("tempdir");
    let plugin_skill_path = write_plugin_skill(
        &codex_home,
        "test",
        "sample",
        "search",
        "search",
        "shared skill",
    );
    let plugin_skill_root =
        plugin_skill_root_for_skill_path(&plugin_skill_path, "sample@test", "sample");
    symlink(
        plugin_skill_root.path.as_path(),
        codex_home.path().join("skills"),
    )
    .expect("symlink user skills root to plugin skills root");
    let config_layer_stack = config_stack(&codex_home, "[skills.bundled]\nenabled = false\n");
    let skills_service = HostSkillsService::new(
        codex_home.path().abs(),
        /*bundled_skills_enabled*/ false,
    );

    let outcome = skills_for_config_with_stack(
        &skills_service,
        &cwd,
        &config_layer_stack,
        &[plugin_skill_root],
    )
    .await;

    assert_eq!(
        outcome.skills,
        vec![codex_skills::SkillMetadata {
            name: "sample:search".to_string(),
            description: "shared skill".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            path_to_skills_md: dunce::canonicalize(plugin_skill_path)
                .expect("canonical plugin skill path")
                .abs(),
            scope: SkillScope::User,
            plugin_id: None,
            remote_plugin_id: None,
        }]
    );
}

#[tokio::test]
async fn skills_list_snapshots_share_host_roots_only_within_one_request() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let first_cwd = tempfile::tempdir().expect("tempdir");
    let second_cwd = tempfile::tempdir().expect("tempdir");
    let config_layer_stack = config_stack(&codex_home, "");
    let disabled_config_layer_stack = config_stack(
        &codex_home,
        &name_toggle_config("first-skill", /*enabled*/ false),
    );
    let skills_service = HostSkillsService::new(
        codex_home.path().abs(),
        /*bundled_skills_enabled*/ true,
    );
    let request = skills_service.for_request();
    let input = |cwd: &TempDir, config_layer_stack| {
        HostSkillsLoadInput::new(cwd.path().abs(), Vec::new(), config_layer_stack)
    };

    write_user_skill(&codex_home, "first", "first-skill", "first skill");
    let first = request
        .snapshot_for_cwd(
            &input(&first_cwd, config_layer_stack.clone()),
            /*force_reload*/ false,
            Some(Arc::clone(&LOCAL_FS)),
        )
        .await;

    write_user_skill(&codex_home, "second", "second-skill", "second skill");
    let second = request
        .snapshot_for_cwd(
            &input(&second_cwd, disabled_config_layer_stack),
            /*force_reload*/ false,
            Some(Arc::clone(&LOCAL_FS)),
        )
        .await;
    assert_eq!(second.outcome().skills, first.outcome().skills);
    let first_skill_path = first
        .outcome()
        .skills
        .iter()
        .find(|skill| skill.name == "first-skill")
        .expect("first skill should be discovered")
        .path_to_skills_md
        .clone();
    assert!(!first.outcome().disabled_paths.contains(&first_skill_path));
    assert!(second.outcome().disabled_paths.contains(&first_skill_path));

    let third = skills_service
        .for_request()
        .snapshot_for_cwd(
            &input(&first_cwd, config_layer_stack),
            /*force_reload*/ true,
            Some(Arc::clone(&LOCAL_FS)),
        )
        .await;
    assert!(
        third
            .outcome()
            .skills
            .iter()
            .any(|skill| skill.name == "second-skill")
    );
}

#[tokio::test]
async fn skills_for_config_refreshes_cache_when_remote_plugin_id_changes() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let cwd = tempfile::tempdir().expect("tempdir");
    let skill_path = write_plugin_skill(
        &codex_home,
        "test",
        "sample",
        "sample-search",
        "sample-search",
        "search sample data",
    );
    let config_layer_stack = config_stack(&codex_home, "");
    let mut plugin_skill_root =
        plugin_skill_root_for_skill_path(&skill_path, "sample@test", "sample");
    let skills_service = HostSkillsService::new(
        codex_home.path().abs(),
        /*bundled_skills_enabled*/ true,
    );

    let plugin_input = HostSkillsLoadInput::new(
        cwd.path().abs(),
        vec![plugin_skill_root.clone()],
        config_layer_stack.clone(),
    )
    .with_plugin_skill_snapshots(Some(test_plugin_skill_snapshots()));
    let plugin_snapshot = skills_service
        .snapshot_for_config(&plugin_input, Some(Arc::clone(&LOCAL_FS)))
        .await;
    fs::write(
        &skill_path,
        "---\nname: sample-search\ndescription: updated sample data\n---\n\n# Body\n",
    )
    .expect("update plugin skill");
    let listing_input = plugin_input
        .clone()
        .with_plugin_skill_snapshots(/*plugin_skill_snapshots*/ None);
    let listing_snapshot = skills_service
        .for_request()
        .snapshot_for_cwd(
            &listing_input,
            /*force_reload*/ false,
            Some(Arc::clone(&LOCAL_FS)),
        )
        .await;
    assert_eq!(
        (
            plugin_snapshot
                .outcome()
                .skills
                .iter()
                .find(|skill| skill.name == "sample:sample-search")
                .map(|skill| skill.description.as_str()),
            listing_snapshot
                .outcome()
                .skills
                .iter()
                .find(|skill| skill.name == "sample:sample-search")
                .map(|skill| skill.description.as_str()),
        ),
        (Some("search sample data"), Some("updated sample data"))
    );

    plugin_skill_root.plugin_identity.remote_plugin_id = Some("plugins~Plugin_sample".to_string());
    let refreshed = skills_for_config_with_stack(
        &skills_service,
        &cwd,
        &config_layer_stack,
        &[plugin_skill_root],
    )
    .await;

    assert_eq!(
        refreshed
            .skills
            .iter()
            .find(|skill| skill.name == "sample:sample-search")
            .and_then(|skill| skill.remote_plugin_id.as_deref()),
        Some("plugins~Plugin_sample")
    );
}

#[tokio::test]
async fn set_extra_roots_replaces_runtime_roots_and_clears_cache() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let cwd = tempfile::tempdir().expect("tempdir");
    let extra_root = tempfile::tempdir().expect("tempdir");
    let config_layer_stack = config_stack(&codex_home, "");
    let skills_service = HostSkillsService::new(
        codex_home.path().abs(),
        /*bundled_skills_enabled*/ true,
    );

    let skills_input =
        HostSkillsLoadInput::new(cwd.path().abs(), Vec::new(), config_layer_stack.clone());
    let empty_snapshot = skills_service
        .for_request()
        .snapshot_for_cwd(
            &skills_input,
            /*force_reload*/ false,
            Some(Arc::clone(&LOCAL_FS)),
        )
        .await;
    let empty_outcome = empty_snapshot.outcome();
    assert!(
        empty_outcome
            .skills
            .iter()
            .all(|skill| skill.name != "runtime-skill")
    );

    let extra_skills_root = extra_root.path().join("skills");
    let skill_dir = extra_skills_root.join("runtime-skill");
    fs::create_dir_all(&skill_dir).expect("create skill dir");
    fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: runtime-skill\ndescription: runtime skill\n---\n\n# Body\n",
    )
    .expect("write skill");
    skills_service.set_extra_roots(vec![extra_skills_root.abs()]);

    let runtime_snapshot = skills_service
        .for_request()
        .snapshot_for_cwd(
            &skills_input,
            /*force_reload*/ false,
            Some(Arc::clone(&LOCAL_FS)),
        )
        .await;
    let runtime_outcome = runtime_snapshot.outcome();
    assert!(
        runtime_outcome
            .skills
            .iter()
            .any(|skill| skill.name == "runtime-skill")
    );

    skills_service.set_extra_roots(vec![extra_root.path().join("missing-skills").abs()]);
    let replaced_snapshot = skills_service
        .for_request()
        .snapshot_for_cwd(
            &skills_input,
            /*force_reload*/ false,
            Some(Arc::clone(&LOCAL_FS)),
        )
        .await;
    let replaced_outcome = replaced_snapshot.outcome();
    assert_eq!(replaced_outcome.errors, Vec::new());
    assert!(
        replaced_outcome
            .skills
            .iter()
            .all(|skill| skill.name != "runtime-skill")
    );
}

#[tokio::test]
async fn set_extra_roots_applies_to_config_loads_and_empty_clears() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let cwd = tempfile::tempdir().expect("tempdir");
    let extra_root = tempfile::tempdir().expect("tempdir");
    let config_layer_stack = config_stack(&codex_home, "");
    let skills_service = HostSkillsService::new(
        codex_home.path().abs(),
        /*bundled_skills_enabled*/ true,
    );

    let empty_outcome =
        skills_for_config_with_stack(&skills_service, &cwd, &config_layer_stack, &[]).await;
    assert!(
        empty_outcome
            .skills
            .iter()
            .all(|skill| skill.name != "runtime-skill")
    );

    let extra_skills_root = extra_root.path().join("skills");
    let skill_dir = extra_skills_root.join("runtime-skill");
    fs::create_dir_all(&skill_dir).expect("create skill dir");
    fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: runtime-skill\ndescription: runtime skill\n---\n\n# Body\n",
    )
    .expect("write skill");
    skills_service.set_extra_roots(vec![extra_skills_root.abs()]);

    let runtime_outcome =
        skills_for_config_with_stack(&skills_service, &cwd, &config_layer_stack, &[]).await;
    assert!(
        runtime_outcome
            .skills
            .iter()
            .any(|skill| skill.name == "runtime-skill")
    );

    skills_service.set_extra_roots(Vec::new());
    let cleared_outcome =
        skills_for_config_with_stack(&skills_service, &cwd, &config_layer_stack, &[]).await;
    assert!(
        cleared_outcome
            .skills
            .iter()
            .all(|skill| skill.name != "runtime-skill")
    );
}

#[tokio::test]
async fn skills_for_config_disables_plugin_skills_by_name() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let cwd = tempfile::tempdir().expect("tempdir");
    let skill_path = write_plugin_skill(
        &codex_home,
        "test",
        "sample",
        "sample-search",
        "sample-search",
        "search sample data",
    );
    let config_layer_stack = config_stack(
        &codex_home,
        &name_toggle_config("sample:sample-search", /*enabled*/ false),
    );
    let plugin_skill_root =
        plugin_skill_root_for_skill_path(&skill_path, "test-plugin@test", "sample");
    let skills_service = HostSkillsService::new(
        codex_home.path().abs(),
        /*bundled_skills_enabled*/ true,
    );

    let outcome = skills_for_config_with_stack(
        &skills_service,
        &cwd,
        &config_layer_stack,
        &[plugin_skill_root],
    )
    .await;
    let skill = outcome
        .skills
        .iter()
        .find(|skill| skill.name == "sample:sample-search")
        .expect("plugin skill should load");
    let skill_path = dunce::canonicalize(skill_path)
        .expect("skill path should canonicalize")
        .abs();

    assert_eq!(skill.path_to_skills_md, skill_path);
    assert!(outcome.disabled_paths.contains(&skill.path_to_skills_md));
    assert!(
        outcome
            .implicit_skill_for_doc_path(&skill.path_to_skills_md)
            .is_none()
    );
}

#[tokio::test]
async fn skills_for_cwd_loads_repo_and_user_roots_with_local_fs() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let cwd = tempfile::tempdir().expect("tempdir");
    let repo_dot_codex = cwd.path().join(".codex");
    fs::create_dir_all(&repo_dot_codex).expect("create repo config dir");

    write_user_skill(&codex_home, "user", "user-skill", "from local user root");
    let repo_skill_dir = repo_dot_codex.join("skills/repo");
    fs::create_dir_all(&repo_skill_dir).expect("create repo skill dir");
    fs::write(
        repo_skill_dir.join("SKILL.md"),
        "---\nname: repo-skill\ndescription: from repo root\n---\n\n# Body\n",
    )
    .expect("write repo skill");

    let config_layer_stack = ConfigLayerStack::new(
        vec![
            user_config_layer(&codex_home, ""),
            ConfigLayerEntry::new(
                ConfigLayerSource::Project {
                    dot_codex_folder: repo_dot_codex.abs(),
                },
                toml::Value::Table(toml::map::Map::new()),
            ),
        ],
        Default::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("valid config layer stack");
    let skills_input =
        HostSkillsLoadInput::new(cwd.path().abs(), Vec::new(), config_layer_stack.clone());
    let skills_service = HostSkillsService::new(
        codex_home.path().abs(),
        /*bundled_skills_enabled*/ true,
    );

    let snapshot = skills_service
        .for_request()
        .snapshot_for_cwd(
            &skills_input,
            /*force_reload*/ true,
            Some(Arc::clone(&LOCAL_FS)),
        )
        .await;
    let outcome = snapshot.outcome();

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    let loaded_names = outcome
        .skills
        .iter()
        .map(|skill| skill.name.as_str())
        .collect::<HashSet<_>>();
    assert!(loaded_names.contains("user-skill"));
    assert!(loaded_names.contains("repo-skill"));
    let other_file_system: Arc<dyn codex_exec_server::ExecutorFileSystem> =
        Arc::new(codex_exec_server::LocalFileSystem::unsandboxed());
    let other_snapshot = skills_service
        .snapshot_for_config(&skills_input, Some(other_file_system))
        .await;
    assert!(!std::ptr::eq(snapshot.outcome(), other_snapshot.outcome()));
}

#[tokio::test]
async fn skills_for_cwd_without_fs_skips_repo_roots() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let cwd = tempfile::tempdir().expect("tempdir");
    let repo_dot_codex = cwd.path().join(".codex");
    fs::create_dir_all(&repo_dot_codex).expect("create repo config dir");

    write_user_skill(&codex_home, "user", "user-skill", "from local user root");
    let repo_skill_dir = repo_dot_codex.join("skills/repo");
    fs::create_dir_all(&repo_skill_dir).expect("create repo skill dir");
    fs::write(
        repo_skill_dir.join("SKILL.md"),
        "---\nname: repo-skill\ndescription: from repo root\n---\n\n# Body\n",
    )
    .expect("write repo skill");

    let config_layer_stack = ConfigLayerStack::new(
        vec![
            user_config_layer(&codex_home, ""),
            ConfigLayerEntry::new(
                ConfigLayerSource::Project {
                    dot_codex_folder: repo_dot_codex.abs(),
                },
                toml::Value::Table(toml::map::Map::new()),
            ),
        ],
        Default::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("valid config layer stack");
    let skills_input =
        HostSkillsLoadInput::new(cwd.path().abs(), Vec::new(), config_layer_stack.clone());
    let skills_service = HostSkillsService::new(
        codex_home.path().abs(),
        /*bundled_skills_enabled*/ true,
    );

    let snapshot = skills_service
        .for_request()
        .snapshot_for_cwd(&skills_input, /*force_reload*/ true, /*fs*/ None)
        .await;
    let outcome = snapshot.outcome();

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    let loaded_names = outcome
        .skills
        .iter()
        .map(|skill| skill.name.as_str())
        .collect::<HashSet<_>>();
    assert!(loaded_names.contains("user-skill"));
    assert!(!loaded_names.contains("repo-skill"));
}

#[tokio::test]
async fn skills_for_config_excludes_bundled_skills_when_disabled_in_config() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let cwd = tempfile::tempdir().expect("tempdir");
    let bundled_skill_dir = codex_home.path().join("skills/.system/bundled-skill");
    fs::create_dir_all(&bundled_skill_dir).expect("create bundled skill dir");
    fs::write(
        bundled_skill_dir.join("SKILL.md"),
        "---\nname: bundled-skill\ndescription: from bundled root\n---\n\n# Body\n",
    )
    .expect("write bundled skill");
    let config_layer_stack = config_stack(&codex_home, "[skills.bundled]\nenabled = false\n");
    let skills_service = HostSkillsService::new(
        codex_home.path().abs(),
        /*bundled_skills_enabled*/ false,
    );

    let outcome =
        skills_for_config_with_stack(&skills_service, &cwd, &config_layer_stack, &[]).await;
    assert!(
        outcome
            .skills
            .iter()
            .all(|skill| skill.name != "bundled-skill")
    );
    assert!(
        outcome
            .skills
            .iter()
            .all(|skill| skill.scope != SkillScope::System)
    );
}

#[tokio::test]
async fn skills_for_cwd_uses_cached_result_until_force_reload() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let cwd = tempfile::tempdir().expect("tempdir");
    let config_layer_stack = config_stack(&codex_home, "");
    let skills_service = HostSkillsService::new(
        codex_home.path().abs(),
        /*bundled_skills_enabled*/ true,
    );
    let base_input =
        HostSkillsLoadInput::new(cwd.path().abs(), Vec::new(), config_layer_stack.clone());
    let config_input = base_input
        .clone()
        .with_plugin_skill_snapshots(Some(test_plugin_skill_snapshots()));
    let request = skills_service.for_request();
    let (config_snapshot, snapshot_a) = tokio::join!(
        skills_service.snapshot_for_config(&config_input, Some(Arc::clone(&LOCAL_FS))),
        request.snapshot_for_cwd(
            &base_input,
            /*force_reload*/ false,
            Some(Arc::clone(&LOCAL_FS)),
        )
    );
    assert!(std::ptr::eq(
        config_snapshot.outcome(),
        snapshot_a.outcome()
    ));
    let outcome_a = snapshot_a.outcome();
    assert!(
        outcome_a
            .skills
            .iter()
            .all(|skill| skill.name != "late-skill")
    );

    write_user_skill(&codex_home, "late", "late-skill", "added after cache");

    let snapshot_b = skills_service
        .for_request()
        .snapshot_for_cwd(
            &base_input,
            /*force_reload*/ false,
            Some(Arc::clone(&LOCAL_FS)),
        )
        .await;
    let outcome_b = snapshot_b.outcome();
    assert!(
        outcome_b
            .skills
            .iter()
            .all(|skill| skill.name != "late-skill")
    );

    let snapshot_reloaded = skills_service
        .for_request()
        .snapshot_for_cwd(
            &base_input,
            /*force_reload*/ true,
            Some(Arc::clone(&LOCAL_FS)),
        )
        .await;
    let outcome_reloaded = snapshot_reloaded.outcome();
    assert!(
        outcome_reloaded
            .skills
            .iter()
            .any(|skill| skill.name == "late-skill")
    );
}

#[tokio::test]
async fn skills_for_config_ignores_cwd_cache_when_session_flags_reenable_skill() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let cwd = tempfile::tempdir().expect("tempdir");
    let skill_dir = codex_home.path().join("skills").join("demo");
    fs::create_dir_all(&skill_dir).expect("create skill dir");
    let skill_path = skill_dir.join("SKILL.md");
    fs::write(
        &skill_path,
        "---\nname: demo-skill\ndescription: demo description\n---\n\n# Body\n",
    )
    .expect("write skill");
    let disabled_skill_config = path_toggle_config(&skill_path, /*enabled*/ false);
    let enabled_skill_config = path_toggle_config(&skill_path, /*enabled*/ true);
    let parent_stack = config_stack(&codex_home, &disabled_skill_config);
    let child_stack =
        config_stack_with_session_flags(&codex_home, &disabled_skill_config, &enabled_skill_config);
    let skills_service = HostSkillsService::new(
        codex_home.path().abs(),
        /*bundled_skills_enabled*/ true,
    );
    let parent_input = HostSkillsLoadInput::new(cwd.path().abs(), Vec::new(), parent_stack.clone());

    let parent_snapshot = skills_service
        .for_request()
        .snapshot_for_cwd(
            &parent_input,
            /*force_reload*/ true,
            Some(Arc::clone(&LOCAL_FS)),
        )
        .await;
    let parent_outcome = parent_snapshot.outcome();
    let parent_skill = parent_outcome
        .skills
        .iter()
        .find(|skill| skill.name == "demo-skill")
        .expect("demo skill should be discovered");
    assert_eq!(parent_outcome.is_skill_enabled(parent_skill), false);

    let child_outcome =
        skills_for_config_with_stack(&skills_service, &cwd, &child_stack, &[]).await;
    let child_skill = child_outcome
        .skills
        .iter()
        .find(|skill| skill.name == "demo-skill")
        .expect("demo skill should be discovered");
    assert_eq!(child_outcome.is_skill_enabled(child_skill), true);
}
