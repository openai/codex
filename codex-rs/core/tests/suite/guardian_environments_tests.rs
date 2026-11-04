//! Reviews bind each action to its captured target policy while reusing the reviewer context.

use super::*;
use codex_core::context::UserGoalUpdate;
use codex_history::RolloutItem;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::protocol::GuardianAssessmentStatus;
use codex_protocol::protocol::TurnEnvironmentRequests;
use codex_utils_cargo_bin::cargo_bin;
use codex_utils_path_uri::PathUri;
use core_test_support::exec_server::ExecServerProcess;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use pretty_assertions::assert_eq;
use test_case::test_case;
use tokio_util::task::AbortOnDropHandle;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_partial_json;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn direct_mcp_approval_uses_the_server_owners_denied_paths() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let workspace = TempDir::new()?;
    let owner_temp = TempDir::new()?;
    let denied = PathUri::from_abs_path(&owner_temp.path().abs().canonicalize()?)
        .inferred_native_path_string();
    // Give the remote executor its own temp directory on both Unix and Windows.
    let executor = ExecServerProcess::start_with_env(&[
        ("TMPDIR", &denied),
        ("TEMP", &denied),
        ("TMP", &denied),
    ])
    .await?;
    let mcp_servers = serde_json::from_value(json!({
        "files": {
            "command": cargo_bin("test_stdio_server")?,
            "cwd": workspace.path(),
            "environment_id": REMOTE_ENVIRONMENT_ID,
            "default_tools_approval_mode": "prompt",
        }
    }))?;
    let test = test_codex()
        .with_exec_server_url(executor.websocket_url.clone())
        .with_config(move |config| {
            // This test reviews the denied paths without reading project documents under them.
            config.project_doc_max_bytes = 0;
            config.mcp_servers.set(mcp_servers).expect("set MCP server");
            config.mcp_optional_startup_grace = Duration::ZERO;
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config
                .features
                .disable(Feature::GuardianV2)
                .expect("use synchronous Guardian");
        })
        .build_with_remote_and_local_env(&server)
        .await?;

    let primary = local(workspace.path().abs());
    let mut owner = primary.clone();
    owner.environment_id = REMOTE_ENVIRONMENT_ID.to_string();
    let mut owner_config = environment_config_for_selection(&test.config, &owner);
    owner.config = EnvironmentConfigState::Ready(owner_config.clone());
    submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            environments: Some(TurnEnvironmentRequests::new(
                test.config.cwd.clone(),
                vec![primary.into_request(), owner.clone().into_request()],
            )),
            ..Default::default()
        },
    )
    .await?;

    let (release, gate) = tokio::sync::oneshot::channel();
    let (parent, _) = start_streaming_sse_server(vec![vec![StreamingSseChunk {
        gate: Some(gate),
        body: sse(vec![
            ev_function_call_with_namespace("call", "mcp__files", "echo", r#"{"message":"hi"}"#),
            ev_completed("parent"),
        ]),
    }]])
    .await;
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(
            ResponseTemplate::new(307)
                .insert_header("location", format!("{}/v1/responses", parent.uri())),
        )
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_assistant_message("review", r#"{"outcome":"deny"}"#),
                ev_completed("review"),
            ]),
            sse(vec![ev_completed("done")]),
        ],
    )
    .await;
    let mut policy = FileSystemSandboxPolicy::read_only();
    policy.entries.push(FileSystemSandboxEntry::new(
        FileSystemPath::Special {
            value: FileSystemSpecialPath::Tmpdir,
        },
        FileSystemAccessMode::Deny,
    ));
    owner_config.permission_profile = PermissionProfileSnapshot::legacy(
        PermissionProfile::from_runtime_permissions(&policy, NetworkSandboxPolicy::Restricted),
    );
    // The step didn't see the deny rule. MCP receives it before preparing the call.
    tokio::try_join!(test.submit_text_turn("Call the files tool."), async {
        parent.wait_for_request_count(/*count*/ 1).await;
        test.codex.environment_ready(&owner, owner_config).await?;
        release.send(()).expect("let the model call the tool");
        Ok(())
    })?;
    let review = responses
        .requests()
        .into_iter()
        .find(|request| request.body_json()["client_metadata"]["x-openai-subagent"] == "guardian")
        .context("expected a Guardian review")?;
    let owner_label = format!("permission profile for environment {REMOTE_ENVIRONMENT_ID:?}");
    let permissions = review
        .message_input_texts("user")
        .into_iter()
        .find(|text| text.contains(&owner_label))
        .context("expected the MCP server owner's permission rules")?;
    assert!(permissions.contains(&denied), "{permissions}");
    Ok(())
}

#[test_case("exec_command")]
#[test_case("apply_patch")]
#[test_case("request_permissions")]
#[test_case("mcp_tool_call"; "mcp_owner_does_not_reuse_primary_score")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_reviews_target_environment_and_reuses_prefix(tool: &str) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let executor_url = format!("ws://{}", listener.local_addr()?);
    let (attach, connection) = tokio::sync::oneshot::channel();
    let (shutdown, stop) = tokio::sync::oneshot::channel();
    let executor = tokio_util::task::AbortOnDropHandle::new(tokio::spawn(
        serve_environment_with_agents_md(listener, "", connection, stop),
    ));
    attach.send(()).expect("attach secondary executor");

    let server = start_mock_server().await;
    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            std::fs::write(
                home.join("config.toml"),
                "[features.guardianv2]\nenabled = true\npersist_scores = true\n\n[features.guardianv2.review_scope]\ncomputer_use_only = false\n",
            )
            .expect("configure asynchronous review");
        })
        .with_model_info_override("guardian-environments-parent", |model| {
            model.guardian = None;
            model.auto_review_model_override = Some("gpt-5.5".to_owned());
        })
        .with_config(|config| {
            config.project_doc_max_bytes = 0;
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config
                .permissions
                .set_permission_profile(PermissionProfile::read_only())
                .expect("set read-only permissions");
            config
                .features
                .enable(Feature::RequestPermissionsTool)
                .expect("enable permission requests");
            config
                .features
                .disable(Feature::DeferredExecutor)
                .expect("disable deferred executor");
        });
    let test = if tool == "mcp_tool_call" {
        // The MCP server runs on the host, not on the mock executor.
        let servers = serde_json::from_value(json!({
            "files": {
                "command": cargo_bin("test_stdio_server")?,
                "environment_id": LOCAL_ENVIRONMENT_ID,
                "default_tools_approval_mode": "prompt",
            }
        }))?;
        builder
            .with_config(move |config| config.mcp_servers.set(servers).expect("set MCP server"))
            .build(&server)
            .await?
    } else {
        builder.build_with_auto_env(&server).await?
    };
    let manager = test.thread_manager.environment_manager();
    let secondary_id = "guardian-secondary";
    manager.upsert_environment(
        secondary_id.to_string(),
        executor_url,
        /*connect_timeout*/ None,
    )?;
    manager
        .get_environment(secondary_id)
        .context("secondary executor")?
        .wait_until_ready()
        .await?;

    let mut primary = test.executor_environment().selection().clone();
    primary.config =
        EnvironmentConfigState::Ready(environment_config_for_selection(&test.config, &primary));
    let mut secondary = primary.clone();
    secondary.environment_id = secondary_id.to_string();
    let denied = secondary.cwd.join("secondary-private")?;
    let mut secondary_config = environment_config_for_selection(&test.config, &secondary);
    let mut file_system = secondary_config
        .permission_profile
        .permission_profile()
        .file_system_sandbox_policy();
    file_system.entries.push(FileSystemSandboxEntry::new(
        denied.clone().into(),
        FileSystemAccessMode::Deny,
    ));
    secondary_config.permission_profile = PermissionProfileSnapshot::legacy(
        PermissionProfile::from_runtime_permissions(&file_system, NetworkSandboxPolicy::Restricted),
    );
    secondary.config = EnvironmentConfigState::Ready(secondary_config);

    let targets = [secondary_id, primary.environment_id.as_str()];
    let tools = if tool == "mcp_tool_call" {
        ["exec_command", tool]
    } else {
        [tool; 2]
    };
    let mut events = Vec::new();
    for (index, (environment_id, tool)) in targets.iter().zip(tools).enumerate() {
        let call_id = format!("action-{index}");
        let action = match tool {
            "exec_command" => ev_function_call(
                &call_id,
                tool,
                &json!({
                    "environment_id": environment_id,
                    "cmd": "echo review-only",
                    "sandbox_permissions": "require_escalated",
                    "justification": "Review the target environment.",
                })
                .to_string(),
            ),
            "apply_patch" => ev_apply_patch_custom_tool_call(
                &call_id,
                &format!(
                    "*** Begin Patch\n*** Environment ID: {environment_id}\n*** Add File: guardian-marker.txt\n+review-only\n*** End Patch\n"
                ),
            ),
            "request_permissions" => ev_function_call(
                &call_id,
                tool,
                &json!({
                    "environment_id": environment_id,
                    "permissions": {"network": {"enabled": true}},
                    "reason": "Review the target environment.",
                })
                .to_string(),
            ),
            "mcp_tool_call" => ev_function_call_with_namespace(
                &call_id,
                "mcp__files",
                "echo",
                r#"{"message":"hi"}"#,
            ),
            _ => unreachable!(),
        };
        events.push(sse(vec![action, ev_completed(&call_id)]));
        let review_id = format!("review-{index}");
        events.push(sse(vec![
            ev_assistant_message(&review_id, r#"{"outcome":"deny"}"#),
            ev_completed(&review_id),
        ]));
    }
    events.push(sse(vec![
        ev_assistant_message("done", "done"),
        ev_completed("done"),
    ]));
    // Hold the second action until the first score is published. Its own classifier
    // response stays pending, so only the score for the other computer is available.
    let (advance, next_action) = tokio::sync::oneshot::channel();
    let mut next_action = Some(next_action);
    let (parent, _) = start_streaming_sse_server(
        events
            .iter()
            .step_by(2)
            .enumerate()
            .map(|(index, body)| {
                vec![StreamingSseChunk {
                    gate: if index == 1 { next_action.take() } else { None },
                    body: body.clone(),
                }]
            })
            .collect(),
    )
    .await;
    let (publish, score_ready) = tokio::sync::oneshot::channel();
    let (_hold_score, pending_score) = tokio::sync::oneshot::channel();
    let (classifier, _) = start_streaming_sse_server(vec![
        vec![StreamingSseChunk {
            gate: Some(score_ready),
            body: sse(vec![
                ev_assistant_message("score-0", "low"),
                ev_completed("score-0"),
            ]),
        }],
        vec![StreamingSseChunk {
            gate: Some(pending_score),
            body: sse(vec![
                ev_assistant_message("score-1", "low"),
                ev_completed("score-1"),
            ]),
        }],
    ])
    .await;
    for (model, destination) in [
        ("guardian-environments-parent", parent.uri()),
        ("gpt-5.6-luna", classifier.uri()),
    ] {
        Mock::given(method("POST"))
            .and(path("/v1/responses"))
            .and(body_partial_json(json!({"model": model})))
            .respond_with(
                ResponseTemplate::new(/*s*/ 307)
                    .insert_header("location", format!("{destination}/v1/responses")),
            )
            .with_priority(/*p*/ 1)
            .mount(&server)
            .await;
    }
    let responses =
        mount_sse_sequence(&server, events.into_iter().skip(1).step_by(2).collect()).await;
    let mut environments = vec![primary.clone().into_request(), secondary.into_request()];
    if tool == "mcp_tool_call" {
        // The primary produced the low score; the MCP server belongs to the other computer.
        environments.reverse();
    }
    test.codex.ensure_rollout_materialized().await;
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "Review each action on its requested environment.".to_string(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(TurnEnvironmentRequests::new(
                    test.config.cwd.clone(),
                    environments,
                )),
                ..Default::default()
            }),
        )
        .await?;
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        parent.wait_for_request_count(/*count*/ 2).await;
        publish.send(()).expect("publish the first computer score");
        loop {
            let history = test.codex.load_history(/*include_archived*/ false).await?;
            if history.items.into_iter().any(|item| {
                matches!(item, RolloutItem::SecurityRiskScore(score) if score.call_id.as_deref() == Some("action-0"))
            }) {
                return Ok::<_, anyhow::Error>(());
            }
            tokio::task::yield_now().await;
        }
    })
    .await??;
    advance
        .send(())
        .expect("start action on the other computer");
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = responses.requests();
    let reviews = requests
        .iter()
        .filter(|request| request.body_json()["client_metadata"]["x-openai-subagent"] == "guardian")
        .collect::<Vec<_>>();
    assert_eq!(reviews.len(), targets.len());
    for ((review, environment_id), tool) in reviews.iter().zip(targets).zip(tools) {
        let groups = review.message_input_text_groups("user");
        let latest = groups.last().context("current review input")?;
        assert_eq!(
            latest
                .concat()
                .matches("user: Review each action on its requested environment.")
                .count(),
            usize::from(environment_id == secondary_id),
            "the transcript delivers the instruction once, only in the first review"
        );
        let start = latest
            .iter()
            .position(|text| text == "\n>>> PARENT TURN PERMISSION CONTEXT START\n")
            .context("permission context")?;
        let permissions = &latest[start + 1];
        assert!(permissions.contains(&format!("environment {environment_id:?}")));
        if environment_id == secondary_id {
            assert!(permissions.contains(&denied.inferred_native_path_string()));
        } else {
            assert!(permissions.contains("no explicit denied-read"));
            assert!(!permissions.contains(&denied.inferred_native_path_string()));
        }
        let action = latest
            .iter()
            .find_map(|text| serde_json::from_str::<Value>(text).ok())
            .context("planned action JSON")?;
        assert_eq!(action["tool"], json!(tool));
        if tool == "mcp_tool_call" {
            assert_eq!(action["server"], json!("files"));
        } else {
            assert_eq!(action["environment_id"], json!(environment_id));
        }
    }
    assert_eq!(
        reviews[0].body_json()["client_metadata"]["thread_id"],
        reviews[1].body_json()["client_metadata"]["thread_id"]
    );
    assert!(
        reviews[1].input().starts_with(&reviews[0].input()),
        "earlier Guardian input must remain unchanged"
    );
    let classifier_requests = classifier.requests().await;
    let request: Value = serde_json::from_slice(&classifier_requests[0])?;
    let input = request["input"].to_string();
    assert!(input.contains(secondary_id));
    assert!(input.contains("secondary-private"));
    test.codex.shutdown_and_wait().await?;
    parent.shutdown().await;
    classifier.shutdown().await;
    shutdown.send(()).expect("stop secondary executor");
    executor.await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_reviews_with_offline_primary_executor() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let executor_url = format!("ws://{}", listener.local_addr()?);
    let (attach, connection) = tokio::sync::oneshot::channel();
    let (disconnect, stop) = tokio::sync::oneshot::channel();
    let executor = AbortOnDropHandle::new(tokio::spawn(serve_environment_with_instruction_files(
        listener,
        "",
        Some(
            "---\nname: guardian-fixture-skill\ndescription: Source executor skill catalog entry.\n---\nUse the source skill.\n",
        ),
        connection,
        stop,
    )));
    attach.send(()).expect("attach primary executor");

    let environment_id = "guardian-primary";
    let (request_permission, pending_permission) = tokio::sync::oneshot::channel();
    let (model, _) = start_streaming_sse_server(vec![
        vec![StreamingSseChunk {
            gate: Some(pending_permission),
            body: sse(vec![
                ev_function_call(
                    "permission",
                    "request_permissions",
                    &json!({
                        "environment_id": environment_id,
                        "permissions": {"network": {"enabled": true}},
                        "reason": "Review permission while the executor is offline.",
                    })
                    .to_string(),
                ),
                ev_completed("parent"),
            ]),
        }],
        vec![StreamingSseChunk {
            gate: None,
            body: sse(vec![
                ev_assistant_message("deny", r#"{"outcome":"deny"}"#),
                ev_completed("review"),
            ]),
        }],
        vec![StreamingSseChunk {
            gate: None,
            body: sse(vec![ev_completed("done")]),
        }],
    ])
    .await;

    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    codex_skills_extension::install(&mut extensions, |config: &Config| {
        codex_skills_extension::SkillsExtensionConfig {
            include_instructions: config.include_skill_instructions,
            catalog_deltas_enabled: config
                .features
                .enabled(codex_features::Feature::SkillCatalogDeltas),
            omit_descriptions: false,
            max_context_tokens: config.skill_max_context_tokens,
            bundled_skills_enabled: false,
            cloud_skill_enabled: false,
            shadow_selection_enabled: false,
        }
    });
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_model_info_override("guardian-offline-skills-parent", |info| {
            info.guardian = None;
            info.auto_review_model_override = Some(info.slug.clone());
        })
        .with_config(|config| {
            // Isolate skill discovery from the unchanged repository-instruction discovery path.
            config.project_doc_max_bytes = 0;
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            // Start the reviewer only after the executor goes offline, not during prewarm.
            config.approvals_reviewer = ApprovalsReviewer::User;
            config
                .permissions
                .set_permission_profile(PermissionProfile::read_only())
                .expect("set read-only permissions");
            config
                .features
                .enable(Feature::RequestPermissionsTool)
                .expect("enable permission requests");
            config
                .features
                .disable(Feature::SkipHostSkillDiscovery)
                .expect("keep host skill discovery enabled for the parent");
            config
                .features
                .disable(Feature::DeferredExecutor)
                .expect("disable deferred executor");
        })
        // This test supplies its own executor mock, using host-native paths.
        .build_with_streaming_server(&model)
        .await?;
    let manager = test.thread_manager.environment_manager();
    manager.upsert_environment(
        environment_id.to_owned(),
        executor_url,
        /*connect_timeout*/ None,
    )?;
    manager
        .get_environment(environment_id)
        .context("primary executor")?
        .wait_until_ready()
        .await?;
    let mut primary = test.executor_environment().selection().clone();
    primary.environment_id = environment_id.to_owned();
    primary.config =
        EnvironmentConfigState::Ready(environment_config_for_selection(&test.config, &primary));

    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "Do not grant network permission.".to_owned(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                approvals_reviewer: Some(ApprovalsReviewer::AutoReview),
                environments: Some(TurnEnvironmentRequests::new(
                    test.config.cwd.clone(),
                    vec![primary.into_request()],
                )),
                ..Default::default()
            }),
        )
        .await?;
    timeout(
        Duration::from_secs(/*secs*/ 10),
        model.wait_for_request_count(/*count*/ 1),
    )
    .await
    .context("parent must discover skills and reach the model while online")?;
    disconnect.send(()).expect("disconnect primary executor");
    executor.await?;
    request_permission.send(()).expect("request permission");

    timeout(
        Duration::from_secs(/*secs*/ 10),
        model.wait_for_request_count(/*count*/ 2),
    )
    .await
    .context("Guardian startup and turn creation must not wait for host skill discovery")?;
    let requests = model.requests().await;
    let parent: Value = serde_json::from_slice(&requests[0])?;
    let review: Value = serde_json::from_slice(&requests[1])?;
    assert!(
        parent["input"]
            .to_string()
            .contains("guardian-fixture-skill"),
        "parent must advertise the skill discovered on its executor"
    );
    assert_eq!(
        review["client_metadata"]["x-openai-subagent"],
        json!("guardian")
    );
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::GuardianAssessment(assessment)
            if assessment.status == GuardianAssessmentStatus::Denied)
    })
    .await;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    test.codex.shutdown_and_wait().await?;
    model.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_revalidates_allow_with_offline_secondary_executor() -> Result<()> {
    skip_if_no_network!(Ok(()));
    const PROJECT_INSTRUCTIONS: &str =
        "Repository rule: network access requires explicit approval.";
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let executor_url = format!("ws://{}", listener.local_addr()?);
    let (attach, connection) = tokio::sync::oneshot::channel();
    let (disconnect, stop) = tokio::sync::oneshot::channel();
    let executor = AbortOnDropHandle::new(tokio::spawn(serve_environment_with_agents_md(
        listener,
        PROJECT_INSTRUCTIONS,
        connection,
        stop,
    )));
    attach.send(()).expect("attach secondary executor");

    let server = start_mock_server().await;
    let test = test_codex()
        .with_model_info_override("guardian-offline-parent", |info| {
            info.guardian = None;
            info.auto_review_model_override = Some(info.slug.clone());
        })
        .with_config(|config| {
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config
                .permissions
                .set_permission_profile(PermissionProfile::read_only())
                .expect("set read-only permissions");
            config
                .features
                .enable(Feature::RequestPermissionsTool)
                .expect("enable permission requests");
            config
                .features
                .disable(Feature::DeferredExecutor)
                .expect("disable deferred executor");
            config
                .features
                .disable(Feature::CwdRelativeTurnDiffs)
                .expect("exercise Git-relative diff display");
        })
        .build_with_auto_env(&server)
        .await?;
    let manager = test.thread_manager.environment_manager();
    let secondary_id = "guardian-secondary";
    manager.upsert_environment(
        secondary_id.to_owned(),
        executor_url,
        /*connect_timeout*/ None,
    )?;
    manager
        .get_environment(secondary_id)
        .context("secondary executor")?
        .wait_until_ready()
        .await?;

    let mut primary = test.executor_environment().selection().clone();
    primary.config =
        EnvironmentConfigState::Ready(environment_config_for_selection(&test.config, &primary));
    let mut secondary = primary.clone();
    secondary.environment_id = secondary_id.to_owned();
    secondary.config =
        EnvironmentConfigState::Ready(environment_config_for_selection(&test.config, &secondary));
    let (allow, pending_allow) = tokio::sync::oneshot::channel();
    let (model, _) = start_streaming_sse_server(vec![
        vec![StreamingSseChunk {
            gate: None,
            body: sse(vec![
                ev_function_call(
                    "permission",
                    "request_permissions",
                    &json!({
                        "environment_id": primary.environment_id,
                        "permissions": {"network": {"enabled": true}},
                        "reason": "Review the primary executor only.",
                    })
                    .to_string(),
                ),
                ev_completed("parent"),
            ]),
        }],
        vec![StreamingSseChunk {
            gate: Some(pending_allow),
            body: sse(vec![
                ev_assistant_message(
                    "allow",
                    r#"{"outcome":"allow","rationale":"Superseded network approval"}"#,
                ),
                ev_completed("first-review"),
            ]),
        }],
        vec![StreamingSseChunk {
            gate: None,
            body: sse(vec![
                ev_assistant_message(
                    "updated-review",
                    r#"{"outcome":"deny","rationale":"Apply the updated authorization."}"#,
                ),
                ev_completed("retry-review"),
            ]),
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
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(
            ResponseTemplate::new(/*s*/ 307)
                .insert_header("location", format!("{}/v1/responses", model.uri())),
        )
        .mount(&server)
        .await;
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "Approve network permission for the primary executor.".to_owned(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(TurnEnvironmentRequests::new(
                    test.config.cwd.clone(),
                    vec![primary.into_request(), secondary.into_request()],
                )),
                ..Default::default()
            }),
        )
        .await?;
    timeout(
        Duration::from_secs(10),
        model.wait_for_request_count(/*count*/ 2),
    )
    .await
    .context("first Guardian request")?;
    let first: Value = serde_json::from_slice(&model.requests().await[1])?;
    assert_eq!(
        first["client_metadata"]["x-openai-subagent"],
        json!("guardian")
    );
    assert!(first["input"].to_string().contains(PROJECT_INSTRUCTIONS));

    // Invalidate the pending allow, then disconnect the unrelated executor. The
    // retry must still reach the model and respect the updated authorization.
    let guidance = "Do not grant network permission after all.";
    let mut expected_authorization = test.codex.guardian_authorization_version().await;
    test.codex
        .record_user_goal_update(UserGoalUpdate::Set {
            objective: Some(guidance.to_owned()),
            status: None,
        })
        .await?;
    expected_authorization.user_message_revision += 1;
    assert_eq!(
        test.codex.guardian_authorization_version().await,
        expected_authorization
    );
    disconnect.send(()).expect("disconnect secondary executor");
    executor.await?;
    allow.send(()).expect("complete stale allow");

    timeout(
        Duration::from_secs(10),
        model.wait_for_request_count(/*count*/ 3),
    )
    .await
    .context("Guardian retry must not wait for the offline secondary executor")?;
    let retry: Value = serde_json::from_slice(&model.requests().await[2])?;
    assert_eq!(
        retry["client_metadata"]["x-openai-subagent"],
        json!("guardian")
    );
    let retry_input = retry["input"].to_string();
    assert!(retry_input.contains(guidance));
    assert!(retry_input.contains(PROJECT_INSTRUCTIONS));
    assert!(!retry_input.contains("Superseded network approval"));
    let status = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::GuardianAssessment(assessment)
            if assessment.status != GuardianAssessmentStatus::InProgress =>
        {
            Some(assessment.status)
        }
        _ => None,
    })
    .await;
    assert_eq!(status, GuardianAssessmentStatus::Denied);
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    test.codex.shutdown_and_wait().await?;
    model.shutdown().await;
    Ok(())
}
