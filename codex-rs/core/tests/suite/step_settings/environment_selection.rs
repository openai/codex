//! Active settings keep environment selection and model changes scoped to the running turn.

use super::*;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_exec_server::CreateDirectoryOptions;
use codex_protocol::models::PermissionProfileSnapshot;
use codex_protocol::protocol::EnvironmentConfigState;
use codex_protocol::protocol::TurnEnvironmentSelections;
use core_test_support::submit_thread_settings;
use core_test_support::test_codex::environment_config_for_selection;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_update_preserves_active_environment_and_next_turn_uses_new_selection() -> Result<()>
{
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let patch_response = |id, contents| {
        sse(vec![
            ev_response_created(id),
            ev_apply_patch_custom_tool_call(
                id,
                &format!("*** Begin Patch\n*** Add File: marker.txt\n+{contents}\n*** End Patch\n"),
            ),
            ev_completed(id),
        ])
    };
    let responses = mount_sse_sequence(
        &server,
        vec![
            paused_response("resp-a", "pause-a"),
            patch_response("active-patch", "active step"),
            sse_completed("active-done"),
            patch_response("next-turn-patch", "next turn"),
            sse_completed("next-turn-done"),
        ],
    )
    .await;
    let test = direct_tool_settings_test()
        .with_config(|config| {
            config
                .permissions
                .set_permission_profile(PermissionProfile::Disabled)
                .expect("test permissions");
            for model in &mut config.model_catalog.as_mut().expect("models").models {
                model.apply_patch_tool_type = Some(ApplyPatchToolType::Freeform);
            }
        })
        .build_with_auto_env(&server)
        .await?;
    let mut next_environment = test.executor_environment().selection().clone();
    next_environment.cwd = test.workspace_path_uri("future-environment")?;
    next_environment.workspace_roots = vec![next_environment.cwd.clone()];
    test.fs()
        .create_directory(
            &next_environment.cwd,
            CreateDirectoryOptions {
                recursive: false,
                follow_symlinks: true,
            },
            /*sandbox*/ None,
        )
        .await?;
    let next_marker = next_environment.cwd.join("marker.txt")?;

    let paused = start_paused_turn(&test.codex).await?;
    submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            environments: Some(TurnEnvironmentSelections::new(
                test.config.cwd.join("future-environment"),
                vec![next_environment],
            )),
            ..Default::default()
        },
    )
    .await?;
    apply_turn_settings(
        &test.codex,
        &paused.turn_id,
        TurnSettingsUpdate {
            model: Some(MODEL_B.to_string()),
            ..Default::default()
        },
    )
    .await?;
    answer_paused_turn(&test.codex, &paused.turn_id).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    test.submit_text_turn("write in the newly selected environment")
        .await?;

    assert_eq!(
        responses
            .requests()
            .iter()
            .map(|request| request.body_json()["model"].clone())
            .collect::<Vec<_>>(),
        [MODEL_A, MODEL_B, MODEL_B, MODEL_A, MODEL_A].map(|model| json!(model)),
    );
    assert_eq!(
        (
            test.fs()
                .read_file_text(
                    &test.workspace_path_uri("marker.txt")?,
                    Default::default(),
                    /*sandbox*/ None,
                )
                .await?,
            test.fs()
                .read_file_text(&next_marker, Default::default(), /*sandbox*/ None)
                .await?,
        ),
        ("active step\n".to_string(), "next turn\n".to_string()),
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn combined_active_model_and_environment_updates_apply_or_reject_together() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            paused_response("pause-before-invalid-update", "pause"),
            paused_response("pause-before-valid-update", "pause-again"),
            sse(vec![
                ev_response_created("write-in-new-environment"),
                ev_apply_patch_custom_tool_call(
                    "write-in-new-environment",
                    "*** Begin Patch\n*** Add File: marker.txt\n+selected B\n*** End Patch\n",
                ),
                ev_completed("write-in-new-environment"),
            ]),
            sse_completed("after-valid-update"),
        ],
    )
    .await;
    let test = direct_tool_settings_test()
        .with_config(|config| {
            config
                .permissions
                .set_permission_profile(PermissionProfile::Disabled)
                .expect("test permissions");
        })
        .build_with_auto_env(&server)
        .await?;
    let original = test.executor_environment().selection().clone();
    let mut next_environment = original.clone();
    next_environment.cwd = original.cwd.join("selected-environment")?;
    next_environment.workspace_roots = vec![next_environment.cwd.clone()];
    next_environment.config = EnvironmentConfigState::Ready(environment_config_for_selection(
        &test.config,
        &next_environment,
    ));
    test.fs()
        .create_directory(
            &next_environment.cwd,
            CreateDirectoryOptions {
                recursive: false,
                follow_symlinks: true,
            },
            /*sandbox*/ None,
        )
        .await?;
    let next_marker = next_environment.cwd.join("marker.txt")?;
    let next_cwd = next_environment.cwd.clone();

    let paused = start_paused_turn(&test.codex).await?;
    let mut owned_environment = original.clone();
    owned_environment.config =
        EnvironmentConfigState::Ready(environment_config_for_selection(&test.config, &original));
    apply_turn_settings(
        &test.codex,
        &paused.turn_id,
        TurnSettingsUpdate {
            environments: Some(vec![owned_environment]),
            ..Default::default()
        },
    )
    .await?;
    let mut inherited_environment = next_environment.clone();
    inherited_environment.config = EnvironmentConfigState::FromThread;
    let inherited = submit_turn_settings(
        &test.codex,
        &paused.turn_id,
        TurnSettingsUpdate {
            model: Some(MODEL_B.to_string()),
            environments: Some(vec![inherited_environment]),
            ..Default::default()
        },
    )
    .await?;
    let TurnSettingsUpdateOutcome::Rejected { reason } = inherited else {
        anyhow::bail!(
            "an owner-configured environment cannot go back to inheriting: {inherited:?}"
        );
    };
    assert!(
        reason.contains("owner-provided environment configuration"),
        "{reason}"
    );
    answer_paused_turn(&test.codex, &paused.turn_id).await?;
    let paused = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::RequestUserInput(request) => Some(request.clone()),
        _ => None,
    })
    .await;
    apply_turn_settings(
        &test.codex,
        &paused.turn_id,
        TurnSettingsUpdate {
            model: Some(MODEL_B.to_string()),
            environments: Some(vec![next_environment]),
            ..Default::default()
        },
    )
    .await?;
    answer_paused_turn(&test.codex, &paused.turn_id).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = responses.requests();
    assert_eq!(
        requests
            .iter()
            .map(|request| request.body_json()["model"].clone())
            .collect::<Vec<_>>(),
        [MODEL_A, MODEL_A, MODEL_B, MODEL_B].map(|model| json!(model)),
    );
    let context = requests[1].message_input_texts("user");
    let original_cwd = format!("<cwd>{}</cwd>", original.cwd.inferred_native_path_string());
    let rejected_cwd = format!("<cwd>{}</cwd>", next_cwd.inferred_native_path_string());
    assert!(context.iter().any(|text| text.contains(&original_cwd)));
    assert!(!context.iter().any(|text| text.contains(&rejected_cwd)));
    assert_eq!(
        test.fs()
            .read_file_text(&next_marker, Default::default(), /*sandbox*/ None)
            .await?,
        "selected B\n",
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn active_updates_check_known_permissions_from_the_running_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    mount_sse_sequence(
        &server,
        vec![
            paused_response("pause-before-environment-policy", "pause"),
            sse_completed("after-environment-policy"),
        ],
    )
    .await;
    let test = direct_tool_settings_test()
        .with_cloud_config_bundle(
            CloudConfigBundleFixture::loader_with_enterprise_requirement(format!(
                "[auto_review]\nrequired_on_models = [\"{MODEL_A}\"]\n"
            )),
        )
        .with_config(|config| {
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config
                .permissions
                .set_permission_profile(PermissionProfile::read_only())
                .expect("set restricted thread permissions");
            for feature in [Feature::GuardianApproval, Feature::DeferredExecutor] {
                config
                    .features
                    .enable(feature)
                    .expect("enable test feature");
            }
        })
        .build_with_auto_env(&server)
        .await?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    test.thread_manager
        .environment_manager()
        .upsert_environment(
            "policy-secondary".to_string(),
            format!("ws://{}", listener.local_addr()?),
            /*connect_timeout*/ None,
        )?;
    let mut primary = test.executor_environment().selection().clone();
    primary.config =
        EnvironmentConfigState::Ready(environment_config_for_selection(&test.config, &primary));
    let mut secondary = primary.clone();
    secondary.environment_id = "policy-secondary".to_string();
    let mut unrestricted = environment_config_for_selection(&test.config, &secondary);
    unrestricted.permission_profile =
        PermissionProfileSnapshot::legacy(PermissionProfile::Disabled);
    let paused = start_paused_turn(&test.codex).await?;
    let current_model = TurnSettingsUpdate {
        model: Some(MODEL_A.to_string()),
        ..Default::default()
    };
    apply_turn_settings(&test.codex, &paused.turn_id, current_model.clone()).await?;
    secondary.config = EnvironmentConfigState::Ready(unrestricted.clone());
    let supplied = submit_turn_settings(
        &test.codex,
        &paused.turn_id,
        TurnSettingsUpdate {
            environments: Some(vec![primary.clone(), secondary.clone()]),
            ..current_model.clone()
        },
    )
    .await?;
    let TurnSettingsUpdateOutcome::Rejected { reason } = supplied else {
        anyhow::bail!("unrestricted secondary must reject the reviewed model: {supplied:?}");
    };
    assert!(reason.contains("you need to use auto review"), "{reason}");

    secondary.config = EnvironmentConfigState::Pending;
    apply_turn_settings(
        &test.codex,
        &paused.turn_id,
        TurnSettingsUpdate {
            environments: Some(vec![primary, secondary.clone()]),
            ..Default::default()
        },
    )
    .await?;
    test.codex
        .environment_ready(&secondary, unrestricted)
        .await?;
    // Its permissions are known even though the executor hasn't connected yet.
    let without_selection =
        submit_turn_settings(&test.codex, &paused.turn_id, current_model).await?;
    assert_eq!(
        without_selection,
        TurnSettingsUpdateOutcome::Rejected { reason }
    );
    answer_paused_turn(&test.codex, &paused.turn_id).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn inherited_permission_update_applies_to_the_next_turn_not_the_next_step() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            paused_response("before-permission-update", "pause"),
            sse_completed("current-turn-done"),
            sse_completed("next-turn-done"),
        ],
    )
    .await;
    let test = direct_tool_settings_test()
        .with_config(|config| {
            config
                .permissions
                .set_permission_profile(PermissionProfile::read_only())
                .expect("set restricted thread permissions");
        })
        .build_with_auto_env(&server)
        .await?;

    let paused = start_paused_turn(&test.codex).await?;
    submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            permission_profile: Some(PermissionProfile::Disabled),
            ..Default::default()
        },
    )
    .await?;
    // Recreate the inherited selection after saving future permissions; it must use this turn's.
    let mut inherited = test.executor_environment().selection().clone();
    inherited.workspace_roots.clear();
    apply_turn_settings(
        &test.codex,
        &paused.turn_id,
        TurnSettingsUpdate {
            environments: Some(vec![inherited]),
            ..Default::default()
        },
    )
    .await?;
    answer_paused_turn(&test.codex, &paused.turn_id).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    test.submit_text_turn("start the next turn").await?;

    let unrestricted = responses
        .requests()
        .iter()
        .map(|request| {
            let filesystem = request
                .message_input_texts("user")
                .into_iter()
                .rfind(|text| text.contains("<filesystem>"))
                .expect("model request includes filesystem permissions");
            filesystem.contains("<file_system type=\"unrestricted\"")
        })
        .collect::<Vec<_>>();
    assert_eq!(unrestricted, [false, false, true]);
    Ok(())
}
