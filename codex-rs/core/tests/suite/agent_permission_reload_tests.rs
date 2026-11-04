//! An unloaded worker retains its own permissions when a sibling wakes it.

use super::*;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_core::ThreadEvictionOutcome;
use codex_core::TurnInputRequest;
use codex_core::config::Config;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_protocol::models::ActivePermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::user_input::UserInput;
use core_test_support::ThreadIdle;
use core_test_support::responses::sse_response;
use pretty_assertions::assert_eq;
use test_case::test_case;

#[derive(Clone, Copy)]
enum WorkerPermissions {
    ReadOnly,
    Named,
    Legacy,
}

#[test_case(WorkerPermissions::ReadOnly; "read-only worker")]
#[test_case(WorkerPermissions::Named; "named worker with retained grants and denials")]
#[test_case(WorkerPermissions::Legacy; "unnamed worker with retained network policy")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sibling_wake_preserves_worker_permissions(selection: WorkerPermissions) -> Result<()> {
    let server = start_mock_server().await;
    // Windows' restricted-token backend requires full-disk reads. Other targets
    // additionally exercise retention of read allowlists and explicit denials.
    let supports_read_restrictions =
        core_test_support::test_target_os() != core_test_support::TestTargetOs::Windows;
    let root_read = if supports_read_restrictions {
        ""
    } else {
        "\":root\" = \"read\""
    };
    let files = tempfile::tempdir()?;
    let instructions = files.path().join("instructions.md");
    std::fs::write(&instructions, "Keep responses brief.")?;
    let cloud_config = CloudConfigBundleFixture::loader_with_enterprise_config(format!(
        r#"
default_permissions = "startup"
model_instructions_file = {}
[features]
memories = {supports_read_restrictions}
network_proxy = true
[memories]
generate_memories = false
use_memories = {supports_read_restrictions}
[permissions.startup.filesystem]
":workspace_roots" = "read"
{root_read}
[permissions.startup.network]
enabled = true
enable_socks5 = false
[permissions.startup.network.domains]
"allowed.example" = "allow"
"blocked.example" = "allow"
[permissions.worker]
extends = "startup"
[permissions.worker.network.domains]
"blocked.example" = "deny"
"#,
        serde_json::to_string(&instructions)?,
    ));
    // A child's completion can require another sampling request before its sender becomes idle.
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/v1/responses"))
        .and(|request: &wiremock::Request| {
            [
                "spawn-worker",
                "spawn-sender",
                "loaded-status",
                "unloaded-status",
            ]
            .iter()
            .any(|call_id| has_function_call_output(request, call_id))
        })
        .respond_with(sse_response(sse(vec![ev_completed(
            "resp-drain-completion",
        )])))
        .with_priority(/* priority */ 10)
        .mount(&server)
        .await;
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.thread_lifecycle_contributor(Arc::new(ThreadIdle));
    let configure = |config: &mut Config| {
        config.model = Some("gpt-5.6-sol".to_string());
        config
            .features
            .enable(Feature::Collab)
            .expect("enable collaboration");
        config
            .features
            .enable(Feature::MultiAgentV2)
            .expect("enable multi-agent v2");
        config
            .features
            .enable(Feature::Sqlite)
            .expect("enable SQLite");
        config.multi_agent_v2.max_concurrent_threads_per_session = 3;
    };
    let mut builder = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_cloud_config_bundle(cloud_config)
        .with_config(configure);
    let initial = builder.build_with_auto_env(&server).await?;
    // Start with the loader's profile, including a memory read grant where supported.
    let profile = initial.config.permissions.permission_profile().clone();
    let mut filesystem = profile.file_system_sandbox_policy();
    assert_eq!(
        filesystem.has_full_disk_read_access(),
        !supports_read_restrictions
    );
    if supports_read_restrictions {
        let memory_root = initial
            .config
            .codex_home
            .join(initial.config.memories.version.directory_name());
        assert!(
            filesystem
                .can_read_local_path_with_cwd(memory_root.as_path(), initial.config.cwd.as_path())
        );
        filesystem.entries.push(FileSystemSandboxEntry::new(
            initial.workspace_path_uri("secrets")?.into(),
            FileSystemAccessMode::Deny,
        ));
    }
    submit_thread_settings(
        &initial.codex,
        ThreadSettingsOverrides {
            permission_profile: Some(PermissionProfile::from_runtime_permissions(
                &filesystem,
                NetworkSandboxPolicy::Enabled,
            )),
            ..Default::default()
        },
    )
    .await?;
    // Selecting another profile retains its restrictions and switches to a stricter proxy policy.
    submit_thread_settings(
        &initial.codex,
        ThreadSettingsOverrides {
            permission_profile: Some(profile.clone()),
            active_permission_profile: Some(ActivePermissionProfile::new("worker")),
            ..Default::default()
        },
    )
    .await?;
    let mut created = initial.thread_manager.subscribe_thread_created();
    let mut children = Vec::new();
    for (name, prompt, task, call_id) in [
        (
            "worker",
            "start the worker",
            "inspect the permission reload fixture",
            "spawn-worker",
        ),
        (
            "sender",
            "start the sender",
            "coordinate the permission reload fixture",
            "spawn-sender",
        ),
    ] {
        mount_root_collaboration_call(
            &server,
            prompt,
            call_id,
            "spawn_agent",
            json!({"task_name": name, "message": task, "fork_turns": "none"}),
        )
        .await;
        mount_completed_worker(&server, task, call_id).await;
        initial.submit_text_turn(prompt).await?;
        ThreadIdle::wait(&initial.codex).await;
        let child_id = created.recv().await?;
        let child = initial.thread_manager.get_thread(child_id).await?;
        wait_for_event(&child, |event| matches!(event, EventMsg::TurnComplete(_))).await;
        ThreadIdle::wait(&child).await;
        children.push((child_id, child));
    }
    let (worker_id, worker) = &children[0];
    let (_, sender) = &children[1];
    let overrides = match selection {
        WorkerPermissions::ReadOnly => ThreadSettingsOverrides {
            permission_profile: Some(PermissionProfile::read_only()),
            active_permission_profile: Some(ActivePermissionProfile::read_only()),
            ..Default::default()
        },
        WorkerPermissions::Named => ThreadSettingsOverrides {
            permission_profile: Some(profile),
            active_permission_profile: Some(ActivePermissionProfile::new("worker")),
            ..Default::default()
        },
        WorkerPermissions::Legacy => ThreadSettingsOverrides {
            sandbox_policy: Some(SandboxPolicy::WorkspaceWrite {
                writable_roots: Vec::new(),
                network_access: true,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: false,
            }),
            ..Default::default()
        },
    };
    submit_thread_settings(worker, overrides).await?;
    let worker_snapshot = worker.config_snapshot().await;
    let worker_permissions = worker_snapshot.permission_profile;
    let worker_network = worker.config().await.permissions.network.clone();
    assert_eq!(
        worker_permissions
            .file_system_sandbox_policy()
            .has_denied_read_restrictions(),
        supports_read_restrictions
    );
    submit_thread_settings(
        sender,
        ThreadSettingsOverrides {
            permission_profile: Some(PermissionProfile::workspace_write()),
            active_permission_profile: Some(ActivePermissionProfile::new(":workspace")),
            ..Default::default()
        },
    )
    .await?;

    let parent_permissions = initial.codex.config_snapshot().await.permission_profile;
    let writable_permissions = sender.config_snapshot().await.permission_profile;
    assert_ne!(writable_permissions, PermissionProfile::read_only());

    // Compare the same status-only delivery while loaded and after eviction.
    for (prompt, call_id, message) in [
        (
            "check the loaded worker",
            "loaded-status",
            "report loaded status",
        ),
        (
            "check the unloaded worker",
            "unloaded-status",
            "report reloaded status",
        ),
    ] {
        if call_id == "unloaded-status" {
            assert_eq!(
                initial
                    .thread_manager
                    .try_evict_v2_thread(Arc::clone(worker))
                    .await?,
                ThreadEvictionOutcome::Evicted,
            );
            assert!(initial.thread_manager.get_thread(*worker_id).await.is_err());
            std::fs::remove_file(&instructions)?;
        }
        let delivery = mount_root_collaboration_call(
            &server,
            prompt,
            call_id,
            "followup_task",
            json!({"target": "/root/worker", "message": message}),
        )
        .await;
        mount_completed_worker(&server, message, call_id).await;
        sender
            .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
                text: prompt.to_string(),
                text_elements: Vec::new(),
            }]))
            .await?;
        wait_for_event(sender, |event| matches!(event, EventMsg::TurnComplete(_))).await;
        assert_eq!(
            delivery.function_call_output_text(call_id),
            Some(String::new())
        );
        let worker = initial.thread_manager.get_thread(*worker_id).await?;
        wait_for_event(&worker, |event| matches!(event, EventMsg::TurnComplete(_))).await;
        ThreadIdle::wait(sender).await;
        ThreadIdle::wait(&worker).await;
        let current = worker.config_snapshot().await;
        assert_eq!(
            (
                initial.codex.config_snapshot().await.permission_profile,
                sender.config_snapshot().await.permission_profile,
                current.permission_profile,
                current.active_permission_profile,
                worker.config().await.permissions.network.clone(),
            ),
            (
                parent_permissions.clone(),
                writable_permissions.clone(),
                worker_permissions.clone(),
                worker_snapshot.active_permission_profile.clone(),
                worker_network.clone(),
            ),
            "{call_id} must preserve the worker's effective permissions and network policy",
        );
    }
    initial
        .thread_manager
        .shutdown_all_threads_bounded(Duration::from_secs(/*secs*/ 5))
        .await;
    Ok(())
}
