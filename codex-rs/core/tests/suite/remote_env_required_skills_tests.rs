//! Required environment skills gate model requests and stop applying after deselection.

use super::super::skills_extension::CatalogSkillProvider;
use super::*;
use codex_config::ScopedSkillsConfig;
use codex_exec_server::RemoteEnvironmentOptions;
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
use core_test_support::responses::mount_sse_once;
use core_test_support::test_codex::test_env;
use pretty_assertions::assert_eq;
use test_case::test_case;
use tokio_util::task::AbortOnDropHandle;

#[derive(Clone, Copy)]
enum Availability {
    Available,
    Missing,
    Disabled,
    OtherEnvironment,
    FailedEnvironment,
}

/// Only an enabled skill from the required environment permits inference; deselection recovers.
#[test_case(Availability::Available; "available")]
#[test_case(Availability::Missing; "missing")]
#[test_case(Availability::Disabled; "disabled")]
#[test_case(Availability::OtherEnvironment; "other environment")]
#[test_case(Availability::FailedEnvironment; "failed environment")]
#[tokio::test]
async fn required_environment_skill_gates_inference(availability: Availability) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let environment = test_env().await?;
    let primary = environment.selection().clone();
    let source = if matches!(availability, Availability::OtherEnvironment) {
        primary.environment_id.as_str()
    } else {
        "required"
    };
    let mut entry = SkillCatalogEntry::new(
        SkillPackageId("test/review".to_string()),
        SkillAuthority::new(SkillSourceKind::Executor, source),
        "review",
        "Review the workspace.",
        SkillResourceId::environment(
            "skill://test/review/SKILL.md",
            source,
            primary.cwd.join("SKILL.md")?,
        ),
    );
    entry.enabled = !matches!(availability, Availability::Disabled);
    let catalog = SkillCatalog {
        entries: if matches!(
            availability,
            Availability::Missing | Availability::FailedEnvironment
        ) {
            Vec::new()
        } else {
            vec![entry]
        },
        warnings: Vec::new(),
    };
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    install_with_providers(
        &mut extensions,
        SkillProviders::new().with_executor_provider(Arc::new(CatalogSkillProvider { catalog })),
        |_: &Config| SkillsExtensionConfig {
            include_instructions: false,
            max_context_tokens: None,
            bundled_skills_enabled: false,
            cloud_skill_enabled: false,
            shadow_selection_enabled: false,
        },
    );
    let server = start_mock_server().await;
    let response = mount_sse_once(&server, sse(vec![ev_completed("done")])).await;
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| {
            config.project_doc_max_bytes = 0;
            config
                .features
                .enable(Feature::ExecutorCapabilityDiscovery)
                .expect("enable executor capability discovery");
        })
        .build_with_environment(&server, environment)
        .await?;

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let executor_url = format!("ws://{}", listener.local_addr()?);
    let (attach, connection) = tokio::sync::oneshot::channel();
    let (shutdown, stop) = tokio::sync::oneshot::channel();
    let executor = AbortOnDropHandle::new(tokio::spawn(serve_environment_with_agents_md(
        listener, "", connection, stop,
    )));
    attach.send(()).expect("attach required environment");
    let manager = test.thread_manager.environment_manager();
    manager.upsert_environment_with_options(
        "required".to_string(),
        RemoteEnvironmentOptions {
            exec_server_url: executor_url,
            connect_timeout: None,
            http_headers: HashMap::new(),
        },
        ScopedSkillsConfig {
            required: vec!["review".to_string()],
        },
    )?;
    manager
        .get_environment("required")
        .context("required environment")?
        .info()
        .await?;
    let mut required = TurnEnvironmentSelection {
        environment_id: "required".to_string(),
        ..primary.clone()
    };
    if matches!(availability, Availability::FailedEnvironment) {
        required.config = EnvironmentConfigState::Failed("configuration unavailable".to_string());
    }
    submit_turn_with_approval_and_environments(
        &test,
        "Review the workspace.",
        vec![primary.clone(), required],
        AskForApproval::Never,
    )
    .await?;
    let mut errors = Vec::new();
    wait_for_event(&test.codex, |event| {
        if let EventMsg::Error(error) = event {
            errors.push(error.message.clone());
        }
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    if matches!(availability, Availability::Available) {
        assert_eq!(errors, Vec::<String>::new());
        assert_eq!(response.requests().len(), 1);
    } else {
        assert!(response.requests().is_empty());
        assert_eq!(
            errors,
            vec![
                "Fatal error: Required skill \"review\" from environment \"required\" is unavailable"
            ]
        );
        test.submit_turn_with_environments(
            "Continue without that environment.",
            Some(vec![primary]),
        )
        .await?;
        assert_eq!(response.requests().len(), 1);
    }
    shutdown.send(()).expect("stop required environment");
    executor.await?;
    Ok(())
}
