use super::*;
use pretty_assertions::assert_eq;

#[test]
fn shell_environment_policy_accepts_legacy_lists_and_rules() {
    let legacy: ShellEnvironmentPolicyToml = toml::from_str(
        r#"
exclude = ["LEGACY_*", "SHARED_*"]
include_only = ["PATH", "HOME"]
"#,
    )
    .expect("legacy arrays should remain valid in config.toml");
    assert_eq!(
        legacy,
        ShellEnvironmentPolicyToml {
            exclude: Some(vec!["LEGACY_*".to_string(), "SHARED_*".to_string()]),
            include_only: Some(vec!["PATH".to_string(), "HOME".to_string()]),
            ..Default::default()
        }
    );

    let ruled: ShellEnvironmentPolicyToml = toml::from_str(
        r#"
exclude = ["KEEP_EXCLUDED", "FLIP_TO_INCLUDE"]
include_only = ["KEEP_INCLUDED", "FLIP_TO_EXCLUDE"]

[rules]
"FLIP_TO_EXCLUDE" = "exclude"
"FLIP_TO_INCLUDE" = "include"
"#,
    )
    .expect("rules should be valid in config.toml");
    assert_eq!(
        ruled,
        ShellEnvironmentPolicyToml {
            exclude: Some(vec![
                "KEEP_EXCLUDED".to_string(),
                "FLIP_TO_INCLUDE".to_string(),
            ]),
            include_only: Some(vec![
                "KEEP_INCLUDED".to_string(),
                "FLIP_TO_EXCLUDE".to_string(),
            ]),
            rules: Some(BTreeMap::from([
                (
                    "FLIP_TO_EXCLUDE".to_string(),
                    ShellEnvironmentPolicyRule::Exclude,
                ),
                (
                    "FLIP_TO_INCLUDE".to_string(),
                    ShellEnvironmentPolicyRule::Include,
                ),
            ])),
            ..Default::default()
        }
    );
    assert_eq!(
        ShellEnvironmentPolicy::from(ruled),
        ShellEnvironmentPolicy::from(ShellEnvironmentPolicyToml {
            exclude: Some(vec![
                "KEEP_EXCLUDED".to_string(),
                "FLIP_TO_EXCLUDE".to_string(),
            ]),
            include_only: Some(vec![
                "KEEP_INCLUDED".to_string(),
                "FLIP_TO_INCLUDE".to_string(),
            ]),
            ..Default::default()
        })
    );
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

#[test]
fn memories_config_clamps_count_limits_to_nonzero_values() {
    let config = MemoriesConfig::from(MemoriesToml {
        max_raw_memories_for_consolidation: Some(0),
        max_rollouts_per_startup: Some(0),
        ..Default::default()
    });

    assert_eq!(
        config,
        MemoriesConfig {
            max_raw_memories_for_consolidation: 1,
            max_rollouts_per_startup: 1,
            ..MemoriesConfig::default()
        }
    );
}

#[test]
fn memories_config_clamps_rate_limit_remaining_threshold() {
    let config = MemoriesConfig::from(MemoriesToml {
        min_rate_limit_remaining_percent: Some(101),
        ..Default::default()
    });
    assert_eq!(
        config,
        MemoriesConfig {
            min_rate_limit_remaining_percent: 100,
            ..MemoriesConfig::default()
        }
    );

    let config = MemoriesConfig::from(MemoriesToml {
        min_rate_limit_remaining_percent: Some(-1),
        ..Default::default()
    });
    assert_eq!(
        config,
        MemoriesConfig {
            min_rate_limit_remaining_percent: 0,
            ..MemoriesConfig::default()
        }
    );
}
