use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use codex_core::config::Config;
use codex_core::config::ConfigBuilder;
use codex_core_skills::HostLoadedSkills;
use codex_core_skills::SkillsLoadInput;
use codex_core_skills::SkillsManager;
use codex_core_skills::injection::InjectedHostSkillPrompts;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::TurnInputContext;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_OPEN_TAG;
use codex_protocol::protocol::SessionSource;
use codex_protocol::user_input::UserInput;
use codex_skills_extension::SkillProviders;
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
use codex_skills_extension::provider::SkillProvider;
use codex_skills_extension::provider::SkillProviderFuture;
use codex_skills_extension::provider::SkillReadRequest;
use codex_skills_extension::provider::SkillSearchRequest;
use pretty_assertions::assert_eq;

type TestResult = Result<(), Box<dyn std::error::Error>>;

static NEXT_CODEX_HOME_ID: AtomicUsize = AtomicUsize::new(0);

#[tokio::test]
async fn installed_extension_loads_host_skills_from_legacy_roots() -> TestResult {
    let codex_home = test_codex_home();
    let skill_path = codex_home.join("skills").join("demo").join("SKILL.md");
    std::fs::create_dir_all(
        skill_path
            .parent()
            .ok_or("skill path should have a parent")?,
    )?;
    std::fs::write(
        &skill_path,
        "---\nname: demo\ndescription: Demo skill.\n---\n# Demo\n\nUse the demo skill.\n",
    )?;
    let config = ConfigBuilder::default()
        .codex_home(codex_home.clone())
        .fallback_cwd(Some(codex_home.clone()))
        .build()
        .await?;

    let mut builder = ExtensionRegistryBuilder::new();
    install(&mut builder);
    let registry = builder.build();
    let session_store = ExtensionData::new("session");
    let thread_store = ExtensionData::new("thread");
    let session_source = SessionSource::Cli;
    registry.thread_lifecycle_contributors()[0]
        .on_thread_start(ThreadStartInput {
            config: &config,
            session_source: &session_source,
            persistent_thread_state_available: true,
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await;

    let manager = SkillsManager::new(config.codex_home.clone(), config.bundled_skills_enabled());
    let input = SkillsLoadInput::new(
        config.cwd.clone(),
        Vec::new(),
        config.config_layer_stack.clone(),
        config.bundled_skills_enabled(),
    );
    let loaded_skills = Arc::new(manager.skills_for_config(&input, /*fs*/ None).await);
    let skill_path_string = loaded_skills
        .skills
        .iter()
        .find(|skill| skill.name == "demo")
        .ok_or("demo skill should load")?
        .path_to_skills_md
        .to_string_lossy()
        .into_owned();
    let skill_prompt_path = skill_path_string.replace('\\', "/");
    let turn_store = ExtensionData::new("turn-1");
    turn_store.insert(HostLoadedSkills::new(Arc::clone(&loaded_skills)));

    let fragments = registry.turn_input_contributors()[0]
        .contribute(
            TurnInputContext {
                turn_id: "turn-1".to_string(),
                user_input: vec![UserInput::Text {
                    text: "$demo".to_string(),
                    text_elements: Vec::new(),
                }],
                environments: Vec::new(),
            },
            &session_store,
            &thread_store,
            &turn_store,
        )
        .await;

    assert_eq!(2, fragments.len());
    assert_eq!("developer", fragments[0].role());
    assert!(fragments[0].render().contains("demo"));
    assert!(fragments[0].render().contains(&skill_prompt_path));
    assert_eq!("user", fragments[1].role());
    assert!(fragments[1].render().contains("<name>demo</name>"));
    assert!(fragments[1].render().contains("# Demo"));
    assert!(fragments[1].render().contains(&skill_prompt_path));
    let injected_host_skill_prompts = turn_store
        .get::<InjectedHostSkillPrompts>()
        .ok_or("host skill prompt marker should be set")?;
    assert!(injected_host_skill_prompts.contains_path(&skill_path_string));

    std::fs::remove_dir_all(codex_home)?;
    Ok(())
}

#[tokio::test]
async fn selected_executor_catalog_is_context_and_selected_entrypoint_is_turn_input() -> TestResult
{
    let read_requests = Arc::new(Mutex::new(Vec::new()));
    let executor_provider = Arc::new(StaticSkillProvider {
        catalog: SkillCatalog {
            entries: vec![test_entry(
                SkillSourceKind::Executor,
                "env-1",
                "executor/lint-fix",
                "lint-fix/SKILL.md",
            )],
            warnings: Vec::new(),
        },
        read_requests: Arc::clone(&read_requests),
        list_calls: None,
        fail_first_list: false,
    });
    let providers = SkillProviders::new().with_executor_provider(executor_provider);
    let mut builder = ExtensionRegistryBuilder::new();
    install_with_providers(&mut builder, providers);
    let registry = builder.build();

    let session_store = ExtensionData::new("session");
    let thread_store = ExtensionData::new("thread");
    thread_store.insert(vec![SelectedCapabilityRoot {
        id: "lint-fix".to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: "env-1".to_string(),
            path: "/skills/lint-fix".to_string(),
        },
    }]);
    let session_source = SessionSource::Cli;
    let config = default_config().await?;
    registry.thread_lifecycle_contributors()[0]
        .on_thread_start(ThreadStartInput {
            config: &config,
            session_source: &session_source,
            persistent_thread_state_available: true,
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await;

    let prompt_fragments = registry.context_contributors()[0]
        .contribute(&session_store, &thread_store)
        .await;
    assert_eq!(1, prompt_fragments.len());
    assert!(
        prompt_fragments[0]
            .text()
            .starts_with(SKILLS_INSTRUCTIONS_OPEN_TAG)
    );
    assert!(prompt_fragments[0].text().contains("lint-fix"));

    let turn_store = ExtensionData::new("turn-1");
    let fragments = registry.turn_input_contributors()[0]
        .contribute(
            TurnInputContext {
                turn_id: "turn-1".to_string(),
                user_input: vec![UserInput::Text {
                    text: "$lint-fix please".to_string(),
                    text_elements: Vec::new(),
                }],
                environments: Vec::new(),
            },
            &session_store,
            &thread_store,
            &turn_store,
        )
        .await;

    assert_eq!(1, fragments.len());
    assert_eq!("user", fragments[0].role());
    assert!(fragments[0].render().contains("<name>lint-fix</name>"));
    assert!(fragments[0].render().contains("# Lint Fix"));
    assert_eq!(
        vec![(
            SkillAuthority::new(SkillSourceKind::Executor, "env-1"),
            SkillPackageId("executor/lint-fix".to_string()),
            SkillResourceId::new("lint-fix/SKILL.md"),
        )],
        read_request_keys(&read_requests)
    );
    let rebuilt_prompt_fragments = registry.context_contributors()[0]
        .contribute(&session_store, &thread_store)
        .await;
    assert_eq!(1, rebuilt_prompt_fragments.len());
    assert!(rebuilt_prompt_fragments[0].text().contains("lint-fix"));

    let next_turn_store = ExtensionData::new("turn-2");
    let next_fragments = registry.turn_input_contributors()[0]
        .contribute(
            TurnInputContext {
                turn_id: "turn-2".to_string(),
                user_input: vec![UserInput::Text {
                    text: "no skill this time".to_string(),
                    text_elements: Vec::new(),
                }],
                environments: Vec::new(),
            },
            &session_store,
            &thread_store,
            &next_turn_store,
        )
        .await;

    assert!(next_fragments.is_empty());

    Ok(())
}

#[tokio::test]
async fn orchestrator_catalog_snapshot_retries_failure_then_is_reused() -> TestResult {
    let list_calls = Arc::new(AtomicUsize::new(0));
    let providers =
        SkillProviders::new().with_orchestrator_provider(Arc::new(StaticSkillProvider {
            catalog: SkillCatalog {
                entries: vec![test_entry(
                    SkillSourceKind::Orchestrator,
                    "codex_apps",
                    "orchestrator/first",
                    "skill://orchestrator/first/SKILL.md",
                )],
                warnings: Vec::new(),
            },
            read_requests: Arc::new(Mutex::new(Vec::new())),
            list_calls: Some(Arc::clone(&list_calls)),
            fail_first_list: true,
        }));
    let mut builder = ExtensionRegistryBuilder::new();
    install_with_providers(&mut builder, providers);
    let registry = builder.build();
    let session_store = ExtensionData::new("session");
    let thread_store = ExtensionData::new("thread");
    let session_source = SessionSource::Cli;
    let config = default_config().await?;
    registry.thread_lifecycle_contributors()[0]
        .on_thread_start(ThreadStartInput {
            config: &config,
            session_source: &session_source,
            persistent_thread_state_available: true,
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await;

    let initial_fragments = registry.context_contributors()[0]
        .contribute(&session_store, &thread_store)
        .await;
    assert!(initial_fragments.is_empty());

    for turn_id in ["turn-1", "turn-2"] {
        let fragments = registry.turn_input_contributors()[0]
            .contribute(
                TurnInputContext {
                    turn_id: turn_id.to_string(),
                    user_input: vec![UserInput::Text {
                        text: "$first".to_string(),
                        text_elements: Vec::new(),
                    }],
                    environments: Vec::new(),
                },
                &session_store,
                &thread_store,
                &ExtensionData::new(turn_id),
            )
            .await;
        assert_eq!(1, fragments.len());
        assert!(fragments[0].render().contains("<name>first</name>"));
    }
    assert_eq!(2, list_calls.load(Ordering::Relaxed));

    Ok(())
}

#[tokio::test]
async fn root_qualified_locator_selects_only_the_matching_executor_skill() -> TestResult {
    let read_requests = Arc::new(Mutex::new(Vec::new()));
    let root_a_locator = "skill://root-a/shared/lint-fix/SKILL.md";
    let root_b_locator = "skill://root-b/shared/lint-fix/SKILL.md";
    let executor_provider = Arc::new(StaticSkillProvider {
        catalog: SkillCatalog {
            entries: [("root-a", root_a_locator), ("root-b", root_b_locator)]
                .into_iter()
                .map(|(root_id, locator)| {
                    SkillCatalogEntry::new(
                        SkillPackageId(locator.to_string()),
                        SkillAuthority::new(SkillSourceKind::Executor, root_id),
                        "lint-fix",
                        "Fix lint errors.",
                        SkillResourceId::new(locator),
                    )
                    .with_display_path(locator)
                })
                .collect(),
            warnings: Vec::new(),
        },
        read_requests: Arc::clone(&read_requests),
        list_calls: None,
        fail_first_list: false,
    });
    let providers = SkillProviders::new().with_executor_provider(executor_provider);
    let mut builder = ExtensionRegistryBuilder::new();
    install_with_providers(&mut builder, providers);
    let registry = builder.build();
    let session_store = ExtensionData::new("session");
    let thread_store = ExtensionData::new("thread");
    thread_store.insert(
        [("root-a", "/skills/root-a"), ("root-b", "/skills/root-b")]
            .into_iter()
            .map(|(id, path)| SelectedCapabilityRoot {
                id: id.to_string(),
                location: CapabilityRootLocation::Environment {
                    environment_id: "env-1".to_string(),
                    path: path.to_string(),
                },
            })
            .collect::<Vec<_>>(),
    );
    let session_source = SessionSource::Cli;
    let config = default_config().await?;
    registry.thread_lifecycle_contributors()[0]
        .on_thread_start(ThreadStartInput {
            config: &config,
            session_source: &session_source,
            persistent_thread_state_available: true,
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await;

    let fragments = registry.turn_input_contributors()[0]
        .contribute(
            TurnInputContext {
                turn_id: "turn-1".to_string(),
                user_input: vec![UserInput::Mention {
                    name: "lint-fix".to_string(),
                    path: root_b_locator.to_string(),
                }],
                environments: Vec::new(),
            },
            &session_store,
            &thread_store,
            &ExtensionData::new("turn-1"),
        )
        .await;

    assert_eq!(1, fragments.len());
    assert!(fragments[0].render().contains(root_b_locator));
    assert_eq!(
        vec![(
            SkillAuthority::new(SkillSourceKind::Executor, "root-b"),
            SkillPackageId(root_b_locator.to_string()),
            SkillResourceId::new(root_b_locator),
        )],
        read_request_keys(&read_requests)
    );

    Ok(())
}

#[tokio::test]
async fn prompt_hidden_skill_can_still_be_invoked() -> TestResult {
    let read_requests = Arc::new(Mutex::new(Vec::new()));
    let provider = Arc::new(StaticSkillProvider {
        catalog: SkillCatalog {
            entries: vec![
                test_entry(
                    SkillSourceKind::Host,
                    "host",
                    "host/visible-skill",
                    "visible-skill/SKILL.md",
                ),
                test_entry(
                    SkillSourceKind::Host,
                    "host",
                    "host/hidden-skill",
                    "hidden-skill/SKILL.md",
                )
                .hidden_from_prompt(),
            ],
            warnings: Vec::new(),
        },
        read_requests: Arc::clone(&read_requests),
        list_calls: None,
        fail_first_list: false,
    });
    let providers = SkillProviders::new().with_host_provider(provider);
    let mut builder = ExtensionRegistryBuilder::new();
    install_with_providers(&mut builder, providers);
    let registry = builder.build();
    let session_store = ExtensionData::new("session");
    let thread_store = ExtensionData::new("thread");
    let session_source = SessionSource::Cli;
    let config = default_config().await?;
    registry.thread_lifecycle_contributors()[0]
        .on_thread_start(ThreadStartInput {
            config: &config,
            session_source: &session_source,
            persistent_thread_state_available: true,
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await;

    let fragments = registry.turn_input_contributors()[0]
        .contribute(
            TurnInputContext {
                turn_id: "turn-1".to_string(),
                user_input: vec![UserInput::Text {
                    text: "$hidden-skill".to_string(),
                    text_elements: Vec::new(),
                }],
                environments: Vec::new(),
            },
            &session_store,
            &thread_store,
            &ExtensionData::new("turn-1"),
        )
        .await;

    assert_eq!(2, fragments.len());
    assert!(fragments[0].render().contains("visible-skill"));
    assert!(!fragments[0].render().contains("hidden-skill"));
    assert!(fragments[1].render().contains("<name>hidden-skill</name>"));
    assert_eq!(
        vec![(
            SkillAuthority::new(SkillSourceKind::Host, "host"),
            SkillPackageId("host/hidden-skill".to_string()),
            SkillResourceId::new("hidden-skill/SKILL.md"),
        )],
        read_request_keys(&read_requests)
    );

    Ok(())
}

#[derive(Clone)]
struct StaticSkillProvider {
    catalog: SkillCatalog,
    read_requests: Arc<Mutex<Vec<SkillReadRequest>>>,
    list_calls: Option<Arc<AtomicUsize>>,
    fail_first_list: bool,
}

impl SkillProvider for StaticSkillProvider {
    fn list(&self, _query: SkillListQuery) -> SkillProviderFuture<'_, SkillCatalog> {
        let list_call = self
            .list_calls
            .as_ref()
            .map(|list_calls| list_calls.fetch_add(1, Ordering::Relaxed));
        let fail = self.fail_first_list && list_call == Some(0);
        let catalog = self.catalog.clone();
        Box::pin(async move {
            if fail {
                Err(SkillProviderError::new("temporary orchestrator failure"))
            } else {
                Ok(catalog)
            }
        })
    }

    fn read(&self, request: SkillReadRequest) -> SkillProviderFuture<'_, SkillReadResult> {
        let read_requests = Arc::clone(&self.read_requests);
        Box::pin(async move {
            read_requests
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(request.clone());
            Ok(SkillReadResult {
                resource: request.resource,
                contents: "# Lint Fix\n\nRun the formatter.".to_string(),
            })
        })
    }

    fn search(&self, _request: SkillSearchRequest) -> SkillProviderFuture<'_, SkillSearchResult> {
        Box::pin(async { Ok(SkillSearchResult::default()) })
    }
}

fn test_entry(
    kind: SkillSourceKind,
    authority_id: &str,
    package_id: &str,
    main_prompt: &str,
) -> SkillCatalogEntry {
    let name = package_id.rsplit('/').next().unwrap_or(package_id);
    SkillCatalogEntry::new(
        SkillPackageId(package_id.to_string()),
        SkillAuthority::new(kind, authority_id),
        name,
        "Fix lint errors.",
        SkillResourceId::new(main_prompt),
    )
    .with_display_path(format!("skill://{package_id}/SKILL.md"))
}

async fn default_config() -> std::io::Result<Config> {
    let codex_home = test_codex_home();
    std::fs::create_dir_all(&codex_home)?;
    let config =
        Config::load_default_with_cli_overrides_for_codex_home(codex_home.clone(), vec![]).await?;
    std::fs::remove_dir_all(codex_home)?;
    Ok(config)
}

fn test_codex_home() -> PathBuf {
    let id = NEXT_CODEX_HOME_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "codex-skills-extension-test-{}-{id}",
        std::process::id(),
    ))
}

fn read_request_keys(
    requests: &Arc<Mutex<Vec<SkillReadRequest>>>,
) -> Vec<(SkillAuthority, SkillPackageId, SkillResourceId)> {
    requests
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .iter()
        .map(|request| {
            (
                request.authority.clone(),
                request.package.clone(),
                request.resource.clone(),
            )
        })
        .collect()
}
