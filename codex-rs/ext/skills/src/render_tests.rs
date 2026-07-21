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
        SkillMetadataBudget::Characters(8_000),
    )
    .expect("catalog should render");
    let extension = available_skills_fragment(
        &catalog,
        /*include_skills_usage_instructions*/ false,
        SkillCatalogRenderPolicy::ExtensionCompatible,
        SkillMetadataBudget::Characters(8_000),
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

#[test]
fn catalog_budget_uses_capped_context_percentage_or_character_fallback() {
    assert_eq!(
        capped_skill_metadata_budget(Some(100_000)),
        SkillMetadataBudget::Tokens(2_000)
    );
    assert_eq!(
        capped_skill_metadata_budget(Some(400_000)),
        SkillMetadataBudget::Tokens(4_000)
    );
    assert_eq!(
        capped_skill_metadata_budget(/*context_window*/ None),
        SkillMetadataBudget::Characters(8_000)
    );
}

#[test]
fn omission_marker_is_charged_to_catalog_budget() {
    let catalog = SkillCatalog {
        entries: (0..20)
            .map(|index| {
                entry(
                    &format!("skill-{index:02}"),
                    "A description long enough to put the catalog under budget pressure.",
                    /*short_description*/ None,
                )
            })
            .collect(),
        warnings: Vec::new(),
    };
    let fragment = available_skills_fragment(
        &catalog,
        /*include_skills_usage_instructions*/ false,
        SkillCatalogRenderPolicy::ExtensionCompatible,
        SkillMetadataBudget::Tokens(100),
    )
    .expect("catalog should render");
    let rendered_metadata_cost = fragment
        .body()
        .lines()
        .filter(|line| line.starts_with("- "))
        .map(|line| approx_token_count(&format!("{line}\n")))
        .sum::<usize>();

    assert!(fragment.body().contains("additional skills omitted"));
    assert!(rendered_metadata_cost <= 100);
}

#[test]
fn character_fallback_counts_multibyte_metadata_by_characters() {
    let description = "💡".repeat(MAX_CATALOG_SKILL_DESCRIPTION_CHARS);
    let catalog = SkillCatalog {
        entries: vec![
            entry(
                "multibyte-one",
                &description,
                /*short_description*/ None,
            ),
            entry(
                "multibyte-two",
                &description,
                /*short_description*/ None,
            ),
        ],
        warnings: Vec::new(),
    };

    let fragment = available_skills_fragment(
        &catalog,
        /*include_skills_usage_instructions*/ false,
        SkillCatalogRenderPolicy::ExtensionCompatible,
        SkillMetadataBudget::Characters(8_000),
    )
    .expect("catalog should render");

    assert!(fragment.body().contains("multibyte-one"));
    assert!(fragment.body().contains("multibyte-two"));
    assert!(!fragment.body().contains("additional skills omitted"));
}

#[test]
fn catalog_emits_omission_marker_when_every_skill_exceeds_budget() {
    let catalog = SkillCatalog {
        entries: vec![entry(
            "oversized",
            &"x".repeat(MAX_CATALOG_SKILL_DESCRIPTION_CHARS),
            /*short_description*/ None,
        )],
        warnings: Vec::new(),
    };

    let fragment = available_skills_fragment(
        &catalog,
        /*include_skills_usage_instructions*/ false,
        SkillCatalogRenderPolicy::ExtensionCompatible,
        SkillMetadataBudget::Tokens(100),
    )
    .expect("omission marker should fit");

    assert!(!fragment.body().contains("- oversized:"));
    assert!(
        fragment
            .body()
            .contains("- 1 additional skill omitted from this bounded skills list.")
    );
}
