use std::sync::Arc;

use anyhow::Result;
use codex_config::ConfigLayerEntry;
use codex_config::ConfigLayerSource;
use codex_config::ConfigLayerStack;
use codex_config::ConfigRequirements;
use codex_config::ConfigRequirementsToml;
use codex_core::config::Config;
use codex_extension_api::ExtensionEventSink;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ExtensionWarning;
use codex_features::Feature;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use codex_skills_extension::HostSkillProvider;
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
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_string::approx_token_count;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::apps_enabled_builder;
use core_test_support::responses;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_mcp_server;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::Duration;
use tokio::time::Instant;
use toml::toml;

struct StaticSkillProvider {
    catalog: SkillCatalog,
    main_prompt_contents: Option<String>,
}

struct ExecutorSkillProvider {
    catalog: SkillCatalog,
}

#[derive(Debug)]
enum CapturedExtensionEvent {
    Event(Box<Event>),
    Warning(ExtensionWarning),
}

impl CapturedExtensionEvent {
    fn into_warning(self) -> ExtensionWarning {
        match self {
            Self::Warning(warning) => warning,
            Self::Event(event) => panic!("expected extension warning, got {event:?}"),
        }
    }
}

struct ChannelEventSink(std::sync::mpsc::Sender<CapturedExtensionEvent>);

impl ExtensionEventSink for ChannelEventSink {
    fn emit(&self, event: Event) {
        let _ = self.0.send(CapturedExtensionEvent::Event(Box::new(event)));
    }

    fn emit_warning(&self, warning: ExtensionWarning) {
        let _ = self.0.send(CapturedExtensionEvent::Warning(warning));
    }
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

    fn read(&self, request: SkillReadRequest) -> SkillProviderFuture<'_, SkillReadResult> {
        let result = self
            .main_prompt_contents
            .clone()
            .map(|contents| SkillReadResult {
                resource: request.resource,
                contents,
            })
            .ok_or_else(|| {
                SkillProviderError::new("production-flow catalog test does not read skills")
            });
        Box::pin(async move { result })
    }

    fn search(&self, _request: SkillSearchRequest) -> SkillProviderFuture<'_, SkillSearchResult> {
        Box::pin(async { Ok(SkillSearchResult::default()) })
    }
}

impl SkillProvider for ExecutorSkillProvider {
    fn list(&self, _query: SkillListQuery) -> SkillProviderFuture<'_, SkillCatalog> {
        Box::pin(async { Ok(self.catalog.clone()) })
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

const FULL_CATALOG_CONTEXT_WINDOW: i64 = 40_000;
const SHORTENING_CONTEXT_WINDOW: i64 = 12_000;
const EXECUTOR_OMITTING_CONTEXT_WINDOW: i64 = 2_000;
const HOST_OMITTING_CONTEXT_WINDOW: i64 = 2_000;
const MIXED_EXECUTOR_OMITTING_CONTEXT_WINDOW: i64 = 2_000;
const MIXED_HOST_OMITTING_CONTEXT_WINDOW: i64 = 6_000;
const HOST_CATALOG: [(&str, &str); 4] = [
    (
        "host-alpha",
        "Host alpha reads local build files, checks repository conventions, and explains the safest small change before editing. It keeps host-only paths visible so the model can choose the right local instructions.",
    ),
    (
        "host-beta",
        "Host beta reviews local test output, follows project-specific validation rules, and reports the smallest useful verification step. It is deliberately detailed enough to exercise description shortening.",
    ),
    (
        "host-delta",
        "Host delta checks local dependency and formatting conventions before a change is finalized. It exists to make the host catalog explicit and to prove later host entries remain visible under pressure.",
    ),
    (
        "host-gamma",
        "Host gamma inspects local configuration layers, keeps repository defaults intact, and points the model at the narrowest relevant file. Its description is long enough to share pressure with every other skill.",
    ),
];
const EXECUTOR_CATALOG: [(&str, &str); 6] = [
    (
        "exec-alpha",
        "Executor alpha inspects environment-owned resources, resolves their exact package identifiers, and reads only the relevant instructions. It demonstrates the executor catalog rendering path under pressure.",
    ),
    (
        "exec-beta",
        "Executor beta searches selected environment capabilities, keeps resource access bounded, and explains which remote instructions are available. It is intentionally long enough to be shortened fairly.",
    ),
    (
        "exec-gamma",
        "Executor gamma reads environment-owned build metadata, preserves authority-aware locators, and avoids inventing filesystem paths. It makes the executor catalog large enough for shared allocation to matter.",
    ),
    (
        "exec-delta",
        "Executor delta follows the selected environment workflow, loads only relevant resources, and reports remote constraints clearly. Its text should remain a visible prefix when descriptions are shortened.",
    ),
    (
        "exec-epsilon",
        "Executor epsilon handles environment-specific validation steps, keeps reads bounded, and leaves unrelated capabilities alone. It proves later executor skills participate in round-robin shortening.",
    ),
    (
        "exec-zeta",
        "Executor zeta resolves the final environment resource carefully, keeps package identifiers exact, and documents what remains available. It is the last explicit executor entry in this fixture.",
    ),
];

fn executor_catalog(skills: &[(&str, &str)]) -> SkillCatalog {
    SkillCatalog {
        entries: skills
            .iter()
            .map(|(name, description)| {
                SkillCatalogEntry::new(
                    SkillPackageId(format!("test/{name}")),
                    SkillAuthority::new(SkillSourceKind::Executor, "test"),
                    *name,
                    *description,
                    SkillResourceId::new(format!("{name}/SKILL.md")),
                )
                .with_display_path(format!("skill://test/{name}/SKILL.md"))
            })
            .collect(),
        warnings: Vec::new(),
    }
}

fn write_host_skills(codex_home: &std::path::Path, skills: &[(&str, &str)]) -> Result<()> {
    for (name, description) in skills {
        let skill_dir = codex_home.join("skills").join(name);
        std::fs::create_dir_all(&skill_dir)?;
        std::fs::write(
            skill_dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: {description}\n---\n\n# body\n"),
        )?;
    }
    Ok(())
}

fn catalog_extensions(
    executor_catalog: SkillCatalog,
    include_host_provider: bool,
) -> (
    Arc<codex_extension_api::ExtensionRegistry<Config>>,
    std::sync::mpsc::Receiver<CapturedExtensionEvent>,
) {
    let (event_tx, event_rx) = std::sync::mpsc::channel();
    let mut extensions =
        ExtensionRegistryBuilder::<Config>::with_event_sink(Arc::new(ChannelEventSink(event_tx)));
    let mut providers =
        SkillProviders::new().with_executor_provider(Arc::new(ExecutorSkillProvider {
            catalog: executor_catalog,
        }));
    if include_host_provider {
        providers = providers.with_host_provider(Arc::new(HostSkillProvider::new()));
    }
    install_with_providers(&mut extensions, providers, |config: &Config| {
        SkillsExtensionConfig {
            include_instructions: config.include_skill_instructions,
            bundled_skills_enabled: false,
            orchestrator_skills_enabled: false,
            shadow_selection_enabled: false,
        }
    });
    (Arc::new(extensions.build()), event_rx)
}

fn configure_catalog_test(config: &mut Config) {
    config.include_skill_instructions = true;
    config
        .features
        .enable(Feature::ExecutorCapabilityDiscovery)
        .expect("executor capability discovery should be configurable in tests");
    // A user layer also discovers the real `$HOME/.agents/skills`. Use a temporary system layer so
    // exact catalog and omission assertions only see the skills written under this test's home.
    let system_config_path = config.codex_home.join("config.toml");
    config.config_layer_stack = ConfigLayerStack::new(
        vec![ConfigLayerEntry::new(
            ConfigLayerSource::System {
                file: system_config_path,
            },
            toml! { skills = { bundled = { enabled = false } } }.into(),
        )],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("skills test config should be valid");
}

fn catalog_text<'a>(developer_texts: &'a [String], name_prefix: &str) -> &'a str {
    developer_texts
        .iter()
        .find(|text| text.contains(&format!("- {name_prefix}-")))
        .map(String::as_str)
        .unwrap_or_else(|| {
            panic!(
                "production request should include {name_prefix} skills, got {developer_texts:?}"
            )
        })
}

fn skill_lines<'a>(catalog_text: &'a str, name_prefix: &str) -> Vec<&'a str> {
    catalog_text
        .lines()
        .filter(|line| line.starts_with(&format!("- {name_prefix}-")))
        .collect()
}

fn skill_names<'a>(skill_lines: &[&'a str]) -> Vec<&'a str> {
    skill_lines
        .iter()
        .map(|line| {
            line.strip_prefix("- ")
                .and_then(|line| line.split_once(": ").map(|(name, _)| name))
                .unwrap_or_else(|| panic!("skill line should contain a name separator: {line}"))
        })
        .collect()
}

fn rendered_description(skill_line: &str) -> &str {
    let (_, after_name) = skill_line
        .split_once(": ")
        .unwrap_or_else(|| panic!("skill line should contain a name separator: {skill_line}"));
    after_name
        .split_once(" (")
        .map_or("", |(description, _)| description)
}

fn assert_full_descriptions(skill_lines: &[&str], expected: &[(&str, &str)]) {
    assert_eq!(
        skill_names(skill_lines),
        expected.iter().map(|(name, _)| *name).collect::<Vec<_>>()
    );
    for (skill_line, (_, expected_description)) in skill_lines.iter().zip(expected) {
        assert_eq!(rendered_description(skill_line), *expected_description);
    }
}

fn assert_shortened_descriptions(skill_lines: &[&str], expected: &[(&str, &str)]) {
    assert_eq!(
        skill_names(skill_lines),
        expected.iter().map(|(name, _)| *name).collect::<Vec<_>>()
    );
    for (skill_line, (_, full_description)) in skill_lines.iter().zip(expected) {
        let description = rendered_description(skill_line);
        assert!(!description.is_empty());
        assert!(full_description.starts_with(description));
        assert!(description.chars().count() < full_description.chars().count());
    }
}

fn metadata_cost(skill_lines: &[&str]) -> usize {
    skill_lines.iter().fold(0usize, |cost, line| {
        cost.saturating_add(approx_token_count(&format!("{line}\n")))
    })
}

fn executor_omission_text(developer_texts: &[String]) -> &str {
    developer_texts
        .iter()
        .find(|text| text.contains("additional skills omitted from this bounded skills list"))
        .map(String::as_str)
        .unwrap_or_else(|| {
            panic!(
                "production request should include the executor omission marker, got {developer_texts:?}"
            )
        })
}

async fn rendered_catalogs(
    host_skills: &[(&str, &str)],
    executor_skills: &[(&str, &str)],
    context_window: i64,
) -> Result<(Vec<String>, Vec<String>)> {
    rendered_catalogs_for_turns(
        host_skills,
        executor_skills,
        context_window,
        /*turn_count*/ 1,
    )
    .await
}

async fn rendered_catalogs_for_turns(
    host_skills: &[(&str, &str)],
    executor_skills: &[(&str, &str)],
    context_window: i64,
    turn_count: usize,
) -> Result<(Vec<String>, Vec<String>)> {
    let server = responses::start_mock_server().await;
    let response = responses::mount_sse_sequence(
        &server,
        (0..turn_count)
            .map(|index| {
                let response_id = format!("resp-{index}");
                sse(vec![
                    ev_response_created(&response_id),
                    ev_completed(&response_id),
                ])
            })
            .collect(),
    )
    .await;
    let codex_home = Arc::new(TempDir::new()?);
    if !host_skills.is_empty() {
        write_host_skills(codex_home.path(), host_skills)?;
    }
    let (extensions, event_rx) =
        catalog_extensions(executor_catalog(executor_skills), !host_skills.is_empty());
    let mut builder = test_codex()
        .with_home(Arc::clone(&codex_home))
        .with_extensions(extensions)
        .with_model_info_override("gpt-5.5", move |model_info| {
            model_info.context_window = Some(context_window);
            model_info.max_context_window = None;
        })
        .with_config(configure_catalog_test);
    let test = builder.build_with_auto_env(&server).await?;

    let mut client_warning_messages = Vec::new();
    for _ in 0..turn_count {
        test.codex
            .submit(Op::UserInput {
                items: vec![UserInput::Text {
                    text: "Inspect the available skills.".to_string(),
                    text_elements: Vec::new(),
                }],
                final_output_json_schema: None,
                responsesapi_client_metadata: None,
                additional_context: Default::default(),
                thread_settings: Default::default(),
            })
            .await?;
        loop {
            match core_test_support::wait_for_event(&test.codex, |_| true).await {
                EventMsg::Warning(warning) => client_warning_messages.push(warning.message),
                EventMsg::TurnComplete(_) => break,
                _ => {}
            }
        }
    }
    let developer_texts = response
        .last_request()
        .expect("production turn should issue a responses request")
        .message_input_texts("developer");
    // Extension warnings are client-visible through the app-server event sink,
    // while core warnings are delivered through the TestCodex event stream.
    // Count both paths so duplicate warning ownership cannot hide in this test.
    client_warning_messages.extend(event_rx.try_iter().filter_map(|event| match event {
        CapturedExtensionEvent::Warning(warning) => Some(warning.message),
        CapturedExtensionEvent::Event(_) => None,
    }));
    let _codex_home_guard = codex_home;
    Ok((developer_texts, client_warning_messages))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn production_turn_scales_extension_catalog_from_resolved_model_window() -> Result<()> {
    let skill_count = 800;
    let mut included_counts = Vec::new();
    for (context_window, max_context_window, expected_budget) in
        [(Some(10_000), None, 200), (None, Some(400_000), 8_000)]
    {
        let server = responses::start_mock_server().await;
        let response = mount_sse_once(
            &server,
            sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
        )
        .await;
        let source_kind = SkillSourceKind::Custom("test".to_string());
        let catalog = SkillCatalog {
            entries: (0..skill_count)
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
        let (event_tx, event_rx) = std::sync::mpsc::channel();
        let mut extensions = ExtensionRegistryBuilder::<Config>::with_event_sink(Arc::new(
            ChannelEventSink(event_tx),
        ));
        install_with_providers(
            &mut extensions,
            SkillProviders::new().with_provider(SkillProviderSource::new(
                source_kind,
                "test",
                Arc::new(StaticSkillProvider {
                    catalog,
                    main_prompt_contents: None,
                }),
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
        let warning = event_rx.try_recv()?.into_warning();
        let omitted_count = skill_count - included_count;

        assert!(catalog_text.contains("additional skills omitted"));
        assert!(!catalog_text.contains(
            "A description long enough to keep the catalog under sustained budget pressure."
        ));
        assert!(metadata_cost <= expected_budget);
        assert_eq!(
            warning.message,
            format!(
                "Exceeded skills context budget. All skill descriptions were removed and {omitted_count} additional skills were not included in the model-visible skills list."
            )
        );
        included_counts.push(included_count);
    }

    assert!(included_counts[0] > 0);
    assert!(included_counts[0] < included_counts[1]);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn production_turn_shortens_host_only_catalog_with_the_full_budget() -> Result<()> {
    let (developer_texts, _) =
        rendered_catalogs(&HOST_CATALOG, &[], SHORTENING_CONTEXT_WINDOW).await?;
    let host_lines = skill_lines(catalog_text(&developer_texts, "host"), "host");

    assert_shortened_descriptions(&host_lines, &HOST_CATALOG);
    assert!(metadata_cost(&host_lines) <= 240);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn production_turn_shortens_executor_only_catalog_with_the_full_budget() -> Result<()> {
    let (developer_texts, _) =
        rendered_catalogs(&[], &EXECUTOR_CATALOG, SHORTENING_CONTEXT_WINDOW).await?;
    let executor_lines = skill_lines(catalog_text(&developer_texts, "exec"), "exec");

    assert_shortened_descriptions(&executor_lines, &EXECUTOR_CATALOG);
    assert!(metadata_cost(&executor_lines) <= 240);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn production_turn_shares_catalog_budget_across_host_and_executor_sections() -> Result<()> {
    let (developer_texts, _) =
        rendered_catalogs(&HOST_CATALOG, &EXECUTOR_CATALOG, SHORTENING_CONTEXT_WINDOW).await?;
    let host_lines = skill_lines(catalog_text(&developer_texts, "host"), "host");
    let executor_lines = skill_lines(catalog_text(&developer_texts, "exec"), "exec");
    let combined_lines = host_lines
        .iter()
        .chain(executor_lines.iter())
        .copied()
        .collect::<Vec<_>>();

    assert_shortened_descriptions(&host_lines, &HOST_CATALOG);
    assert_shortened_descriptions(&executor_lines, &EXECUTOR_CATALOG);
    assert!(metadata_cost(&combined_lines) <= 240);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn production_turn_keeps_full_host_only_catalog_when_it_fits() -> Result<()> {
    let (developer_texts, _) =
        rendered_catalogs(&HOST_CATALOG, &[], FULL_CATALOG_CONTEXT_WINDOW).await?;
    let host_lines = developer_texts
        .iter()
        .flat_map(|text| skill_lines(text, "host"))
        .collect::<Vec<_>>();

    assert_full_descriptions(&host_lines, &HOST_CATALOG);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn production_turn_preserves_host_alias_root_order_across_turns() -> Result<()> {
    let server = responses::start_mock_server().await;
    let response = responses::mount_sse_sequence(
        &server,
        ["resp-1", "resp-2"]
            .into_iter()
            .map(|response_id| {
                sse(vec![
                    ev_response_created(response_id),
                    ev_completed(response_id),
                ])
            })
            .collect(),
    )
    .await;
    let temp_parent = TempDir::new()?;
    let long_parent = temp_parent
        .path()
        .join("codex-home-with-long-shared-prefix-for-production-alias-order-test");
    std::fs::create_dir_all(&long_parent)?;
    let codex_home = Arc::new(TempDir::new_in(&long_parent)?);
    let first_root_path = long_parent.join("first-discovered-skills-root-with-long-shared-prefix");
    let second_root_path =
        long_parent.join("second-discovered-skills-root-with-long-shared-prefix");
    for (root, prefix) in [
        (&first_root_path, "z-first"),
        (&second_root_path, "a-second"),
    ] {
        for index in 0..6 {
            let name = format!("{prefix}-{index:02}");
            let skill_dir = root.join(&name);
            std::fs::create_dir_all(&skill_dir)?;
            std::fs::write(
                skill_dir.join("SKILL.md"),
                format!("---\nname: {name}\ndescription: d\n---\n\n# body\n"),
            )?;
        }
    }
    let first_root = AbsolutePathBuf::try_from(std::fs::canonicalize(&first_root_path)?)?;
    let second_root = AbsolutePathBuf::try_from(std::fs::canonicalize(&second_root_path)?)?;
    let (extensions, _) =
        catalog_extensions(SkillCatalog::default(), /*include_host_provider*/ true);
    let mut builder = test_codex()
        .with_home(Arc::clone(&codex_home))
        .with_extensions(extensions)
        .with_model_info_override("gpt-5.5", |model_info| {
            model_info.context_window = Some(SHORTENING_CONTEXT_WINDOW);
            model_info.max_context_window = None;
        })
        .with_config(configure_catalog_test);
    let test = builder.build_with_auto_env(&server).await?;
    test.thread_manager
        .skills_service()
        .set_extra_roots(vec![first_root.clone(), second_root.clone()]);

    test.submit_turn("Inspect the available skills.").await?;
    test.submit_turn("Inspect the available skills again.")
        .await?;

    let expected_alias_lines = vec![
        format!(
            "- `r0` = `{}`",
            first_root.to_string_lossy().replace('\\', "/")
        ),
        format!(
            "- `r1` = `{}`",
            second_root.to_string_lossy().replace('\\', "/")
        ),
    ];
    let expected_skill_lines = vec![
        "- a-second-00: d (file: r1/a-second-00/SKILL.md)".to_string(),
        "- z-first-00: d (file: r0/z-first-00/SKILL.md)".to_string(),
    ];
    let actual = response
        .requests()
        .iter()
        .map(|request| {
            let developer_text = request.message_input_texts("developer").join("\n");
            let alias_lines = developer_text
                .lines()
                .filter(|line| line.starts_with("- `r"))
                .map(str::to_string)
                .collect::<Vec<_>>();
            let skill_lines = developer_text
                .lines()
                .filter(|line| {
                    line.starts_with("- a-second-00:") || line.starts_with("- z-first-00:")
                })
                .map(str::to_string)
                .collect::<Vec<_>>();
            (alias_lines, skill_lines)
        })
        .collect::<Vec<_>>();

    assert_eq!(
        actual,
        vec![
            (expected_alias_lines.clone(), expected_skill_lines.clone()),
            (expected_alias_lines, expected_skill_lines),
        ]
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn production_turn_uses_provider_host_catalog_and_core_snapshot_injection() -> Result<()> {
    let server = responses::start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    let response = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let codex_home = Arc::new(TempDir::new()?);
    let skill_name = "snapshot-backed";
    let snapshot_description = "This description comes from Core's host skills snapshot.";
    write_host_skills(codex_home.path(), &[(skill_name, snapshot_description)])?;
    let skill_path = codex_home
        .path()
        .join("skills")
        .join(skill_name)
        .join("SKILL.md");
    let snapshot_contents = format!(
        "---\nname: {skill_name}\ndescription: {snapshot_description}\n---\n\nUse $calendar.\n"
    );
    std::fs::write(&skill_path, &snapshot_contents)?;
    let skill_resource = skill_path.to_string_lossy().into_owned();
    let provider_description = "This skill comes from the extension host provider.";
    let provider_contents = "# Provider instructions that Core must not inject.";
    let provider_catalog = SkillCatalog {
        entries: vec![
            SkillCatalogEntry::new(
                SkillPackageId(skill_resource.clone()),
                SkillAuthority::new(SkillSourceKind::Host, "host"),
                skill_name,
                provider_description,
                SkillResourceId::new(skill_resource.clone()),
            )
            .with_display_path(skill_resource.replace('\\', "/")),
        ],
        warnings: Vec::new(),
    };
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    install_with_providers(
        &mut extensions,
        SkillProviders::new().with_host_provider(Arc::new(StaticSkillProvider {
            catalog: provider_catalog,
            main_prompt_contents: Some(provider_contents.to_string()),
        })),
        |config: &Config| SkillsExtensionConfig {
            include_instructions: config.include_skill_instructions,
            bundled_skills_enabled: false,
            orchestrator_skills_enabled: false,
            shadow_selection_enabled: false,
        },
    );
    let mut builder = apps_enabled_builder(apps_server.chatgpt_base_url)
        .with_home(Arc::clone(&codex_home))
        .with_extensions(Arc::new(extensions.build()))
        .with_config(configure_catalog_test);
    let test = builder.build_with_auto_env(&server).await?;
    wait_for_mcp_server(&test.codex, CODEX_APPS_MCP_SERVER_NAME).await?;

    test.submit_turn(&format!("Use ${skill_name}.")).await?;
    let request = response.single_request();
    let developer_texts = request.message_input_texts("developer");
    let cutover_skill_lines = developer_texts
        .iter()
        .flat_map(|text| text.lines())
        .filter(|line| line.contains(skill_name))
        .collect::<Vec<_>>();

    assert_eq!(
        cutover_skill_lines,
        vec![format!(
            "- {skill_name}: {provider_description} (file: {})",
            skill_resource.replace('\\', "/")
        )]
    );
    let user_text = request.message_input_texts("user").join("\n");
    assert!(user_text.contains(&snapshot_contents));
    assert!(!user_text.contains(provider_contents));
    let deadline = Instant::now() + Duration::from_secs(10);
    let app_mentioned_event = loop {
        let requests = server.received_requests().await.unwrap_or_default();
        if let Some(event) = requests
            .into_iter()
            .filter(|request| request.url.path() == "/codex/analytics-events/events")
            .find_map(|request| {
                let payload: serde_json::Value = serde_json::from_slice(&request.body).ok()?;
                payload["events"].as_array().and_then(|events| {
                    events
                        .iter()
                        .find(|event| event["event_type"] == "codex_app_mentioned")
                        .cloned()
                })
            })
        {
            break event;
        }
        if Instant::now() >= deadline {
            panic!("timed out waiting for app mentioned analytics");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    assert_eq!(
        app_mentioned_event["event_params"]["connector_id"],
        "calendar"
    );
    assert_eq!(
        app_mentioned_event["event_params"]["invoke_type"],
        "explicit"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn production_turn_keeps_full_snapshot_host_skill_prompt() -> Result<()> {
    let server = responses::start_mock_server().await;
    let response = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let codex_home = Arc::new(TempDir::new()?);
    let skill_dir = codex_home.path().join("skills").join("long-host");
    std::fs::create_dir_all(&skill_dir)?;
    let prompt_tail = "full host prompt tail";
    let skill_contents = format!(
        "---\nname: long-host\ndescription: Long host skill.\n---\n\n# Long host skill\n\n{}\n{prompt_tail}\n",
        "x".repeat(8_000)
    );
    std::fs::write(skill_dir.join("SKILL.md"), &skill_contents)?;
    let (extensions, _) =
        catalog_extensions(SkillCatalog::default(), /*include_host_provider*/ true);
    let mut builder = test_codex()
        .with_home(Arc::clone(&codex_home))
        .with_extensions(extensions)
        .with_config(configure_catalog_test);
    let test = builder.build_with_auto_env(&server).await?;

    test.submit_turn("Use $long-host.").await?;
    let user_text = response
        .single_request()
        .message_input_texts("user")
        .join("\n");

    assert!(user_text.contains(&skill_contents));
    assert_eq!(
        user_text.matches("<skill>\n<name>long-host</name>").count(),
        1
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn production_turn_keeps_core_host_injection_when_catalog_listings_are_disabled() -> Result<()>
{
    let server = responses::start_mock_server().await;
    let response = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let codex_home = Arc::new(TempDir::new()?);
    let skill_dir = codex_home.path().join("skills").join("long-host");
    std::fs::create_dir_all(&skill_dir)?;
    let prompt_tail = "full host prompt tail";
    let skill_contents = format!(
        "---\nname: long-host\ndescription: Long host skill.\n---\n\n# Long host skill\n\n{}\n{prompt_tail}\n",
        "x".repeat(8_000)
    );
    std::fs::write(skill_dir.join("SKILL.md"), &skill_contents)?;
    let (extensions, _) =
        catalog_extensions(SkillCatalog::default(), /*include_host_provider*/ true);
    let mut builder = test_codex()
        .with_home(Arc::clone(&codex_home))
        .with_extensions(extensions)
        .with_config(configure_catalog_test)
        .with_config(|config| {
            config.include_skill_instructions = false;
        });
    let test = builder.build_with_auto_env(&server).await?;

    test.submit_turn("Use $long-host.").await?;
    let user_text = response
        .single_request()
        .message_input_texts("user")
        .join("\n");

    assert!(user_text.contains(&skill_contents));
    assert_eq!(
        user_text.matches("<skill>\n<name>long-host</name>").count(),
        1
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn production_turn_keeps_full_executor_only_catalog_when_it_fits() -> Result<()> {
    let (developer_texts, _) =
        rendered_catalogs(&[], &EXECUTOR_CATALOG, FULL_CATALOG_CONTEXT_WINDOW).await?;
    let executor_lines = skill_lines(catalog_text(&developer_texts, "exec"), "exec");

    assert_full_descriptions(&executor_lines, &EXECUTOR_CATALOG);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn production_turn_keeps_full_host_and_executor_catalogs_when_they_fit() -> Result<()> {
    let (developer_texts, _) = rendered_catalogs(
        &HOST_CATALOG,
        &EXECUTOR_CATALOG,
        FULL_CATALOG_CONTEXT_WINDOW,
    )
    .await?;
    let host_lines = skill_lines(catalog_text(&developer_texts, "host"), "host");
    let executor_lines = skill_lines(catalog_text(&developer_texts, "exec"), "exec");

    assert_full_descriptions(&host_lines, &HOST_CATALOG);
    assert_full_descriptions(&executor_lines, &EXECUTOR_CATALOG);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn production_turn_omits_host_skills_under_extreme_host_only_pressure() -> Result<()> {
    let (developer_texts, warning_messages) =
        rendered_catalogs(&HOST_CATALOG, &[], HOST_OMITTING_CONTEXT_WINDOW).await?;
    let host_lines = skill_lines(catalog_text(&developer_texts, "host"), "host");

    assert_eq!(
        skill_names(&host_lines),
        vec!["host-alpha", "host-beta", "host-delta"]
    );
    let expected_warning = "Exceeded skills context budget. All skill descriptions were removed and 1 additional skill was not included in the model-visible skills list.";
    assert_eq!(
        warning_messages
            .iter()
            .filter(|message| message.as_str() == expected_warning)
            .count(),
        1
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn production_turn_preserves_empty_core_compatible_host_fragment() -> Result<()> {
    let host_skills = [(
        "host-a-b-c-d-e-f-g-h-i-j-k-l-m-n-o-p-q-r-s-t-u-v-w-x-y-z-a-b",
        "Host-only skill.",
    )];
    let (developer_texts, warning_messages) =
        rendered_catalogs(&host_skills, &[], /*context_window*/ 1_000).await?;
    let host_fragment = developer_texts
        .iter()
        .find(|text| text.contains("## Skills"))
        .unwrap_or_else(|| {
            panic!(
                "production request should preserve the empty host skills fragment, got {developer_texts:?}"
            )
        });

    assert!(!host_fragment.contains(&format!("- {}:", host_skills[0].0)));
    assert!(!host_fragment.contains("## Host skills update"));
    assert_eq!(
        warning_messages,
        vec![
            "Exceeded skills context budget. All skill descriptions were removed and 1 additional skill was not included in the model-visible skills list."
                .to_string()
        ]
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn successive_turns_do_not_repeat_unchanged_host_budget_warning() -> Result<()> {
    let (_, warning_messages) = rendered_catalogs_for_turns(
        &HOST_CATALOG,
        &[],
        HOST_OMITTING_CONTEXT_WINDOW,
        /*turn_count*/ 2,
    )
    .await?;
    let expected_warning = "Exceeded skills context budget. All skill descriptions were removed and 1 additional skill was not included in the model-visible skills list.";

    assert_eq!(
        warning_messages
            .iter()
            .filter(|message| message.as_str() == expected_warning)
            .count(),
        1
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn production_turn_omits_executor_skills_under_extreme_executor_only_pressure() -> Result<()>
{
    let (developer_texts, _) =
        rendered_catalogs(&[], &EXECUTOR_CATALOG, EXECUTOR_OMITTING_CONTEXT_WINDOW).await?;
    let executor_text = executor_omission_text(&developer_texts);
    let executor_lines = skill_lines(executor_text, "exec");

    assert_eq!(skill_names(&executor_lines), vec!["exec-alpha"]);
    assert!(executor_text.contains("- 5 additional skills omitted from this bounded skills list."));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn production_turn_omits_executor_skills_after_host_skills_under_extreme_pressure()
-> Result<()> {
    let (developer_texts, _) = rendered_catalogs(
        &HOST_CATALOG,
        &EXECUTOR_CATALOG,
        MIXED_EXECUTOR_OMITTING_CONTEXT_WINDOW,
    )
    .await?;
    let host_lines = developer_texts
        .iter()
        .flat_map(|text| skill_lines(text, "host"))
        .collect::<Vec<_>>();
    let executor_text = executor_omission_text(&developer_texts);
    let executor_lines = skill_lines(executor_text, "exec");

    assert_eq!(skill_names(&host_lines), Vec::<&str>::new());
    assert_eq!(skill_names(&executor_lines), vec!["exec-alpha"]);
    assert!(executor_text.contains("- 5 additional skills omitted from this bounded skills list."));
    assert!(developer_texts.iter().any(|text| text.contains(
        "Host skills are available but omitted from the model-visible skills list because the skills context budget was exceeded."
    )));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn production_turn_omits_host_skills_before_executor_skills_under_extreme_mixed_pressure()
-> Result<()> {
    let (developer_texts, warning_messages) = rendered_catalogs(
        &HOST_CATALOG,
        &EXECUTOR_CATALOG,
        MIXED_HOST_OMITTING_CONTEXT_WINDOW,
    )
    .await?;
    let host_lines = skill_lines(catalog_text(&developer_texts, "host"), "host");
    let executor_lines = skill_lines(catalog_text(&developer_texts, "exec"), "exec");

    assert_eq!(skill_names(&host_lines), vec!["host-beta"]);
    assert_eq!(
        skill_names(&executor_lines),
        EXECUTOR_CATALOG
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<_>>()
    );
    assert!(warning_messages.contains(
        &"Exceeded skills context budget. All skill descriptions were removed and 3 additional skills were not included in the model-visible skills list."
            .to_string()
    ));

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
    let (event_tx, event_rx) = std::sync::mpsc::channel();
    let mut extensions =
        ExtensionRegistryBuilder::<Config>::with_event_sink(Arc::new(ChannelEventSink(event_tx)));
    install_with_providers(
        &mut extensions,
        SkillProviders::new().with_provider(SkillProviderSource::new(
            source_kind,
            "test",
            Arc::new(StaticSkillProvider {
                catalog,
                main_prompt_contents: None,
            }),
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
    let warning = event_rx.try_recv()?.into_warning();
    assert_eq!(
        warning.message,
        "Skill descriptions were shortened to fit the skills context budget. Codex can still see every skill, but some descriptions are shorter. Disable unused skills or plugins to leave more room for the rest."
    );

    Ok(())
}
