use super::*;
use codex_app_server_protocol::ConfigLayerSource;
use codex_config::ConfigLayerEntry;
use codex_config::ConfigLayerStack;
use codex_config::ConfigRequirements;
use codex_config::ConfigRequirementsToml;
use codex_protocol::protocol::SkillScope;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::fs;
use std::path::Path;
use tempfile::tempdir;
use toml::Value as TomlValue;

fn write_skill(root: &Path, dir: &str, name: &str) {
    let skill_dir = root.join(dir);
    fs::create_dir_all(&skill_dir).expect("create skill dir");
    fs::write(
        skill_dir.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {name}\n---\n"),
    )
    .expect("write skill");
}

fn user_config_layer_stack(codex_home: &Path) -> ConfigLayerStack {
    ConfigLayerStack::new(
        vec![ConfigLayerEntry::new(
            ConfigLayerSource::User {
                file: AbsolutePathBuf::from_absolute_path(codex_home.join("config.toml"))
                    .expect("absolute config path"),
            },
            TomlValue::Table(toml::map::Map::new()),
        )],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("config layer stack")
}

#[test]
fn skill_roots_for_request_omits_system_scope_when_bundled_skills_are_disabled() {
    let codex_home = tempdir().expect("tempdir");
    let manager = SkillsManager::new(codex_home.path().to_path_buf(), false);
    let request = SkillLoadRequest {
        cwd: codex_home.path().to_path_buf(),
        config_layer_stack: user_config_layer_stack(codex_home.path()),
        bundled_skills_enabled: false,
        plugin_skill_roots: Vec::new(),
        extra_user_roots: Vec::new(),
    };

    let roots = manager.skill_roots_for_request(&request);

    assert!(roots.iter().all(|root| root.scope != SkillScope::System));
    assert!(roots.iter().any(
        |root| root.scope == SkillScope::User && root.path == codex_home.path().join("skills")
    ));
}

#[test]
fn skills_for_request_loads_plugin_and_extra_user_roots() {
    let codex_home = tempdir().expect("tempdir");
    let plugin_root = tempdir().expect("tempdir");
    let extra_root = tempdir().expect("tempdir");
    write_skill(plugin_root.path(), "plugin-skill", "plugin-skill");
    write_skill(extra_root.path(), "extra-skill", "extra-skill");

    let manager = SkillsManager::new(codex_home.path().to_path_buf(), false);
    let outcome = manager.skills_for_request(&SkillLoadRequest {
        cwd: codex_home.path().to_path_buf(),
        config_layer_stack: ConfigLayerStack::default(),
        bundled_skills_enabled: false,
        plugin_skill_roots: vec![plugin_root.path().to_path_buf()],
        extra_user_roots: vec![extra_root.path().to_path_buf()],
    });

    assert_eq!(
        outcome
            .skills
            .iter()
            .map(|skill| skill.name.clone())
            .collect::<Vec<_>>(),
        vec!["extra-skill".to_string(), "plugin-skill".to_string()]
    );
}
