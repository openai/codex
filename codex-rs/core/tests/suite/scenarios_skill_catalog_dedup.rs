//! Snapshots cloud-preferred skill deduplication and stable executor aliases in model requests.

use std::sync::Arc;

use anyhow::Result;
use codex_core::config::Config;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_features::Feature;
use codex_skills_extension::SkillProviders;
use codex_skills_extension::SkillsExtensionConfig;
use codex_skills_extension::catalog::SkillAuthority;
use codex_skills_extension::catalog::SkillCatalog;
use codex_skills_extension::catalog::SkillCatalogEntry;
use codex_skills_extension::catalog::SkillPackageId;
use codex_skills_extension::catalog::SkillResourceId;
use codex_skills_extension::catalog::SkillSourceKind;
use codex_skills_extension::install_with_providers;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;

use super::super::skills_extension::CatalogSkillProvider;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cloud_preference_preserves_executor_aliases_and_description_budget() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let roots = [
        format!("skill://executor-a/{}", "a".repeat(180)),
        format!("skill://executor-b/{}", "b".repeat(180)),
    ];
    let description = format!("{} UNIQUE_DESCRIPTION_END", "Skill guidance. ".repeat(60));
    let mut executor_entries = Vec::new();
    for (plugin, root) in ["demo", "other"].into_iter().zip(&roots) {
        for index in 0..3 {
            let package = format!("{root}/s{index}");
            executor_entries.push(
                SkillCatalogEntry::new(
                    SkillPackageId(package.clone()),
                    SkillAuthority::new(SkillSourceKind::Executor, "executor"),
                    format!("{plugin}:s{index}"),
                    &description,
                    SkillResourceId::new(format!("{package}/SKILL.md")),
                )
                .with_alias_root(root),
            );
        }
    }
    let cloud_entries = (0..3)
        .map(|index| {
            SkillCatalogEntry::new(
                SkillPackageId(format!("skill://cloud/s{index}")),
                SkillAuthority::new(SkillSourceKind::Cloud, "cloud"),
                format!("demo:s{index}"),
                "Cloud skill instructions.",
                SkillResourceId::new(format!("skill://cloud/s{index}/SKILL.md")),
            )
        })
        .collect();
    let provider = |entries| {
        Arc::new(CatalogSkillProvider {
            catalog: SkillCatalog {
                entries,
                warnings: Vec::new(),
            },
        })
    };
    let mut extensions = ExtensionRegistryBuilder::new();
    install_with_providers(
        &mut extensions,
        SkillProviders::new()
            .with_executor_provider(provider(executor_entries))
            .with_cloud_provider(provider(cloud_entries)),
        |config: &Config| SkillsExtensionConfig {
            include_instructions: config.include_skill_instructions,
            max_context_tokens: config.skill_max_context_tokens,
            bundled_skills_enabled: false,
            cloud_skill_enabled: true,
            shadow_selection_enabled: false,
        },
    );
    let mock = responses::mount_sse_sequence(
        &server,
        (0..2)
            .map(|index| {
                responses::sse(vec![
                    responses::ev_assistant_message(
                        &format!("message-{index}"),
                        "Skills are available.",
                    ),
                    responses::ev_completed(&format!("response-{index}")),
                ])
            })
            .collect(),
    )
    .await;
    let test = test_codex()
        .with_exec_server_url("none")
        .with_extensions(Arc::new(extensions.build()))
        .with_model_info_override("gpt-6-astra", |model| {
            model.context_window = Some(70_000);
            model.max_context_window = None;
            model.include_skills_usage_instructions = false;
        })
        .with_config(|config| {
            super::configure_scenario_catalog(config);
            config.cloud_skill_enabled = true;
            config
                .features
                .enable(Feature::ExecutorCapabilityDiscovery)
                .expect("enable executor capability discovery");
        })
        .build_with_auto_env(&server)
        .await?;
    test.submit_turn("Which skills are available?").await?;
    test.submit_turn("Are those same skills still available?")
        .await?;

    let requests = mock.requests();
    assert_eq!(requests.len(), 2);
    for (request_index, request) in requests.iter().enumerate() {
        let body = request.message_input_texts("developer").join("\n");
        for index in 0..3 {
            assert_eq!(
                body.matches(&format!("- demo:s{index}:")).count(),
                1,
                "request {request_index}"
            );
            assert!(body.contains(&format!("(cloud package: skill://cloud/s{index})")));
            assert!(body.contains(&format!("(executor package: e1/s{index})")));
            assert!(!body.contains(&format!("(executor package: e0/s{index})")));
        }
        for (index, root) in roots.iter().enumerate() {
            assert!(body.contains(&format!("- `e{index}` = `{root}`")));
        }
        assert_eq!(body.matches("UNIQUE_DESCRIPTION_END").count(), 3);
    }
    insta::assert_snapshot!(
        "dedup_before_budgeting",
        context_snapshot::format_request_history_snapshot(
            "Cloud skills replace duplicate executor listings before budgeting; unique executor skills retain their descriptions and e1 aliases across turns.",
            &requests,
            &ContextSnapshotOptions::default().include_request_settings(),
        )
    );
    Ok(())
}
