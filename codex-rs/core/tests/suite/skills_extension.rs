use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use anyhow::Result;
use codex_config::ConfigLayerEntry;
use codex_config::ConfigLayerSource;
use codex_config::ConfigLayerStack;
use codex_config::ConfigRequirements;
use codex_config::ConfigRequirementsToml;
use codex_core::StartThreadOptions;
use codex_core::TurnInputRequest;
use codex_core::config::Config;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::LOCAL_ENVIRONMENT_ID;
use codex_exec_server::RemoveOptions;
use codex_extension_api::ExtensionDataInit;
use codex_extension_api::ExtensionEventSink;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ExtensionWarning;
use codex_extension_api::SkillInvocationContributor;
use codex_extension_api::SkillInvocationInput;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_models_manager::bundled_models_response;
use codex_otel::OtelExporter;
use codex_otel::OtelHttpProtocol;
use codex_otel::OtelProvider;
use codex_otel::OtelSettings;
use codex_otel::THREAD_SKILLS_KEPT_TOTAL_METRIC;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_protocol::models::PermissionProfile;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::TruncationPolicyConfig;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::protocol::TurnSettingsUpdate;
use codex_protocol::protocol::TurnSettingsUpdateOutcome;
use codex_protocol::user_input::UserInput;
use codex_skills_extension::ExecutorSkillProvider;
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
use codex_skills_extension::install;
use codex_skills_extension::install_with_providers;
use codex_skills_extension::provider::SkillListQuery;
use codex_skills_extension::provider::SkillProviderFuture;
use codex_skills_extension::provider::SkillReadRequest;
use codex_skills_extension::provider::SkillSearchRequest;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use codex_utils_string::approx_token_count;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::apps_enabled_builder;
use core_test_support::responses;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_websocket_server;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_remote;
use core_test_support::skip_if_target_windows;
use core_test_support::skip_if_wine_exec;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::test_env;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_event;
use core_test_support::wait_for_mcp_server;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::MetricData;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use sha1::Digest;
use tempfile::TempDir;
use test_case::test_case;
use tokio::time::Duration;
use tokio::time::Instant;
use toml::toml;
use tracing::Level;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_test::internal::MockWriter;
use wiremock::MockServer;

#[path = "skills_extension/cloud_skill_tests.rs"]
mod cloud_skill_tests;

#[path = "skills_extension/cloud_lifecycle_tests.rs"]
mod cloud_lifecycle_tests;

struct StaticSkillProvider {
    catalog: SkillCatalog,
    main_prompt_contents: Option<String>,
}

struct CatalogSkillProvider {
    catalog: SkillCatalog,
}

struct PausedCatalogSkillProvider {
    inner: CatalogSkillProvider,
    first_discovery: AtomicBool,
    started: tokio::sync::mpsc::UnboundedSender<String>,
    resume: tokio::sync::Semaphore,
}

impl SkillProvider for PausedCatalogSkillProvider {
    fn list(&self, query: SkillListQuery) -> SkillProviderFuture<'_, SkillCatalog> {
        Box::pin(async move {
            if query.host_snapshot.is_none() {
                return Ok(SkillCatalog::default());
            }
            if self.first_discovery.swap(/*val*/ false, Ordering::SeqCst) {
                self.started
                    .send(query.turn_id.clone())
                    .expect("discovery receiver");
                self.resume
                    .acquire()
                    .await
                    .expect("resume discovery")
                    .forget();
            }
            self.inner.list(query).await
        })
    }

    fn read<'a>(
        &'a self,
        request: SkillReadRequest<'a>,
    ) -> SkillProviderFuture<'a, SkillReadResult> {
        self.inner.read(request)
    }

    fn search(&self, request: SkillSearchRequest) -> SkillProviderFuture<'_, SkillSearchResult> {
        self.inner.search(request)
    }
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

    fn read<'a>(
        &'a self,
        request: SkillReadRequest<'a>,
    ) -> SkillProviderFuture<'a, SkillReadResult> {
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

impl SkillProvider for CatalogSkillProvider {
    fn list(&self, _query: SkillListQuery) -> SkillProviderFuture<'_, SkillCatalog> {
        Box::pin(async { Ok(self.catalog.clone()) })
    }

    fn read<'a>(
        &'a self,
        _request: SkillReadRequest<'a>,
    ) -> SkillProviderFuture<'a, SkillReadResult> {
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
        SkillProviders::new().with_executor_provider(Arc::new(CatalogSkillProvider {
            catalog: executor_catalog,
        }));
    if include_host_provider {
        providers = providers.with_host_provider(Arc::new(HostSkillProvider::new()));
    }
    install_with_providers(&mut extensions, providers, |config: &Config| {
        SkillsExtensionConfig {
            include_instructions: config.include_skill_instructions,
            max_context_tokens: config.skill_max_context_tokens,
            bundled_skills_enabled: false,
            cloud_skill_enabled: false,
            shadow_selection_enabled: false,
        }
    });
    (Arc::new(extensions.build()), event_rx)
}

async fn wait_for_analytics_events(
    server: &MockServer,
    event_type: &str,
    expected_count: usize,
) -> Vec<Value> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let events = server
            .received_requests()
            .await
            .unwrap_or_default()
            .into_iter()
            .filter(|request| request.url.path() == "/codex/analytics-events/events")
            .filter_map(|request| serde_json::from_slice::<Value>(&request.body).ok())
            .flat_map(|payload| payload["events"].as_array().cloned().unwrap_or_default())
            .filter(|event| event["event_type"] == event_type)
            .collect::<Vec<_>>();
        if events.len() >= expected_count {
            return events;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {event_type} analytics"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
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
            .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
                text: "Inspect the available skills.".to_string(),
                text_elements: Vec::new(),
            }]))
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
async fn capability_sections_render_in_order_with_host_repo_and_plugin_skills() -> Result<()> {
    skip_if_wine_exec!(
        Ok(()),
        "executor-backed repo skills require matching host and executor path conventions"
    );
    skip_if_no_network!(Ok(()));

    const HOST_SKILL_BODY: &str = "Use the host skill instructions.";
    const REPO_SKILL_BODY: &str = "Use the repository skill instructions.";
    const PLUGIN_SKILL_BODY: &str = "Use the legacy plugin skill instructions.";

    let server = responses::start_mock_server().await;
    let apps_server = AppsTestServer::mount_with_connector_name(&server, "Google Calendar").await?;
    let response = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp1"), ev_completed("resp1")]),
    )
    .await;

    let codex_home = Arc::new(TempDir::new()?);
    let host_skill_dir = codex_home.path().join("skills/host-search");
    std::fs::create_dir_all(&host_skill_dir)?;
    let host_skill_path = host_skill_dir.join("SKILL.md");
    std::fs::write(
        &host_skill_path,
        format!(
            "---\nname: host-search\ndescription: inspect host data\n---\n\n{HOST_SKILL_BODY}\n"
        ),
    )?;
    let host_skill_path = dunce::canonicalize(host_skill_path)?;
    let plugin_root = codex_home.path().join("plugins/cache/test/sample/local");
    std::fs::create_dir_all(plugin_root.join(".codex-plugin"))?;
    std::fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        r#"{"name":"sample","description":"inspect sample data"}"#,
    )?;
    let plugin_skill_dir = plugin_root.join("skills/sample-search");
    std::fs::create_dir_all(&plugin_skill_dir)?;
    let plugin_skill_path = plugin_skill_dir.join("SKILL.md");
    std::fs::write(
        &plugin_skill_path,
        format!("---\ndescription: inspect sample data\n---\n\n{PLUGIN_SKILL_BODY}\n"),
    )?;
    let plugin_skill_path = dunce::canonicalize(plugin_skill_path)?;
    std::fs::write(
        plugin_root.join(".app.json"),
        r#"{"apps":{"sample":{"id":"calendar"}}}"#,
    )?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        "[features]\nplugins = true\n\n[plugins.\"sample@test\"]\nenabled = true\n",
    )?;

    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    install(&mut extensions, |config: &Config| SkillsExtensionConfig {
        include_instructions: config.include_skill_instructions,
        max_context_tokens: config.skill_max_context_tokens,
        bundled_skills_enabled: config.bundled_skills_enabled(),
        cloud_skill_enabled: config.cloud_skill_enabled,
        shadow_selection_enabled: config.features.enabled(Feature::SkillSearch),
    });
    let mut builder = test_codex()
        .with_home(Arc::clone(&codex_home))
        .with_extensions(Arc::new(extensions.build()))
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_workspace_setup(|cwd, fs| async move {
            let skill_dir = cwd.join(".agents/skills/repo-search");
            fs.create_directory(
                &PathUri::from_host_native_path(&skill_dir)?,
                CreateDirectoryOptions { recursive: true, follow_symlinks: true },
                /*sandbox*/ None,
            )
            .await?;
            fs.write_file(
                &PathUri::from_host_native_path(skill_dir.join("SKILL.md"))?,
                format!(
                    "---\nname: repo-search\ndescription: inspect repo data\n---\n\n{REPO_SKILL_BODY}\n"
                )
                .into_bytes(),
                Default::default(), /*sandbox*/ None,
            )
            .await?;
            Ok(())
        })
        .with_config(move |config| {
            config
                .features
                .enable(Feature::Apps)
                .expect("test config should allow feature update");
            config.chatgpt_base_url = apps_server.chatgpt_base_url;
        });
    let test = builder.build_with_auto_env(&server).await?;
    let repo_skill_path = test
        .fs()
        .canonicalize(
            &PathUri::from_abs_path(&test.config.cwd.join(".agents/skills/repo-search/SKILL.md")),
            /*sandbox*/ None,
        )
        .await?
        .to_abs_path()?
        .to_path_buf();

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![
            UserInput::Text {
                text: "use all skills".to_string(),
                text_elements: Vec::new(),
            },
            UserInput::Skill {
                name: "host-search".to_string(),
                path: host_skill_path.clone(),
            },
            UserInput::Skill {
                name: "repo-search".to_string(),
                path: repo_skill_path.clone(),
            },
            UserInput::Skill {
                name: "sample:sample-search".to_string(),
                path: plugin_skill_path.clone(),
            },
        ]))
        .await?;

    core_test_support::wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let request = response.single_request();
    let developer_messages = request.message_input_texts("developer");
    let developer_text = developer_messages.join("\n\n");
    let apps_pos = developer_text
        .find("## Apps")
        .expect("expected apps section in developer message");
    let skills_pos = developer_text
        .find("## Skills")
        .expect("expected skills section in developer message");
    let plugins_pos = developer_text
        .find("## Plugins")
        .expect("expected plugins section in developer message");
    assert!(
        skills_pos < apps_pos && apps_pos < plugins_pos,
        "expected Skills -> Apps -> Plugins order: {developer_messages:?}"
    );
    assert!(
        !developer_text.contains("`sample`: inspect sample data"),
        "did not expect plugin description in developer message: {developer_messages:?}"
    );
    assert!(
        developer_text.contains("skill entries are prefixed with `plugin_name:`"),
        "expected plugin skill naming guidance in developer message: {developer_messages:?}"
    );
    assert!(
        developer_text.contains("sample:sample-search: inspect sample data"),
        "expected namespaced plugin skill summary in developer message: {developer_messages:?}"
    );
    assert!(
        developer_text.contains("repo-search: inspect repo data"),
        "expected repo skill summary in developer message: {developer_messages:?}"
    );
    assert!(
        developer_text.contains("host-search: inspect host data"),
        "expected host skill summary in developer message: {developer_messages:?}"
    );

    let user_text = request.message_input_texts("user").join("\n");
    for (name, path, body) in [
        ("host-search", &host_skill_path, HOST_SKILL_BODY),
        ("repo-search", &repo_skill_path, REPO_SKILL_BODY),
        (
            "sample:sample-search",
            &plugin_skill_path,
            PLUGIN_SKILL_BODY,
        ),
    ] {
        assert!(
            user_text.contains(&format!("<skill>\n<name>{name}</name>")),
            "expected injected skill `{name}` in user input: {user_text}"
        );
        assert!(
            user_text.contains(path.to_string_lossy().as_ref()),
            "expected path for `{name}` in user input: {user_text}"
        );
        assert!(
            user_text.contains(body),
            "expected body for `{name}` in user input: {user_text}"
        );
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn agent_plugin_skill_prompt_stays_bounded_without_skills_extension() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let response = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp1"), ev_completed("resp1")]),
    )
    .await;

    let codex_home = Arc::new(TempDir::new()?);
    let plugin_root = codex_home
        .path()
        .join("plugins/cache/test/acme.tools/local");
    let skill_dir = plugin_root.join("skills/review");
    std::fs::create_dir_all(&skill_dir)?;
    std::fs::write(
        plugin_root.join("plugin.json"),
        r#"{"$schema":"https://agent-plugins.org/schemas/1.0.0/plugin.schema.json","name":"acme.tools","extensions":{"com.openai":{"interface":{"displayName":"Acme Developer Tools"}}}}"#,
    )?;
    std::fs::write(
        skill_dir.join("SKILL.md"),
        format!(
            "---\nname: review\ndescription: Review code\n---\n\n{}\nAGENT_SKILL_TRUNCATED_TAIL\n",
            "x".repeat(9_000)
        ),
    )?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        "[features]\nplugins = true\n\n[plugins.\"acme.tools@test\"]\nenabled = true\n",
    )?;
    let skill_path = dunce::canonicalize(skill_dir.join("SKILL.md"))?;
    let mut builder = test_codex().with_home(codex_home);
    let test = builder.build_with_auto_env(&server).await?;

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Skill {
            name: "acme.tools:review".into(),
            path: skill_path,
        }]))
        .await?;
    let warning = core_test_support::wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::Warning(warning)
                if warning.message.contains("main prompt context limit")
        )
    })
    .await;
    core_test_support::wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let user_text = response
        .single_request()
        .message_input_texts("user")
        .join("\n");
    assert!(user_text.contains("acme.tools:review"));
    assert!(!user_text.contains("AGENT_SKILL_TRUNCATED_TAIL"));
    let EventMsg::Warning(warning) = warning else {
        unreachable!("wait_for_event matched an Agent skill truncation warning")
    };
    assert!(warning.message.contains("acme.tools:review"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_skill_prompt_precedes_plugin_instructions() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let response = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;

    let codex_home = Arc::new(TempDir::new()?);
    let plugin_root = codex_home.path().join("plugins/cache/test/sample/local");
    let skill_dir = plugin_root.join("skills/sample-search");
    std::fs::create_dir_all(plugin_root.join(".codex-plugin"))?;
    std::fs::create_dir_all(&skill_dir)?;
    std::fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        r#"{"name":"sample","description":"inspect sample data"}"#,
    )?;
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\ndescription: inspect sample data\n---\n\n# body\n",
    )?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        "[features]\nplugins = true\n\n[plugins.\"sample@test\"]\nenabled = true\n",
    )?;
    let skill_path = dunce::canonicalize(skill_dir.join("SKILL.md"))?;
    let (extensions, _) =
        catalog_extensions(SkillCatalog::default(), /*include_host_provider*/ true);
    let mut builder = test_codex()
        .with_home(codex_home)
        .with_extensions(extensions);
    let test = builder.build_with_auto_env(&server).await?;

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![
            UserInput::Skill {
                name: "sample:sample-search".to_string(),
                path: skill_path,
            },
            UserInput::Mention {
                name: "sample".to_string(),
                path: "plugin://sample@test".to_string(),
            },
        ]))
        .await?;
    core_test_support::wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let input = response.single_request().input();
    let prompt_position = |expected: &str| {
        input
            .iter()
            .position(|item| {
                item["content"].as_array().is_some_and(|content| {
                    content.iter().any(|part| {
                        part["text"]
                            .as_str()
                            .is_some_and(|text| text.contains(expected))
                    })
                })
            })
            .unwrap_or_else(|| panic!("missing prompt containing `{expected}`: {input:?}"))
    };
    let skill_position = prompt_position("<skill>\n<name>sample:sample-search</name>");
    let plugin_position = prompt_position("Capabilities from the `sample` plugin:");
    assert!(
        skill_position < plugin_position,
        "host skill prompts should precede plugin instructions: {input:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn production_turn_aliases_executor_skill_roots() -> Result<()> {
    const SKILL_ROOT: &str =
        "skill://integration-executor/workspace/plugins/cache/executor-plugin/1.0.0/skills";

    let server = responses::start_mock_server().await;
    let response = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let catalog = SkillCatalog {
        entries: ["search", "review", "summarize"]
            .into_iter()
            .map(|name| {
                let resource = format!("{SKILL_ROOT}/{name}/SKILL.md");
                SkillCatalogEntry::new(
                    SkillPackageId(format!("{SKILL_ROOT}/{name}")),
                    SkillAuthority::new(SkillSourceKind::Executor, "integration-executor"),
                    name,
                    "Inspect executor resources.",
                    SkillResourceId::new(resource.clone()),
                )
                .with_display_path(resource)
                .with_alias_root(SKILL_ROOT)
            })
            .collect(),
        warnings: Vec::new(),
    };
    let (extensions, _) = catalog_extensions(catalog, /*include_host_provider*/ false);
    let mut builder = test_codex()
        .with_extensions(extensions)
        .with_model_info_override("gpt-5.6-sol", |model_info| {
            model_info.context_window = Some(3_000);
            model_info.max_context_window = None;
        })
        .with_config(configure_catalog_test);
    let test = builder.build_with_auto_env(&server).await?;

    test.submit_turn("Inspect the available skills.").await?;

    let developer_text = response
        .single_request()
        .message_input_texts("developer")
        .join("\n");
    assert!(
        developer_text.contains(&format!("- `e0` = `{SKILL_ROOT}`")),
        "model request should include the executor skill root: {developer_text}"
    );
    assert!(
        developer_text.contains("executor package: e0/search"),
        "model request should include the aliased executor skill: {developer_text}"
    );
    let executor_catalog = developer_text
        .split("<skills_instructions>")
        .filter_map(|fragment| fragment.split_once("</skills_instructions>"))
        .map(|(fragment, _)| fragment)
        .find(|fragment| fragment.contains("executor package: e0/search"))
        .expect("model request should include an executor skills catalog");
    assert!(
        executor_catalog.contains("Read a skill package directly with `skills.read"),
        "Sol should receive direct-read instructions for executor skills: {executor_catalog}"
    );
    assert!(
        !executor_catalog.contains("### How to use skills"),
        "Sol should omit optional skill usage instructions: {executor_catalog}"
    );
    assert!(
        !executor_catalog.contains("- Root aliases: Pass short package locators directly"),
        "Sol should not include optional resource-alias instructions: {executor_catalog}"
    );
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn opted_in_executor_provider_skips_host_discovery_but_injects_discovered_skill() -> Result<()>
{
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(
        Ok(()),
        "executor-backed repo skills require matching host and executor path conventions"
    );

    const AMBIENT_SKILL_NAME: &str = "ambient-repo";
    const AMBIENT_SKILL_BODY: &str = "AMBIENT_REPO_SKILL_SHOULD_NOT_BE_LOADED";
    const HOST_SKILL_NAME: &str = "ambient-home";
    const HOST_SKILL_DESCRIPTION: &str = "NON_REPO_HOST_SKILL_SHOULD_NOT_BE_LOADED";
    const EXECUTOR_SKILL_NAME: &str = "selected-executor";
    const EXECUTOR_SKILL_BODY: &str = "SELECTED_EXECUTOR_SKILL_REMAINS_AVAILABLE";
    const EXECUTOR_ROOT_ID: &str = "selected-capabilities";

    let buffer: &'static Mutex<Vec<u8>> = Box::leak(Box::new(Mutex::new(Vec::new())));
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_max_level(Level::TRACE)
        .with_span_events(FmtSpan::NEW)
        .with_writer(MockWriter::new(buffer))
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let server = responses::start_mock_server().await;
    let websocket_server = start_websocket_server(vec![vec![
        vec![ev_response_created("warm-1"), ev_completed("warm-1")],
        vec![ev_response_created("resp-1"), ev_completed("resp-1")],
        vec![ev_response_created("resp-2"), ev_completed("resp-2")],
    ]])
    .await;
    let codex_home = Arc::new(TempDir::new()?);
    write_host_skills(
        codex_home.path(),
        &[(HOST_SKILL_NAME, HOST_SKILL_DESCRIPTION)],
    )?;
    let host_skill_path = codex_home
        .path()
        .join("skills")
        .join(HOST_SKILL_NAME)
        .join("SKILL.md");
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    install_with_providers(
        &mut extensions,
        SkillProviders::new().with_executor_provider(Arc::new(
            ExecutorSkillProvider::new_with_restriction_product(
                Arc::new(EnvironmentManager::default_for_tests()),
                /*restriction_product*/ None,
            ),
        )),
        |config: &Config| SkillsExtensionConfig {
            include_instructions: config.include_skill_instructions,
            max_context_tokens: config.skill_max_context_tokens,
            bundled_skills_enabled: false,
            cloud_skill_enabled: false,
            shadow_selection_enabled: false,
        },
    );
    let mut builder = test_codex()
        .with_home(codex_home)
        .with_extensions(Arc::new(extensions.build()))
        .with_workspace_setup(|cwd, fs| async move {
            for (skill_dir, name, description, body) in [
                (
                    cwd.join(".agents/skills").join(AMBIENT_SKILL_NAME),
                    AMBIENT_SKILL_NAME,
                    "Ambient repository skill.",
                    AMBIENT_SKILL_BODY,
                ),
                (
                    cwd.join("selected-capabilities").join(EXECUTOR_SKILL_NAME),
                    EXECUTOR_SKILL_NAME,
                    "Selected executor skill.",
                    EXECUTOR_SKILL_BODY,
                ),
            ] {
                fs.create_directory(
                    &PathUri::from_host_native_path(&skill_dir)?,
                    CreateDirectoryOptions {
                        recursive: true,
                        follow_symlinks: true,
                    },
                    /*sandbox*/ None,
                )
                .await?;
                fs.write_file(
                    &PathUri::from_host_native_path(skill_dir.join("SKILL.md"))?,
                    format!("---\nname: {name}\ndescription: {description}\n---\n\n{body}\n")
                        .into_bytes(),
                    Default::default(),
                    /*sandbox*/ None,
                )
                .await?;
            }
            Ok(())
        })
        .with_config(|config| {
            configure_catalog_test(config);
            config
                .features
                .enable(Feature::SkipHostSkillDiscovery)
                .expect("host skill discovery opt-out should be configurable");
        });
    let test = builder.build_with_auto_env(&server).await?;
    let environment = test.executor_environment().selection().clone();
    let executor_skill_root = SelectedCapabilityRoot {
        id: EXECUTOR_ROOT_ID.to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: environment.environment_id.clone(),
            path: environment.cwd.join("selected-capabilities")?,
        },
    };
    let mut thread_extension_init = ExtensionDataInit::default();
    thread_extension_init.insert(vec![executor_skill_root]);
    let mut executor_config = test.config.clone();
    executor_config.model_provider.base_url = Some(format!("{}/v1", websocket_server.uri()));
    executor_config.model_provider.supports_websockets = true;
    let executor_thread = test
        .thread_manager
        .start_thread(StartThreadOptions {
            // Keep the trace fixture's legacy mode: paginated SQLite workers can close
            // spans through a different subscriber than this test's scoped collector.
            history_mode: Some(codex_protocol::protocol::ThreadHistoryMode::Legacy),
            environments: Some(vec![environment.clone()]),
            thread_extension_init,
            ..StartThreadOptions::new(executor_config)
        })
        .await?;
    let executor_skill_path = environment
        .cwd
        .join("selected-capabilities")?
        .join(EXECUTOR_SKILL_NAME)?
        .join("SKILL.md")?;
    let normalized_executor_skill_path = executor_skill_path
        .inferred_native_path_string()
        .replace('\\', "/");
    let normalized_executor_skill_path = normalized_executor_skill_path.trim_start_matches('/');
    let executor_skill_locator =
        format!("skill://{EXECUTOR_ROOT_ID}/{normalized_executor_skill_path}");

    let prewarm = tokio::time::timeout(
        Duration::from_secs(10),
        websocket_server.wait_for_request(/*connection_index*/ 0, /*request_index*/ 0),
    )
    .await?
    .body_json();
    assert_eq!(prewarm["generate"].as_bool(), Some(false));

    // Prewarm already materialized this file through DiscoverV1. Both turns must use that
    // snapshot for catalog metadata and instruction reads instead of rescanning the executor.
    test.fs()
        .remove(
            &executor_skill_path,
            RemoveOptions {
                recursive: false,
                force: false,
                follow_symlinks: true,
            },
            /*sandbox*/ None,
        )
        .await?;

    for turn in ["first", "next"] {
        executor_thread
            .thread
            .start_or_steer_turn(TurnInputRequest::user_input(vec![
                UserInput::Text {
                    text: format!(
                        "For the {turn} turn, use ${AMBIENT_SKILL_NAME}, ${HOST_SKILL_NAME}, and ${EXECUTOR_SKILL_NAME}."
                    ),
                    text_elements: Vec::new(),
                },
                UserInput::Skill {
                    name: HOST_SKILL_NAME.to_string(),
                    path: host_skill_path.clone(),
                },
                // Executor skills are authority-scoped resources, not host filesystem paths.
                UserInput::Mention {
                    name: EXECUTOR_SKILL_NAME.to_string(),
                    path: executor_skill_locator.clone(),
                },
            ]))
            .await?;
        core_test_support::wait_for_event(&executor_thread.thread, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
    }

    let requests = websocket_server.single_connection();
    assert_eq!(requests.len(), 3);
    for (index, request) in requests.iter().skip(1).enumerate() {
        let request = request.body_json();
        let message_text = |role| {
            request["input"]
                .as_array()
                .expect("response.create input array")
                .iter()
                .filter(|item| item["role"].as_str() == Some(role))
                .flat_map(|item| item["content"].as_array().into_iter().flatten())
                .filter_map(|part| part["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n")
        };
        let developer_text = message_text("developer");
        let user_text = message_text("user");
        // Later websocket requests can omit previously sent developer context.
        if index == 0 {
            assert!(
                developer_text.contains(EXECUTOR_SKILL_NAME),
                "selected executor skill should remain advertised: {developer_text}"
            );
        }
        assert!(
            user_text.contains(&format!("<skill>\n<name>{EXECUTOR_SKILL_NAME}</name>"))
                && user_text.contains(EXECUTOR_SKILL_BODY),
            "turn {index} should inject the discovered executor skill: {user_text}"
        );
        assert!(
            !developer_text.contains(AMBIENT_SKILL_NAME)
                && !developer_text.contains(HOST_SKILL_NAME)
                && !user_text.contains(AMBIENT_SKILL_BODY)
                && !user_text.contains(HOST_SKILL_DESCRIPTION)
                && !user_text.contains(&format!("<skill>\n<name>{AMBIENT_SKILL_NAME}</name>"))
                && !user_text.contains(&format!("<skill>\n<name>{HOST_SKILL_NAME}</name>")),
            "turn {index} must not load repository or non-repository host skills; developer: {developer_text}; user: {user_text}"
        );
    }

    executor_thread.thread.shutdown_and_wait().await?;
    test.codex.shutdown_and_wait().await?;
    websocket_server.shutdown().await;

    let logs = String::from_utf8(buffer.lock().unwrap().clone())?;
    for span in [
        "startup_prewarm",
        "turn_context.build",
        "capability_roots.snapshot_for_step",
        "skills.executor.refresh_executor_catalog",
    ] {
        assert!(
            logs.contains(span),
            "expected positive trace control {span}"
        );
    }
    assert!(
        !logs.contains("skills_for_config"),
        "session startup, prewarm, and both turns must bypass host discovery: {logs}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn executor_only_provider_preserves_structured_repo_skill_without_discovery_opt_out()
-> Result<()> {
    skip_if_wine_exec!(
        Ok(()),
        "structured host skill inputs require matching host and executor path conventions"
    );

    const AMBIENT_SKILL_NAME: &str = "ambient-repo";
    const AMBIENT_SKILL_BODY: &str = "AMBIENT_REPO_SKILL_REMAINS_AVAILABLE";

    let server = responses::start_mock_server().await;
    let response = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    install_with_providers(
        &mut extensions,
        SkillProviders::new().with_executor_provider(Arc::new(
            ExecutorSkillProvider::new_with_restriction_product(
                Arc::new(EnvironmentManager::default_for_tests()),
                /*restriction_product*/ None,
            ),
        )),
        |config: &Config| SkillsExtensionConfig {
            include_instructions: config.include_skill_instructions,
            max_context_tokens: config.skill_max_context_tokens,
            bundled_skills_enabled: false,
            cloud_skill_enabled: false,
            shadow_selection_enabled: false,
        },
    );
    let mut builder = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_workspace_setup(|cwd, fs| async move {
            let skill_dir = cwd.join(".agents/skills").join(AMBIENT_SKILL_NAME);
            fs.create_directory(
                &PathUri::from_host_native_path(&skill_dir)?,
                CreateDirectoryOptions {
                    recursive: true,
                    follow_symlinks: true,
                },
                /*sandbox*/ None,
            )
            .await?;
            fs.write_file(
                &PathUri::from_host_native_path(skill_dir.join("SKILL.md"))?,
                format!(
                    "---\nname: {AMBIENT_SKILL_NAME}\ndescription: Ambient repository skill.\n---\n\n{AMBIENT_SKILL_BODY}\n"
                )
                .into_bytes(),
                Default::default(),
                /*sandbox*/ None,
            )
            .await?;
            Ok(())
        })
        .with_config(configure_catalog_test);
    let test = builder.build_with_auto_env(&server).await?;
    assert!(
        test.codex
            .inspect_selected_capability_roots()
            .ready_roots
            .is_empty(),
        "desktop sessions without selected capability roots must retain repository skills"
    );
    let ambient_skill_path = test
        .executor_environment()
        .cwd()
        .join(".agents/skills")
        .join(AMBIENT_SKILL_NAME)
        .join("SKILL.md")
        .to_path_buf();
    let ambient_skill_path = test
        .fs()
        .canonicalize(
            &PathUri::from_host_native_path(&ambient_skill_path)?,
            /*sandbox*/ None,
        )
        .await?
        .to_abs_path()?
        .to_path_buf();

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![
            UserInput::Text {
                text: format!("Use ${AMBIENT_SKILL_NAME}."),
                text_elements: Vec::new(),
            },
            UserInput::Skill {
                name: AMBIENT_SKILL_NAME.to_string(),
                path: ambient_skill_path,
            },
        ]))
        .await?;
    core_test_support::wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let request = response.single_request();
    let user_text = request.message_input_texts("user").join("\n");
    assert!(
        user_text.contains(&format!("<skill>\n<name>{AMBIENT_SKILL_NAME}</name>"))
            && user_text.contains(AMBIENT_SKILL_BODY),
        "desktop's structured absolute-path skill selection must remain available: {user_text}"
    );

    Ok(())
}

#[derive(Clone, Copy)]
enum ExecutorReferenceRead {
    Allowed,
    Denied,
    Continuation,
    EditedContinuation,
    DeniedContinuation,
}

/// Live references and continuation pages use current permissions, including after discovery
/// materialized the main prompt. Continuations preserve the cached snapshot across file edits.
#[test_case(ExecutorReferenceRead::Allowed; "full disk read")]
#[test_case(ExecutorReferenceRead::Denied; "denied reference")]
#[test_case(ExecutorReferenceRead::Continuation; "unchanged continuation")]
#[test_case(ExecutorReferenceRead::EditedContinuation; "edited continuation")]
#[test_case(ExecutorReferenceRead::DeniedContinuation; "denied continuation")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn executor_skill_tool_reads_references_under_current_permissions(
    read: ExecutorReferenceRead,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    if matches!(
        read,
        ExecutorReferenceRead::Denied | ExecutorReferenceRead::DeniedContinuation
    ) {
        skip_if_target_windows!(
            Ok(()),
            "restricted reads require the elevated Windows sandbox backend, unavailable in this fixture"
        );
    }

    const REFERENCE_CONTENTS: &str = "Live executor reference instructions.";
    let contents = match read {
        ExecutorReferenceRead::Allowed | ExecutorReferenceRead::Denied => {
            REFERENCE_CONTENTS.to_string()
        }
        ExecutorReferenceRead::Continuation
        | ExecutorReferenceRead::EditedContinuation
        | ExecutorReferenceRead::DeniedContinuation => "reference line\n".repeat(800),
    };
    let reference_access = match read {
        ExecutorReferenceRead::Denied => FileSystemAccessMode::Deny,
        ExecutorReferenceRead::Allowed
        | ExecutorReferenceRead::Continuation
        | ExecutorReferenceRead::EditedContinuation
        | ExecutorReferenceRead::DeniedContinuation => FileSystemAccessMode::Read,
    };
    let server = responses::start_mock_server().await;
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    install_with_providers(
        &mut extensions,
        SkillProviders::new().with_executor_provider(Arc::new(
            ExecutorSkillProvider::new_with_restriction_product(
                Arc::new(EnvironmentManager::default_for_tests()),
                /*restriction_product*/ None,
            ),
        )),
        |config: &Config| SkillsExtensionConfig {
            include_instructions: config.include_skill_instructions,
            max_context_tokens: config.skill_max_context_tokens,
            bundled_skills_enabled: false,
            cloud_skill_enabled: false,
            shadow_selection_enabled: false,
        },
    );
    let mut builder = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_model_info_override("gpt-5.4", |model_info| {
            model_info.truncation_policy = TruncationPolicyConfig::bytes(/*limit*/ 8192);
        })
        .with_config(configure_catalog_test);
    let test = builder.build_with_auto_env(&server).await?;
    let selection = test
        .codex
        .environment_selections()
        .await
        .into_iter()
        .next()
        .expect("thread should select an executor environment");
    let file_system = test.fs();
    let skill_dir = selection.cwd.join("skill")?;
    file_system
        .create_directory(
            &skill_dir,
            CreateDirectoryOptions {
                recursive: true,
                follow_symlinks: true,
            },
            /*sandbox*/ None,
        )
        .await?;
    // Discovery retains the selected root's spelling, including macOS /var aliases.
    // Only permission entries should use the canonical path.
    let policy_skill_dir = file_system
        .canonicalize(&skill_dir, /*sandbox*/ None)
        .await?;
    for (name, contents) in [
        (
            "SKILL.md",
            "---\nname: skill\ndescription: Read executor references.\n---\n\nRead reference.md.\n",
        ),
        ("reference.md", contents.as_str()),
    ] {
        file_system
            .write_file(
                &skill_dir.join(name)?,
                contents.as_bytes().to_vec(),
                Default::default(),
                /*sandbox*/ None,
            )
            .await?;
    }
    let package = format!(
        "skill://reference-root/{}",
        skill_dir
            .inferred_native_path_string()
            .replace('\\', "/")
            .trim_start_matches('/')
    );
    let resource = format!("{package}/reference.md");
    let mut thread_extension_init = ExtensionDataInit::new();
    thread_extension_init.insert(vec![SelectedCapabilityRoot {
        id: "reference-root".to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: selection.environment_id.clone(),
            path: selection.cwd.clone(),
        },
    }]);
    let mut config = test.config.clone();
    config
        .permissions
        .set_permission_profile(PermissionProfile::from_runtime_permissions(
            &FileSystemSandboxPolicy::restricted(vec![
                FileSystemSandboxEntry::new(
                    FileSystemPath::Special {
                        value: FileSystemSpecialPath::Root,
                    },
                    FileSystemAccessMode::Read,
                ),
                FileSystemSandboxEntry::new(
                    FileSystemPath::Path {
                        path: policy_skill_dir.join("reference.md")?,
                    },
                    reference_access,
                ),
            ]),
            NetworkSandboxPolicy::Restricted,
        ))?;
    let thread = test
        .thread_manager
        .start_thread(StartThreadOptions {
            environments: Some(vec![selection]),
            thread_extension_init,
            ..StartThreadOptions::new(config)
        })
        .await?;
    let response = responses::mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                responses::ev_function_call_with_namespace(
                    "read-reference",
                    "skills",
                    "read",
                    &json!({ "package": package, "resource": resource }).to_string(),
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
        ],
    )
    .await;
    thread
        .thread
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Read the executor skill reference.".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&thread.thread, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let output = response.requests()[1]
        .function_call_output_text("read-reference")
        .expect("skills.read should return a tool result");
    if matches!(read, ExecutorReferenceRead::Denied) {
        assert_eq!(output, "failed to read skill resource");
    } else if matches!(read, ExecutorReferenceRead::Allowed) {
        assert_eq!(
            serde_json::from_str::<Value>(&output)?,
            json!({
                "resource": resource,
                "contents": contents,
                "skill_root": skill_dir.inferred_native_path_string(),
                "next_cursor": null,
            })
        );
    } else {
        let page: Value = serde_json::from_str(&output)?;
        let cursor = page["next_cursor"]
            .as_str()
            .expect("reference needs another page");
        let first_contents = page["contents"].as_str().expect("first page contents");
        assert_eq!(
            page,
            json!({
                "resource": resource,
                "contents": &contents[..first_contents.len()],
                "skill_root": skill_dir.inferred_native_path_string(),
                "next_cursor": cursor,
            })
        );
        if matches!(read, ExecutorReferenceRead::EditedContinuation) {
            file_system
                .write_file(
                    &skill_dir.join("reference.md")?,
                    b"edited reference".to_vec(),
                    Default::default(),
                    /*sandbox*/ None,
                )
                .await?;
        }
        let response = responses::mount_sse_sequence(
            &server,
            vec![
                sse(vec![
                    ev_response_created("resp-3"),
                    responses::ev_function_call_with_namespace(
                        "continue-reference",
                        "skills",
                        "read",
                        &json!({"package": package, "resource": resource, "cursor": cursor})
                            .to_string(),
                    ),
                    ev_completed("resp-3"),
                ]),
                sse(vec![ev_response_created("resp-4"), ev_completed("resp-4")]),
            ],
        )
        .await;
        let mut input = TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Read the next page of the executor reference.".to_string(),
            text_elements: Vec::new(),
        }]);
        if matches!(read, ExecutorReferenceRead::DeniedContinuation) {
            let permissions = PermissionProfile::from_runtime_permissions(
                &FileSystemSandboxPolicy::restricted(vec![
                    FileSystemSandboxEntry::new(
                        FileSystemPath::Special {
                            value: FileSystemSpecialPath::Root,
                        },
                        FileSystemAccessMode::Read,
                    ),
                    FileSystemSandboxEntry::new(
                        FileSystemPath::Path {
                            path: policy_skill_dir.join("reference.md")?,
                        },
                        FileSystemAccessMode::Deny,
                    ),
                ]),
                NetworkSandboxPolicy::Restricted,
            );
            let (sandbox_policy, permission_profile) =
                turn_permission_fields(permissions, test.config.cwd.as_path());
            input = input.with_thread_settings(ThreadSettingsOverrides {
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                ..Default::default()
            });
        }
        thread.thread.start_or_steer_turn(input).await?;
        wait_for_event(&thread.thread, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
        let output = response.requests()[1]
            .function_call_output_text("continue-reference")
            .expect("skills.read continuation should return a tool result");
        match read {
            ExecutorReferenceRead::Continuation | ExecutorReferenceRead::EditedContinuation => {
                assert_eq!(
                    serde_json::from_str::<Value>(&output)?,
                    json!({
                        "resource": resource,
                        "contents": &contents[first_contents.len()..],
                        "skill_root": skill_dir.inferred_native_path_string(),
                        "next_cursor": null,
                    })
                )
            }
            ExecutorReferenceRead::DeniedContinuation => {
                assert_eq!(output, "failed to read skill resource")
            }
            ExecutorReferenceRead::Allowed | ExecutorReferenceRead::Denied => {
                unreachable!("single-page cases handled above")
            }
        }
    }
    Ok(())
}

/// Live executor prompts reject oversized resources without rewriting earlier injected content.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_executor_skill_prompt_rejects_oversized_resource() -> Result<()> {
    skip_if_no_network!(Ok(()));

    const FIRST_BODY: &str = "Instructions from the allowed live read.";
    let server = responses::start_mock_server().await;
    let environment = test_env().await?;
    let environment_id = environment.selection().environment_id.clone();
    let skill_path = environment.selection().cwd.join("SKILL.md")?;
    let file_system = environment.environment().get_filesystem();
    file_system
        .write_file(
            &skill_path,
            FIRST_BODY.as_bytes().to_vec(),
            Default::default(),
            /*sandbox*/ None,
        )
        .await?;
    let catalog = SkillCatalog {
        entries: vec![SkillCatalogEntry::new(
            SkillPackageId("skill://prompt-limit/live".to_string()),
            SkillAuthority::new(SkillSourceKind::Executor, "prompt-limit"),
            "live",
            "Read executor instructions.",
            SkillResourceId::environment(
                "skill://prompt-limit/live/SKILL.md",
                &environment_id,
                skill_path.clone(),
            ),
        )],
        warnings: Vec::new(),
    };
    let environment_manager = match environment.exec_server_url() {
        Some(url) => {
            EnvironmentManager::create_for_tests(
                Some(url.to_string()),
                /*local_runtime_paths*/ None,
            )
            .await
        }
        None => EnvironmentManager::default_for_tests(),
    };
    let (event_tx, event_rx) = std::sync::mpsc::channel();
    let mut extensions =
        ExtensionRegistryBuilder::<Config>::with_event_sink(Arc::new(ChannelEventSink(event_tx)));
    install_with_providers(
        &mut extensions,
        SkillProviders::new()
            .with_executor_provider(Arc::new(CatalogSkillProvider { catalog }))
            .with_executor_provider(Arc::new(
                ExecutorSkillProvider::new_with_restriction_product(
                    Arc::new(environment_manager),
                    /*restriction_product*/ None,
                ),
            )),
        |config: &Config| SkillsExtensionConfig {
            include_instructions: config.include_skill_instructions,
            max_context_tokens: config.skill_max_context_tokens,
            bundled_skills_enabled: false,
            cloud_skill_enabled: false,
            shadow_selection_enabled: false,
        },
    );
    let mut builder = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_config(configure_catalog_test);
    let test = builder.build_with_environment(&server, environment).await?;
    let first_response = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    test.submit_turn("$live").await?;
    let expected_prompts = vec![format!(
        "<skill>\n<name>live</name>\n<path>skill://prompt-limit/live/SKILL.md</path>\n{FIRST_BODY}\n</skill>"
    )];
    let first_prompts = first_response
        .single_request()
        .message_input_texts("user")
        .into_iter()
        .filter(|text| text.starts_with("<skill>"))
        .collect::<Vec<_>>();
    assert_eq!(first_prompts, expected_prompts);

    file_system
        .write_file(
            &skill_path,
            vec![b'x'; 1024 * 1024 + 1],
            Default::default(),
            /*sandbox*/ None,
        )
        .await?;
    let second_response = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
    )
    .await;
    test.submit_turn("$live").await?;
    let prompts = second_response
        .single_request()
        .message_input_texts("user")
        .into_iter()
        .filter(|text| text.starts_with("<skill>"))
        .collect::<Vec<_>>();
    assert_eq!(prompts, expected_prompts);
    let warnings = event_rx
        .try_iter()
        .filter_map(|event| match event {
            CapturedExtensionEvent::Warning(warning) => Some(warning.message),
            CapturedExtensionEvent::Event(_) => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(warnings, vec![
        "Failed to load skill `live`: executor skill resource skill://prompt-limit/live/SKILL.md exceeds 1048576 bytes".to_string()
    ]);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn executor_skill_invocation_is_environment_scoped_and_deduplicated() -> Result<()> {
    skip_if_remote!(Ok(()), "executor fixture uses a host-local skill path");
    skip_if_no_network!(Ok(()));

    const SELECTED_RESOURCE: &str = "skill://selected-root/demo/SKILL.md";

    let server = responses::start_mock_server().await;
    let codex_home = Arc::new(TempDir::new()?);
    let skill_path = codex_home.path().join("executor-skill/SKILL.md");
    std::fs::create_dir_all(
        skill_path
            .parent()
            .expect("skill path should have a parent"),
    )?;
    std::fs::write(&skill_path, "executor skill contents\n")?;
    let skill_uri = PathUri::from_host_native_path(&skill_path)?;
    let catalog = SkillCatalog {
        entries: vec![
            SkillCatalogEntry::new(
                SkillPackageId("skill://other-root/demo".to_string()),
                SkillAuthority::new(SkillSourceKind::Executor, "other-root"),
                "other-environment-skill",
                "Skill from another environment.",
                SkillResourceId::environment(
                    "skill://other-root/demo/SKILL.md",
                    "other-environment",
                    skill_uri.clone(),
                ),
            ),
            SkillCatalogEntry::new(
                SkillPackageId("skill://selected-root/demo".to_string()),
                SkillAuthority::new(SkillSourceKind::Executor, "selected-root"),
                "selected-environment-skill",
                "Skill from the selected environment.",
                SkillResourceId::environment(SELECTED_RESOURCE, LOCAL_ENVIRONMENT_ID, skill_uri),
            ),
        ],
        warnings: Vec::new(),
    };
    let read_command = if cfg!(windows) {
        format!("Get-Content -LiteralPath \"{}\"", skill_path.display())
    } else {
        format!("cat {}", skill_path.display())
    };
    let command = json!({
        "cmd": read_command,
        "login": false,
    })
    .to_string();
    let response = responses::mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                responses::ev_function_call("executor-skill-first", "exec_command", &command),
                responses::ev_function_call("executor-skill-again", "exec_command", &command),
                ev_completed("resp-1"),
            ]),
            sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
        ],
    )
    .await;

    let (extensions, _) = catalog_extensions(catalog, /*include_host_provider*/ false);
    let chatgpt_base_url = server.uri();
    let mut builder = test_codex()
        .with_home(codex_home)
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_extensions(extensions)
        .with_config(move |config| {
            configure_catalog_test(config);
            config.chatgpt_base_url = chatgpt_base_url;
        });
    let test = builder.build_with_auto_env(&server).await?;
    test.submit_turn("Read the executor skill twice.").await?;

    for call_id in ["executor-skill-first", "executor-skill-again"] {
        let output = response
            .function_call_output_text(call_id)
            .expect("executor skill command should return output");
        assert!(
            output.contains("executor skill contents"),
            "command output: {output}"
        );
    }

    let events = wait_for_analytics_events(&server, "skill_invocation", /*expected_count*/ 1).await;
    assert_eq!(events.len(), 1, "executor skill should be counted once");
    assert_eq!(events[0]["skill_name"], "selected-environment-skill");
    assert_eq!(
        events[0]["skill_id"],
        format!("{:x}", sha1::Sha1::digest(SELECTED_RESOURCE.as_bytes()))
    );
    assert_eq!(events[0]["event_params"]["invoke_type"], "implicit");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn production_turn_aliases_combined_skill_catalogs_under_shared_budget() -> Result<()> {
    const EXECUTOR_ROOT: &str =
        "skill://integration-executor/workspace/plugins/cache/executor-plugin/1.0.0/skills";
    const CLOUD_ROOT: &str = "skill://plugin_connector_1p_2330815c823c8191941e5dc465bb899f";

    let server = responses::start_mock_server().await;
    let response = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let codex_home = Arc::new(
        tempfile::Builder::new()
            .prefix("codex-integration-shared-skill-catalog-roots-")
            .tempdir()?,
    );
    write_host_skills(
        codex_home.path(),
        &[
            ("host-search", "Inspect host resources."),
            ("host-review", "Review host resources."),
            ("host-summarize", "Summarize host resources."),
        ],
    )?;

    let resource_catalog =
        |kind: SkillSourceKind, authority: &str, root: &str, prefix: &str| SkillCatalog {
            entries: ["search", "review", "summarize"]
                .into_iter()
                .map(|suffix| {
                    let name = format!("{prefix}-{suffix}");
                    let resource = format!("{root}/{name}/SKILL.md");
                    SkillCatalogEntry::new(
                        SkillPackageId(format!("{root}/{name}")),
                        SkillAuthority::new(kind.clone(), authority),
                        name,
                        "Inspect provider resources.",
                        SkillResourceId::new(resource.clone()),
                    )
                    .with_display_path(resource)
                    .with_alias_root(root)
                })
                .collect(),
            warnings: Vec::new(),
        };

    let mut extensions = ExtensionRegistryBuilder::new();
    install_with_providers(
        &mut extensions,
        SkillProviders::new()
            .with_executor_provider(Arc::new(CatalogSkillProvider {
                catalog: resource_catalog(
                    SkillSourceKind::Executor,
                    "integration-executor",
                    EXECUTOR_ROOT,
                    "executor",
                ),
            }))
            .with_cloud_provider(Arc::new(CatalogSkillProvider {
                catalog: resource_catalog(
                    SkillSourceKind::Cloud,
                    "integration-cloud",
                    CLOUD_ROOT,
                    "cloud",
                ),
            }))
            .with_host_provider(Arc::new(HostSkillProvider::new())),
        |config: &Config| SkillsExtensionConfig {
            include_instructions: config.include_skill_instructions,
            max_context_tokens: config.skill_max_context_tokens,
            bundled_skills_enabled: false,
            cloud_skill_enabled: true,
            shadow_selection_enabled: false,
        },
    );
    let mut builder = test_codex()
        .with_home(Arc::clone(&codex_home))
        .with_exec_server_url("none")
        .with_extensions(Arc::new(extensions.build()))
        .with_model_info_override("gpt-5.5", |model_info| {
            model_info.context_window = Some(10_000);
            model_info.max_context_window = None;
        })
        .with_config(|config| {
            configure_catalog_test(config);
            config.cloud_skill_enabled = true;
        });
    let test = builder.build_with_auto_env(&server).await?;

    test.submit_turn("Inspect the available skills.").await?;

    let developer_text = response
        .single_request()
        .message_input_texts("developer")
        .join("\n");
    let host_root = dunce::canonicalize(codex_home.path().join("skills"))?
        .to_string_lossy()
        .replace('\\', "/");
    for (alias, root) in [
        ("e0", EXECUTOR_ROOT),
        ("c0", CLOUD_ROOT),
        ("r0", host_root.as_str()),
    ] {
        assert!(
            developer_text.contains(&format!("- `{alias}` = `{root}`")),
            "model request should include the {alias} skill root: {developer_text}"
        );
    }
    for (source, alias, prefix, suffix) in [
        ("executor package", "e0", "executor", ""),
        ("cloud package", "c0", "cloud", ""),
        ("file", "r0", "host", "/SKILL.md"),
    ] {
        for name in ["search", "review", "summarize"] {
            assert!(
                developer_text.contains(&format!("{source}: {alias}/{prefix}-{name}{suffix}")),
                "model request should retain the aliased {prefix}-{name} skill: {developer_text}"
            );
        }
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial]
async fn production_turn_scales_extension_catalog_from_resolved_model_window() -> Result<()> {
    assert_catalog_model_switch(/*max_context_tokens*/ None).await
}

async fn assert_catalog_model_switch(max_context_tokens: Option<usize>) -> Result<()> {
    const MODEL_A: &str = "skills-model-a";
    const MODEL_B: &str = "skills-model-b";
    let server = responses::start_mock_server().await;
    let response = responses::mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-a"),
                responses::ev_function_call(
                    "plan",
                    "update_plan",
                    &json!({
                        "plan": [{"step": "Inspect skills", "status": "completed"}]
                    })
                    .to_string(),
                ),
                ev_completed("resp-a"),
            ]),
            sse(vec![ev_response_created("resp-b"), ev_completed("resp-b")]),
        ],
    )
    .await;
    let codex_home = Arc::new(TempDir::new()?);
    // Use the normal metrics sink to verify core's model attribution.
    let telemetry = OtelProvider::try_new(&OtelSettings {
        http_client_factory: codex_core::test_support::default_http_client_factory(),
        environment: "test".to_string(),
        service_name: "skills-model-switch".to_string(),
        service_version: env!("CARGO_PKG_VERSION").to_string(),
        codex_home: codex_home.path().to_path_buf(),
        exporter: OtelExporter::None,
        trace_exporter: OtelExporter::None,
        metrics_exporter: OtelExporter::OtlpHttp {
            endpoint: format!("{}/metrics", server.uri()),
            headers: Default::default(),
            protocol: OtelHttpProtocol::Json,
            tls: None,
        },
        runtime_metrics: true,
        span_attributes: Default::default(),
        tracestate: Default::default(),
    })
    .map_err(|error| anyhow::anyhow!("{error}"))?
    .expect("metrics provider");
    let skill_count = 800;
    let catalog = SkillCatalog {
        entries: (0..skill_count)
            .map(|index| {
                let name = format!("skill-{index:03}");
                SkillCatalogEntry::new(
                    SkillPackageId(format!("test/{name}")),
                    SkillAuthority::new(SkillSourceKind::Host, "test"),
                    name.clone(),
                    "A description long enough to keep the catalog under sustained budget pressure.",
                    SkillResourceId::new(format!("{name}/SKILL.md")),
                )
                .with_display_path(format!("skill://test/{name}/SKILL.md"))
            })
            .collect(),
        warnings: Vec::new(),
    };
    let (started, mut discovery) = tokio::sync::mpsc::unbounded_channel();
    let provider = Arc::new(PausedCatalogSkillProvider {
        inner: CatalogSkillProvider { catalog },
        first_discovery: AtomicBool::new(/*v*/ true),
        started,
        resume: tokio::sync::Semaphore::new(/*permits*/ 0),
    });
    let (event_tx, event_rx) = std::sync::mpsc::channel();
    let mut extensions =
        ExtensionRegistryBuilder::<Config>::with_event_sink(Arc::new(ChannelEventSink(event_tx)));
    install_with_providers(
        &mut extensions,
        SkillProviders::new().with_host_provider(provider.clone()),
        |config: &Config| SkillsExtensionConfig {
            include_instructions: config.include_skill_instructions,
            max_context_tokens: config.skill_max_context_tokens,
            bundled_skills_enabled: false,
            cloud_skill_enabled: false,
            shadow_selection_enabled: false,
        },
    );
    let mut builder = test_codex()
        .with_home(codex_home)
        .with_model(MODEL_A)
        .with_extensions(Arc::new(extensions.build()))
        .with_config(move |config| {
            configure_catalog_test(config);
            config
                .features
                .enable(Feature::StepModelSwitching)
                .expect("model switching");
            config.skill_max_context_tokens =
                max_context_tokens.and_then(std::num::NonZeroUsize::new);
            let template = bundled_models_response()
                .expect("bundled models")
                .models
                .into_iter()
                .find(|model| model.slug == "gpt-5.5")
                .expect("gpt-5.5");
            config.model_catalog = Some(ModelsResponse {
                models: [
                    (MODEL_A, Some(10_000), None, false),
                    (MODEL_B, None, Some(400_000), true),
                ]
                .into_iter()
                .map(|(slug, window, max_window, usage)| {
                    let mut model = template.clone();
                    model.slug = slug.to_string();
                    model.context_window = window;
                    model.max_context_window = max_window;
                    // Distinguish the resolved window from the smaller usable limit.
                    model.effective_context_window_percent = 50;
                    model.include_skills_usage_instructions = usage;
                    model
                })
                .collect(),
            });
        });
    let test = builder.build_with_auto_env(&server).await?;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Inspect the available skills.".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let turn_id = tokio::time::timeout(Duration::from_secs(/*secs*/ 10), discovery.recv())
        .await?
        .expect("A catalog discovery started");
    let (reply, outcome) = tokio::sync::oneshot::channel();
    test.codex
        .submit(Op::TurnSettings {
            turn_id,
            update: TurnSettingsUpdate {
                model: Some(MODEL_B.to_string()),
                ..Default::default()
            },
            reply,
        })
        .await?;
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(/*secs*/ 10), outcome).await??,
        TurnSettingsUpdateOutcome::Applied
    );
    provider.resume.add_permits(/*n*/ 1);
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = response.requests();
    assert_eq!(requests.len(), 2);
    let mut included_counts = Vec::new();
    for (request, (model, default_budget, include_usage)) in requests
        .iter()
        .zip([(MODEL_A, 200, false), (MODEL_B, 8_000, true)])
    {
        assert_eq!(request.body_json()["model"], model);
        let developer_texts = request.message_input_texts("developer");
        // B retains A in history and appends its own changed catalog.
        let catalog_text = developer_texts
            .iter()
            .rev()
            .find(|text| text.contains("skill://test/"))
            .expect("catalog in provider request");
        assert_eq!(
            catalog_text.contains("### How to use skills"),
            include_usage
        );
        let metadata_lines = catalog_text
            .lines()
            .skip_while(|line| *line != "### Available skills")
            .skip(/*n*/ 1)
            .take_while(|line| !line.starts_with("### "))
            .filter(|line| line.starts_with("- "))
            .collect::<Vec<_>>();
        let metadata_cost = metadata_lines
            .iter()
            .map(|line| approx_token_count(&format!("{line}\n")))
            .sum::<usize>();
        let budget = max_context_tokens.unwrap_or(default_budget);
        assert!(
            metadata_cost <= budget,
            "catalog cost {metadata_cost} exceeds {budget}"
        );
        assert!(
            metadata_cost > budget / 2,
            "catalog incorrectly used the usable inference limit"
        );
        let included_count = metadata_lines
            .iter()
            .filter(|line| line.starts_with("- skill-"))
            .count();
        assert!(included_count > 0 && included_count < skill_count);
        assert!(!catalog_text.contains(
            "A description long enough to keep the catalog under sustained budget pressure."
        ));
        included_counts.push(included_count);
    }
    if max_context_tokens.is_some() {
        assert_eq!(included_counts[0], included_counts[1]);
    } else {
        assert!(included_counts[0] < included_counts[1]);
    }
    let warnings = event_rx
        .try_iter()
        .map(|event| event.into_warning().message)
        .collect::<Vec<_>>();
    let mut expected_warnings = included_counts
        .iter()
        .map(|count| {
            format!(
                "Exceeded skills context budget. All skill descriptions were removed and {} additional skills were not included in the model-visible skills list.",
                skill_count - count,
            )
        })
        .collect::<Vec<_>>();
    expected_warnings.dedup();
    assert_eq!(warnings, expected_warnings);

    let snapshot = telemetry.metrics().expect("metrics client").snapshot()?;
    let metric = snapshot
        .scope_metrics()
        .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
        .find(|metric| metric.name() == THREAD_SKILLS_KEPT_TOTAL_METRIC)
        .expect("catalog metrics");
    let AggregatedMetrics::F64(MetricData::Histogram(histogram)) = metric.data() else {
        panic!("catalog metric should be a histogram");
    };
    let mut samples = histogram
        .data_points()
        .filter_map(|point| {
            let attributes = point
                .attributes()
                .map(|attribute| (attribute.key.as_str(), attribute.value.as_str().to_string()))
                .collect::<std::collections::BTreeMap<_, _>>();
            (attributes.get("catalog_surface").map(String::as_str) == Some("host_world_state")
                && matches!(
                    attributes.get("model").map(String::as_str),
                    Some(MODEL_A | MODEL_B)
                ))
            .then(|| (attributes["model"].clone(), point.count(), point.sum()))
        })
        .collect::<Vec<_>>();
    samples.sort_by(|left, right| left.0.cmp(&right.0));
    assert_eq!(
        samples,
        vec![
            (MODEL_A.to_string(), 1, included_counts[0] as f64),
            (MODEL_B.to_string(), 1, included_counts[1] as f64),
        ]
    );
    telemetry
        .shutdown_with_timeout(Duration::from_secs(/*secs*/ 5))
        .await?;
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
#[serial_test::serial]
async fn production_turn_uses_configured_skill_catalog_token_budget() -> Result<()> {
    let server = responses::start_mock_server().await;
    let response = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let codex_home = Arc::new(TempDir::new()?);
    std::fs::write(
        codex_home.path().join("config.toml"),
        "[skills]\nmax_context_tokens = 800\n",
    )?;
    write_host_skills(codex_home.path(), &HOST_CATALOG)?;
    let (extensions, _event_rx) = catalog_extensions(
        executor_catalog(&EXECUTOR_CATALOG),
        /*include_host_provider*/ true,
    );
    let mut builder = test_codex()
        .with_home(codex_home)
        .with_extensions(extensions)
        .with_model_info_override("gpt-5.5", |model_info| {
            model_info.context_window = Some(SHORTENING_CONTEXT_WINDOW);
            model_info.max_context_window = None;
        })
        .with_config(configure_catalog_test);
    let test = builder.build_with_auto_env(&server).await?;

    test.submit_turn("Inspect the available skills.").await?;
    let request = response.single_request();
    let developer_texts = request.message_input_texts("developer");
    let host_lines = skill_lines(catalog_text(&developer_texts, "host"), "host");
    let executor_lines = skill_lines(catalog_text(&developer_texts, "exec"), "exec");
    let combined_lines = host_lines
        .iter()
        .chain(executor_lines.iter())
        .copied()
        .collect::<Vec<_>>();

    assert_full_descriptions(&host_lines, &HOST_CATALOG);
    assert_full_descriptions(&executor_lines, &EXECUTOR_CATALOG);
    assert!(metadata_cost(&combined_lines) <= 800);

    assert_catalog_model_switch(Some(800)).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn production_turn_keeps_full_host_only_catalog_when_it_fits() -> Result<()> {
    let (developer_texts, _) =
        rendered_catalogs(&HOST_CATALOG, &[], FULL_CATALOG_CONTEXT_WINDOW).await?;
    let host_catalog = catalog_text(&developer_texts, "host");
    let host_lines = skill_lines(host_catalog, "host");

    assert_full_descriptions(&host_lines, &HOST_CATALOG);
    assert!(host_catalog.contains("### Skill roots"));
    assert!(host_catalog.contains("(file: r0/host-alpha/SKILL.md)"));

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
            max_context_tokens: config.skill_max_context_tokens,
            bundled_skills_enabled: false,
            cloud_skill_enabled: false,
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
    let app_mentioned_events =
        wait_for_analytics_events(&server, "codex_app_mentioned", /*expected_count*/ 1).await;
    let app_mentioned_event = &app_mentioned_events[0];
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
async fn production_turn_suppresses_only_the_superseded_host_skill_prompt() -> Result<()> {
    #[derive(Default)]
    struct SkillInvocationRecorder(Mutex<Vec<String>>);

    impl SkillInvocationContributor for SkillInvocationRecorder {
        fn on_skill_invocation<'a>(
            &'a self,
            input: SkillInvocationInput<'a>,
        ) -> ExtensionFuture<'a, ()> {
            Box::pin(async move {
                self.0
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push(input.skill_resource.to_owned());
            })
        }
    }

    let server = responses::start_mock_server().await;
    let response = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let codex_home = Arc::new(TempDir::new()?);
    write_host_skills(
        codex_home.path(),
        &[
            ("first-host", "First host skill."),
            ("second-host", "Second host skill."),
        ],
    )?;
    let first_skill_path = codex_home.path().join("skills/first-host/SKILL.md");
    let second_skill_path = codex_home.path().join("skills/second-host/SKILL.md");
    let first_host_contents =
        "---\nname: first-host\ndescription: First host skill.\n---\n\nFIRST_HOST_BODY\n";
    let second_host_contents =
        "---\nname: second-host\ndescription: Second host skill.\n---\n\nSECOND_HOST_BODY\n";
    std::fs::write(&first_skill_path, first_host_contents)?;
    std::fs::write(&second_skill_path, second_host_contents)?;
    let second_skill_path = dunce::canonicalize(second_skill_path)?;

    let source_kind = SkillSourceKind::Custom("test".to_string());
    let provider_resource = "skill://test/first-host/SKILL.md";
    let provider_contents = "FIRST_PROVIDER_BODY";
    let catalog = SkillCatalog {
        entries: vec![
            SkillCatalogEntry::new(
                SkillPackageId("test/first-host".to_string()),
                SkillAuthority::new(source_kind.clone(), "test"),
                "first-host",
                "Provider skill supersedes the matching host skill.",
                SkillResourceId::new(provider_resource),
            )
            .with_display_path(provider_resource),
        ],
        warnings: Vec::new(),
    };
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    let recorder = Arc::new(SkillInvocationRecorder::default());
    extensions.skill_invocation_contributor(recorder.clone());
    install_with_providers(
        &mut extensions,
        SkillProviders::new().with_provider(SkillProviderSource::new(
            source_kind,
            "test",
            Arc::new(StaticSkillProvider {
                catalog,
                main_prompt_contents: Some(provider_contents.to_string()),
            }),
        )),
        |config: &Config| SkillsExtensionConfig {
            include_instructions: config.include_skill_instructions,
            max_context_tokens: config.skill_max_context_tokens,
            bundled_skills_enabled: false,
            cloud_skill_enabled: false,
            shadow_selection_enabled: false,
        },
    );
    let mut builder = test_codex()
        .with_home(codex_home)
        .with_extensions(Arc::new(extensions.build()))
        .with_config(configure_catalog_test);
    let test = builder.build_with_auto_env(&server).await?;

    test.submit_turn("Use $first-host and $second-host.")
        .await?;

    let user_messages = response.single_request().message_input_texts("user");
    let skill_messages = user_messages
        .into_iter()
        .filter(|message| message.starts_with("<skill>"))
        .collect::<Vec<_>>();
    assert_eq!(
        skill_messages,
        vec![
            format!(
                "<skill>\n<name>second-host</name>\n<path>{}</path>\n{second_host_contents}\n</skill>",
                second_skill_path.display()
            ),
            format!(
                "<skill>\n<name>first-host</name>\n<path>{provider_resource}</path>\n{provider_contents}\n</skill>"
            ),
        ]
    );
    assert_eq!(
        *recorder
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        vec![second_skill_path.display().to_string()]
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn production_turn_warns_and_omits_unreadable_host_skill() -> Result<()> {
    let server = responses::start_mock_server().await;
    let response = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let codex_home = Arc::new(TempDir::new()?);
    write_host_skills(
        codex_home.path(),
        &[
            ("missing-host", "Missing host skill."),
            ("available-host", "Available host skill."),
        ],
    )?;
    let missing_skill_path =
        dunce::canonicalize(codex_home.path().join("skills/missing-host/SKILL.md"))?;
    let available_skill_path =
        dunce::canonicalize(codex_home.path().join("skills/available-host/SKILL.md"))?;
    let available_skill_contents = std::fs::read_to_string(&available_skill_path)?;
    let (extensions, _) =
        catalog_extensions(SkillCatalog::default(), /*include_host_provider*/ true);
    let mut builder = test_codex()
        .with_home(Arc::clone(&codex_home))
        .with_extensions(extensions)
        .with_config(configure_catalog_test);
    let test = builder.build_with_auto_env(&server).await?;

    std::fs::remove_file(&missing_skill_path)?;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![
            UserInput::Skill {
                name: "missing-host".to_string(),
                path: missing_skill_path.clone(),
            },
            UserInput::Skill {
                name: "available-host".to_string(),
                path: available_skill_path.clone(),
            },
        ]))
        .await?;

    let mut warnings = Vec::new();
    loop {
        match core_test_support::wait_for_event(&test.codex, |_| true).await {
            EventMsg::Warning(warning) => warnings.push(warning.message),
            EventMsg::TurnComplete(_) => break,
            _ => {}
        }
    }

    let expected_warning_prefix = format!(
        "Failed to load skill missing-host at {}:",
        missing_skill_path.display()
    );
    assert_eq!(warnings.len(), 1);
    assert!(
        warnings[0].starts_with(&expected_warning_prefix),
        "expected unreadable skill warning, got {warnings:?}"
    );

    let skill_messages = response
        .single_request()
        .message_input_texts("user")
        .into_iter()
        .filter(|message| message.starts_with("<skill>"))
        .collect::<Vec<_>>();
    assert_eq!(
        skill_messages,
        vec![format!(
            "<skill>\n<name>available-host</name>\n<path>{}</path>\n{available_skill_contents}\n</skill>",
            available_skill_path.display()
        )]
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
        .with_config(|config| {
            configure_catalog_test(config);
            config
                .features
                .enable(Feature::SkipHostSkillDiscovery)
                .expect("host skill provider must override the discovery opt-out");
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
async fn production_turn_keeps_cloud_world_state_incremental_across_turns() -> Result<()> {
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
    let skill_name = "cloud-search";
    let skill_description = "Search available company knowledge.";
    let skill_resource = "skill://codex_apps/cloud-search/SKILL.md";
    let catalog = SkillCatalog {
        entries: vec![
            SkillCatalogEntry::new(
                SkillPackageId("cloud/cloud-search".to_string()),
                SkillAuthority::new(SkillSourceKind::Cloud, CODEX_APPS_MCP_SERVER_NAME),
                skill_name,
                skill_description,
                SkillResourceId::new(skill_resource),
            )
            .with_display_path(skill_resource),
        ],
        warnings: Vec::new(),
    };
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    install_with_providers(
        &mut extensions,
        SkillProviders::new().with_cloud_provider(Arc::new(CatalogSkillProvider { catalog })),
        |config: &Config| SkillsExtensionConfig {
            include_instructions: config.include_skill_instructions,
            max_context_tokens: config.skill_max_context_tokens,
            bundled_skills_enabled: false,
            cloud_skill_enabled: config.cloud_skill_enabled,
            shadow_selection_enabled: false,
        },
    );
    let mut builder = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| {
            configure_catalog_test(config);
            config.cloud_skill_enabled = true;
        });
    let test = builder.build_with_auto_env(&server).await?;
    let cloud_thread = test
        .thread_manager
        .start_thread(StartThreadOptions {
            environments: Some(Vec::new()),
            ..StartThreadOptions::new(test.config.clone())
        })
        .await?;

    for prompt in [
        "Inspect the available skills.",
        "Inspect the available skills again.",
    ] {
        cloud_thread
            .thread
            .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
                text: prompt.to_string(),
                text_elements: Vec::new(),
            }]))
            .await?;
        core_test_support::wait_for_event(&cloud_thread.thread, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
    }

    let requests = response.requests();
    assert_eq!(requests.len(), 2);
    let expected_line =
        format!("- {skill_name}: {skill_description} (cloud package: cloud/cloud-search)");
    for (index, request) in requests.iter().enumerate() {
        let developer_texts = request.message_input_texts("developer");
        let occurrences = developer_texts
            .iter()
            .map(|text| text.matches(&expected_line).count())
            .sum::<usize>();
        assert_eq!(
            occurrences, 1,
            "request {index} should contain the cloud catalog exactly once: {developer_texts:?}"
        );
        assert!(
            developer_texts
                .iter()
                .any(|text| text.contains("Read a skill package directly with `skills.read"))
        );
    }

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

    assert_eq!(
        skill_names(&host_lines),
        vec!["host-alpha", "host-beta", "host-delta"]
    );
    assert_eq!(
        skill_names(&executor_lines),
        EXECUTOR_CATALOG
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<_>>()
    );
    assert!(warning_messages.contains(
        &"Exceeded skills context budget. All skill descriptions were removed and 1 additional skill was not included in the model-visible skills list."
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
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
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
            max_context_tokens: config.skill_max_context_tokens,
            bundled_skills_enabled: false,
            cloud_skill_enabled: false,
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
