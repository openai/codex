//! Snapshots skill deduplication and incremental catalogs across turns and context resets.

use std::collections::HashSet;
use std::num::NonZeroUsize;
use std::sync::Arc;

use anyhow::Result;
use codex_core::config::Config;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_skills_extension::HostSkillProvider;
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
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;

use super::super::skills_extension::CatalogSkillProvider;

#[test_case::test_case(false; "descriptions")]
#[test_case::test_case(true; "omit_descriptions")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cloud_preference_preserves_executor_aliases_and_description_budget(
    omit_descriptions: bool,
) -> Result<()> {
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
                is_partial: false,
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
            omit_descriptions: config
                .features
                .enabled(codex_features::Feature::OmitSkillDescriptions),
            max_context_tokens: config.skill_max_context_tokens,
            bundled_skills_enabled: false,
            cloud_skill_enabled: true,
            shadow_selection_enabled: false,
            catalog_deltas_enabled: config.features.enabled(Feature::SkillCatalogDeltas),
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
        .with_config(move |config| {
            config
                .features
                .set_enabled(Feature::OmitSkillDescriptions, omit_descriptions)
                .expect("set omit_skill_descriptions feature");
            super::configure_scenario_catalog(config);
            config.cloud_skill_enabled = true;
            config
                .features
                .enable(Feature::ExecutorCapabilityDiscovery)
                .expect("enable executor capability discovery");
        })
        .with_model("gpt-6-astra")
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
            assert_eq!(
                body.contains(&format!("- `e{index}` = `{root}`")),
                index == 1
            );
        }
        assert_eq!(
            body.matches("UNIQUE_DESCRIPTION_END").count(),
            if omit_descriptions { 0 } else { 3 }
        );
    }
    insta::assert_snapshot!(
        if omit_descriptions {
            "dedup_without_descriptions"
        } else {
            "dedup_before_budgeting"
        },
        context_snapshot::format_request_history_snapshot(
            if omit_descriptions {
                "Cloud and executor skill catalogs retain names and aliases across turns with descriptions omitted."
            } else {
                "Cloud skills replace duplicate executor listings before budgeting; unique executor skills retain their descriptions and e1 aliases across turns."
            },
            &requests,
            &ContextSnapshotOptions::default().include_request_settings(),
        )
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn skill_catalog_deltas_survive_resume_and_rebuild_after_compaction() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    // Both skill roots include the home path, so its length affects the metadata budget.
    let temp_root = dunce::canonicalize(std::env::temp_dir())?;
    let home = Arc::new(
        tempfile::Builder::new()
            .rand_bytes(186_usize.saturating_sub(temp_root.to_string_lossy().len()))
            .tempdir_in(temp_root)?,
    );
    for (name, description) in [
        ("code-review", "Review code changes for correctness."),
        ("release-notes", "Draft release notes from recent commits."),
        ("browser-testing", "Run browser tests for web applications."),
    ] {
        let directory = home.path().join("skills").join(name);
        std::fs::create_dir_all(&directory)?;
        std::fs::write(
            directory.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: {description}\n---\n\n# Instructions\n"),
        )?;
    }
    let mut extensions = ExtensionRegistryBuilder::new();
    install_with_providers(
        &mut extensions,
        SkillProviders::new().with_host_provider(Arc::new(HostSkillProvider::new())),
        |config: &Config| SkillsExtensionConfig {
            include_instructions: config.include_skill_instructions,
            omit_descriptions: config.features.enabled(Feature::OmitSkillDescriptions),
            max_context_tokens: config.skill_max_context_tokens,
            bundled_skills_enabled: false,
            cloud_skill_enabled: false,
            shadow_selection_enabled: false,
            catalog_deltas_enabled: config.features.enabled(Feature::SkillCatalogDeltas),
        },
    );
    let extensions = Arc::new(extensions.build());
    let builder = || {
        test_codex()
            .with_home(Arc::clone(&home))
            .with_extensions(Arc::clone(&extensions))
            .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
            .with_model_info_override("gpt-6-astra", |model| {
                model.use_responses_lite = true;
                model.include_skills_usage_instructions = false;
            })
            .with_config(|config| {
                super::configure_scenario_catalog(config);
                config.workspace_roots = vec![config.cwd.clone()];
                config
                    .features
                    .enable(Feature::SkillCatalogDeltas)
                    .expect("enable skill catalog deltas");
            })
    };
    let mock = responses::mount_sse_sequence(
        &server,
        (0..19)
            .map(|index| {
                let mut events = Vec::new();
                if index == 6 || index == 10 || index == 17 {
                    events.push(json!({
                        "type": "response.output_item.done",
                        "item": {
                            "type": "compaction",
                            "encrypted_content": "SKILL_CATALOG_CHECKPOINT",
                        },
                    }));
                }
                events.push(responses::ev_completed(&format!("response-{index}")));
                responses::sse(events)
            })
            .collect(),
    )
    .await;
    let test = builder().build_with_auto_env(&server).await?;
    test.submit_turn("Inspect the available skills.").await?;

    std::fs::write(
        home.path().join("skills/release-notes/SKILL.md"),
        "---\nname: release-notes\ndescription: Draft release notes grouped by feature and bug fix.\n---\n\n# Instructions\n",
    )?;
    test.thread_manager.skills_service().clear_cache();
    test.submit_turn("Check the release-notes skill.").await?;

    let incident_summary = home.path().join("skills/incident-summary");
    std::fs::create_dir_all(&incident_summary)?;
    std::fs::write(
        incident_summary.join("SKILL.md"),
        "---\nname: incident-summary\ndescription: Summarize incidents and follow-up actions.\n---\n\n# Instructions\n",
    )?;
    test.thread_manager.skills_service().clear_cache();
    test.submit_turn("Check the incident-summary skill.")
        .await?;

    std::fs::remove_dir_all(home.path().join("skills/browser-testing"))?;
    test.thread_manager.skills_service().clear_cache();
    test.submit_turn("List the available skills after removing browser-testing.")
        .await?;
    test.submit_turn("Continue with the same skills.").await?;

    let resumed = builder().restart_with_auto_env(&server, &test).await?;
    resumed.submit_turn("Resume with the same skills.").await?;
    resumed.codex.submit(Op::Compact).await?;
    wait_for_event(&resumed.codex, |event| {
        assert!(!matches!(event, EventMsg::Error(_)), "{event:?}");
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    resumed.submit_turn("Continue after compaction.").await?;

    let limited = builder()
        .with_config(|config| {
            super::configure_scenario_catalog(config);
            config.workspace_roots = vec![config.cwd.clone()];
            config
                .features
                .enable(Feature::SkillCatalogDeltas)
                .expect("enable skill catalog deltas");
            config.skill_max_context_tokens = NonZeroUsize::new(172);
        })
        .restart_with_auto_env(&server, &resumed)
        .await?;
    limited
        .submit_turn("Continue with a smaller skills list budget.")
        .await?;
    let translation = home.path().join("skills/translation");
    std::fs::create_dir_all(&translation)?;
    std::fs::write(
        translation.join("SKILL.md"),
        "---\nname: translation\ndescription: Translate text while preserving its tone.\n---\n\n# Instructions\n",
    )?;
    limited.thread_manager.skills_service().clear_cache();
    limited.submit_turn("Check the translation skill.").await?;
    limited.codex.submit(Op::Compact).await?;
    wait_for_event(&limited.codex, |event| {
        assert!(!matches!(event, EventMsg::Error(_)), "{event:?}");
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    limited
        .submit_turn("Continue after compacting the smaller skills list.")
        .await?;

    // Compaction omitted translation from the new catalog. Editing and then
    // removing it must not advertise a skill that is no longer shown.
    std::fs::write(
        translation.join("SKILL.md"),
        "---\nname: translation\ndescription: Translate and localize text.\n---\n\n# Instructions\n",
    )?;
    limited.thread_manager.skills_service().clear_cache();
    limited
        .submit_turn("Continue after editing translation.")
        .await?;
    std::fs::remove_dir_all(&translation)?;
    limited.thread_manager.skills_service().clear_cache();
    limited
        .submit_turn("Continue after removing translation.")
        .await?;

    // Editing a shown skill also needs remaining budget. Removing skills does
    // not refund the space their earlier descriptions still occupy in history.
    std::fs::write(
        home.path().join("skills/code-review/SKILL.md"),
        format!(
            "---\nname: code-review\ndescription: {}\n---\n\n# Instructions\n",
            "Review the entire project. ".repeat(40)
        ),
    )?;
    limited.thread_manager.skills_service().clear_cache();
    limited
        .submit_turn("Check the updated code-review skill.")
        .await?;
    for name in ["code-review", "incident-summary", "release-notes"] {
        std::fs::remove_dir_all(home.path().join("skills").join(name))?;
    }
    limited.thread_manager.skills_service().clear_cache();
    limited
        .submit_turn("Continue after removing more skills.")
        .await?;
    let documentation = home.path().join("skills/documentation");
    std::fs::create_dir_all(&documentation)?;
    std::fs::write(
        documentation.join("SKILL.md"),
        "---\nname: documentation\ndescription: Write project documentation.\n---\n\n# Instructions\n",
    )?;
    limited.thread_manager.skills_service().clear_cache();
    limited
        .submit_turn("Check the documentation skill.")
        .await?;
    limited.codex.submit(Op::Compact).await?;
    wait_for_event(&limited.codex, |event| {
        assert!(!matches!(event, EventMsg::Error(_)), "{event:?}");
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    limited
        .submit_turn("Continue with the rebuilt skills catalog.")
        .await?;

    let requests = mock.requests();
    assert_eq!(requests.len(), 19);
    // Count newly injected catalog parts by message ID and content index, so retained
    // history is not counted again on later requests or after resume.
    let mut seen_catalog_parts = HashSet::new();
    let injections = requests
        .iter()
        .map(|request| {
            let mut injected = 0;
            for item in request.input() {
                let Some(content) = item["content"].as_array() else {
                    continue;
                };
                for (index, part) in content.iter().enumerate() {
                    if !part["text"]
                        .as_str()
                        .is_some_and(|text| text.starts_with("<skills_instructions>"))
                    {
                        continue;
                    }
                    let metadata = &item["internal_chat_message_metadata_passthrough"];
                    assert_eq!(
                        (
                            &metadata["content_item_kinds"][index],
                            &metadata["content_item_metadata"][index],
                        ),
                        (
                            &json!("host_skills.instructions"),
                            &json!({"provenance": {"type": "skills"}}),
                        ),
                    );
                    let id = item["id"].as_str().expect("catalog message ID");
                    if seen_catalog_parts.insert((id.to_owned(), index)) {
                        injected += 1;
                    }
                }
            }
            injected
        })
        .collect::<Vec<_>>();
    assert_eq!(
        injections,
        vec![1, 1, 1, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 1]
    );
    let exhausted = requests[16].message_input_texts("developer").join("\n");
    assert!(!exhausted.contains("- translation:"), "{exhausted}");
    assert!(!exhausted.contains("- documentation:"), "{exhausted}");
    let rebuilt = requests[18].message_input_texts("developer").join("\n");
    assert!(rebuilt.contains("- documentation:"), "{rebuilt}");
    assert!(!rebuilt.contains("- release-notes:"), "{rebuilt}");
    assert!(!rebuilt.contains("- code-review:"), "{rebuilt}");
    let skill_versions = requests
        .iter()
        .take(8)
        .map(|request| {
            let developer = request.message_input_texts("developer").join("\n");
            [
                "- code-review: Review code changes for correctness.",
                "- release-notes: Draft release notes from recent commits.",
                "- release-notes: Draft release notes grouped by feature and bug fix.",
                "- incident-summary: Summarize incidents and follow-up actions.",
                "- browser-testing: Run browser tests for web applications.",
            ]
            .map(|entry| developer.matches(entry).count())
        })
        .collect::<Vec<_>>();
    assert_eq!(
        skill_versions,
        vec![
            [1, 1, 0, 0, 1],
            [1, 1, 1, 0, 1],
            [1, 1, 1, 1, 1],
            [1, 1, 1, 1, 1],
            [1, 1, 1, 1, 1],
            [1, 1, 1, 1, 1],
            [1, 1, 1, 1, 1],
            [1, 0, 1, 1, 0],
        ]
    );
    insta::assert_snapshot!(
        "skill_catalog_deltas",
        context_snapshot::format_request_history_snapshot(
            "Skill additions and edits share the remaining metadata budget across turns and resume. Removals stay silent and do not refund budget. Compaction rebuilds the current catalog and resets the allowance.",
            &requests,
            &ContextSnapshotOptions::default().include_request_settings(),
        )
    );
    Ok(())
}
