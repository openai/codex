//! Exercises real local, remote, and foreign-OS copy paths through model tool calls.

use super::*;
use codex_config::ConfigLayerSource;
use codex_config::ConfigLayerStack;
use codex_models_manager::bundled_models_response;
use codex_protocol::protocol::TurnEnvironmentSelection;
use core_test_support::TempDirExt;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::context_snapshot::SnapshotEntry;
use core_test_support::responses::ResponsesRequest;
use pretty_assertions::assert_eq;
use test_case::test_case;

/// The selected executor may be local, Docker, or Wine; the second executor is host-native.
/// One flow covers same-environment rejection and both directions across executor boundaries.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn copy_file_transfers_between_selected_and_native_executors() -> Result<()> {
    use core_test_support::exec_server::ExecServerProcess;

    let executor = ExecServerProcess::start().await?;
    let root = TempDir::new()?;
    let server = start_mock_server().await;
    let mut extensions = ExtensionRegistryBuilder::new();
    codex_multi_env_copy_file::install(&mut extensions);
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| {
            // Keep the fixture independent of the checkout's project configuration.
            let stack = &config.config_layer_stack;
            config.config_layer_stack = ConfigLayerStack::new(
                stack
                    .all_layers_low_to_high()
                    .filter(|layer| !matches!(&layer.name, ConfigLayerSource::Project { .. }))
                    .cloned()
                    .collect(),
                stack.requirements().clone(),
                stack.requirements_toml().clone(),
            )
            .expect("fixture config layers");
            config.model_catalog = Some(bundled_models_response().expect("bundled model catalog"));
            config.cloud_skill_enabled = false;
        })
        .build_with_auto_env(&server)
        .await?;
    let manager = test.thread_manager.environment_manager();
    manager.upsert_environment(
        "native".to_string(),
        executor.websocket_url.clone(),
        /*connect_timeout*/ None,
    )?;
    let native = manager
        .get_environment("native")
        .context("native environment")?;
    native.wait_until_ready().await?;
    let native_fs = native.get_filesystem();
    let native_cwd = PathUri::from_host_native_path(root.path())?;
    let primary = test.executor_environment().selection();
    let source = test.workspace_path_uri("source.bin")?;
    let native_source = native_cwd.join("source.bin")?;
    let contents =
        b"file contents that must not enter model context\0\xff".repeat(/*n*/ 30_000);
    for (fs, path) in [(test.fs(), &source), (native_fs.clone(), &native_source)] {
        fs.write_file(
            path,
            contents.clone(),
            Default::default(),
            /*sandbox*/ None,
        )
        .await?;
    }
    let same_target = test.workspace_path_uri("same.bin")?;
    let original = b"leave this file intact";
    test.fs()
        .write_file(
            &same_target,
            original.to_vec(),
            Default::default(),
            /*sandbox*/ None,
        )
        .await?;
    let remote_target = native_cwd.join("target.bin")?;
    let return_target = test.workspace_path_uri("returned.bin")?;
    let cases = [
        (
            "same",
            primary.environment_id.as_str(),
            &source,
            primary.environment_id.as_str(),
            &same_target,
        ),
        (
            "outbound",
            primary.environment_id.as_str(),
            &source,
            "native",
            &remote_target,
        ),
        (
            "inbound",
            "native",
            &native_source,
            primary.environment_id.as_str(),
            &return_target,
        ),
    ];
    let mut calls = Vec::new();
    for (id, source_id, source, target_id, target) in &cases {
        calls.push(ev_function_call_with_namespace(
            id,
            "functions",
            "multi_env_copy_file",
            &json!({
                "source_environment_id": source_id,
                "source_path": source.inferred_native_path_string(),
                "target_environment_id": target_id,
                "target_path": target.inferred_native_path_string(),
            })
            .to_string(),
        ));
    }
    calls.push(ev_completed("copies"));
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
        "Copy the binary artifact within and between environments.",
        Some(vec![
            primary.clone(),
            TurnEnvironmentSelection {
                selected_capability_roots: Default::default(),
                environment_id: "native".to_string(),
                cwd: native_cwd.clone(),
                workspace_roots: vec![native_cwd],
                config: EnvironmentConfigState::FromThread,
            },
        ]),
    )
    .await?;
    for (id, source_id, _, target_id, target) in &cases {
        let output = mock.function_call_output_text(id).context("copy result")?;
        let expected_contents = if source_id == target_id {
            assert!(
                output.contains("source and target environment IDs must differ"),
                "{output}"
            );
            original.as_slice()
        } else {
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&output)?,
                json!({"bytes_copied": contents.len()})
            );
            contents.as_slice()
        };
        let fs = manager
            .get_environment(target_id)
            .context("target environment")?
            .get_filesystem();
        assert_eq!(
            fs.read_file(target, ReadFileOptions::default(), /*sandbox*/ None)
                .await?,
            expected_contents
        );
    }
    let requests = mock.requests();
    assert!(!requests.iter().any(|request| {
        request.body_contains_text("file contents that must not enter model context")
    }));
    // Normalize platform-specific arguments before rendering truncates and fingerprints them.
    let mut bodies = requests
        .iter()
        .map(ResponsesRequest::body_json)
        .collect::<Vec<_>>();
    for body in &mut bodies {
        for item in body["input"].as_array_mut().context("request input")? {
            if item["type"] == "function_call" && item["name"] == "multi_env_copy_file" {
                let mut args: serde_json::Value =
                    serde_json::from_str(item["arguments"].as_str().context("copy arguments")?)?;
                for endpoint in ["source", "target"] {
                    args[format!("{endpoint}_path")] = json!("<ABSOLUTE_PATH>");
                    let id = format!("{endpoint}_environment_id");
                    if args[&id] == primary.environment_id {
                        args[id] = json!("<PRIMARY>");
                    }
                }
                item["arguments"] = json!(args.to_string());
            }
        }
    }
    let snapshot = context_snapshot::format_context_snapshot(
        "File contents stay outside model context. Copies within one environment are rejected.",
        &bodies.iter().map(SnapshotEntry::body).collect::<Vec<_>>(),
        &ContextSnapshotOptions::default()
            .rewrite_known_segments()
            .include_request_settings(),
    );
    insta::assert_snapshot!("multi_env_copy_file", snapshot);
    Ok(())
}

/// Under wine-exec this crosses Linux/Windows with one local endpoint in both directions.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn copy_file_transfers_between_local_and_foreign_executors() -> Result<()> {
    core_test_support::skip_if_no_remote_env!(Ok(()));
    let local_root = TempDir::new()?;
    let server = start_mock_server().await;
    let mut extensions = ExtensionRegistryBuilder::new();
    codex_multi_env_copy_file::install(&mut extensions);
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .build_with_remote_and_local_env(&server)
        .await?;
    let local = core_test_support::test_codex::local(local_root.abs());
    let remote = test.executor_environment().selection().clone();
    let local_source = local.cwd.join("source.bin")?;
    let remote_target = remote.cwd.join("target.bin")?;
    let local_target = local.cwd.join("returned.bin")?;
    let contents = [0, 255, 17, 42].repeat(/*n*/ 300_000);
    fs::write(local_root.path().join("source.bin"), &contents)?;
    let calls = [
        (
            "outbound",
            &local.environment_id,
            &local_source,
            &remote.environment_id,
            &remote_target,
        ),
        (
            "inbound",
            &remote.environment_id,
            &remote_target,
            &local.environment_id,
            &local_target,
        ),
    ]
    .into_iter()
    .map(|(id, source_id, source, target_id, target)| {
        ev_function_call_with_namespace(id, "functions", "multi_env_copy_file", &json!({
            "source_environment_id": source_id, "source_path": source.inferred_native_path_string(),
            "target_environment_id": target_id, "target_path": target.inferred_native_path_string(),
        }).to_string())
    });
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(calls.chain([ev_completed("copies")]).collect()),
            sse(vec![
                ev_assistant_message("done", "done"),
                ev_completed("done"),
            ]),
        ],
    )
    .await;
    test.submit_turn_with_environments(
        "Copy the artifact to the remote environment and back.",
        Some(vec![local, remote]),
    )
    .await?;
    for id in ["outbound", "inbound"] {
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(
                &mock.function_call_output_text(id).context("copy result")?
            )?,
            json!({"bytes_copied": contents.len()})
        );
    }
    assert_eq!(
        test.fs()
            .read_file(
                &remote_target,
                ReadFileOptions::default(),
                /*sandbox*/ None
            )
            .await?,
        contents
    );
    assert_eq!(fs::read(local_root.path().join("returned.bin"))?, contents);
    Ok(())
}

#[test_case(false; "rewrite paths within the selected environments")]
#[test_case(true; "reject swapping source and target")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn copy_hooks_follow_source_and_target(swap_endpoints: bool) -> Result<()> {
    use codex_protocol::models::PermissionProfile;
    use codex_protocol::models::PermissionProfileSnapshot;
    use codex_protocol::permissions::NetworkSandboxPolicy;
    use core_test_support::exec_server::ExecServerProcess;

    let server = start_mock_server().await;
    let mut extensions = ExtensionRegistryBuilder::new();
    codex_multi_env_copy_file::install(&mut extensions);
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| {
            config
                .features
                .enable(Feature::CodexHooks)
                .expect("enable hooks");
        })
        .build_with_auto_env(&server)
        .await?;
    let executor = ExecServerProcess::start().await?;
    let root = TempDir::new()?;
    let source = root.path().join("a/source");
    let original_target = root.path().join("b/original");
    let target = root.path().join("b/rewritten");
    let mut rewrite = json!({"source_path": source, "target_path": target});
    if swap_endpoints {
        rewrite["source_environment_id"] = json!("b");
        rewrite["target_environment_id"] = json!("a");
    }
    let hook_output = json!({"hookSpecificOutput": {
        "hookEventName": "PreToolUse", "permissionDecision": "allow", "updatedInput": rewrite,
    }});
    let manager = test.thread_manager.environment_manager();
    let mut selections = Vec::new();
    // Select the target first to check that the copy still reads from the source.
    for id in ["b", "c", "a"] {
        let dir = root.path().join(id);
        fs::create_dir(&dir)?;
        manager.upsert_environment(
            id.to_string(),
            executor.websocket_url.clone(),
            /*connect_timeout*/ None,
        )?;
        manager
            .get_environment(id)
            .context("test executor")?
            .wait_until_ready()
            .await?;
        let cwd = PathUri::from_host_native_path(&dir)?;
        let mut selection = TurnEnvironmentSelection {
            environment_id: id.to_string(),
            cwd: cwd.clone(),
            workspace_roots: vec![cwd],
            selected_capability_roots: Default::default(),
            config: EnvironmentConfigState::FromThread,
        };
        let mut config = environment_config_for_selection(&test.config, &selection);
        config.permission_profile =
            PermissionProfileSnapshot::legacy(PermissionProfile::workspace_write_with(
                &[],
                NetworkSandboxPolicy::Restricted,
                /*exclude_tmpdir_env_var*/ true,
                /*exclude_slash_tmp*/ true,
            ));
        let hook = |marker, output: &serde_json::Value| {
            let output = output
                .to_string()
                .replace('\'', if cfg!(windows) { "''" } else { "'\\''" });
            let command = if cfg!(windows) {
                // The hook receives its event on the PowerShell pipeline.
                format!(
                    "Out-Null; Add-Content -Path {marker} -Value run -ErrorAction Stop; Write-Output '{output}'"
                )
            } else {
                format!("echo run >> {marker}; printf '%s' '{output}'")
            };
            json!([{"matcher":"multi_env_copy_file","hooks":[{"type":"command","command": command}]}])
        };
        let pass = json!({});
        config.environment_hooks = serde_json::from_value(json!({
            "PreToolUse": hook("pre", if id == "a" { &hook_output } else { &pass }),
            "PostToolUse": hook("post", &pass),
        }))?;
        selection.config = EnvironmentConfigState::Ready(config);
        selections.push(selection);
    }
    fs::write(&source, b"from a")?;
    let args = json!({
        "source_environment_id": "a", "source_path": source,
        "target_environment_id": "b", "target_path": original_target,
    });
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_function_call_with_namespace(
                    "copy",
                    "functions",
                    "multi_env_copy_file",
                    &args.to_string(),
                ),
                ev_completed("copy"),
            ]),
            sse(vec![
                ev_assistant_message("done", "done"),
                ev_completed("done"),
            ]),
        ],
    )
    .await;
    test.submit_turn_with_environments("Copy the file.", Some(selections))
        .await?;
    let output = mock
        .function_call_output_text("copy")
        .context("copy result")?;
    if swap_endpoints {
        assert!(
            output.contains("cannot change the tool environment"),
            "{output}"
        );
        assert!(!target.exists());
    } else {
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&output)
                .with_context(|| format!("expected a successful copy, got {output}"))?,
            json!({"bytes_copied":6})
        );
        assert_eq!(fs::read(&target)?, b"from a");
    }
    assert!(!original_target.exists());
    for id in ["a", "b", "c"] {
        for marker in ["pre", "post"] {
            let path = root.path().join(id).join(marker);
            if id != "c" && (marker == "pre" || !swap_endpoints) {
                let contents = fs::read_to_string(&path)
                    .with_context(|| format!("read hook marker {}", path.display()))?;
                assert_eq!(contents.lines().count(), 1);
            } else {
                assert!(!path.exists(), "unexpected hook at {}", path.display());
            }
        }
    }
    Ok(())
}
