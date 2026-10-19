//! Keeps the tool stable across unavailable attachments and reports lost executor connections.

use super::*;
use pretty_assertions::assert_eq;

/// The tool stays visible and reports the unavailable endpoint when called.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn copy_file_remains_available_with_unavailable_attachment() -> Result<()> {
    use codex_protocol::protocol::TurnEnvironmentSelection;

    let server = start_mock_server().await;
    let mut extensions = ExtensionRegistryBuilder::new();
    codex_multi_env_copy_file::install(&mut extensions);
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .build_with_auto_env(&server)
        .await?;
    let manager = test.thread_manager.environment_manager();
    manager.upsert_environment(
        "unavailable".to_string(),
        "ws://127.0.0.1:0".to_string(),
        /*connect_timeout*/ None,
    )?;
    assert!(
        manager
            .get_environment("unavailable")
            .context("attached environment")?
            .wait_until_ready()
            .await
            .is_err()
    );
    let primary = test.executor_environment().selection().clone();
    let mut calls = Vec::new();
    for (id, source, target) in [
        (
            "source-offline",
            "unavailable",
            primary.environment_id.as_str(),
        ),
        (
            "target-offline",
            primary.environment_id.as_str(),
            "unavailable",
        ),
    ] {
        calls.push(ev_function_call_with_namespace(
            id,
            "functions",
            "multi_env_copy_file",
            &json!({
                "source_environment_id": source,
                "source_path": primary.cwd.join("source.bin")?.inferred_native_path_string(),
                "target_environment_id": target,
                "target_path": primary.cwd.join("target.bin")?.inferred_native_path_string(),
            })
            .to_string(),
        ));
    }
    calls.push(ev_completed("unavailable-copies"));
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(calls),
            sse(vec![
                ev_assistant_message("done", "done"),
                ev_completed("done"),
            ]),
        ],
    )
    .await;
    test.submit_turn_with_environments(
        "Try copying to and from the unavailable environment.",
        Some(vec![
            primary.clone(),
            TurnEnvironmentSelection {
                selected_capability_roots: Default::default(),
                environment_id: "unavailable".to_string(),
                cwd: primary.cwd.clone(),
                workspace_roots: vec![primary.cwd],
                config: EnvironmentConfigState::FromThread,
            },
        ]),
    )
    .await?;
    let requests = mock.requests();
    let request = &requests[0];
    assert!(
        request.body_json()["tools"]
            .as_array()
            .context("tools")?
            .iter()
            .any(|tool| tool["name"] == "multi_env_copy_file")
            || request
                .tool_by_name("functions", "multi_env_copy_file")
                .is_some()
    );
    insta::assert_json_snapshot!(
        "unavailable_endpoints",
        json!({
            "source": mock.function_call_output_text("source-offline").context("source error")?,
            "target": mock.function_call_output_text("target-offline").context("target error")?,
        })
    );
    Ok(())
}

/// Losing a captured executor must return a model-visible transport error without copying bytes.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn copy_file_reports_disconnection_after_step_capture() -> Result<()> {
    use codex_exec_server::EnvironmentConnectionState;
    use codex_protocol::protocol::ThreadSettingsOverrides;
    use codex_protocol::protocol::TurnEnvironmentSelection;
    use codex_protocol::protocol::TurnEnvironmentSelections;
    use core_test_support::exec_server::ExecServerProcess;
    use core_test_support::streaming_sse::StreamingSseChunk;
    use core_test_support::streaming_sse::start_streaming_sse_server;

    let executor = ExecServerProcess::start().await?;
    let available = ExecServerProcess::start().await?;
    let root = TempDir::new()?;
    let cwd = PathUri::from_host_native_path(root.path())?;
    fs::write(root.path().join("source.bin"), b"source")?;
    fs::write(root.path().join("target.bin"), b"original")?;
    let mut calls = Vec::new();
    for (id, source, target) in [
        ("source-disconnected", "disconnected", "available"),
        ("target-disconnected", "available", "disconnected"),
    ] {
        calls.push(ev_function_call_with_namespace(
            id,
            "functions",
            "multi_env_copy_file",
            &json!({
                "source_environment_id": source,
                "source_path": cwd.join("source.bin")?.inferred_native_path_string(),
                "target_environment_id": target,
                "target_path": cwd.join("target.bin")?.inferred_native_path_string(),
            })
            .to_string(),
        ));
    }
    calls.push(ev_completed("copies"));
    let (release_response, response_gate) = tokio::sync::oneshot::channel();
    let (streaming, _completions) = start_streaming_sse_server(vec![
        vec![StreamingSseChunk {
            gate: Some(response_gate),
            body: sse(calls),
        }],
        vec![StreamingSseChunk {
            gate: None,
            body: sse(vec![
                ev_assistant_message("done", "done"),
                ev_completed("done"),
            ]),
        }],
    ])
    .await;
    let server = start_mock_server().await;
    let mut extensions = ExtensionRegistryBuilder::new();
    codex_multi_env_copy_file::install(&mut extensions);
    let base_url = format!("{}/v1", streaming.uri());
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_config(move |config| {
            config.model_provider.base_url = Some(base_url);
            // The gated server captures JSON request bodies for the error snapshot.
            config
                .features
                .disable(Feature::EnableRequestCompression)
                .unwrap();
        })
        .build_with_auto_env(&server)
        .await?;
    let manager = test.thread_manager.environment_manager();
    let mut selections = vec![test.executor_environment().selection().clone()];
    for (id, process) in [("available", &available), ("disconnected", &executor)] {
        manager.upsert_environment(
            id.to_string(),
            format!("{}/?sig=copy-test-secret", process.websocket_url),
            /*connect_timeout*/ None,
        )?;
        manager
            .get_environment(id)
            .context("remote environment")?
            .wait_until_ready()
            .await?;
        selections.push(TurnEnvironmentSelection {
            selected_capability_roots: Default::default(),
            environment_id: id.to_string(),
            cwd: cwd.clone(),
            workspace_roots: vec![cwd.clone()],
            config: EnvironmentConfigState::FromThread,
        });
    }
    let environment = manager
        .get_environment("disconnected")
        .context("remote environment")?;
    let mut state = environment
        .subscribe_connection_state()
        .context("connection state")?;
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "Copy files to and from the disconnected executor.".to_string(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(
                    TurnEnvironmentSelections::new(test.config.cwd.clone(), selections)
                        .into_requests(),
                ),
                ..Default::default()
            }),
        )
        .await?;
    // The model request proves the step captured a connected executor. Drop it before the call.
    streaming.wait_for_request_count(/*count*/ 1).await;
    drop(executor);
    state
        .wait_for(|state| *state == EnvironmentConnectionState::Recovering)
        .await?;
    release_response.send(()).unwrap();
    // Allow the client's recovery window to finish before checking the completed tool results.
    streaming.wait_for_request_count(/*count*/ 2).await;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let requests = streaming.requests().await;
    let request = std::str::from_utf8(&requests[1])?;
    assert!(
        !request.contains("copy-test-secret"),
        "executor URL credential reached model context"
    );
    let body: serde_json::Value = serde_json::from_str(request)?;
    let outputs = body["input"]
        .as_array()
        .context("request input")?
        .iter()
        .filter(|item| item["type"] == "function_call_output")
        .map(|item| json!({"call_id": item["call_id"], "output": item["output"]}))
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), 2);
    insta::assert_json_snapshot!("disconnected_executor", outputs);
    assert_eq!(fs::read(root.path().join("target.bin"))?, b"original");
    streaming.shutdown().await;
    Ok(())
}
