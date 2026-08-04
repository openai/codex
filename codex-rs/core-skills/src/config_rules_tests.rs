use codex_config::CONFIG_TOML_FILE;
use codex_config::ConfigLayerEntry;
use codex_config::ConfigLayerSource;
use codex_config::ConfigLayerStack;
use codex_config::ConfigRequirementsToml;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::test_support::PathBufExt;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::SkillConfigRule;
use super::SkillConfigRuleSelector;
use super::SkillConfigRules;
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
    format!(
        r#"[[skills.config]]
path = "{}"
enabled = {enabled}
"#,
        path.display()
    )
}

#[cfg_attr(windows, ignore)]
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

#[cfg_attr(windows, ignore)]
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

#[cfg_attr(windows, ignore)]
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
