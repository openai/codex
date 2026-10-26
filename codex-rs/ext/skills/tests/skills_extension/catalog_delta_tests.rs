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
    Ok(())
}

async fn render_catalogs(
    skills: &[(&str, &str)],
    config: &TestConfig,
    previous: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<String, Box<dyn std::error::Error>> {
    let provider = |kind: SkillSourceKind| {
        let entries = skills
            .iter()
            .map(|(name, description)| {
                let name = format!("{kind}-{name}");
                let path = format!("/skills/{name}/SKILL.md");
                let mut entry =
                    test_entry(kind.clone(), "skills", &format!("{kind}/{name}"), &path)
                        .with_display_path(path);
                entry.description = description.to_string();
                entry
            })
            .collect();
        Arc::new(StaticSkillProvider {
            catalog: SkillCatalog {
                entries,
                ..Default::default()
            },
            read_requests: Arc::new(Mutex::new(Vec::new())),
            list_calls: None,
            fail_first_list: false,
        })
    };
    let providers = SkillProviders::new()
        .with_executor_provider(provider(SkillSourceKind::Executor))
        .with_cloud_provider(provider(SkillSourceKind::Cloud))
        .with_host_provider(provider(SkillSourceKind::Host));
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
    start_registered_turn(&registry, &session_store, &thread_store, "turn").await;
    let roots = [SelectedCapabilityRoot {
        id: "skills".to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: "env-1".to_string(),
            path: PathUri::parse("file:///skills")?,
        },
    }];
    let turn_store = ExtensionData::new("turn");
    turn_store.insert(HostSkillsSnapshot::new(Arc::new(
        SkillLoadOutcome::default(),
    )));
    let sections = prepared_world_state(
        &registry,
        &roots,
        WorldStateContributionInput {
            previous_world_state: Some(previous),
            step_store: &ExtensionData::new("step"),
            model_info: &ModelInfo {
                context_window: Some(5_000),
                ..catalog_model_info()
            },
            thread_id: codex_protocol::ThreadId::new(),
            turn_id: "turn",
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
