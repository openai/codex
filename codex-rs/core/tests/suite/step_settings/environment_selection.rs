//! Active model updates preserve the captured environment until the next turn.

use super::*;
use codex_exec_server::CreateDirectoryOptions;
use codex_protocol::protocol::TurnEnvironmentSelections;
use core_test_support::submit_thread_settings;
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
