use super::*;
use pretty_assertions::assert_eq;

#[test]
fn deserialize_marketplace_source_type_with_legacy_path_alias() {
    let cfg: MarketplaceConfig = toml::from_str(
        r#"
            source_type = "path"
            source = "/tmp/debug"
        "#,
    )
    .expect("should deserialize marketplace config with legacy path source type");

    assert_eq!(cfg.source_type, Some(MarketplaceSourceType::Local));
    assert_eq!(cfg.source.as_deref(), Some("/tmp/debug"));
}

#[test]
fn deserialize_skill_config_with_name_selector() {
    let cfg: SkillConfig = toml::from_str(
        r#"
            name = "github:yeet"
            enabled = false
        "#,
    )
    .expect("should deserialize skill config with name selector");

    assert_eq!(cfg.name.as_deref(), Some("github:yeet"));
    assert_eq!(cfg.path, None);
    assert!(!cfg.enabled);
}

#[test]
fn deserialize_skill_config_with_path_selector() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let skill_path = tempdir.path().join("skills").join("demo").join("SKILL.md");
    let cfg: SkillConfig = toml::from_str(&format!(
        r#"
            path = {path:?}
            enabled = false
        "#,
        path = skill_path.display().to_string(),
    ))
    .expect("should deserialize skill config with path selector");

    assert_eq!(
        cfg,
        SkillConfig {
            path: Some(
                AbsolutePathBuf::from_absolute_path(&skill_path)
                    .expect("skill path should be absolute"),
            ),
            name: None,
            enabled: false,
        }
    );
}
