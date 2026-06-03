use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use codex_core::config::Config;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::TurnInputContext;
use codex_extension_api::TurnInputEnvironment;
use codex_protocol::protocol::SKILLS_INSTRUCTIONS_OPEN_TAG;
use codex_protocol::protocol::SessionSource;
use codex_protocol::user_input::UserInput;
use codex_skills_extension::SkillProviders;
use codex_skills_extension::catalog::SkillAuthority;
use codex_skills_extension::catalog::SkillCatalog;
use codex_skills_extension::catalog::SkillCatalogEntry;
use codex_skills_extension::catalog::SkillPackageId;
use codex_skills_extension::catalog::SkillReadResult;
use codex_skills_extension::catalog::SkillResourceId;
use codex_skills_extension::catalog::SkillSearchResult;
use codex_skills_extension::catalog::SkillSourceKind;
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
async fn installed_extension_injects_available_catalog_and_selected_entrypoint() -> TestResult {
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
        .with_host_provider(host_provider)
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

    let turn_store = ExtensionData::new("turn-1");
    let fragments = registry.turn_input_contributors()[0]
        .contribute(
            TurnInputContext {
                turn_id: "turn-1".to_string(),
                user_input: vec![UserInput::Text {
                    text: "$lint-fix please".to_string(),
                    text_elements: Vec::new(),
                }],
                environments: vec![TurnInputEnvironment {
                    environment_id: "env-1".to_string(),
                    cwd: std::env::temp_dir(),
                    is_primary: true,
                }],
            },
            &session_store,
            &thread_store,
            &turn_store,
        )
        .await;

    assert_eq!(2, fragments.len());
    assert_eq!("developer", fragments[0].role());
    assert!(
        fragments[0]
            .render()
            .starts_with(SKILLS_INSTRUCTIONS_OPEN_TAG)
    );
    assert!(fragments[0].render().contains("lint-fix"));
    assert_eq!("user", fragments[1].role());
    assert!(fragments[1].render().contains("<name>lint-fix</name>"));
    assert!(fragments[1].render().contains("# Lint Fix"));
    assert_eq!(
        vec![SkillReadRequest {
            authority: SkillAuthority::new(SkillSourceKind::Host, "host"),
            package: SkillPackageId("host/lint-fix".to_string()),
            resource: SkillResourceId("lint-fix/SKILL.md".to_string()),
        }],
        host_read_requests
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
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

    assert_eq!(1, next_fragments.len());
    assert_eq!("developer", next_fragments[0].role());
    assert!(next_fragments[0].render().contains("lint-fix"));

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
    let catalog_fragment = fragments[0].render();
    assert!(catalog_fragment.contains("visible-skill"));
    assert!(!catalog_fragment.contains("hidden-skill"));
    assert!(fragments[1].render().contains("<name>hidden-skill</name>"));
    assert_eq!(
        vec![SkillReadRequest {
            authority: SkillAuthority::new(SkillSourceKind::Host, "host"),
            package: SkillPackageId("host/hidden-skill".to_string()),
            resource: SkillResourceId("hidden-skill/SKILL.md".to_string()),
        }],
        read_requests
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    );

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
    let id = NEXT_CODEX_HOME_ID.fetch_add(1, Ordering::Relaxed);
    let codex_home = std::env::temp_dir().join(format!(
        "codex-skills-extension-test-{}-{id}",
        std::process::id(),
    ));
    std::fs::create_dir_all(&codex_home)?;
    let config =
        Config::load_default_with_cli_overrides_for_codex_home(codex_home.clone(), vec![]).await?;
    std::fs::remove_dir_all(codex_home)?;
    Ok(config)
}
