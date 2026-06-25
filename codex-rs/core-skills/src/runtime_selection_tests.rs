use std::collections::HashSet;

use codex_protocol::user_input::UserInput;
use pretty_assertions::assert_eq;

use super::*;

fn entry(name: &str, path: &str) -> SkillCatalogEntry {
    SkillCatalogEntry::new(
        SkillPackageId(path.to_string()),
        SkillAuthority::new(SkillSourceKind::Host, "host"),
        name,
        format!("Use {name}."),
        crate::runtime::SkillResourceId::new(path),
    )
}

fn text_input(text: &str) -> UserInput {
    UserInput::Text {
        text: text.to_string(),
        text_elements: Vec::new(),
    }
}

#[test]
fn text_mentions_follow_catalog_order() {
    let beta = entry("beta", "/skills/beta/SKILL.md");
    let alpha = entry("alpha", "/skills/alpha/SKILL.md");
    let catalog = SkillCatalog {
        entries: vec![beta.clone(), alpha.clone()],
        warnings: Vec::new(),
    };

    let selected = collect_runtime_skill_mentions(
        &[text_input("Use $alpha and $beta")],
        &catalog,
        &HashSet::new(),
    );

    assert_eq!(selected, vec![beta, alpha]);
}

#[test]
fn connector_collision_blocks_plain_name_but_not_exact_locator() {
    let calendar = entry("calendar", "skill://executor/calendar/SKILL.md");
    let catalog = SkillCatalog {
        entries: vec![calendar.clone()],
        warnings: Vec::new(),
    };
    let conflicts = HashSet::from(["calendar".to_string()]);

    assert_eq!(
        collect_runtime_skill_mentions(&[text_input("Use $calendar")], &catalog, &conflicts),
        Vec::new()
    );
    assert_eq!(
        collect_runtime_skill_mentions(
            &[text_input(
                "Use [$calendar](skill://executor/calendar/SKILL.md)",
            )],
            &catalog,
            &conflicts,
        ),
        vec![calendar]
    );
}
