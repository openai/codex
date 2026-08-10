use super::*;
use crate::HostSkillsSnapshot;
use crate::catalog::SkillAuthority;
use crate::catalog::SkillPackageId;
use crate::catalog::SkillResourceId;
use crate::provider::HostSkillProvider;
use crate::provider::SkillListQuery;
use crate::provider::SkillProvider;
use codex_exec_server::LOCAL_FS;
use codex_extension_api::ContextualUserFragment;
use codex_protocol::protocol::SkillScope;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use tokio::sync::Semaphore;

use crate::catalog_prompt::render_available_skills_body;
use crate::loader::HostSkillRoot;
use crate::loader::load_and_merge_host_skill_roots;

#[test]
fn skill_prompt_contents_are_bounded_at_utf8_boundaries() {
    let contents = format!("{}é", "a".repeat(MAX_SKILL_PROMPT_BYTES - 1));

    let (bounded, truncated) = truncate_main_prompt_contents(&contents);

    assert_eq!(bounded.len(), MAX_SKILL_PROMPT_BYTES - 1);
    assert_eq!(truncated, true);
}

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
            SkillPromptKind::Unaliased,
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
            SkillPromptKind::Unaliased,
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
            SkillPromptKind::Unaliased,
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
            SkillPromptKind::Unaliased,
            &[],
            &[
                "- shortened: short description (file: /skills/shortened/SKILL.md)".to_string(),
                "- fallback: fallback description (file: /skills/fallback/SKILL.md)".to_string(),
            ],
        )
    );
}

#[test]
fn catalog_budget_uses_context_percentage_or_character_fallback() {
    assert_eq!(
        skill_metadata_budget(Some(100_000)),
        SkillMetadataBudget::Tokens(2_000)
    );
    assert_eq!(
        skill_metadata_budget(Some(400_000)),
        SkillMetadataBudget::Tokens(8_000)
    );
    assert_eq!(
        skill_metadata_budget(/*context_window*/ None),
        SkillMetadataBudget::Characters(8_000)
    );
}

#[test]
fn host_only_prompts_preserve_existing_behavior_with_and_without_aliases() {
    let root = "/Users/test/.codex/plugins/cache/openai-curated/host-plugin/1.0.0/skills-with-a-long-shared-root";
    let catalog = SkillCatalog {
        entries: [
            ("alpha", "Alpha skill."),
            ("beta", "Beta skill."),
            ("gamma", "Gamma skill."),
        ]
        .into_iter()
        .map(|(name, description)| {
            entry(name, description, /*short_description*/ None)
                .with_display_path(format!("{root}/{name}/SKILL.md"))
                .with_alias_root(root)
        })
        .collect(),
        warnings: Vec::new(),
    };

    let unaliased = available_skills_fragment(
        &catalog,
        /*include_skills_usage_instructions*/ true,
        SkillCatalogRenderPolicy::CoreCompatible,
        SkillMetadataBudget::Characters(usize::MAX),
    )
    .expect("unaliased host catalog should render");
    insta::assert_snapshot!(unaliased.body(), @r###"

    ## Skills
    A skill is a set of instructions provided through a `SKILL.md` source. Below is the list of skills that can be used. Each entry includes a name, description, and source locator. `file` locators are on the host filesystem, `environment resource` locators are owned by an execution environment, `orchestrator package` locators are opaque package identifiers, and `custom resource` locators use their provider's access mechanism.
    ### Available skills
    - alpha: Alpha skill. (file: /Users/test/.codex/plugins/cache/openai-curated/host-plugin/1.0.0/skills-with-a-long-shared-root/alpha/SKILL.md)
    - beta: Beta skill. (file: /Users/test/.codex/plugins/cache/openai-curated/host-plugin/1.0.0/skills-with-a-long-shared-root/beta/SKILL.md)
    - gamma: Gamma skill. (file: /Users/test/.codex/plugins/cache/openai-curated/host-plugin/1.0.0/skills-with-a-long-shared-root/gamma/SKILL.md)
    ### How to use skills
    - Discovery: The list above is the skills available in this session (name + description + source locator). `file` entries live on the host filesystem, `environment resource` entries are accessed through `skills.list` and `skills.read`, `orchestrator package` entries are accessed directly through `skills.read`, and `custom resource` entries use their provider's access mechanism.
    - Trigger rules: If the user names a skill (with `$SkillName` or plain text) OR the task clearly matches a skill's description shown above, you must use that skill for that turn. Multiple mentions mean use them all. Do not carry skills across turns unless re-mentioned.
    - Missing/blocked: If a named skill isn't in the list or its source can't be read, say so briefly and continue with the best fallback.
    - How to use a skill (progressive disclosure):
      1) After deciding to use a skill, the main agent must read its `SKILL.md` completely before taking task actions. For a `file` entry, open the listed path. For an `environment resource`, call `skills.list` with `{"authority":{"kind":"executor"}}`, select the matching package, and pass its exact package and `main_resource` to `skills.read`. For an `orchestrator package`, expand its root alias when present and call `skills.read` with the complete locator as `package`; omit `resource` to read `SKILL.md` directly without calling `skills.list`. Follow `next_cursor`; if a read is paginated, continue until EOF.
      2) When `SKILL.md` references another resource, use the same access mechanism. Resolve relative references beneath an executor skill's returned package and call `skills.read` with the same package. For orchestrator skills, pass the exact referenced resource identifier with the same package to `skills.read`; do not treat `skill://` identifiers as filesystem paths.
      3) If `SKILL.md` points to extra folders such as `references/`, use its routing instructions to identify the resources required for the task. The main agent must read each required instruction or reference file itself before acting on it. Do not delegate reading, summarizing, or interpreting skill instructions to a subagent. Subagents may still perform task work when the selected skill allows it.
      4) For filesystem-backed skills, prefer running or patching provided scripts instead of retyping large code blocks. For environment and orchestrator skills, use `skills.read` and the available tools; do not invent a local path.
      5) Reuse provided assets or templates through the same source access mechanism instead of recreating them.
    - Coordination and sequencing:
      - If multiple skills apply, choose the minimal set that covers the request and state the order you'll use them.
      - Announce which skill(s) you're using and why (one short line). If you skip an obvious skill, say why.
    - Context hygiene:
      - Progressive disclosure applies to selecting relevant files, not partially reading a selected instruction file. Do not load unrelated references, scripts, or assets.
      - Avoid deep reference-chasing: prefer opening only files directly linked from `SKILL.md` unless you're blocked.
      - When variants exist (frameworks, providers, domains), pick only the relevant reference file(s) and note that choice.
    - Safety and fallback: If a skill can't be applied cleanly (missing files, unclear instructions), state the issue, pick the next-best approach, and continue.
    "###);

    let aliased = available_skills_fragment(
        &catalog,
        /*include_skills_usage_instructions*/ true,
        SkillCatalogRenderPolicy::CoreCompatible,
        SkillMetadataBudget::Characters(200),
    )
    .expect("aliased host catalog should render");
    insta::assert_snapshot!(aliased.body(), @r###"

    ## Skills
    A skill is a set of local instructions to follow that is stored in a `SKILL.md` file. Below is the list of skills that can be used. Each entry includes a name, description, and a short path that can be expanded into an absolute path using the skill roots table.
    ### Skill roots
    - `r0` = `/Users/test/.codex/plugins/cache/openai-curated/host-plugin/1.0.0/skills-with-a-long-shared-root`
    ### Available skills
    - alpha: Alpha skill. (file: r0/alpha/SKILL.md)
    - beta: Beta skill. (file: r0/beta/SKILL.md)
    - gamma: Gamma skill. (file: r0/gamma/SKILL.md)
    ### How to use skills
    - Discovery: The list above is the skills available in this session (name + description + short path). Skill bodies live on disk at the listed paths after expanding the matching alias from `### Skill roots`.
    - Trigger rules: If the user names a skill (with `$SkillName` or plain text) OR the task clearly matches a skill's description shown above, you must use that skill for that turn. Multiple mentions mean use them all. Do not carry skills across turns unless re-mentioned.
    - Missing/blocked: If a named skill isn't in the list or the path can't be read, say so briefly and continue with the best fallback.
    - How to use a skill (progressive disclosure):
      1) After deciding to use a skill, the main agent must expand the listed short `path` with the matching alias from `### Skill roots`, then open and read its `SKILL.md` completely before taking task actions. If a read is truncated or paginated, continue until EOF.
      2) When `SKILL.md` references relative paths (e.g., `scripts/foo.py`), resolve them relative to the directory containing that expanded `SKILL.md` first, and only consider other paths if needed.
      3) If `SKILL.md` points to extra folders such as `references/`, use its routing instructions to identify the files required for the task. The main agent must read each required instruction or reference file itself before acting on it. Do not delegate reading, summarizing, or interpreting skill instructions to a subagent. Subagents may still perform task work when the selected skill allows it.
      4) If `scripts/` exist, prefer running or patching them instead of retyping large code blocks.
      5) If `assets/` or templates exist, reuse them instead of recreating from scratch.
    - Coordination and sequencing:
      - If multiple skills apply, choose the minimal set that covers the request and state the order you'll use them.
      - Announce which skill(s) you're using and why (one short line). If you skip an obvious skill, say why.
    - Context hygiene:
      - Progressive disclosure applies to selecting relevant files, not partially reading a selected instruction file. Do not load unrelated references, scripts, or assets.
      - Avoid deep reference-chasing: prefer opening only files directly linked from `SKILL.md` unless you're blocked.
      - When variants exist (frameworks, providers, domains), pick only the relevant reference file(s) and note that choice.
    - Safety and fallback: If a skill can't be applied cleanly (missing files, unclear instructions), state the issue, pick the next-best approach, and continue.
    "###);
}

#[test]
fn path_aliases_are_not_used_without_budget_pressure() {
    let root = "/Users/test/.codex/plugins/cache/openai-curated/example/hash/skills";
    let catalog = SkillCatalog {
        entries: vec![
            entry("alpha", "Alpha skill.", /*short_description*/ None)
                .with_display_path(format!("{root}/alpha/SKILL.md"))
                .with_alias_root(root),
            entry("beta", "Beta skill.", /*short_description*/ None)
                .with_display_path(format!("{root}/beta/SKILL.md"))
                .with_alias_root(root),
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
                .with_alias_root(root)
        })
        .collect::<Vec<_>>();
    let catalog = SkillCatalog {
        entries,
        warnings: Vec::new(),
    };
    let visible_entries = catalog.entries.iter().collect::<Vec<_>>();
    let plan = build_alias_plan(&visible_entries).expect("alias plan should build");
    let table_cost = aliased_metadata_overhead_cost(
        SkillMetadataBudget::Characters(usize::MAX),
        SkillPromptKind::HostAliases,
        &plan.root_lines(),
        /*include_skills_usage_instructions*/ true,
    );
    let alias_minimum = visible_entries.iter().fold(table_cost, |cost, entry| {
        cost.saturating_add(
            SkillLine::with_locator(
                entry,
                SkillCatalogRenderPolicy::ExtensionCompatible,
                render_skill_locator_with_aliases(entry, &plan),
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

#[tokio::test]
async fn host_alias_roots_follow_core_discovery_order() -> Result<(), Box<dyn std::error::Error>> {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let parent = std::env::temp_dir().join(format!(
        "codex-skills-extension-alias-order-{}-{unique}",
        std::process::id()
    ));
    let user_root_path = parent.join("user-root");
    let system_root_path = parent.join("system-root");
    for (root, name) in [
        (&user_root_path, "user-skill"),
        (&user_root_path, "another-user-skill"),
        (&system_root_path, "system-skill"),
        (&system_root_path, "another-system-skill"),
    ] {
        let skill_dir = root.join(name);
        std::fs::create_dir_all(&skill_dir)?;
        std::fs::write(
            skill_dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: {name}\n---\n"),
        )?;
    }
    let user_root = AbsolutePathBuf::try_from(std::fs::canonicalize(&user_root_path)?)?;
    let system_root = AbsolutePathBuf::try_from(std::fs::canonicalize(&system_root_path)?)?;
    let outcome = load_and_merge_host_skill_roots(
        vec![
            HostSkillRoot::host(user_root.clone(), SkillScope::User, Arc::clone(&LOCAL_FS)),
            HostSkillRoot::host(
                system_root.clone(),
                SkillScope::System,
                Arc::clone(&LOCAL_FS),
            ),
        ],
        &Semaphore::new(/*permits*/ 2),
        /*restriction_product*/ None,
        /*plugin_skill_snapshots*/ None,
    )
    .await;
    let catalog = HostSkillProvider::new()
        .list(SkillListQuery {
            turn_id: "turn-1".to_string(),
            executor_roots: Vec::new(),
            resolved_executor_roots: Vec::new(),
            host_snapshot: Some(Arc::new(HostSkillsSnapshot::new(Arc::new(outcome)))),
            include_host_skills: true,
            include_bundled_skills: false,
            include_orchestrator_skills: false,
            mcp_resources: None,
            executor_capability_discovery: None,
        })
        .await?;
    let mut entries = catalog.entries.iter().collect::<Vec<_>>();
    SkillCatalogRenderPolicy::CoreCompatible.order_entries(&mut entries);
    let actual = build_alias_plan(&entries)
        .expect("alias plan should build")
        .root_lines();
    std::fs::remove_dir_all(parent)?;

    assert_eq!(
        actual,
        vec![
            format!(
                "- `r0` = `{}`",
                user_root.to_string_lossy().replace('\\', "/")
            ),
            format!(
                "- `r1` = `{}`",
                system_root.to_string_lossy().replace('\\', "/")
            ),
        ]
    );
    Ok(())
}

#[test]
fn mixed_catalogs_keep_absolute_authority_aware_rendering_under_budget_pressure() {
    let root = "/Users/test/.codex/plugins/cache/openai-curated/example/hash1234567890/skills-with-a-very-long-shared-prefix";
    let mut entries = (0..12)
        .map(|index| {
            let name = format!("host-skill-{index}");
            entry(&name, "Description.", /*short_description*/ None)
                .with_display_path(format!("{root}/skill-{index}/SKILL.md"))
                .with_alias_root(root)
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

    assert!(build_alias_plan(&visible_entries).is_none());

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
fn mixed_catalog_reserves_executor_omission_marker_by_omitting_host_first() {
    let host_catalog = SkillCatalog {
        entries: vec![entry_with_path(
            "h", "", /*short_description*/ None, "/h",
        )],
        warnings: Vec::new(),
    };
    let executor_entry = |name: &str, resource: &str| {
        SkillCatalogEntry::new(
            SkillPackageId(name.to_string()),
            SkillAuthority::new(SkillSourceKind::Executor, "env-1"),
            name,
            "",
            SkillResourceId::new(resource),
        )
        .with_display_path(resource)
    };
    let executor_catalog = SkillCatalog {
        entries: vec![
            executor_entry("e1", "skill://executor/one"),
            executor_entry(
                "e2",
                "skill://executor/this-resource-is-intentionally-too-long",
            ),
        ],
        warnings: Vec::new(),
    };

    let RenderedSkillCatalogs { host, executor, .. } = render_combined_available_skills(
        &executor_catalog,
        &SkillCatalog::default(),
        &host_catalog,
        SkillMetadataBudget::Tokens(28),
        /*include_skills_usage_instructions*/ false,
    );
    let host = host.expect("host catalog should render");
    let executor = executor.expect("executor catalog should render");

    assert_eq!(
        host.report,
        SkillRenderReport {
            total_count: 1,
            included_count: 0,
            omitted_count: 1,
            truncated_description_chars: 0,
            truncated_description_count: 0,
        }
    );
    assert_eq!(
        executor.report,
        SkillRenderReport {
            total_count: 2,
            included_count: 1,
            omitted_count: 1,
            truncated_description_chars: 0,
            truncated_description_count: 0,
        }
    );
    assert_eq!(
        executor.skill_lines,
        vec![
            "- e1: (environment resource: skill://executor/one)".to_string(),
            "- 1 additional skill omitted from this bounded skills list.".to_string(),
        ]
    );
    assert!(
        host.into_fragment(/*include_skills_usage_instructions*/ false)
            .is_none()
    );
}

#[test]
fn mixed_catalogs_alias_all_skill_sources_under_budget_pressure() {
    let executor_root =
        "skill://executor-environment-with-a-long-shared-root/workspaces/project/.agents/skills";
    let orchestrator_root = "skill://plugin_connector_1p_2330815c823c8191941e5dc465bb899f";
    let host_root = "/Users/test/.codex/plugins/cache/openai-curated/example/hash1234567890/skills";
    let catalog =
        |source: SkillSourceKind, authority: &str, root: &str, prefix: &str| SkillCatalog {
            entries: (0..3)
                .map(|index| {
                    let name = format!("{prefix}-{index}");
                    let locator = match source {
                        SkillSourceKind::Orchestrator => format!("{root}/{name}"),
                        SkillSourceKind::Host
                        | SkillSourceKind::Executor
                        | SkillSourceKind::Custom(_) => format!("{root}/{name}/SKILL.md"),
                    };
                    SkillCatalogEntry::new(
                        SkillPackageId(locator.clone()),
                        SkillAuthority::new(source.clone(), authority),
                        name,
                        "Description.",
                        SkillResourceId::new(locator.clone()),
                    )
                    .with_display_path(locator)
                    .with_alias_root(root)
                })
                .collect(),
            warnings: Vec::new(),
        };
    let executor_catalog = catalog(
        SkillSourceKind::Executor,
        "executor-environment",
        executor_root,
        "executor",
    );
    let orchestrator_catalog = catalog(
        SkillSourceKind::Orchestrator,
        "codex_apps",
        orchestrator_root,
        "orchestrator",
    );
    let host_catalog = catalog(SkillSourceKind::Host, "host", host_root, "host");

    let rendered = render_combined_available_skills(
        &executor_catalog,
        &orchestrator_catalog,
        &host_catalog,
        SkillMetadataBudget::Characters(900),
        /*include_skills_usage_instructions*/ true,
    );
    let executor = rendered.executor.expect("executor catalog should render");
    let orchestrator = rendered
        .orchestrator
        .expect("orchestrator catalog should render");
    let host = rendered.host.expect("host catalog should render");

    assert_eq!(
        executor.skill_root_lines,
        vec![format!("- `e0` = `{executor_root}`")]
    );
    assert_eq!(
        orchestrator.skill_root_lines,
        vec![format!("- `o0` = `{orchestrator_root}`")]
    );
    assert_eq!(
        host.skill_root_lines,
        vec![format!("- `r0` = `{host_root}`")]
    );
    assert!(
        executor
            .skill_lines
            .iter()
            .any(|line| line.contains("(environment resource: e0/executor-0/SKILL.md)"))
    );
    assert!(
        orchestrator
            .skill_lines
            .iter()
            .any(|line| line.contains("(orchestrator package: o0/orchestrator-0)"))
    );
    assert!(
        host.skill_lines
            .iter()
            .any(|line| line.contains("(file: r0/host-0/SKILL.md)"))
    );
    assert_eq!(
        (
            executor.report.included_count,
            orchestrator.report.included_count,
            host.report.included_count,
        ),
        (3, 3, 3)
    );
}

#[test]
fn mixed_catalog_prefers_executor_inclusion_over_total_aliased_inclusion() {
    let root = format!("/{}", "r".repeat(219));
    let host_catalog = SkillCatalog {
        entries: ["h1", "h2"]
            .into_iter()
            .map(|name| {
                entry(name, "", /*short_description*/ None)
                    .with_display_path(format!("{root}/{name}/SKILL.md"))
                    .with_alias_root(root.as_str())
            })
            .collect(),
        warnings: Vec::new(),
    };
    let executor_entry = |name: &str, resource: &str| {
        SkillCatalogEntry::new(
            SkillPackageId(name.to_string()),
            SkillAuthority::new(SkillSourceKind::Executor, "env-1"),
            name,
            "",
            SkillResourceId::new(resource),
        )
        .with_display_path(resource)
    };
    let executor_catalog = SkillCatalog {
        entries: vec![
            executor_entry("e1", "skill://executor/one"),
            executor_entry("e2", &format!("skill://{}", "e".repeat(132))),
        ],
        warnings: Vec::new(),
    };
    let orchestrator_resource = "skill://orchestrator/one";
    let orchestrator_catalog = SkillCatalog {
        entries: vec![
            SkillCatalogEntry::new(
                SkillPackageId("o1".to_string()),
                SkillAuthority::new(SkillSourceKind::Orchestrator, "codex_apps"),
                "o1",
                "",
                SkillResourceId::new(orchestrator_resource),
            )
            .with_display_path(orchestrator_resource),
        ],
        warnings: Vec::new(),
    };

    let RenderedSkillCatalogs {
        host,
        executor,
        orchestrator,
    } = render_combined_available_skills(
        &executor_catalog,
        &orchestrator_catalog,
        &host_catalog,
        SkillMetadataBudget::Tokens(74),
        /*include_skills_usage_instructions*/ false,
    );
    let host = host.expect("host catalog should render");
    let executor = executor.expect("executor catalog should render");
    let orchestrator = orchestrator.expect("orchestrator catalog should render");

    assert_eq!(
        executor.report,
        SkillRenderReport {
            total_count: 2,
            included_count: 2,
            omitted_count: 0,
            truncated_description_chars: 0,
            truncated_description_count: 0,
        }
    );
    assert_eq!(
        host.report,
        SkillRenderReport {
            total_count: 2,
            included_count: 0,
            omitted_count: 2,
            truncated_description_chars: 0,
            truncated_description_count: 0,
        }
    );
    assert_eq!(orchestrator.report.total_count, 1);
    assert_eq!(host.skill_root_lines, Vec::<String>::new());
}

#[tokio::test]
async fn singleton_plugin_versions_share_the_marketplace_alias_root()
-> Result<(), Box<dyn std::error::Error>> {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let parent = std::env::temp_dir().join(format!(
        "codex-skills-extension-marketplace-alias-{}-{unique}",
        std::process::id()
    ));
    let marketplace_path = parent.join("plugins/cache/openai-curated");
    let mut roots = Vec::new();
    for (plugin, version) in [("github", "hash123"), ("slack", "hash456")] {
        let root = marketplace_path.join(plugin).join(version).join("skills");
        let skill_dir = root.join(plugin);
        std::fs::create_dir_all(&skill_dir)?;
        std::fs::write(
            skill_dir.join("SKILL.md"),
            format!("---\nname: {plugin}\ndescription: {plugin} skill.\n---\n"),
        )?;
        roots.push(HostSkillRoot::host(
            AbsolutePathBuf::try_from(std::fs::canonicalize(root)?)?,
            SkillScope::User,
            Arc::clone(&LOCAL_FS),
        ));
    }
    let marketplace_root = dunce::canonicalize(&marketplace_path)?
        .to_string_lossy()
        .replace('\\', "/");
    let outcome = load_and_merge_host_skill_roots(
        roots,
        &Semaphore::new(/*permits*/ 2),
        /*restriction_product*/ None,
        /*plugin_skill_snapshots*/ None,
    )
    .await;
    let catalog = HostSkillProvider::new()
        .list(SkillListQuery {
            turn_id: "turn-1".to_string(),
            executor_roots: Vec::new(),
            resolved_executor_roots: Vec::new(),
            host_snapshot: Some(Arc::new(HostSkillsSnapshot::new(Arc::new(outcome)))),
            include_host_skills: true,
            include_bundled_skills: false,
            include_orchestrator_skills: false,
            mcp_resources: None,
            executor_capability_discovery: None,
        })
        .await?;
    let entries = catalog.entries.iter().collect::<Vec<_>>();
    let plan = build_alias_plan(&entries).expect("alias plan should build");
    let github = entries
        .iter()
        .find(|entry| entry.name == "github")
        .ok_or("GitHub skill should be discovered")?;
    let slack = entries
        .iter()
        .find(|entry| entry.name == "slack")
        .ok_or("Slack skill should be discovered")?;

    assert_eq!(
        plan.root_lines(),
        vec![format!("- `r0` = `{marketplace_root}`")]
    );
    assert_eq!(
        render_skill_locator_with_aliases(github, &plan),
        "r0/github/hash123/skills/github/SKILL.md"
    );
    assert_eq!(
        render_skill_locator_with_aliases(slack, &plan),
        "r0/slack/hash456/skills/slack/SKILL.md"
    );

    std::fs::remove_dir_all(parent)?;
    Ok(())
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
        /*include_skills_usage_instructions*/ false,
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
        /*include_skills_usage_instructions*/ false,
    )
    .expect("core-compatible report should render");
    assert_eq!(core_render.report, expected_report);
    let core_fragment = core_render
        .into_fragment(/*include_skills_usage_instructions*/ false)
        .expect("core-compatible rendering should preserve an empty skills fragment");
    assert!(core_fragment.body().contains("## Skills"));
    assert!(!core_fragment.body().contains("- oversized:"));
    let render = render_available_skills(
        &catalog,
        SkillCatalogRenderPolicy::ExtensionCompatible,
        SkillMetadataBudget::Tokens(100),
        /*include_skills_usage_instructions*/ false,
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
        /*include_skills_usage_instructions*/ false,
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
        /*include_skills_usage_instructions*/ false,
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
