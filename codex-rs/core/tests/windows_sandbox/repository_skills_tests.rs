//! Verifies cached repository skills cannot bypass the elevated Windows read policy.

use super::*;
use codex_core::config::Config;
use codex_exec_server::FileSystemEnvironmentAccessor;
use codex_exec_server::FileSystemSandboxContext;
use codex_exec_server::WindowsSandboxSelection;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_skills_extension::HostSkillsLoadInput;
use codex_skills_extension::SkillsExtensionConfig;
use codex_skills_extension::install;
use pretty_assertions::assert_eq;
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial(codex_home)]
async fn windows_elevated_repository_skills_enforce_cached_read_denials() -> anyhow::Result<()> {
    let _account_guard = WindowsSandboxAccountTestGuard::acquire()?;
    let codex_home = codex_home_for_windows_sandbox_test("windows-repository-skills-codex-home")?;
    let _codex_home_guard = EnvVarGuard::set("CODEX_HOME", codex_home.path().as_os_str());
    stage_windows_sandbox_helpers()?;

    const PRIVATE_BODY: &str = "WINDOWS_DENIED_SKILL_INSTRUCTIONS";
    const ORIGINAL_BODY: &str = "WINDOWS_ORIGINAL_SKILL_INSTRUCTIONS";
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    install(&mut extensions, |config: &Config| SkillsExtensionConfig {
        include_instructions: config.include_skill_instructions,
        max_context_tokens: config.skill_max_context_tokens,
        catalog_deltas_enabled: config.features.enabled(Feature::SkillCatalogDeltas),
        omit_descriptions: false,
        bundled_skills_enabled: false,
        cloud_skill_enabled: false,
        shadow_selection_enabled: false,
    });
    let configured_codex_home = dunce::canonicalize(codex_home.path())?.abs();
    let builder = test_codex()
        .with_windows_cmd_shell()
        .with_extensions(Arc::new(extensions.build()))
        .with_config(move |config| {
            config.codex_home = configured_codex_home;
            config.prefer_mxc = false;
            config.set_windows_elevated_sandbox_enabled(true);
            config.include_skill_instructions = true;
            config.features.enable(Feature::ExecutorCapabilityDiscovery).expect("enable executor discovery");
        })
        .with_workspace_setup(|cwd, _fs| async move {
            for name in ["allowed", "revoked", "file", "directory"] {
                let root = cwd.join(format!(".agents/skills/windows-{name}"));
                std::fs::create_dir_all(&root)?;
                std::fs::write(
                    root.join("SKILL.md"),
                    format!("---\nname: windows-{name}\ndescription: Windows repository skill.\n---\n\n{ORIGINAL_BODY}\n"),
                )?;
            }
            std::fs::create_dir(cwd.join("denied"))?;
            std::fs::write(
                cwd.join("denied/SKILL.md"),
                format!("---\nname: windows-private\ndescription: Denied skill.\n---\n\n{PRIVATE_BODY}\n"),
            )?;
            Ok(())
        });
    let harness = TestCodexHarness::with_builder(builder).await?;
    let test = harness.test();
    let responses = mount_sse_sequence(
        harness.server(),
        vec![
            sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
            sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
        ],
    )
    .await;
    let cwd = &test.config.cwd;
    let mut policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry::new(
            FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            FileSystemAccessMode::Read,
        ),
        FileSystemSandboxEntry::new(
            FileSystemPath::Path {
                path: cwd.join("denied").into(),
            },
            FileSystemAccessMode::Deny,
        ),
    ]);
    let initial_permissions =
        PermissionProfile::from_runtime_permissions(&policy, NetworkSandboxPolicy::Restricted);
    test.submit_turn_with_permission_profile("List repository skills.", initial_permissions)
        .await?;

    let selection = test.executor_environment().selection();
    let environment = test
        .thread_manager
        .environment_manager()
        .get_environment(&selection.environment_id)
        .expect("local environment");
    let file_system = environment.get_filesystem();
    let open = FileSystemEnvironmentAccessor::unrestricted(&file_system);
    let snapshot = test
        .thread_manager
        .skills_service()
        .snapshot_for_config(
            &HostSkillsLoadInput::new(
                cwd.clone(),
                Vec::new(),
                test.config.config_layer_stack.clone(),
            ),
            Some(&open),
        )
        .await;
    let [allowed, revoked, file, directory] =
        ["allowed", "revoked", "file", "directory"].map(|name| {
            snapshot
                .outcome()
                .skills
                .iter()
                .find(|skill| skill.name == format!("windows-{name}"))
                .expect("initial repository skill")
                .clone()
        });

    policy.entries.push(FileSystemSandboxEntry::new(
        FileSystemPath::Path {
            path: cwd.join(".agents/skills/windows-revoked/SKILL.md").into(),
        },
        FileSystemAccessMode::Deny,
    ));
    let permissions =
        PermissionProfile::from_runtime_permissions(&policy, NetworkSandboxPolicy::Restricted);
    let mut sandbox = FileSystemSandboxContext::from_permission_profile(
        permissions.clone(),
        selection.cwd.clone(),
    );
    sandbox.windows_sandbox_selection = WindowsSandboxSelection::Elevated;
    let restricted = FileSystemEnvironmentAccessor::new(&file_system, sandbox);
    // The same restricted accessor must work, so a missing backend cannot satisfy the denials.
    assert!(
        snapshot
            .read_skill_text(&allowed, &[&restricted])
            .await?
            .contains(ORIGINAL_BODY)
    );
    snapshot
        .read_skill_text(&revoked, &[&restricted])
        .await
        .expect_err("revoked read must be denied");

    let file_path = cwd.join(".agents/skills/windows-file/SKILL.md");
    std::fs::remove_file(&file_path)?;
    std::os::windows::fs::symlink_file(cwd.join("denied/SKILL.md"), &file_path)?;
    let directory_path = cwd.join(".agents/skills/windows-directory");
    std::fs::remove_dir_all(&directory_path)?;
    std::os::windows::fs::symlink_dir(cwd.join("denied"), &directory_path)?;
    for skill in [&file, &directory] {
        assert!(
            snapshot
                .read_skill_text(skill, &[&open])
                .await?
                .contains(PRIVATE_BODY)
        );
        snapshot
            .read_skill_text(skill, &[&restricted])
            .await
            .expect_err("symlink target must be denied");
    }
    test.submit_turn_with_permission_profile(
        "Use $windows-revoked, $windows-file, and $windows-directory if available.",
        permissions,
    )
    .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    let initial = requests[0].message_input_texts("developer").join("\n");
    for name in ["allowed", "revoked", "file", "directory"] {
        assert!(initial.contains(&format!("- windows-{name}:")));
    }
    let developer_texts = requests[1].message_input_texts("developer");
    let latest_catalog = developer_texts
        .iter()
        .rev()
        .find(|text| text.contains("- windows-allowed:"))
        .expect("allowed skill remains available");
    for name in ["revoked", "file", "directory", "private"] {
        assert!(!latest_catalog.contains(&format!("- windows-{name}:")));
    }
    let user_texts = requests[1].message_input_texts("user").join("\n");
    assert!(!user_texts.contains(PRIVATE_BODY));
    assert!(!user_texts.contains(ORIGINAL_BODY));
    assert!(
        snapshot
            .read_skill_text(&allowed, &[&restricted])
            .await?
            .contains(ORIGINAL_BODY)
    );
    assert!(
        snapshot
            .read_skill_text(&revoked, &[&open])
            .await?
            .contains(ORIGINAL_BODY)
    );
    Ok(())
}
