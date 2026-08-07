use pretty_assertions::assert_eq;

use super::AliasPlan;

#[test]
fn assigns_aliases_in_first_seen_order() {
    let roots = [
        "/skills/beta",
        "/skills/alpha",
        "/skills/beta",
        "/skills/alpha",
    ];
    let plan = AliasPlan::build("r", &roots).expect("alias plan should build");

    assert_eq!(
        vec!["- `r0` = `/skills/beta`", "- `r1` = `/skills/alpha`"],
        plan.root_lines()
    );
}

#[test]
fn reuses_aliases_for_duplicate_roots() {
    let root = "/skills/shared";
    let plan = AliasPlan::build("r", &[root, root, root]).expect("alias plan should build");

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
    for (prefix, root, locator, expected) in [
        (
            "r",
            "/skills/shared",
            "/skills/shared/alpha/SKILL.md",
            "r0/alpha/SKILL.md",
        ),
        (
            "e",
            "skill://executor/workspace/skills",
            "skill://executor/workspace/skills/alpha/SKILL.md",
            "e0/alpha/SKILL.md",
        ),
        ("o", "skill://plugin", "skill://plugin/alpha", "o0/alpha"),
    ] {
        let plan = AliasPlan::build(prefix, &[root, root]).expect("alias plan should build");

        assert_eq!(Some(expected.to_string()), plan.shorten(locator));
    }
}

#[test]
fn rejects_unknown_roots_and_locators_outside_the_root() {
    let root = "/skills/shared";
    let plan = AliasPlan::build("r", &[root, root]).expect("alias plan should build");

    assert_eq!(None, plan.shorten("/skills/other/alpha"));
    assert_eq!(None, plan.shorten("/skills/shared-other/alpha"));
    assert_eq!(None, plan.shorten(root));
}

#[test]
fn returns_none_when_no_roots_are_provided() {
    assert!(AliasPlan::build("r", &[]).is_none());
}

#[test]
fn aliases_roots_used_by_only_one_skill() {
    let root = "/skills/singleton";
    let plan = AliasPlan::build("r", &[root]).expect("singleton root should produce an alias");

    assert_eq!(vec!["- `r0` = `/skills/singleton`"], plan.root_lines());
    assert_eq!(
        Some("r0/alpha/SKILL.md".to_string()),
        plan.shorten("/skills/singleton/alpha/SKILL.md")
    );
}
