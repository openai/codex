use super::*;
use crate::config::ConfigBuilder;
use crate::config::ConfigOverrides;
use crate::config_loader::ConfigLayerEntry;
use crate::config_loader::ConfigLayerStack;
use crate::config_loader::ConfigRequirementsToml;
use crate::plugins::PluginsManager;
use codex_protocol::protocol::SkillScope;
use pretty_assertions::assert_eq;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

fn write_user_skill(codex_home: &TempDir, dir: &str, name: &str, description: &str) {
    let skill_dir = codex_home.path().join("skills").join(dir);
    fs::create_dir_all(&skill_dir).unwrap();
    let content = format!("---\nname: {name}\ndescription: {description}\n---\n\n# Body\n");
    fs::write(skill_dir.join("SKILL.md"), content).unwrap();
}

#[test]
fn new_with_disabled_bundled_skills_removes_stale_cached_system_skills() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let stale_system_skill_dir = codex_home.path().join("skills/.system/stale-skill");
    fs::create_dir_all(&stale_system_skill_dir).expect("create stale system skill dir");
    fs::write(stale_system_skill_dir.join("SKILL.md"), "# stale\n")
        .expect("write stale system skill");

    let plugins_manager = Arc::new(PluginsManager::new(codex_home.path().to_path_buf()));
    let _skills_manager =
        SkillsManager::new(codex_home.path().to_path_buf(), plugins_manager, false);

    assert!(
        !codex_home.path().join("skills/.system").exists(),
        "expected disabling system skills to remove stale cached bundled skills"
    );
}

#[tokio::test]
async fn skills_for_config_reuses_cache_for_same_effective_config() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let cwd = tempfile::tempdir().expect("tempdir");

    let cfg = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .harness_overrides(ConfigOverrides {
            cwd: Some(cwd.path().to_path_buf()),
            ..Default::default()
        })
        .build()
        .await
        .expect("defaults for test should always succeed");

    let plugins_manager = Arc::new(PluginsManager::new(codex_home.path().to_path_buf()));
    let skills_manager = SkillsManager::new(codex_home.path().to_path_buf(), plugins_manager, true);

    write_user_skill(&codex_home, "a", "skill-a", "from a");
    let outcome1 = skills_manager.skills_for_config(&cfg);
    assert!(
        outcome1.skills.iter().any(|s| s.name == "skill-a"),
        "expected skill-a to be discovered"
    );

    // Write a new skill after the first call; the second call should reuse the config-aware cache
    // entry because the effective skill config is unchanged.
    write_user_skill(&codex_home, "b", "skill-b", "from b");
    let outcome2 = skills_manager.skills_for_config(&cfg);
    assert_eq!(outcome2.errors, outcome1.errors);
    assert_eq!(outcome2.skills, outcome1.skills);
}

#[tokio::test]
async fn skills_for_cwd_reuses_cached_entry_even_when_entry_has_extra_roots() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let cwd = tempfile::tempdir().expect("tempdir");
    let extra_root = tempfile::tempdir().expect("tempdir");

    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .harness_overrides(ConfigOverrides {
            cwd: Some(cwd.path().to_path_buf()),
            ..Default::default()
        })
        .build()
        .await
        .expect("defaults for test should always succeed");

    let plugins_manager = Arc::new(PluginsManager::new(codex_home.path().to_path_buf()));
    let skills_manager = SkillsManager::new(codex_home.path().to_path_buf(), plugins_manager, true);
    let _ = skills_manager.skills_for_config(&config);

    write_user_skill(&extra_root, "x", "extra-skill", "from extra root");
    let extra_root_path = extra_root.path().to_path_buf();
    let outcome_with_extra = skills_manager
        .skills_for_cwd_with_extra_user_roots(
            cwd.path(),
            &config,
            true,
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
    let outcome_without_extra = skills_manager
        .skills_for_cwd(cwd.path(), &config, false)
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

    fs::write(
        codex_home.path().join(crate::config::CONFIG_TOML_FILE),
        "[skills.bundled]\nenabled = false\n",
    )
    .expect("write config");

    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .harness_overrides(ConfigOverrides {
            cwd: Some(cwd.path().to_path_buf()),
            ..Default::default()
        })
        .build()
        .await
        .expect("load config");

    let plugins_manager = Arc::new(PluginsManager::new(codex_home.path().to_path_buf()));
    let skills_manager = SkillsManager::new(
        codex_home.path().to_path_buf(),
        plugins_manager,
        config.bundled_skills_enabled(),
    );

    // Recreate the cached bundled skill after startup cleanup so this assertion exercises
    // root selection rather than relying on directory removal succeeding.
    fs::create_dir_all(&bundled_skill_dir).expect("recreate bundled skill dir");
    fs::write(
        bundled_skill_dir.join("SKILL.md"),
        "---\nname: bundled-skill\ndescription: from bundled root\n---\n\n# Body\n",
    )
    .expect("rewrite bundled skill");

    let outcome = skills_manager.skills_for_config(&config);
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

    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .harness_overrides(ConfigOverrides {
            cwd: Some(cwd.path().to_path_buf()),
            ..Default::default()
        })
        .build()
        .await
        .expect("defaults for test should always succeed");

    let plugins_manager = Arc::new(PluginsManager::new(codex_home.path().to_path_buf()));
    let skills_manager = SkillsManager::new(codex_home.path().to_path_buf(), plugins_manager, true);
    let _ = skills_manager.skills_for_config(&config);

    write_user_skill(&extra_root_a, "x", "extra-skill-a", "from extra root a");
    write_user_skill(&extra_root_b, "x", "extra-skill-b", "from extra root b");

    let extra_root_a_path = extra_root_a.path().to_path_buf();
    let outcome_a = skills_manager
        .skills_for_cwd_with_extra_user_roots(
            cwd.path(),
            &config,
            true,
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

    let extra_root_b_path = extra_root_b.path().to_path_buf();
    let outcome_b = skills_manager
        .skills_for_cwd_with_extra_user_roots(
            cwd.path(),
            &config,
            false,
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
            cwd.path(),
            &config,
            true,
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
    let a = PathBuf::from("/tmp/a");
    let b = PathBuf::from("/tmp/b");

    let first = normalize_extra_user_roots(&[a.clone(), b.clone(), a.clone()]);
    let second = normalize_extra_user_roots(&[b, a]);

    assert_eq!(first, second);
}

#[cfg_attr(windows, ignore)]
#[test]
fn disabled_paths_from_stack_allows_session_flags_to_override_user_layer() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let skill_path = tempdir.path().join("skills").join("demo").join("SKILL.md");
    let user_file = AbsolutePathBuf::try_from(tempdir.path().join("config.toml"))
        .expect("user config path should be absolute");
    let user_layer = ConfigLayerEntry::new(
        ConfigLayerSource::User { file: user_file },
        toml::from_str(&format!(
            r#"[[skills.config]]
path = "{}"
enabled = false
"#,
            skill_path.display()
        ))
        .expect("user layer toml"),
    );
    let session_layer = ConfigLayerEntry::new(
        ConfigLayerSource::SessionFlags,
        toml::from_str(&format!(
            r#"[[skills.config]]
path = "{}"
enabled = true
"#,
            skill_path.display()
        ))
        .expect("session layer toml"),
    );
    let stack = ConfigLayerStack::new(
        vec![user_layer, session_layer],
        Default::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("valid config layer stack");

    assert_eq!(disabled_paths_from_stack(&stack), HashSet::new());
}

#[cfg_attr(windows, ignore)]
#[test]
fn disabled_paths_from_stack_allows_session_flags_to_disable_user_enabled_skill() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let skill_path = tempdir.path().join("skills").join("demo").join("SKILL.md");
    let user_file = AbsolutePathBuf::try_from(tempdir.path().join("config.toml"))
        .expect("user config path should be absolute");
    let user_layer = ConfigLayerEntry::new(
        ConfigLayerSource::User { file: user_file },
        toml::from_str(&format!(
            r#"[[skills.config]]
path = "{}"
enabled = true
"#,
            skill_path.display()
        ))
        .expect("user layer toml"),
    );
    let session_layer = ConfigLayerEntry::new(
        ConfigLayerSource::SessionFlags,
        toml::from_str(&format!(
            r#"[[skills.config]]
path = "{}"
enabled = false
"#,
            skill_path.display()
        ))
        .expect("session layer toml"),
    );
    let stack = ConfigLayerStack::new(
        vec![user_layer, session_layer],
        Default::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("valid config layer stack");

    assert_eq!(
        disabled_paths_from_stack(&stack),
        HashSet::from([skill_path])
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
    fs::write(
        codex_home.path().join(crate::config::CONFIG_TOML_FILE),
        format!(
            r#"[[skills.config]]
path = "{}"
enabled = false
"#,
            skill_path.display()
        ),
    )
    .expect("write config");

    let parent_config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .harness_overrides(ConfigOverrides {
            cwd: Some(cwd.path().to_path_buf()),
            ..Default::default()
        })
        .build()
        .await
        .expect("load parent config");
    let role_path = codex_home.path().join("enable-role.toml");
    fs::write(
        &role_path,
        format!(
            r#"[[skills.config]]
path = "{}"
enabled = true
"#,
            skill_path.display()
        ),
    )
    .expect("write role config");
    let mut child_config = parent_config.clone();
    child_config.agent_roles.insert(
        "custom".to_string(),
        crate::config::AgentRoleConfig {
            description: None,
            config_file: Some(role_path),
            nickname_candidates: None,
        },
    );
    crate::agent::role::apply_role_to_config(&mut child_config, Some("custom"))
        .await
        .expect("custom role should apply");

    let plugins_manager = Arc::new(PluginsManager::new(codex_home.path().to_path_buf()));
    let skills_manager = SkillsManager::new(codex_home.path().to_path_buf(), plugins_manager, true);

    let parent_outcome = skills_manager
        .skills_for_cwd(cwd.path(), &parent_config, true)
        .await;
    let parent_skill = parent_outcome
        .skills
        .iter()
        .find(|skill| skill.name == "demo-skill")
        .expect("demo skill should be discovered");
    assert_eq!(parent_outcome.is_skill_enabled(parent_skill), false);

    let child_outcome = skills_manager.skills_for_config(&child_config);
    let child_skill = child_outcome
        .skills
        .iter()
        .find(|skill| skill.name == "demo-skill")
        .expect("demo skill should be discovered");
    assert_eq!(child_outcome.is_skill_enabled(child_skill), true);
}
