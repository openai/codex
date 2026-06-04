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
use codex_extension_api::ContextContributionInput;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::TurnInputContext;
use codex_extension_api::TurnInputEnvironment;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_OPEN_TAG;
use codex_protocol::protocol::SessionSource;
use codex_protocol::user_input::UserInput;
use codex_skills_extension::SkillProviderSource;
use codex_skills_extension::SkillProviders;
use codex_skills_extension::catalog::SkillAuthority;
use codex_skills_extension::catalog::SkillCatalog;
use codex_skills_extension::catalog::SkillCatalogEntry;
use codex_skills_extension::catalog::SkillPackageId;
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

    let initial_fragments = registry.context_contributors()[0]
        .contribute(
            context_input("turn-1", Vec::new()),
            &session_store,
            &thread_store,
            &turn_store,
        )
        .await;

    assert_eq!(1, initial_fragments.len());
    assert_eq!(
        codex_extension_api::PromptSlot::DeveloperCapabilities,
        initial_fragments[0].slot()
    );
    assert!(
        initial_fragments[0]
            .text()
            .starts_with(SKILLS_INSTRUCTIONS_OPEN_TAG)
    );
    assert!(initial_fragments[0].text().contains("demo"));
    assert!(initial_fragments[0].text().contains(&skill_prompt_path));

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

    assert_eq!(1, fragments.len());
    assert_eq!("user", fragments[0].role());
    assert!(fragments[0].render().contains("<name>demo</name>"));
    assert!(fragments[0].render().contains("# Demo"));
    assert!(fragments[0].render().contains(&skill_prompt_path));
    let injected_host_skill_prompts = turn_store
        .get::<InjectedHostSkillPrompts>()
        .ok_or("host skill prompt marker should be set")?;
    assert!(injected_host_skill_prompts.contains_path(&skill_path_string));

    std::fs::remove_dir_all(codex_home)?;
    Ok(())
}

#[tokio::test]
async fn installed_extension_injects_selected_entrypoint() -> TestResult {
    let host_read_requests = Arc::new(Mutex::new(Vec::new()));
    let remote_read_requests = Arc::new(Mutex::new(Vec::new()));
    let host_provider = Arc::new(StaticSkillProvider {
        catalog: SkillCatalog {
            entries: vec![test_entry(
                SkillSourceKind::Host,
                "host",
                "host/lint-fix",
                "lint-fix/SKILL.md",
            )],
            warnings: Vec::new(),
        },
        read_requests: Arc::clone(&host_read_requests),
    });
    let remote_provider = Arc::new(StaticSkillProvider {
        catalog: SkillCatalog {
            entries: vec![test_entry(
                SkillSourceKind::Remote,
                "remote",
                "remote/lint-fix",
                "lint-fix/SKILL.md",
            )],
            warnings: Vec::new(),
        },
        read_requests: Arc::clone(&remote_read_requests),
    });
    let providers = SkillProviders::new()
        .with_provider(SkillProviderSource::host("host", host_provider))
        .with_remote_provider(remote_provider);
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

    let environments = vec![TurnInputEnvironment {
        environment_id: "env-1".to_string(),
        cwd: std::env::temp_dir(),
        is_primary: true,
    }];
    let turn_store = ExtensionData::new("turn-1");
    let initial_fragments = registry.context_contributors()[0]
        .contribute(
            context_input("turn-1", environments.clone()),
            &session_store,
            &thread_store,
            &turn_store,
        )
        .await;

    assert_eq!(1, initial_fragments.len());
    assert_eq!(
        codex_extension_api::PromptSlot::DeveloperCapabilities,
        initial_fragments[0].slot()
    );
    assert!(
        initial_fragments[0]
            .text()
            .starts_with(SKILLS_INSTRUCTIONS_OPEN_TAG)
    );
    assert!(initial_fragments[0].text().contains("lint-fix"));

    let fragments = registry.turn_input_contributors()[0]
        .contribute(
            TurnInputContext {
                turn_id: "turn-1".to_string(),
                user_input: vec![UserInput::Text {
                    text: "$lint-fix please".to_string(),
                    text_elements: Vec::new(),
                }],
                environments,
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
            SkillAuthority::new(SkillSourceKind::Host, "host"),
            SkillPackageId("host/lint-fix".to_string()),
            SkillResourceId("lint-fix/SKILL.md".to_string()),
        )],
        read_request_keys(&host_read_requests)
    );
    assert!(
        remote_read_requests
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty()
    );

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
async fn prompt_hidden_host_skill_can_still_be_invoked_by_extension() -> TestResult {
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
    });
    let providers =
        SkillProviders::new().with_provider(SkillProviderSource::host("host", provider));
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
        .contribute(
            context_input("turn-1", Vec::new()),
            &session_store,
            &thread_store,
            &ExtensionData::new("turn-1"),
        )
        .await;

    assert_eq!(1, initial_fragments.len());
    let catalog_fragment = initial_fragments[0].text();
    assert!(catalog_fragment.contains("visible-skill"));
    assert!(!catalog_fragment.contains("hidden-skill"));

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

    assert_eq!(1, fragments.len());
    assert!(fragments[0].render().contains("<name>hidden-skill</name>"));
    assert_eq!(
        vec![(
            SkillAuthority::new(SkillSourceKind::Host, "host"),
            SkillPackageId("host/hidden-skill".to_string()),
            SkillResourceId("hidden-skill/SKILL.md".to_string()),
        )],
        read_request_keys(&read_requests)
    );

    Ok(())
}

#[tokio::test]
async fn installed_extension_reemits_catalog_when_catalog_changes() -> TestResult {
    let catalog = Arc::new(Mutex::new(SkillCatalog {
        entries: vec![test_entry(
            SkillSourceKind::Host,
            "host",
            "host/first-skill",
            "first-skill/SKILL.md",
        )],
        warnings: Vec::new(),
    }));
    let provider = Arc::new(MutableCatalogProvider {
        catalog: Arc::clone(&catalog),
    });
    let providers =
        SkillProviders::new().with_provider(SkillProviderSource::host("host", provider));
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

    let first_fragments = registry.context_contributors()[0]
        .contribute(
            context_input("turn-1", Vec::new()),
            &session_store,
            &thread_store,
            &ExtensionData::new("turn-1"),
        )
        .await;

    assert_eq!(1, first_fragments.len());
    assert_eq!(
        codex_extension_api::PromptSlot::DeveloperCapabilities,
        first_fragments[0].slot()
    );
    assert!(first_fragments[0].text().contains("first-skill"));

    let unchanged_fragments = registry.turn_input_contributors()[0]
        .contribute(
            TurnInputContext {
                turn_id: "turn-2".to_string(),
                user_input: Vec::new(),
                environments: Vec::new(),
            },
            &session_store,
            &thread_store,
            &ExtensionData::new("turn-2"),
        )
        .await;

    assert!(unchanged_fragments.is_empty());

    *catalog
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = SkillCatalog {
        entries: vec![test_entry(
            SkillSourceKind::Host,
            "host",
            "host/second-skill",
            "second-skill/SKILL.md",
        )],
        warnings: Vec::new(),
    };

    let changed_fragments = registry.turn_input_contributors()[0]
        .contribute(
            TurnInputContext {
                turn_id: "turn-3".to_string(),
                user_input: Vec::new(),
                environments: Vec::new(),
            },
            &session_store,
            &thread_store,
            &ExtensionData::new("turn-3"),
        )
        .await;

    assert_eq!(1, changed_fragments.len());
    assert_eq!("developer", changed_fragments[0].role());
    assert!(changed_fragments[0].render().contains("second-skill"));
    assert!(!changed_fragments[0].render().contains("first-skill"));

    Ok(())
}

#[derive(Clone)]
struct StaticSkillProvider {
    catalog: SkillCatalog,
    read_requests: Arc<Mutex<Vec<SkillReadRequest>>>,
}

impl SkillProvider for StaticSkillProvider {
    fn list(&self, query: SkillListQuery) -> SkillProviderFuture<'_, SkillCatalog> {
        let catalog = self.catalog.clone();
        Box::pin(async move {
            assert!(query.include_host_skills);
            assert!(query.include_bundled_skills);
            Ok(catalog)
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

#[derive(Clone)]
struct MutableCatalogProvider {
    catalog: Arc<Mutex<SkillCatalog>>,
}

impl SkillProvider for MutableCatalogProvider {
    fn list(&self, query: SkillListQuery) -> SkillProviderFuture<'_, SkillCatalog> {
        let catalog = self
            .catalog
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        Box::pin(async move {
            assert!(query.include_host_skills);
            assert!(query.include_bundled_skills);
            Ok(catalog)
        })
    }

    fn read(&self, request: SkillReadRequest) -> SkillProviderFuture<'_, SkillReadResult> {
        Box::pin(async move {
            Ok(SkillReadResult {
                resource: request.resource,
                contents: "# Mutable Skill\n".to_string(),
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
        SkillResourceId(main_prompt.to_string()),
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

fn context_input(
    turn_id: &str,
    environments: Vec<TurnInputEnvironment>,
) -> ContextContributionInput {
    ContextContributionInput {
        turn_id: turn_id.to_string(),
        environments,
    }
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
