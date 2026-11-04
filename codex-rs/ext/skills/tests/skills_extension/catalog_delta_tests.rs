//! Multi-source budgeting and feature rollback through the skills extension.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn skill_sources_spend_the_same_remaining_budget() -> TestResult {
    let mut previous = serde_json::Map::new();
    let enabled = TestConfig {
        catalog_deltas_enabled: true,
        ..default_config()
    };
    render_catalogs(&[], &enabled, &mut previous).await?;

    // One release-notes row fits the 100-token budget. The larger entry sorts first.
    let large = "Detailed architectural guidance. ".repeat(40);
    let small = "Draft release notes. ".repeat(9);
    let entries = [
        ("architecture-review", large.as_str()),
        ("release-notes", small.as_str()),
    ];
    let emitted = render_catalogs(&entries, &enabled, &mut previous).await?;
    assert_eq!(
        emitted
            .lines()
            .filter(|line| line.starts_with("- ") && line.contains("release-notes"))
            .count(),
        1
    );
    assert!(emitted.contains("- executor-release-notes:"), "{emitted}");
    assert!(!emitted.contains("architecture-review"), "{emitted}");
    let used = previous
        .values()
        .filter_map(|s| s["usage"]["tokens"].as_u64())
        .sum::<u64>();
    assert!((50..=100).contains(&used), "{used}");

    let edited = [
        ("architecture-review", "Review architecture."),
        ("release-notes", small.as_str()),
    ];
    assert_eq!(render_catalogs(&edited, &enabled, &mut previous).await?, "");
    let disabled = default_config();
    let empty = render_catalogs(&[], &disabled, &mut previous).await?;
    assert!(empty.contains("skills are currently available"), "{empty}");
    assert_eq!(render_catalogs(&[], &disabled, &mut previous).await?, "");
    let omitted = TestConfig {
        omit_descriptions: true,
        ..enabled
    };
    previous.clear();
    let emitted =
        render_catalogs(&[("notes", "Hidden description.")], &enabled, &mut previous).await?;
    assert!(emitted.contains("description shown above"), "{emitted}");
    assert!(
        emitted.contains("- host-notes: Hidden description."),
        "{emitted}"
    );
    // Resume a snapshot written before the description policy was persisted.
    for snapshot in previous.values_mut() {
        snapshot.as_object_mut().unwrap().remove("omitDescriptions");
    }
    let emitted =
        render_catalogs(&[("notes", "Hidden description.")], &omitted, &mut previous).await?;
    assert!(
        emitted.contains("OR the task clearly matches a skill's name shown above, you must use that skill for that turn."),
        "{emitted}"
    );
    assert!(emitted.contains("- executor-notes: "), "{emitted}");
    assert!(!emitted.contains("Hidden description."), "{emitted}");
    assert_eq!(
        render_catalogs(&[("notes", "Edited description.")], &omitted, &mut previous).await?,
        ""
    );
    // A policy change still refreshes instructions when no more rows fit the budget.
    let emitted =
        render_catalogs(&[("notes", "Edited description.")], &enabled, &mut previous).await?;
    assert!(emitted.contains("description shown above"), "{emitted}");
    assert!(!emitted.contains("- executor-notes:"), "{emitted}");
    assert!(!emitted.contains("- cloud-notes:"), "{emitted}");
    assert!(!emitted.contains("- host-notes:"), "{emitted}");
    assert_eq!(
        render_catalogs(&[("notes", "Edited description.")], &enabled, &mut previous).await?,
        ""
    );
    Ok(())
}

#[tokio::test]
async fn grant_expiry_withdraws_only_missing_skills_when_metadata_budget_is_exhausted() -> TestResult
{
    let enabled = TestConfig {
        catalog_deltas_enabled: true,
        ..default_config()
    };
    let survivor = [("permanent", "Always available.")];
    let original = [("unrelated", "Removed independently.")];
    for grant_when_removed in [None, Some("granting-turn")] {
        let mut previous = serde_json::Map::new();
        let baseline = render_catalogs_for_step(
            &original,
            &enabled,
            &mut previous,
            /*granting_turn*/ None,
            /*context_window*/ 60_000,
        )
        .await?;
        assert!(baseline.contains("- executor-unrelated:"), "{baseline}");
        let both = [
            original[0],
            survivor[0],
            ("temporary", "Turn only."),
            ("env-b/independent", "Available without the grant on env-1."),
        ];
        let granted = render_catalogs_for_step(
            &both,
            &enabled,
            &mut previous,
            Some("granting-turn"),
            /*context_window*/ 60_000,
        )
        .await?;
        assert!(granted.contains("- executor-permanent:"), "{granted}");
        assert!(granted.contains("- executor-independent:"), "{granted}");
        assert!(granted.contains("- executor-temporary:"), "{granted}");
        let expired = render_catalogs_for_step(
            &survivor,
            &enabled,
            &mut previous,
            grant_when_removed,
            /*context_window*/ 1,
        )
        .await?;
        assert_eq!(
            expired,
            format!("\n## Selected-environment skills update\n{REVALIDATION}")
        );
        assert_eq!(
            render_catalogs_for_step(
                &survivor,
                &enabled,
                &mut previous,
                /*granting_turn*/ None,
                /*context_window*/ 1
            )
            .await?,
            ""
        );
        let restored = [survivor[0], ("temporary", "Turn only.")];
        for expected in [RESTORATION, ""] {
            assert_eq!(
                render_catalogs_for_step(
                    &restored,
                    &enabled,
                    &mut previous,
                    Some("reapproved"),
                    /*context_window*/ 1,
                )
                .await?,
                expected
            );
        }
        assert_eq!(
            render_catalogs_for_step(
                &survivor,
                &enabled,
                &mut previous,
                /*granting_turn*/ None,
                /*context_window*/ 1,
            )
            .await?,
            format!("\n## Selected-environment skills update\n{REVALIDATION}")
        );
        assert_eq!(
            render_catalogs_for_step(
                &[],
                &enabled,
                &mut previous,
                /*granting_turn*/ None,
                /*context_window*/ 1
            )
            .await?,
            ""
        );
    }
    Ok(())
}

const RESTORATION: &str = "\n## Selected-environment skills update\nSome previously unavailable skills are available again. Verify current access before relying on a skill from this source.\n";
const REVALIDATION: &str = "Some previously listed skills are no longer available. Verify current access before relying on a skill from this source.\n";

#[tokio::test]
async fn legacy_filesystem_catalog_revalidates_missing_rows_once() -> TestResult {
    let config = TestConfig {
        catalog_deltas_enabled: true,
        ..default_config()
    };
    let mut previous = serde_json::Map::new();
    let survivor = [("stay", "Keep this skill.")];
    let both = [survivor[0], ("gone", "Temporarily available.")];
    render_catalogs_for_step(
        &both,
        &config,
        &mut previous,
        /*granting_turn*/ None,
        /*context_window*/ 60_000,
    )
    .await?;
    for snapshot in previous.values_mut() {
        snapshot.as_object_mut().unwrap().remove("tracksGrants");
    }
    assert_eq!(
        render_catalogs(&survivor, &config, &mut previous).await?,
        format!(
            "\n## Selected-environment skills update\n{REVALIDATION}\n## Host skills update\n{REVALIDATION}"
        )
    );
    assert_eq!(
        render_catalogs(&survivor, &config, &mut previous).await?,
        ""
    );
    assert_eq!(render_catalogs(&[], &config, &mut previous).await?, "");
    Ok(())
}

async fn render_catalogs(
    skills: &[(&str, &str)],
    config: &TestConfig,
    previous: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<String, Box<dyn std::error::Error>> {
    render_catalogs_for_step(
        skills, config, previous, /*granting_turn*/ None, /*context_window*/ 5_000,
    )
    .await
}

async fn render_catalogs_for_step(
    skills: &[(&str, &str)],
    config: &TestConfig,
    previous: &mut serde_json::Map<String, serde_json::Value>,
    granting_turn: Option<&str>,
    context_window: i64,
) -> Result<String, Box<dyn std::error::Error>> {
    let turn_id = granting_turn.unwrap_or("turn");
    let step_store = ExtensionData::new("step");
    if granting_turn.is_some() {
        step_store.insert(codex_skills_extension::TurnSkillFilesystemGrants {
            environment_ids: ["env-1".to_string()].into(),
        });
    }
    let provider = |kind: SkillSourceKind| -> Result<_, codex_utils_path_uri::PathUriParseError> {
        let mut entries = Vec::new();
        for &(name, description) in skills {
            let (environment, name) = name.split_once('/').unwrap_or(("env-1", name));
            let name = format!("{kind}-{name}");
            let path = format!("/skills/{name}/SKILL.md");
            let mut entry = test_entry(kind.clone(), "skills", &format!("{kind}/{name}"), &path);
            if kind == SkillSourceKind::Executor {
                entry.main_prompt = SkillResourceId::environment(
                    &path,
                    environment,
                    PathUri::parse(&format!("file://{path}"))?,
                );
            }
            entry.description = description.to_string();
            entries.push(entry.with_display_path(path));
        }
        Ok(Arc::new(StaticSkillProvider {
            catalog: SkillCatalog {
                entries,
                ..Default::default()
            },
            read_requests: Arc::new(Mutex::new(Vec::new())),
            list_calls: None,
            fail_first_list: false,
        }))
    };
    let providers = SkillProviders::new()
        .with_executor_provider(provider(SkillSourceKind::Executor)?)
        .with_cloud_provider(provider(SkillSourceKind::Cloud)?)
        .with_host_provider(provider(SkillSourceKind::Host)?);
    let mut builder = ExtensionRegistryBuilder::new();
    install_with_providers(&mut builder, providers, skills_extension_config);
    let registry = builder.build();
    let session_store = ExtensionData::new("session");
    let thread_store = ExtensionData::new("thread");
    registry.thread_lifecycle_contributors()[0]
        .on_thread_start(ThreadStartInput {
            config,
            session_source: &SessionSource::Cli,
            persistent_thread_state_available: true,
            environments: &[],
            mcp_resource_client: None,
            extension_metrics: None,
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await;
    start_registered_turn(&registry, &session_store, &thread_store, turn_id).await;
    let roots = [SelectedCapabilityRoot {
        id: "skills".to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: "env-1".to_string(),
            path: PathUri::parse("file:///skills")?,
        },
    }];
    let turn_store = ExtensionData::new(turn_id);
    turn_store.insert(HostSkillsSnapshot::new(Arc::new(
        SkillLoadOutcome::default(),
    )));
    let sections = prepared_world_state(
        &registry,
        &roots,
        WorldStateContributionInput {
            previous_world_state: Some(previous),
            step_store: &step_store,
            model_info: &ModelInfo {
                context_window: Some(context_window),
                include_skills_usage_instructions: true,
                ..catalog_model_info()
            },
            thread_id: codex_protocol::ThreadId::new(),
            turn_id,
            environments: &[],
            extension_metrics: None,
            session_store: &session_store,
            thread_store: &thread_store,
            turn_store: &turn_store,
        },
    )
    .await;
    let mut emitted = String::new();
    for section in sections {
        let prior = previous.get(section.id()).map_or(
            PreviousWorldStateSection::Absent,
            PreviousWorldStateSection::Known,
        );
        let (snapshot, fragment) = section.render_diff(prior);
        if let Some(snapshot) = snapshot {
            previous.insert(section.id().to_string(), snapshot);
        }
        if let Some(fragment) = fragment {
            emitted.push_str(&fragment.body().clone().expect_text());
        }
    }
    Ok(emitted)
}
