use crate::CONFIG_TOML_FILE;
use crate::ConfigLayerEntry;
use crate::ConfigLayerSource;
use crate::ConfigLayerStack;
use crate::ConfigRequirementsToml;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::test_support::PathBufExt;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::SkillConfigRule;
use super::SkillConfigRuleSelector;
use super::SkillConfigRules;
use super::bundled_skills_enabled_from_stack;
use super::skill_config_rules_from_stack;

fn user_layer(codex_home: &TempDir, config: &str) -> ConfigLayerEntry {
    let config_path = AbsolutePathBuf::try_from(codex_home.path().join(CONFIG_TOML_FILE))
        .expect("absolute config path");
    ConfigLayerEntry::new(
        ConfigLayerSource::User {
            file: config_path,
            profile: None,
        },
        toml::from_str(config).expect("valid user config"),
    )
}

fn stack(codex_home: &TempDir, user: &str, session: &str) -> ConfigLayerStack {
    ConfigLayerStack::new(
        vec![
            user_layer(codex_home, user),
            ConfigLayerEntry::new(
                ConfigLayerSource::SessionFlags,
                toml::from_str(session).expect("valid session config"),
            ),
        ],
        Default::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("valid config stack")
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

#[test]
fn bundled_skills_follow_effective_configuration() {
    let codex_home = TempDir::new().expect("temp dir");

    assert!(bundled_skills_enabled_from_stack(&stack(
        &codex_home,
        "",
        ""
    )));
    assert!(!bundled_skills_enabled_from_stack(&stack(
        &codex_home,
        "[skills.bundled]\nenabled = false\n",
        ""
    )));
    assert!(bundled_skills_enabled_from_stack(&stack(
        &codex_home,
        "[skills.bundled]\nenabled = false\n",
        "[skills.bundled]\nenabled = true\n"
    )));
}

#[test]
fn malformed_bundled_skills_config_defaults_to_enabled() {
    let codex_home = TempDir::new().expect("temp dir");

    assert!(bundled_skills_enabled_from_stack(&stack(
        &codex_home,
        "[skills]\nbundled = 'invalid'\n",
        ""
    )));
}

#[test]
fn session_flags_can_reenable_user_disabled_path() {
    let codex_home = TempDir::new().expect("temp dir");
    let skill_path = codex_home.path().join("skills/demo/SKILL.md");

    assert_eq!(
        skill_config_rules_from_stack(&stack(
            &codex_home,
            &path_toggle_config(&skill_path, /*enabled*/ false),
            &path_toggle_config(&skill_path, /*enabled*/ true),
        )),
        SkillConfigRules {
            entries: vec![SkillConfigRule {
                selector: SkillConfigRuleSelector::Path(skill_path.abs()),
                enabled: true,
            }],
        }
    );
}

#[test]
fn session_flags_can_disable_user_enabled_path() {
    let codex_home = TempDir::new().expect("temp dir");
    let skill_path = codex_home.path().join("skills/demo/SKILL.md");

    assert_eq!(
        skill_config_rules_from_stack(&stack(
            &codex_home,
            &path_toggle_config(&skill_path, /*enabled*/ true),
            &path_toggle_config(&skill_path, /*enabled*/ false),
        )),
        SkillConfigRules {
            entries: vec![SkillConfigRule {
                selector: SkillConfigRuleSelector::Path(skill_path.abs()),
                enabled: false,
            }],
        }
    );
}

#[test]
fn preserves_name_selectors() {
    let codex_home = TempDir::new().expect("temp dir");

    assert_eq!(
        skill_config_rules_from_stack(&stack(
            &codex_home,
            r#"
[[skills.config]]
name = "github:yeet"
enabled = false
"#,
            "",
        )),
        SkillConfigRules {
            entries: vec![SkillConfigRule {
                selector: SkillConfigRuleSelector::Name("github:yeet".to_string()),
                enabled: false,
            }],
        }
    );
}

#[test]
fn preserves_order_across_path_and_name_selectors() {
    let codex_home = TempDir::new().expect("temp dir");
    let skill_path = codex_home.path().join("skills/demo/SKILL.md");

    assert_eq!(
        skill_config_rules_from_stack(&stack(
            &codex_home,
            &path_toggle_config(&skill_path, /*enabled*/ false),
            r#"
[[skills.config]]
name = "github:yeet"
enabled = true
"#,
        )),
        SkillConfigRules {
            entries: vec![
                SkillConfigRule {
                    selector: SkillConfigRuleSelector::Path(skill_path.abs()),
                    enabled: false,
                },
                SkillConfigRule {
                    selector: SkillConfigRuleSelector::Name("github:yeet".to_string()),
                    enabled: true,
                },
            ],
        }
    );
}

#[test]
fn path_rule_disables_selected_path() {
    let codex_home = TempDir::new().expect("temp dir");
    let path = codex_home.path().join("disable-by-path/SKILL.md").abs();
    let rules = SkillConfigRules {
        entries: vec![SkillConfigRule {
            selector: SkillConfigRuleSelector::Path(path.clone()),
            enabled: false,
        }],
    };

    assert_eq!(
        rules.resolve_disabled_paths(std::iter::empty()),
        [path].into_iter().collect()
    );
}

#[test]
fn later_name_rule_reenables_path_disabled_skill() {
    let codex_home = TempDir::new().expect("temp dir");
    let path = codex_home.path().join("reenable-by-name/SKILL.md").abs();
    let rules = SkillConfigRules {
        entries: vec![
            SkillConfigRule {
                selector: SkillConfigRuleSelector::Path(path.clone()),
                enabled: false,
            },
            SkillConfigRule {
                selector: SkillConfigRuleSelector::Name("demo".to_string()),
                enabled: true,
            },
        ],
    };

    assert_eq!(
        rules.resolve_disabled_paths([("demo", &path)]),
        Default::default()
    );
}

#[test]
fn later_path_rule_reenables_one_skill_disabled_by_name() {
    let codex_home = TempDir::new().expect("temp dir");
    let root = codex_home.path().join("reenable-by-path");
    let first_path = root.join("first/SKILL.md").abs();
    let second_path = root.join("second/SKILL.md").abs();
    let rules = SkillConfigRules {
        entries: vec![
            SkillConfigRule {
                selector: SkillConfigRuleSelector::Name("demo".to_string()),
                enabled: false,
            },
            SkillConfigRule {
                selector: SkillConfigRuleSelector::Path(first_path.clone()),
                enabled: true,
            },
        ],
    };

    assert_eq!(
        rules.resolve_disabled_paths([("demo", &first_path), ("demo", &second_path)]),
        [second_path].into_iter().collect()
    );
}
