use std::sync::Arc;

use anyhow::Result;
use codex_core::StartThreadOptions;
use codex_core::TurnInputRequest;
use codex_core::config::Config;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::EnvironmentManager;
use codex_extension_api::ExtensionDataInit;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_features::Feature;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::EventMsg;
use codex_protocol::user_input::UserInput;
use codex_skills_extension::ExecutorSkillProvider;
use codex_skills_extension::SkillProviders;
use codex_skills_extension::SkillsExtensionConfig;
use codex_skills_extension::install_with_providers;
use codex_utils_path_uri::PathUri;
use core_test_support::responses;
use core_test_support::skip_if_target_windows;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restricted_executor_discovery_preserves_permitted_plugin_skills() -> Result<()> {
    skip_if_target_windows!(
        Ok(()),
        "the unelevated Windows sandbox cannot enforce restricted filesystem reads"
    );

    let server = responses::start_mock_server().await;
    let response = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-contract"),
            responses::ev_completed("resp-contract"),
        ]),
    )
    .await;

    let mut extensions = ExtensionRegistryBuilder::new();
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
            orchestrator_skills_enabled: false,
            shadow_selection_enabled: false,
        },
    );

    let permissions = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry::new(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
            },
            FileSystemAccessMode::Read,
        )]),
        NetworkSandboxPolicy::Restricted,
    );
    let mut builder = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_workspace_setup(|cwd, file_system| async move {
            let plugin = PathUri::from_abs_path(&cwd).join("plugin")?;
            let manifest_dir = plugin.join(".codex-plugin")?;
            let skill_dir = plugin.join("skills/deploy")?;

            for directory in [&manifest_dir, &skill_dir] {
                file_system
                    .create_directory(
                        directory,
                        CreateDirectoryOptions {
                            recursive: true,
                            follow_symlinks: true,
                        },
                        /*sandbox*/ None,
                    )
                    .await?;
            }
            file_system
                .write_file(
                    &manifest_dir.join("plugin.json")?,
                    br#"{"name":"demo-plugin"}"#.to_vec(),
                    Default::default(),
                    /*sandbox*/ None,
                )
                .await?;
            file_system
                .write_file(
                    &skill_dir.join("SKILL.md")?,
                    b"---\nname: deploy\ndescription: Deploy through the executor.\n---\n\nDeploy instructions.\n"
                        .to_vec(),
                    Default::default(),
                    /*sandbox*/ None,
                )
                .await?;
            Ok(())
        })
        .with_config(move |config| {
            config.include_skill_instructions = true;
            assert!(
                config
                    .features
                    .enable(Feature::ExecutorCapabilityDiscovery)
                    .is_ok(),
                "executor capability discovery should be configurable in tests"
            );
            assert!(
                config
                    .permissions
                    .set_permission_profile(permissions)
                    .is_ok(),
                "restricted filesystem permissions should be configurable in tests"
            );
        });
    let test = builder.build_with_auto_env(&server).await?;
    let selection = test.executor_environment().selection().clone();
    let root = SelectedCapabilityRoot {
        id: "demo-plugin@1".to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: selection.environment_id.clone(),
            path: selection.cwd.join("plugin")?,
        },
    };
    let mut thread_extension_init = ExtensionDataInit::new();
    thread_extension_init.insert(vec![root]);
    let thread = test
        .thread_manager
        .start_thread(StartThreadOptions {
            environments: Some(vec![selection]),
            thread_extension_init,
            ..StartThreadOptions::new(test.config.clone())
        })
        .await?
        .thread;

    thread
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Inspect the available executor skills.".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&thread, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    let developer_instructions = response
        .single_request()
        .message_input_texts("developer")
        .join("\n");
    assert!(
        developer_instructions.contains("demo-plugin:deploy"),
        "restricted capability discovery must preserve permitted executor skills: {developer_instructions}"
    );

    Ok(())
}
