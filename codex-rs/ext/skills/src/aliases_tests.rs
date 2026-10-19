use pretty_assertions::assert_eq;

use super::AliasPlan;
use super::SkillLocator;
use crate::catalog::SkillSourceKind;

#[test]
fn assigns_aliases_in_first_seen_order() {
    let roots = [
        "/skills/beta",
        "/skills/alpha",
        "/skills/beta",
        "/skills/alpha",
    ];
    let plan = AliasPlan::build(&SkillSourceKind::Host, &roots).expect("alias plan should build");

    assert_eq!(
        vec!["- `r0` = `/skills/beta`", "- `r1` = `/skills/alpha`"],
        plan.root_lines()
    );
}

#[test]
fn reuses_aliases_for_duplicate_roots() {
    let root = "/skills/shared";
    let plan = AliasPlan::build(&SkillSourceKind::Host, &[root, root, root])
        .expect("alias plan should build");

    assert_eq!(vec!["- `r0` = `/skills/shared`"], plan.root_lines());
    assert_eq!(
        Some("r0/alpha/SKILL.md".to_string()),
        plan.shorten("/skills/shared/alpha/SKILL.md")
    );
    assert_eq!(
        Some("r0/beta/SKILL.md".to_string()),
        plan.shorten("/skills/shared/beta/SKILL.md")
    );
}

#[test]
fn shortens_filesystem_and_skill_resource_locators() {
    for (source, root, locator, expected) in [
        (
            SkillSourceKind::Host,
            "/skills/shared",
            "/skills/shared/alpha/SKILL.md",
            "r0/alpha/SKILL.md",
        ),
        (
            SkillSourceKind::Executor,
            "skill://executor/workspace/skills",
            "skill://executor/workspace/skills/alpha/SKILL.md",
            "e0/alpha/SKILL.md",
        ),
        (
            SkillSourceKind::Cloud,
            "skill://plugin",
            "skill://plugin/alpha",
            "c0/alpha",
        ),
    ] {
        let plan = AliasPlan::build(&source, &[root, root]).expect("alias plan should build");

        let shortened = plan.shorten(locator).expect("known root");
        assert_eq!(shortened, expected);
        let json = serde_json::to_string(&shortened).unwrap();
        let parsed: SkillLocator = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.alias_source(), Some(&source));
        assert_eq!(parsed.as_str(), expected);
        assert_eq!(serde_json::to_string(&parsed).unwrap(), json);
    }
}

#[test]
fn rejects_unknown_roots_and_locators_outside_the_root() {
    let root = "/skills/shared";
    let plan =
        AliasPlan::build(&SkillSourceKind::Host, &[root, root]).expect("alias plan should build");

    assert_eq!(None, plan.shorten("/skills/other/alpha"));
    assert_eq!(None, plan.shorten("/skills/shared-other/alpha"));
    assert_eq!(None, plan.shorten(root));
}

#[test]
fn returns_none_when_no_roots_are_provided() {
    assert!(AliasPlan::build(&SkillSourceKind::Host, &[]).is_none());
}

#[test]
fn aliases_roots_used_by_only_one_skill() {
    let root = "/skills/singleton";
    let plan = AliasPlan::build(&SkillSourceKind::Host, &[root])
        .expect("singleton root should produce an alias");

    assert_eq!(vec!["- `r0` = `/skills/singleton`"], plan.root_lines());
    assert_eq!(
        Some("r0/alpha/SKILL.md".to_string()),
        plan.shorten("/skills/singleton/alpha/SKILL.md")
    );
}

#[test]
fn locator_serde_recognizes_aliases_and_preserves_package_strings() {
    for (value, expected_source) in [
        ("c0/release", Some(SkillSourceKind::Cloud)),
        ("e12/references/details.md", Some(SkillSourceKind::Executor)),
        ("r1/release/SKILL.md", Some(SkillSourceKind::Host)),
        ("skill://Plugin_123/release", None),
        ("/skills/release/SKILL.md", None),
        (r"C:\skills\release\SKILL.md", None),
        ("cloud/release", None),
        ("c/release", None),
        ("c00/release", None),
        ("e-1/release", None),
        ("e+1/release", None),
        ("c0", None),
        ("c0/", None),
        ("/release", None),
        ("é0/release", None),
        ("e999999999999999999999999999999/release", None),
    ] {
        let json = serde_json::to_string(value).unwrap();
        let locator: SkillLocator = serde_json::from_str(&json).unwrap();
        assert_eq!(locator.alias_source(), expected_source.as_ref(), "{value}");
        assert_eq!(locator.as_str(), value);
        assert_eq!(serde_json::to_string(&locator).unwrap(), json);
    }
}
