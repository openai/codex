use super::*;
use crate::catalog::SkillAuthority;
use crate::catalog::SkillPackageId;
use crate::catalog::SkillResourceId;
use codex_core_skills::render_available_skills_body;
use codex_extension_api::ContextualUserFragment;
use codex_protocol::protocol::SkillScope;
use pretty_assertions::assert_eq;

fn entry(name: &str, description: &str, short_description: Option<&str>) -> SkillCatalogEntry {
    entry_with_path(
        name,
        description,
        short_description,
        &format!("/skills/{name}/SKILL.md"),
    )
}

fn entry_with_path(
    name: &str,
    description: &str,
    short_description: Option<&str>,
    path: &str,
) -> SkillCatalogEntry {
    SkillCatalogEntry::new(
        SkillPackageId(path.to_string()),
        SkillAuthority::new(SkillSourceKind::Host, "host"),
        name,
        description,
        SkillResourceId::new(path),
    )
    .with_short_description(short_description.map(str::to_string))
}

#[test]
fn ordering_follows_render_policy() {
    let catalog = SkillCatalog {
        entries: [
            ("repo-zeta", SkillScope::Repo, "/skills/repo-zeta/SKILL.md"),
            (
                "user-alpha",
                SkillScope::User,
                "/skills/user-alpha/SKILL.md",
            ),
            (
                "system-zeta",
                SkillScope::System,
                "/skills/system-zeta/SKILL.md",
            ),
            (
                "admin-alpha",
                SkillScope::Admin,
                "/skills/admin-alpha/SKILL.md",
            ),
            (
                "repo-alpha",
                SkillScope::Repo,
                "/skills/repo-alpha-z/SKILL.md",
            ),
            (
                "repo-alpha",
                SkillScope::Repo,
                "/skills/repo-alpha-a/SKILL.md",
            ),
        ]
        .into_iter()
        .map(|(name, scope, path)| {
            entry_with_path(name, "Description.", /*short_description*/ None, path)
                .with_prompt_scope(scope)
        })
        .collect(),
        warnings: Vec::new(),
    };

    let render = |policy| {
        available_skills_fragment(
            &catalog,
            /*include_skills_usage_instructions*/ false,
            policy,
            SkillMetadataBudget::Characters(usize::MAX),
        )
        .expect("catalog should render")
        .body()
    };

    assert_eq!(
        render(SkillCatalogRenderPolicy::CoreCompatible),
        render_available_skills_body(
            &[],
            &[
                "- system-zeta: Description. (file: /skills/system-zeta/SKILL.md)".to_string(),
                "- admin-alpha: Description. (file: /skills/admin-alpha/SKILL.md)".to_string(),
                "- repo-alpha: Description. (file: /skills/repo-alpha-a/SKILL.md)".to_string(),
                "- repo-alpha: Description. (file: /skills/repo-alpha-z/SKILL.md)".to_string(),
                "- repo-zeta: Description. (file: /skills/repo-zeta/SKILL.md)".to_string(),
                "- user-alpha: Description. (file: /skills/user-alpha/SKILL.md)".to_string(),
            ],
        )
    );
    assert_eq!(
        render(SkillCatalogRenderPolicy::ExtensionCompatible),
        render_available_skills_body(
            &[],
            &[
                "- repo-zeta: Description. (file: /skills/repo-zeta/SKILL.md)".to_string(),
                "- user-alpha: Description. (file: /skills/user-alpha/SKILL.md)".to_string(),
                "- system-zeta: Description. (file: /skills/system-zeta/SKILL.md)".to_string(),
                "- admin-alpha: Description. (file: /skills/admin-alpha/SKILL.md)".to_string(),
                "- repo-alpha: Description. (file: /skills/repo-alpha-z/SKILL.md)".to_string(),
                "- repo-alpha: Description. (file: /skills/repo-alpha-a/SKILL.md)".to_string(),
            ],
        )
    );
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
                "- fallback: fallback description (file: /skills/fallback/SKILL.md)".to_string(),
                "- shortened: full description (file: /skills/shortened/SKILL.md)".to_string(),
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
fn path_aliases_are_not_used_without_budget_pressure() {
    let root = "/Users/test/.codex/plugins/cache/openai-curated/example/hash/skills";
    let catalog = SkillCatalog {
        entries: vec![
            entry("alpha", "Alpha skill.", /*short_description*/ None)
                .with_display_path(format!("{root}/alpha/SKILL.md"))
                .with_display_path_root(root),
            entry("beta", "Beta skill.", /*short_description*/ None)
                .with_display_path(format!("{root}/beta/SKILL.md"))
                .with_display_path_root(root),
        ],
        warnings: Vec::new(),
    };

    let fragment = available_skills_fragment(
        &catalog,
        /*include_skills_usage_instructions*/ false,
        SkillCatalogRenderPolicy::ExtensionCompatible,
        SkillMetadataBudget::Characters(usize::MAX),
    )
    .expect("catalog should render");

    assert!(!fragment.body().contains("### Skill roots"));
    assert!(
        fragment
            .body()
            .contains(&format!("(file: {root}/alpha/SKILL.md)"))
    );
}

#[test]
fn path_aliases_retain_every_skill_under_budget_pressure() {
    let root = "/Users/test/.codex/plugins/cache/openai-curated/example/hash1234567890/skills-with-a-very-long-shared-prefix";
    let entries = (0..12)
        .map(|index| {
            let name = format!("shared-root-skill-{index}");
            entry(&name, "Description.", /*short_description*/ None)
                .with_display_path(format!("{root}/skill-{index}/SKILL.md"))
                .with_display_path_root(root)
        })
        .collect::<Vec<_>>();
    let catalog = SkillCatalog {
        entries,
        warnings: Vec::new(),
    };
    let visible_entries = catalog.entries.iter().collect::<Vec<_>>();
    let plan = build_alias_plan(
        &visible_entries,
        SkillMetadataBudget::Characters(usize::MAX),
    )
    .expect("alias plan should build");
    let alias_minimum = visible_entries.iter().fold(plan.table_cost, |cost, entry| {
        cost.saturating_add(
            SkillLine::with_locator(
                entry,
                SkillCatalogRenderPolicy::ExtensionCompatible,
                render_skill_path_with_aliases(entry, &plan),
            )
            .minimum_cost(SkillMetadataBudget::Characters(usize::MAX)),
        )
    });
    let absolute_minimum = visible_entries.iter().fold(0usize, |cost, entry| {
        cost.saturating_add(
            SkillLine::new(entry, SkillCatalogRenderPolicy::ExtensionCompatible)
                .minimum_cost(SkillMetadataBudget::Characters(usize::MAX)),
        )
    });
    assert!(alias_minimum < absolute_minimum);

    let fragment = available_skills_fragment(
        &catalog,
        /*include_skills_usage_instructions*/ true,
        SkillCatalogRenderPolicy::ExtensionCompatible,
        SkillMetadataBudget::Characters(alias_minimum),
    )
    .expect("catalog should render");
    let body = fragment.body();

    assert!(body.contains(&format!("- `r0` = `{root}`")));
    assert!(body.contains("(file: r0/skill-0/SKILL.md)"));
    assert!(body.contains("(file: r0/skill-11/SKILL.md)"));
    assert!(body.contains("Skill bodies live on disk at the listed paths after expanding"));
    assert!(!body.contains("additional skills omitted"));
}

#[test]
fn mixed_catalogs_keep_absolute_authority_aware_rendering_under_budget_pressure() {
    let root = "/Users/test/.codex/plugins/cache/openai-curated/example/hash1234567890/skills-with-a-very-long-shared-prefix";
    let mut entries = (0..12)
        .map(|index| {
            let name = format!("host-skill-{index}");
            entry(&name, "Description.", /*short_description*/ None)
                .with_display_path(format!("{root}/skill-{index}/SKILL.md"))
                .with_display_path_root(root)
        })
        .collect::<Vec<_>>();
    entries.push(
        SkillCatalogEntry::new(
            SkillPackageId("executor-skill".to_string()),
            SkillAuthority::new(SkillSourceKind::Executor, "env-1"),
            "executor-skill",
            "Description.",
            SkillResourceId::new("skill://executor/demo/SKILL.md"),
        )
        .with_display_path("skill://executor/demo/SKILL.md"),
    );
    let catalog = SkillCatalog {
        entries,
        warnings: Vec::new(),
    };
    let visible_entries = catalog.entries.iter().collect::<Vec<_>>();
    let absolute_minimum = visible_entries.iter().fold(0usize, |cost, entry| {
        cost.saturating_add(
            SkillLine::new(entry, SkillCatalogRenderPolicy::ExtensionCompatible)
                .minimum_cost(SkillMetadataBudget::Characters(usize::MAX)),
        )
    });

    assert!(
        build_alias_plan(
            &visible_entries,
            SkillMetadataBudget::Characters(usize::MAX),
        )
        .is_none()
    );

    let fragment = available_skills_fragment(
        &catalog,
        /*include_skills_usage_instructions*/ true,
        SkillCatalogRenderPolicy::ExtensionCompatible,
        SkillMetadataBudget::Characters(absolute_minimum),
    )
    .expect("catalog should render");
    let body = fragment.body();

    assert!(!body.contains("### Skill roots"));
    assert!(body.contains(&format!("(file: {root}/skill-0/SKILL.md)")));
    assert!(body.contains("(environment resource: skill://executor/demo/SKILL.md)"));
    assert!(body.contains("For a `file` entry, open the listed path."));
    assert!(!body.contains("additional skills omitted"));
}

#[test]
fn singleton_plugin_versions_share_the_marketplace_alias_root() {
    let github_root = "/Users/test/.codex/plugins/cache/openai-curated/github/hash123/skills";
    let slack_root = "/Users/test/.codex/plugins/cache/openai-curated/slack/hash456/skills";
    let entries = [
        entry("github", "GitHub skill.", /*short_description*/ None)
            .with_display_path(format!("{github_root}/github/SKILL.md"))
            .with_display_path_root(github_root),
        entry("slack", "Slack skill.", /*short_description*/ None)
            .with_display_path(format!("{slack_root}/slack/SKILL.md"))
            .with_display_path_root(slack_root),
    ];
    let visible_entries = entries.iter().collect::<Vec<_>>();

    let plan = build_alias_plan(
        &visible_entries,
        SkillMetadataBudget::Characters(usize::MAX),
    )
    .expect("alias plan should build");

    assert_eq!(
        plan.skill_root_lines,
        vec!["- `r0` = `/Users/test/.codex/plugins/cache/openai-curated`".to_string()]
    );
    assert_eq!(
        render_skill_path_with_aliases(&entries[0], &plan),
        "r0/github/hash123/skills/github/SKILL.md"
    );
    assert_eq!(
        render_skill_path_with_aliases(&entries[1], &plan),
        "r0/slack/hash456/skills/slack/SKILL.md"
    );
}

#[test]
fn omission_notice_follows_render_policy_and_is_charged_to_catalog_budget() {
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
    let core_fragment = available_skills_fragment(
        &catalog,
        /*include_skills_usage_instructions*/ false,
        SkillCatalogRenderPolicy::CoreCompatible,
        SkillMetadataBudget::Tokens(100),
    )
    .expect("core-compatible catalog should render");
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

    assert!(!core_fragment.body().contains("additional skills omitted"));
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
fn catalog_report_counts_partial_description_truncation() {
    let catalog = SkillCatalog {
        entries: vec![entry(
            "partial",
            "abcdefghij",
            /*short_description*/ None,
        )],
        warnings: Vec::new(),
    };
    let expected_line = "- partial: abcd (file: /skills/partial/SKILL.md)";
    let budget = SkillMetadataBudget::Characters(metadata_line_cost(
        SkillMetadataBudget::Characters(usize::MAX),
        expected_line,
    ));

    let render = render_available_skills(
        &catalog,
        SkillCatalogRenderPolicy::ExtensionCompatible,
        budget,
    )
    .expect("catalog should render");
    assert_eq!(
        render.report,
        SkillRenderReport {
            total_count: 1,
            included_count: 1,
            omitted_count: 0,
            truncated_description_chars: 6,
            truncated_description_count: 1,
        }
    );
    let fragment = render
        .into_fragment(/*include_skills_usage_instructions*/ false)
        .expect("partial description should render");
    assert!(fragment.body().contains(expected_line));
}

#[test]
fn catalog_emits_omission_marker_when_every_minimum_skill_line_exceeds_budget() {
    let oversized = entry(
        "oversized",
        &"x".repeat(MAX_CATALOG_SKILL_DESCRIPTION_CHARS),
        /*short_description*/ None,
    )
    .with_display_path(format!("skill://{}", "x".repeat(512)));
    let catalog = SkillCatalog {
        entries: vec![oversized],
        warnings: Vec::new(),
    };

    let expected_report = SkillRenderReport {
        total_count: 1,
        included_count: 0,
        omitted_count: 1,
        truncated_description_chars: MAX_CATALOG_SKILL_DESCRIPTION_CHARS,
        truncated_description_count: 1,
    };
    assert_eq!(
        expected_report.warning_message(),
        Some(
            "Exceeded skills context budget. All skill descriptions were removed and 1 additional skill was not included in the model-visible skills list."
                .to_string()
        )
    );
    let core_render = render_available_skills(
        &catalog,
        SkillCatalogRenderPolicy::CoreCompatible,
        SkillMetadataBudget::Tokens(100),
    )
    .expect("core-compatible report should render");
    assert_eq!(core_render.report, expected_report);
    assert_eq!(
        core_render.into_fragment(/*include_skills_usage_instructions*/ false),
        None
    );
    let render = render_available_skills(
        &catalog,
        SkillCatalogRenderPolicy::ExtensionCompatible,
        SkillMetadataBudget::Tokens(100),
    )
    .expect("catalog should render");
    assert_eq!(render.report, expected_report);
    let fragment = render
        .into_fragment(/*include_skills_usage_instructions*/ false)
        .expect("omission marker should fit");

    assert!(!fragment.body().contains("- oversized:"));
    assert!(
        fragment
            .body()
            .contains("- 1 additional skill omitted from this bounded skills list.")
    );
}

#[test]
fn catalog_preserves_report_when_no_fragment_fits_budget() {
    let oversized = entry(
        "oversized",
        &"x".repeat(MAX_CATALOG_SKILL_DESCRIPTION_CHARS),
        /*short_description*/ None,
    )
    .with_display_path(format!("skill://{}", "x".repeat(512)));
    let catalog = SkillCatalog {
        entries: vec![oversized],
        warnings: Vec::new(),
    };

    let render = render_available_skills(
        &catalog,
        SkillCatalogRenderPolicy::ExtensionCompatible,
        SkillMetadataBudget::Tokens(1),
    )
    .expect("catalog should produce a report");
    assert_eq!(
        render.report,
        SkillRenderReport {
            total_count: 1,
            included_count: 0,
            omitted_count: 1,
            truncated_description_chars: MAX_CATALOG_SKILL_DESCRIPTION_CHARS,
            truncated_description_count: 1,
        }
    );
    assert!(
        render
            .into_fragment(/*include_skills_usage_instructions*/ false)
            .is_none()
    );
}

#[test]
fn substantial_description_shortening_emits_warning() {
    let catalog = SkillCatalog {
        entries: vec![
            entry(
                "long-skill",
                &"a".repeat(250),
                /*short_description*/ None,
            ),
            entry("empty-skill", "", /*short_description*/ None),
        ],
        warnings: Vec::new(),
    };
    let skill_lines = catalog
        .entries
        .iter()
        .map(|entry| SkillLine::new(entry, SkillCatalogRenderPolicy::ExtensionCompatible))
        .collect::<Vec<_>>();
    let minimum_cost = skill_lines.iter().fold(0usize, |used, line| {
        used.saturating_add(line.minimum_cost(SkillMetadataBudget::Characters(usize::MAX)))
    });
    let render = render_available_skills(
        &catalog,
        SkillCatalogRenderPolicy::ExtensionCompatible,
        SkillMetadataBudget::Characters(minimum_cost + 49),
    )
    .expect("catalog should render");

    assert_eq!(
        render.report.warning_message(),
        Some(
            "Skill descriptions were shortened to fit the skills context budget. Codex can still see every skill, but some descriptions are shorter. Disable unused skills or plugins to leave more room for the rest."
                .to_string()
        )
    );
}

#[test]
fn substantial_description_shortening_warning_starts_above_threshold() {
    let report_at_threshold = SkillRenderReport {
        total_count: 2,
        included_count: 2,
        omitted_count: 0,
        truncated_description_chars: 200,
        truncated_description_count: 2,
    };
    assert_eq!(report_at_threshold.warning_message(), None);

    let report_above_threshold = SkillRenderReport {
        truncated_description_chars: 201,
        ..report_at_threshold
    };
    assert_eq!(
        report_above_threshold.warning_message(),
        Some(SKILL_DESCRIPTION_TRUNCATED_WARNING.to_string())
    );
}
