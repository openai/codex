use super::*;
use crate::SkillMetadata;
use crate::config_rules::resolve_disabled_skill_paths;
use crate::config_rules::skill_config_rules_from_stack;
use codex_app_server_protocol::ConfigLayerSource;
use codex_config::CONFIG_TOML_FILE;
use codex_config::ConfigLayerEntry;
use codex_config::ConfigLayerStack;
use codex_config::ConfigRequirementsToml;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::test_support::PathBufExt;
use codex_utils_absolute_path::test_support::PathExt;
use codex_utils_absolute_path::test_support::test_path_buf;
use pretty_assertions::assert_eq;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

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

fn test_skill(name: &str, path: PathBuf) -> SkillMetadata {
    SkillMetadata {
        name: name.to_string(),
        description: "test".to_string(),
        short_description: None,
        interface: None,
        dependencies: None,
        policy: None,
        path_to_skills_md: path
            .abs()
            .canonicalize()
            .expect("skill path should canonicalize"),
        scope: SkillScope::User,
    }
}

fn write_demo_skill(tempdir: &TempDir) -> PathBuf {
    let skill_path = tempdir.path().join("skills").join("demo").join("SKILL.md");
    fs::create_dir_all(skill_path.parent().expect("skill path should have parent"))
        .expect("create skill dir");
    fs::write(
        &skill_path,
        "---\nname: demo-skill\ndescription: demo description\n---\n\n# Body\n",
    )
    .expect("write skill");
    skill_path
}

fn user_config_layer(codex_home: &TempDir, config_toml: &str) -> ConfigLayerEntry {
    let config_path = AbsolutePathBuf::try_from(codex_home.path().join(CONFIG_TOML_FILE))
        .expect("user config path should be absolute");
    ConfigLayerEntry::new(
        ConfigLayerSource::User { file: config_path },
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
    format!(
        r#"[[skills.config]]
path = "{}"
enabled = {enabled}
"#,
        path.display()
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

fn skills_for_config_with_stack(
    skills_manager: &SkillsManager,
    cwd: &TempDir,
    config_layer_stack: &ConfigLayerStack,
    effective_skill_roots: &[PathBuf],
) -> SkillLoadOutcome {
    let skills_input = SkillsLoadInput::new(
        cwd.path().abs(),
        effective_skill_roots
            .iter()
            .map(codex_utils_absolute_path::test_support::PathBufExt::abs)
            .collect(),
        config_layer_stack.clone(),
        bundled_skills_enabled_from_stack(config_layer_stack),
    );
    skills_manager.skills_for_config(&skills_input)
}

#[test]
fn new_with_disabled_bundled_skills_removes_stale_cached_system_skills() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let stale_system_skill_dir = codex_home.path().join("skills/.system/stale-skill");
    fs::create_dir_all(&stale_system_skill_dir).expect("create stale system skill dir");
    fs::write(stale_system_skill_dir.join("SKILL.md"), "# stale\n")
        .expect("write stale system skill");

    let _skills_manager = SkillsManager::new(
        codex_home.path().abs(),
        /*bundled_skills_enabled*/ false,
    );

    assert!(
        !codex_home.path().join("skills/.system").exists(),
        "expected disabling system skills to remove stale cached bundled skills"
    );
}

#[tokio::test]
async fn skills_for_config_reuses_cache_for_same_effective_config() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let cwd = tempfile::tempdir().expect("tempdir");
    let config_layer_stack = config_stack(&codex_home, "");
    let skills_manager = SkillsManager::new(
        codex_home.path().abs(),
        /*bundled_skills_enabled*/ true,
    );

    write_user_skill(&codex_home, "a", "skill-a", "from a");
    let outcome1 = skills_for_config_with_stack(&skills_manager, &cwd, &config_layer_stack, &[]);
    assert!(
        outcome1.skills.iter().any(|s| s.name == "skill-a"),
        "expected skill-a to be discovered"
    );

    // Write a new skill after the first call; the second call should reuse the config-aware cache
    // entry because the effective skill config is unchanged.
    write_user_skill(&codex_home, "b", "skill-b", "from b");
    let outcome2 = skills_for_config_with_stack(&skills_manager, &cwd, &config_layer_stack, &[]);
    assert_eq!(outcome2.errors, outcome1.errors);
    assert_eq!(outcome2.skills, outcome1.skills);
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
    let plugin_skill_root = skill_path
        .parent()
        .and_then(std::path::Path::parent)
        .expect("plugin skill should live under a skills root")
        .to_path_buf();
    let skills_manager = SkillsManager::new(
        codex_home.path().abs(),
        /*bundled_skills_enabled*/ true,
    );

    let outcome = skills_for_config_with_stack(
        &skills_manager,
        &cwd,
        &config_layer_stack,
        &[plugin_skill_root],
    );
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
        !outcome
            .allowed_skills_for_implicit_invocation()
            .iter()
            .any(|allowed_skill| allowed_skill.path_to_skills_md == skill.path_to_skills_md)
    );
}

#[tokio::test]
async fn skills_for_cwd_reuses_cached_entry_even_when_entry_has_extra_roots() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let cwd = tempfile::tempdir().expect("tempdir");
    let extra_root = tempfile::tempdir().expect("tempdir");
    let config_layer_stack = config_stack(&codex_home, "");
    let skills_manager = SkillsManager::new(
        codex_home.path().abs(),
        /*bundled_skills_enabled*/ true,
    );
    let _ = skills_for_config_with_stack(&skills_manager, &cwd, &config_layer_stack, &[]);

    write_user_skill(&extra_root, "x", "extra-skill", "from extra root");
    let extra_root_path = extra_root.path().abs();
    let base_input = SkillsLoadInput::new(
        cwd.path().abs(),
        Vec::new(),
        config_layer_stack.clone(),
        bundled_skills_enabled_from_stack(&config_layer_stack),
    );
    let outcome_with_extra = skills_manager
        .skills_for_cwd_with_extra_user_roots(
            &base_input,
            /*force_reload*/ true,
            std::slice::from_ref(&extra_root_path),
        )
        .await;
    assert!(
        outcome_with_extra
            .skills
            .iter()
            .any(|skill| skill.name == "extra-skill")
    );
    assert!(
        outcome_with_extra
            .skills
            .iter()
            .any(|skill| skill.scope == SkillScope::System)
    );

    // The cwd-only API returns the current cached entry for this cwd, even when that entry
    // was produced with extra roots.
    let base_input = SkillsLoadInput::new(
        cwd.path().abs(),
        Vec::new(),
        config_layer_stack.clone(),
        bundled_skills_enabled_from_stack(&config_layer_stack),
    );
    let outcome_without_extra = skills_manager
        .skills_for_cwd(&base_input, /*force_reload*/ false)
        .await;
    assert_eq!(outcome_without_extra.skills, outcome_with_extra.skills);
    assert_eq!(outcome_without_extra.errors, outcome_with_extra.errors);
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
    let skills_manager = SkillsManager::new(
        codex_home.path().abs(),
        /*bundled_skills_enabled*/ false,
    );

    // Recreate the cached bundled skill after startup cleanup so this assertion exercises
    // root selection rather than relying on directory removal succeeding.
    fs::create_dir_all(&bundled_skill_dir).expect("recreate bundled skill dir");
    fs::write(
        bundled_skill_dir.join("SKILL.md"),
        "---\nname: bundled-skill\ndescription: from bundled root\n---\n\n# Body\n",
    )
    .expect("rewrite bundled skill");

    let outcome = skills_for_config_with_stack(&skills_manager, &cwd, &config_layer_stack, &[]);
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
async fn skills_for_cwd_with_extra_roots_only_refreshes_on_force_reload() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let cwd = tempfile::tempdir().expect("tempdir");
    let extra_root_a = tempfile::tempdir().expect("tempdir");
    let extra_root_b = tempfile::tempdir().expect("tempdir");
    let config_layer_stack = config_stack(&codex_home, "");
    let skills_manager = SkillsManager::new(
        codex_home.path().abs(),
        /*bundled_skills_enabled*/ true,
    );
    let _ = skills_for_config_with_stack(&skills_manager, &cwd, &config_layer_stack, &[]);

    write_user_skill(&extra_root_a, "x", "extra-skill-a", "from extra root a");
    write_user_skill(&extra_root_b, "x", "extra-skill-b", "from extra root b");

    let extra_root_a_path = extra_root_a.path().abs();
    let base_input = SkillsLoadInput::new(
        cwd.path().abs(),
        Vec::new(),
        config_layer_stack.clone(),
        bundled_skills_enabled_from_stack(&config_layer_stack),
    );
    let outcome_a = skills_manager
        .skills_for_cwd_with_extra_user_roots(
            &base_input,
            /*force_reload*/ true,
            std::slice::from_ref(&extra_root_a_path),
        )
        .await;
    assert!(
        outcome_a
            .skills
            .iter()
            .any(|skill| skill.name == "extra-skill-a")
    );
    assert!(
        outcome_a
            .skills
            .iter()
            .all(|skill| skill.name != "extra-skill-b")
    );

    let extra_root_b_path = extra_root_b.path().abs();
    let outcome_b = skills_manager
        .skills_for_cwd_with_extra_user_roots(
            &base_input,
            /*force_reload*/ false,
            std::slice::from_ref(&extra_root_b_path),
        )
        .await;
    assert!(
        outcome_b
            .skills
            .iter()
            .any(|skill| skill.name == "extra-skill-a")
    );
    assert!(
        outcome_b
            .skills
            .iter()
            .all(|skill| skill.name != "extra-skill-b")
    );

    let outcome_reloaded = skills_manager
        .skills_for_cwd_with_extra_user_roots(
            &base_input,
            /*force_reload*/ true,
            std::slice::from_ref(&extra_root_b_path),
        )
        .await;
    assert!(
        outcome_reloaded
            .skills
            .iter()
            .any(|skill| skill.name == "extra-skill-b")
    );
    assert!(
        outcome_reloaded
            .skills
            .iter()
            .all(|skill| skill.name != "extra-skill-a")
    );
}

#[test]
fn normalize_extra_user_roots_is_stable_for_equivalent_inputs() {
    let a = test_path_buf("/tmp/a").abs();
    let b = test_path_buf("/tmp/b").abs();

    let first = normalize_extra_user_roots(&[a.clone(), b.clone(), a.clone()]);
    let second = normalize_extra_user_roots(&[b, a]);

    assert_eq!(first, second);
}

#[cfg_attr(windows, ignore)]
#[test]
fn disabled_paths_for_skills_allows_session_flags_to_override_user_layer() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let skill_path = write_demo_skill(&tempdir);
    let skill = test_skill("demo-skill", skill_path.clone());
    let user_file = AbsolutePathBuf::try_from(tempdir.path().join("config.toml"))
        .expect("user config path should be absolute");
    let user_layer = ConfigLayerEntry::new(
        ConfigLayerSource::User { file: user_file },
        toml::from_str(&path_toggle_config(&skill_path, /*enabled*/ false))
            .expect("user layer toml"),
    );
    let session_layer = ConfigLayerEntry::new(
        ConfigLayerSource::SessionFlags,
        toml::from_str(&path_toggle_config(&skill_path, /*enabled*/ true))
            .expect("session layer toml"),
    );
    let stack = ConfigLayerStack::new(
        vec![user_layer, session_layer],
        Default::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("valid config layer stack");

    let skill_config_rules = skill_config_rules_from_stack(&stack);
    assert_eq!(
        resolve_disabled_skill_paths(&[skill], &skill_config_rules),
        HashSet::new()
    );
}

#[cfg_attr(windows, ignore)]
#[test]
fn disabled_paths_for_skills_allows_session_flags_to_disable_user_enabled_skill() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let skill_path = write_demo_skill(&tempdir);
    let skill = test_skill("demo-skill", skill_path.clone());
    let user_file = AbsolutePathBuf::try_from(tempdir.path().join("config.toml"))
        .expect("user config path should be absolute");
    let user_layer = ConfigLayerEntry::new(
        ConfigLayerSource::User { file: user_file },
        toml::from_str(&path_toggle_config(&skill_path, /*enabled*/ true))
            .expect("user layer toml"),
    );
    let session_layer = ConfigLayerEntry::new(
        ConfigLayerSource::SessionFlags,
        toml::from_str(&path_toggle_config(&skill_path, /*enabled*/ false))
            .expect("session layer toml"),
    );
    let stack = ConfigLayerStack::new(
        vec![user_layer, session_layer],
        Default::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("valid config layer stack");

    let skill_config_rules = skill_config_rules_from_stack(&stack);
    assert_eq!(
        resolve_disabled_skill_paths(&[skill], &skill_config_rules),
        HashSet::from([skill_path
            .abs()
            .canonicalize()
            .expect("skill path should canonicalize")])
    );
}

#[cfg_attr(windows, ignore)]
#[test]
fn disabled_paths_for_skills_disables_matching_name_selectors() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let skill_path = write_demo_skill(&tempdir);
    let skill = test_skill("github:yeet", skill_path.clone());
    let user_file = AbsolutePathBuf::try_from(tempdir.path().join("config.toml"))
        .expect("user config path should be absolute");
    let user_layer = ConfigLayerEntry::new(
        ConfigLayerSource::User { file: user_file },
        toml::from_str(&name_toggle_config("github:yeet", /*enabled*/ false))
            .expect("user layer toml"),
    );
    let stack = ConfigLayerStack::new(
        vec![user_layer],
        Default::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("valid config layer stack");

    let skill_config_rules = skill_config_rules_from_stack(&stack);
    assert_eq!(
        resolve_disabled_skill_paths(&[skill], &skill_config_rules),
        HashSet::from([skill_path
            .abs()
            .canonicalize()
            .expect("skill path should canonicalize")])
    );
}

#[cfg_attr(windows, ignore)]
#[test]
fn disabled_paths_for_skills_allows_name_selector_to_override_path_selector() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let skill_path = write_demo_skill(&tempdir);
    let skill = test_skill("github:yeet", skill_path.clone());
    let user_file = AbsolutePathBuf::try_from(tempdir.path().join("config.toml"))
        .expect("user config path should be absolute");
    let user_layer = ConfigLayerEntry::new(
        ConfigLayerSource::User { file: user_file },
        toml::from_str(&path_toggle_config(&skill_path, /*enabled*/ false))
            .expect("user layer toml"),
    );
    let session_layer = ConfigLayerEntry::new(
        ConfigLayerSource::SessionFlags,
        toml::from_str(&name_toggle_config("github:yeet", /*enabled*/ true))
            .expect("session layer toml"),
    );
    let stack = ConfigLayerStack::new(
        vec![user_layer, session_layer],
        Default::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("valid config layer stack");

    let skill_config_rules = skill_config_rules_from_stack(&stack);
    assert_eq!(
        resolve_disabled_skill_paths(&[skill], &skill_config_rules),
        HashSet::new()
    );
}

#[cfg_attr(windows, ignore)]
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
    let skills_manager = SkillsManager::new(
        codex_home.path().abs(),
        /*bundled_skills_enabled*/ true,
    );
    let parent_input = SkillsLoadInput::new(
        cwd.path().abs(),
        Vec::new(),
        parent_stack.clone(),
        bundled_skills_enabled_from_stack(&parent_stack),
    );

    let parent_outcome = skills_manager
        .skills_for_cwd(&parent_input, /*force_reload*/ true)
        .await;
    let parent_skill = parent_outcome
        .skills
        .iter()
        .find(|skill| skill.name == "demo-skill")
        .expect("demo skill should be discovered");
    assert_eq!(parent_outcome.is_skill_enabled(parent_skill), false);

    let child_outcome = skills_for_config_with_stack(&skills_manager, &cwd, &child_stack, &[]);
    let child_skill = child_outcome
        .skills
        .iter()
        .find(|skill| skill.name == "demo-skill")
        .expect("demo skill should be discovered");
    assert_eq!(child_outcome.is_skill_enabled(child_skill), true);
}
