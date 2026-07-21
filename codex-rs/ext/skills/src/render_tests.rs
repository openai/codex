use super::*;
use crate::catalog::SkillAuthority;
use crate::catalog::SkillPackageId;
use crate::catalog::SkillResourceId;
use codex_core_skills::render_available_skills_body;
use codex_extension_api::ContextualUserFragment;
use pretty_assertions::assert_eq;

fn entry(name: &str, description: &str, short_description: Option<&str>) -> SkillCatalogEntry {
    SkillCatalogEntry::new(
        SkillPackageId(name.to_string()),
        SkillAuthority::new(SkillSourceKind::Host, "host"),
        name,
        description,
        SkillResourceId::new(format!("/skills/{name}/SKILL.md")),
    )
    .with_short_description(short_description.map(str::to_string))
}

#[test]
fn description_selection_follows_render_policy() {
    let catalog = SkillCatalog {
        entries: vec![
            entry("shortened", "full description", Some("short description")),
            entry(
                "fallback",
                "fallback description",
                /*short_description*/ None,
            ),
        ],
        warnings: Vec::new(),
    };

    let core = available_skills_fragment(
        &catalog,
        /*include_skills_usage_instructions*/ false,
        SkillCatalogRenderPolicy::CoreCompatible,
    )
    .expect("catalog should render");
    let extension = available_skills_fragment(
        &catalog,
        /*include_skills_usage_instructions*/ false,
        SkillCatalogRenderPolicy::ExtensionCompatible,
    )
    .expect("catalog should render");

    assert_eq!(
        core.body(),
        render_available_skills_body(
            &[],
            &[
                "- shortened: full description (file: /skills/shortened/SKILL.md)".to_string(),
                "- fallback: fallback description (file: /skills/fallback/SKILL.md)".to_string(),
            ],
        )
    );
    assert_eq!(
        extension.body(),
        render_available_skills_body(
            &[],
            &[
                "- shortened: short description (file: /skills/shortened/SKILL.md)".to_string(),
                "- fallback: fallback description (file: /skills/fallback/SKILL.md)".to_string(),
            ],
        )
    );
}
