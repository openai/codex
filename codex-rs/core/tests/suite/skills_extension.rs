use std::sync::Arc;

use anyhow::Result;
use codex_core::config::Config;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_skills_extension::SkillProvider;
use codex_skills_extension::SkillProviderSource;
use codex_skills_extension::SkillProviders;
use codex_skills_extension::SkillsExtensionConfig;
use codex_skills_extension::catalog::SkillAuthority;
use codex_skills_extension::catalog::SkillCatalog;
use codex_skills_extension::catalog::SkillCatalogEntry;
use codex_skills_extension::catalog::SkillPackageId;
use codex_skills_extension::catalog::SkillProviderError;
use codex_skills_extension::catalog::SkillReadResult;
use codex_skills_extension::catalog::SkillResourceId;
use codex_skills_extension::catalog::SkillSearchResult;
use codex_skills_extension::catalog::SkillSourceKind;
use codex_skills_extension::install_with_providers;
use codex_skills_extension::provider::SkillListQuery;
use codex_skills_extension::provider::SkillProviderFuture;
use codex_skills_extension::provider::SkillReadRequest;
use codex_skills_extension::provider::SkillSearchRequest;
use codex_utils_string::approx_token_count;
use core_test_support::responses;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;

struct StaticSkillProvider {
    catalog: SkillCatalog,
}

impl SkillProvider for StaticSkillProvider {
    fn list(&self, query: SkillListQuery) -> SkillProviderFuture<'_, SkillCatalog> {
        // Keep thread context empty so the catalog is exercised through the
        // production turn-input path, where the host snapshot is available.
        let catalog = if query.host_snapshot.is_some() {
            self.catalog.clone()
        } else {
            SkillCatalog::default()
        };
        Box::pin(async move { Ok(catalog) })
    }

    fn read(&self, _request: SkillReadRequest) -> SkillProviderFuture<'_, SkillReadResult> {
        Box::pin(async {
            Err(SkillProviderError::new(
                "production-flow catalog test does not read skills",
            ))
        })
    }

    fn search(&self, _request: SkillSearchRequest) -> SkillProviderFuture<'_, SkillSearchResult> {
        Box::pin(async { Ok(SkillSearchResult::default()) })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn production_turn_scales_extension_catalog_from_resolved_model_window() -> Result<()> {
    let mut included_counts = Vec::new();
    for (context_window, max_context_window, expected_budget) in
        [(Some(10_000), None, 200), (None, Some(400_000), 4_000)]
    {
        let server = responses::start_mock_server().await;
        let response = mount_sse_once(
            &server,
            sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
        )
        .await;
        let source_kind = SkillSourceKind::Custom("test".to_string());
        let catalog = SkillCatalog {
            entries: (0..400)
                .map(|index| {
                    let name = format!("skill-{index:03}");
                    SkillCatalogEntry::new(
                        SkillPackageId(format!("test/{name}")),
                        SkillAuthority::new(source_kind.clone(), "test"),
                        name.clone(),
                        "A description long enough to keep the catalog under sustained budget pressure.",
                        SkillResourceId::new(format!("{name}/SKILL.md")),
                    )
                    .with_display_path(format!("skill://test/{name}/SKILL.md"))
                })
                .collect(),
            warnings: Vec::new(),
        };
        let mut extensions = ExtensionRegistryBuilder::<Config>::new();
        install_with_providers(
            &mut extensions,
            SkillProviders::new().with_provider(SkillProviderSource::new(
                source_kind,
                "test",
                Arc::new(StaticSkillProvider { catalog }),
            )),
            |config: &Config| SkillsExtensionConfig {
                include_instructions: config.include_skill_instructions,
                bundled_skills_enabled: false,
                orchestrator_skills_enabled: false,
                shadow_selection_enabled: false,
            },
        );
        let mut builder = test_codex()
            .with_extensions(Arc::new(extensions.build()))
            .with_model_info_override("gpt-5.5", move |model_info| {
                model_info.context_window = context_window;
                model_info.max_context_window = max_context_window;
            })
            .with_config(|config| {
                config.include_skill_instructions = true;
            });
        let test = builder.build_with_auto_env(&server).await?;

        test.submit_turn("Inspect the available skills.").await?;
        let request = response.single_request();
        let developer_texts = request.message_input_texts("developer");
        let catalog_text = developer_texts
            .iter()
            .find(|text| text.contains("skill://test/"))
            .unwrap_or_else(|| {
                panic!(
                    "production request should include the extension skill catalog, got {developer_texts:?}"
                )
            });
        let metadata_lines = catalog_text
            .lines()
            .skip_while(|line| *line != "### Available skills")
            .skip(1)
            .take_while(|line| !line.starts_with("### "))
            .filter(|line| line.starts_with("- "))
            .collect::<Vec<_>>();
        let metadata_cost = metadata_lines.iter().fold(0usize, |cost, line| {
            cost.saturating_add(approx_token_count(&format!("{line}\n")))
        });
        let included_count = metadata_lines
            .iter()
            .filter(|line| line.starts_with("- skill-"))
            .count();

        assert!(catalog_text.contains("additional skills omitted"));
        assert!(!catalog_text.contains(
            "A description long enough to keep the catalog under sustained budget pressure."
        ));
        assert!(metadata_cost <= expected_budget);
        included_counts.push(included_count);
    }

    assert!(included_counts[0] > 0);
    assert!(included_counts[0] < included_counts[1]);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn production_turn_fairly_shortens_extension_catalog_descriptions() -> Result<()> {
    let server = responses::start_mock_server().await;
    let response = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let source_kind = SkillSourceKind::Custom("test".to_string());
    let description = "x".repeat(1_025);
    let catalog = SkillCatalog {
        entries: (0..10)
            .map(|index| {
                let name = format!("skill-{index:02}");
                SkillCatalogEntry::new(
                    SkillPackageId(format!("test/{name}")),
                    SkillAuthority::new(source_kind.clone(), "test"),
                    name.clone(),
                    description.clone(),
                    SkillResourceId::new(format!("{name}/SKILL.md")),
                )
                .with_display_path(format!("skill://test/{name}/SKILL.md"))
            })
            .collect(),
        warnings: Vec::new(),
    };
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    install_with_providers(
        &mut extensions,
        SkillProviders::new().with_provider(SkillProviderSource::new(
            source_kind,
            "test",
            Arc::new(StaticSkillProvider { catalog }),
        )),
        |config: &Config| SkillsExtensionConfig {
            include_instructions: config.include_skill_instructions,
            bundled_skills_enabled: false,
            orchestrator_skills_enabled: false,
            shadow_selection_enabled: false,
        },
    );
    let mut builder = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_model_info_override("gpt-5.5", |model_info| {
            model_info.context_window = Some(100_000);
            model_info.max_context_window = None;
        })
        .with_config(|config| {
            config.include_skill_instructions = true;
        });
    let test = builder.build_with_auto_env(&server).await?;

    test.submit_turn("Inspect the available skills.").await?;
    let developer_texts = response.single_request().message_input_texts("developer");
    let catalog_text = developer_texts
        .iter()
        .find(|text| text.contains("skill://test/"))
        .unwrap_or_else(|| {
            panic!(
                "production request should include the extension skill catalog, got {developer_texts:?}"
            )
        });
    let description_lengths = catalog_text
        .lines()
        .filter_map(|line| {
            line.strip_prefix("- skill-")
                .and_then(|line| line.split_once(": "))
                .and_then(|(_, line)| line.split_once(" (custom resource:"))
                .map(|(description, _)| description.chars().count())
        })
        .collect::<Vec<_>>();
    assert_eq!(10, description_lengths.len());
    assert!(
        description_lengths
            .iter()
            .all(|length| *length > 0 && *length < 1_024)
    );
    assert!(!catalog_text.contains("additional skills omitted"));

    Ok(())
}
