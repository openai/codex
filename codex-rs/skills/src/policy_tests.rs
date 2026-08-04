use codex_protocol::protocol::SkillScope;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;

use super::resolve_disabled_skill_paths;
use crate::SkillConfigRule;
use crate::SkillConfigRuleSelector;
use crate::SkillConfigRules;
use crate::SkillMetadata;

fn skill(name: &str, path: AbsolutePathBuf) -> SkillMetadata {
    SkillMetadata {
        name: name.to_string(),
        description: "test".to_string(),
        short_description: None,
        interface: None,
        dependencies: None,
        policy: None,
        path_to_skills_md: path,
        scope: SkillScope::User,
        plugin_id: None,
        remote_plugin_id: None,
    }
}

fn test_root() -> AbsolutePathBuf {
    AbsolutePathBuf::try_from(std::env::current_dir().expect("current directory"))
        .expect("absolute current directory")
}

#[test]
fn path_rule_disables_selected_path() {
    let path = test_root().join("disable-by-path/SKILL.md");
    let rules = SkillConfigRules {
        entries: vec![SkillConfigRule {
            selector: SkillConfigRuleSelector::Path(path.clone()),
            enabled: false,
        }],
    };

    assert_eq!(
        resolve_disabled_skill_paths(&[], &rules),
        [path].into_iter().collect()
    );
}

#[test]
fn later_name_rule_reenables_path_disabled_skill() {
    let path = test_root().join("reenable-by-name/SKILL.md");
    let skills = vec![skill("demo", path.clone())];
    let rules = SkillConfigRules {
        entries: vec![
            SkillConfigRule {
                selector: SkillConfigRuleSelector::Path(path),
                enabled: false,
            },
            SkillConfigRule {
                selector: SkillConfigRuleSelector::Name("demo".to_string()),
                enabled: true,
            },
        ],
    };

    assert_eq!(
        resolve_disabled_skill_paths(&skills, &rules),
        Default::default()
    );
}

#[test]
fn later_path_rule_reenables_one_skill_disabled_by_name() {
    let root = test_root().join("reenable-by-path");
    let first_path = root.join("first/SKILL.md");
    let second_path = root.join("second/SKILL.md");
    let skills = vec![
        skill("demo", first_path.clone()),
        skill("demo", second_path.clone()),
    ];
    let rules = SkillConfigRules {
        entries: vec![
            SkillConfigRule {
                selector: SkillConfigRuleSelector::Name("demo".to_string()),
                enabled: false,
            },
            SkillConfigRule {
                selector: SkillConfigRuleSelector::Path(first_path),
                enabled: true,
            },
        ],
    };

    assert_eq!(
        resolve_disabled_skill_paths(&skills, &rules),
        [second_path].into_iter().collect()
    );
}
