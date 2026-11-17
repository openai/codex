//! Original evidence is routed by a retained housekeeper conversation into a real board.

#![allow(clippy::expect_used)]

use chrono::Utc;
use codex_guardian_v3::EvidenceCursor;
use codex_guardian_v3::HOUSEKEEPER_INSTRUCTIONS;
use codex_guardian_v3::Housekeeper;
use codex_guardian_v3::LiveDelivery;
use codex_guardian_v3::local_board;
use codex_history::CodexHarnessMetadata;
use codex_history::ResponseItemEnvelope;
use codex_history::RolloutItem;
use codex_message_board::AgentMessageBoard;
use codex_message_board::ChannelQuery;
use codex_message_board::PageRequest;
use codex_message_board::PostQuery;
use codex_message_board::ReadPostRequest;
use codex_message_board::SortDirection;
use codex_message_board::UpsertChannelRequest;
use codex_protocol::ThreadId;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::openai_models::TruncationPolicyConfig;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_state::SqliteConfig;
use codex_thread_store::AppendThreadItemsParams;
use codex_thread_store::PersistContext;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::responses::*;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

#[path = "common/board_host.rs"]
mod board_host;

use board_host::PausedClockHost;
use board_host::board_host;

enum RoutingCase {
    Success,
    RetryExhausted,
    WindowPressure,
    WriteFailure,
    MissingNewDescription,
    UnroutedDescription,
    UpsertDeadline,
    InvalidResponse,
}

#[test_case::test_case(RoutingCase::Success; "transient_failures_route_each_original_once")]
#[test_case::test_case(RoutingCase::RetryExhausted; "exhausted_retries_leave_the_batch_pending")]
#[test_case::test_case(RoutingCase::WindowPressure; "evicts_only_completed_batches")]
#[test_case::test_case(RoutingCase::WriteFailure; "write_failure")]
#[test_case::test_case(RoutingCase::MissingNewDescription; "new_destination_needs_description")]
#[test_case::test_case(RoutingCase::UnroutedDescription; "only_routed_channels_can_be_edited")]
#[test_case::test_case(RoutingCase::UpsertDeadline; "stalled_channel_creation")]
#[test_case::test_case(RoutingCase::InvalidResponse; "redacted_model_error")]
#[tokio::test]
async fn routes_captured_originals_and_updates_descriptions(
    case: RoutingCase,
) -> anyhow::Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let input_policy = TruncationPolicyConfig::bytes(/*limit*/ 1024);
    let window_pressure = matches!(case, RoutingCase::WindowPressure);
    let test = test_codex()
        .with_history_mode(ThreadHistoryMode::Paginated)
        .with_model_info_override("gpt-5.6-luna", move |model| {
            model.truncation_policy = input_policy;
            if window_pressure {
                model.context_window = Some(4096);
            }
        })
        .with_config(|config| {
            config.approvals_reviewer = ApprovalsReviewer::User;
            config.include_environment_context = false;
            config.model_provider.supports_websockets = true;
        })
        .build_with_auto_env(&server)
        .await?;
    let owner = test.session_configured.thread_id;
    test.thread_store
        .persist_thread(owner, PersistContext::Standard)
        .await?;
    test.thread_store.flush_thread(owner).await?;
    let cursor = EvidenceCursor::from_now(test.thread_store.as_ref(), owner).await?;
    let initial_cursor = cursor.clone();
    let original = ResponseItemEnvelope {
        item: serde_json::from_value(json!({"type":"message","id":"msg-policy","role":"user",
            "content":[{"type":"input_text","text":"Do not publish the release."}]}))?,
        metadata: Some(CodexHarnessMetadata {
            user_input_order: Some(1),
            ..Default::default()
        }),
    };
    test.thread_store
        .append_items(AppendThreadItemsParams {
            thread_id: owner,
            items: vec![RolloutItem::ResponseItem(original.clone())],
        })
        .await?;
    test.thread_store
        .persist_thread(owner, PersistContext::Standard)
        .await?;
    test.thread_store.flush_thread(owner).await?;
    let snapshot = cursor.capture(test.thread_store.as_ref()).await?;
    assert_eq!(snapshot.records().len(), 1);
    for user_input_order in 2..=3 {
        let later = ResponseItemEnvelope {
            item: serde_json::from_value(
                json!({"type":"message","id":format!("msg-later-{user_input_order}"),"role":"user",
                "content":[{"type":"input_text","text":"The later message is outside this capture.".repeat(8)}]}),
            )?,
            metadata: Some(CodexHarnessMetadata {
                user_input_order: Some(user_input_order),
                ..Default::default()
            }),
        };
        test.thread_store
            .append_items(AppendThreadItemsParams {
                thread_id: owner,
                items: vec![RolloutItem::ResponseItem(later)],
            })
            .await?;
    }
    test.thread_store.flush_thread(owner).await?;
    let dir = tempfile::tempdir()?;
    let host = board_host(owner);
    let gated_host = Arc::new(PausedClockHost {
        inner: host.clone(),
        pause_next: AtomicBool::new(false),
        started: Notify::new(),
        release: Notify::new(),
    });
    let board = Arc::new(
        local_board(
            SqliteConfig::new_for_testing(dir.path().to_path_buf().try_into()?),
            owner.into(),
        )
        .read_write()
        .open(gated_host.clone())
        .await?,
    );
    board
        .upsert_channel(
            owner,
            UpsertChannelRequest {
                channel_name: "release".into(),
                description: "Release procedure".to_owned().try_into()?,
            },
        )
        .await?;
    let board_handle: Arc<dyn AgentMessageBoard> = board.clone();
    let mut housekeeper = Housekeeper::new(
        board_handle,
        cursor,
        Arc::new(LiveDelivery::new(owner)),
        &test.codex,
        &test.thread_manager,
    )
    .await?;
    let cancellation = CancellationToken::new();
    let reply = match case {
        RoutingCase::MissingNewDescription => {
            r#"{"routes":[["release","_audit"]],"descriptions":{"release":"Release policy"}}"#
        }
        RoutingCase::UnroutedDescription => {
            r#"{"routes":[["_audit"]],"descriptions":{"_audit":"","release":"Unrelated edit"}}"#
        }
        RoutingCase::InvalidResponse => r#"{"private-evidence-canary":true}"#,
        RoutingCase::Success
        | RoutingCase::RetryExhausted
        | RoutingCase::WindowPressure
        | RoutingCase::WriteFailure
        | RoutingCase::UpsertDeadline => {
            r#"{"routes":[["release","_audit"]],"descriptions":{"release":"Release policy","_audit":""}}"#
        }
    };
    let mut events = Vec::new();
    if window_pressure {
        events.push(ev_reasoning_item(
            "large-reasoning",
            &[],
            &[&"r".repeat(24 * 1024)],
        ));
    }
    events.extend([
        ev_assistant_message("routing", reply),
        ev_completed("routing"),
    ]);
    let retry_case = matches!(case, RoutingCase::Success | RoutingCase::RetryExhausted);
    let retry_arrivals = Arc::new(Mutex::new(Vec::new()));
    let response = if retry_case {
        let mut error = json!({"code": "server_is_overloaded"});
        if matches!(case, RoutingCase::RetryExhausted) {
            error["headers"] = json!({"Retry-After": "1"});
        }
        let failed = sse(vec![
            ev_assistant_message("discarded", "partial reply"),
            json!({"type": "response.failed", "response": {"error": error}}),
        ]);
        if matches!(case, RoutingCase::RetryExhausted) {
            let arrivals = Arc::clone(&retry_arrivals);
            mount_response_handler(&server, move |_| {
                arrivals
                    .lock()
                    .expect("retry arrivals")
                    .push(Instant::now());
                sse_response(failed.clone())
            })
            .await
        } else {
            mount_sse_sequence(&server, vec![failed.clone(), failed, sse(events)]).await
        }
    } else {
        mount_sse_once(&server, sse(events)).await
    };
    let calls_before = host.agent_path_calls.load(Ordering::SeqCst);
    host.fail_clock
        .store(matches!(case, RoutingCase::WriteFailure), Ordering::SeqCst);
    let route = housekeeper.route_next(&snapshot, &cancellation);
    let routed = if matches!(case, RoutingCase::UpsertDeadline) {
        gated_host.pause_next.store(true, Ordering::SeqCst);
        tokio::pin!(route);
        tokio::select! {
            () = gated_host.started.notified() => {}
            _ = &mut route => anyhow::bail!("routing ended before creating the channel"),
        }
        // The catalog read and model call finished; only the new channel's timestamp is blocked.
        tokio::time::pause();
        tokio::time::advance(std::time::Duration::from_secs(90)).await;
        tokio::time::resume();
        tokio::time::timeout(std::time::Duration::from_secs(5), route).await?
    } else {
        route.await
    };
    let attempts = response.requests();
    let (request, previous) = attempts.split_last().expect("housekeeper request");
    if retry_case {
        let body = request.body_json();
        for attempt in previous {
            assert_eq!(attempt.body_json(), body);
        }
        let posts = received_responses_requests(&server).await;
        assert_eq!(
            posts
                .iter()
                .filter(|req| req.header("x-openai-subagent").as_deref() == Some("guardian"))
                .count(),
            3
        );
        if matches!(case, RoutingCase::RetryExhausted) {
            let arrivals = retry_arrivals.lock().expect("retry arrivals");
            for pair in arrivals.windows(2) {
                assert!(
                    pair[1] - pair[0] >= Duration::from_secs(1),
                    "retried before Retry-After"
                );
            }
        }
    }
    insta::assert_snapshot!(
        "guardian_housekeeping_originals",
        context_snapshot::format_request_history_snapshot(
            "Housekeeper routes captured originals and updates discovery descriptions.",
            std::slice::from_ref(request),
            &ContextSnapshotOptions::default().rewrite_known_segments(),
        )
    );
    if !matches!(case, RoutingCase::Success | RoutingCase::WindowPressure) {
        let error = routed.expect_err("routing should fail");
        if matches!(case, RoutingCase::InvalidResponse) {
            let diagnostic = format!("{error:#}");
            assert!(diagnostic.contains("Data at line 1 column"));
            assert!(!diagnostic.contains("private-evidence-canary"));
        } else if matches!(
            case,
            RoutingCase::MissingNewDescription
                | RoutingCase::UnroutedDescription
                | RoutingCase::RetryExhausted
        ) {
            // Failed batches stop after the catalog read, before any board writes.
            assert_eq!(
                host.agent_path_calls.load(Ordering::SeqCst) - calls_before,
                1
            );
        } else if matches!(case, RoutingCase::UpsertDeadline) {
            assert!(error.to_string().contains("TimedOut"));
        }
        assert_eq!(housekeeper.cursor(), &initial_cursor);
        assert_eq!(housekeeper.cursor().pending(&snapshot)?.len(), 1);
        test.codex.shutdown_and_wait().await?;
        return Ok(());
    }
    assert_eq!(routed?, 1);
    assert_eq!(housekeeper.route_next(&snapshot, &cancellation).await?, 0);
    assert!(housekeeper.cursor().pending(&snapshot)?.is_empty());
    let next = housekeeper
        .cursor()
        .capture(test.thread_store.as_ref())
        .await?;
    assert_eq!(next.records().len(), 2);
    // The HTTP-only server rejected an upgrade from this housekeeper before accepting its POST.
    let requests = server.received_requests().await.unwrap_or_default();
    assert!(requests.iter().any(|req| {
        req.method == http::Method::GET
            && req.url.path().ends_with("/responses")
            && req
                .headers
                .get("x-openai-subagent")
                .is_some_and(|value| value == "guardian")
    }));
    assert_eq!(request.body_json()["text"]["format"]["strict"], false);
    assert!(
        request.body_json()["tools"]
            .as_array()
            .is_none_or(Vec::is_empty)
    );
    let input = request.inputs_of_type("function_call_output");
    assert_eq!(input.len(), 1);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            input[0]["output"].as_str().expect("operation input")
        )?,
        json!({"catalog":{"release":"Release procedure"},"sources":[serde_json::to_value(&snapshot.records()[0])?]})
    );
    assert!(
        request
            .message_input_texts("developer")
            .iter()
            .any(|text| text.contains(HOUSEKEEPER_INSTRUCTIONS))
    );
    assert!(request.message_input_texts("user").is_empty());
    let channels = board
        .list_channels(
            owner,
            ChannelQuery {
                query: None,
                direction: SortDirection::OldestFirst,
                page: PageRequest::default(),
            },
        )
        .await?;
    assert_eq!(
        channels
            .channels
            .results
            .into_iter()
            .map(|channel| (
                channel.channel_name,
                channel.description,
                channel.message_count
            ))
            .collect::<Vec<_>>(),
        vec![
            ("release".into(), Some("Release policy".into()), 1),
            ("_audit".into(), Some("".into()), 1)
        ]
    );
    for channel in ["release", "_audit"] {
        let posts = board
            .search_posts(
                owner,
                PostQuery {
                    channel_name: Some(channel.into()),
                    query: None,
                    after_message_id: None,
                    author: None,
                    page: PageRequest::default(),
                    max_chars_per_post: NonZeroU32::MAX,
                },
            )
            .await?;
        assert_eq!(posts.results.len(), 1);
        let post = board
            .read_post(
                owner,
                ReadPostRequest {
                    message_id: posts.results[0].metadata.message_id,
                    offset_chars: 0,
                    limit_chars: NonZeroU32::MAX,
                },
            )
            .await?;
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&post.text)?,
            serde_json::to_value(&snapshot.records()[0])?
        );
    }
    // The next capture needs two batches; each request retains the full original
    // and current catalog within the selected model's tool-output budget.
    let catalog = json!({"release":"Release policy","_audit":""});
    assert!(
        serde_json::to_string(&json!({"catalog":catalog,"sources":next.records()}))?.len()
            > input_policy.limit as usize
    );
    let mut previous_input = request.input();
    let mut previous_reply = reply;
    for (index, source) in next.records().iter().enumerate() {
        // Omission preserves the description. A redundant echo is harmless too.
        let reply = if index == 0 {
            r#"{"routes":[["release"]],"descriptions":{}}"#
        } else {
            r#"{"routes":[["release"]],"descriptions":{"release":"Release policy"}}"#
        };
        let response = mount_sse_once(
            &server,
            sse(vec![
                ev_assistant_message("routing-later", reply),
                ev_completed("routing-later"),
            ]),
        )
        .await;
        let calls_before = host.agent_path_calls.load(Ordering::SeqCst);
        let routed = housekeeper.route_next(&next, &cancellation).await;
        assert_eq!(routed?, 1);
        // An unchanged description needs only the catalog read and the evidence append.
        assert_eq!(
            host.agent_path_calls.load(Ordering::SeqCst) - calls_before,
            2
        );
        assert_eq!(
            housekeeper.cursor().pending(&next)?.len(),
            next.records().len() - index - 1
        );
        let request = response.single_request();
        let current_input = request.input();
        if window_pressure && index == 0 {
            // The oversized prior reasoning, its batch and its reply leave the window together.
            assert_eq!(current_input.len(), 2);
            assert_eq!(current_input[0], previous_input[0]);
        } else {
            assert_eq!(current_input.len(), previous_input.len() + 2);
            assert_eq!(
                &current_input[..previous_input.len()],
                previous_input.as_slice()
            );
            let assistant = &current_input[previous_input.len()];
            assert_eq!(assistant["role"], "assistant");
            assert_eq!(assistant["content"][0]["text"], previous_reply);
        }
        previous_input = current_input;
        previous_reply = reply;
        let input = request.inputs_of_type("function_call_output");
        let input = input.last().expect("latest operation input")["output"]
            .as_str()
            .expect("operation input");
        assert!(input.len() <= input_policy.limit as usize);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(input)?,
            json!({"catalog":catalog,"sources":[source]})
        );
    }
    let view = board.read_view(owner).await?;
    let release = view
        .channels()
        .iter()
        .find(|channel| channel.channel_name == "release");
    assert_eq!(
        release.map(|channel| (channel.description.as_deref(), channel.message_count)),
        Some((Some("Release policy"), 3))
    );
    assert!(
        host.notifications
            .lock()
            .expect("notifications lock")
            .is_empty()
    );
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn root_shutdown_waits_for_the_accepted_evidence_append() -> anyhow::Result<()> {
    use codex_extension_api::ExtensionData;
    use codex_extension_api::ExtensionRegistryBuilder;
    use codex_extension_api::ThreadReadyInput;
    use codex_extension_api::ThreadStopInput;
    use codex_extension_api::TurnAbortInput;
    use codex_protocol::protocol::SessionSource;
    use codex_protocol::protocol::TurnAbortReason;

    core_test_support::skip_if_no_network!(Ok(()));
    let home = Arc::new(tempfile::tempdir()?);
    let write_credential = |access_token: &str| {
        let auth = json!({
            "auth_mode": "chatgpt",
            "tokens": {
                // The ID token carries user_id "test" on both sides of the rotation.
                "id_token": "e30.eyJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsidXNlcl9pZCI6InRlc3QifX0.sig",
                "account_id": "workspace",
                "access_token": access_token,
                "refresh_token": "test-refresh"
            },
            "last_refresh": Utc::now()
        });
        std::fs::write(home.path().join("auth.json"), auth.to_string())
    };
    write_credential("old-token")?;
    let auth = codex_login::AuthManager::shared(
        home.path().to_path_buf(),
        /*enable_codex_api_key_env*/ false,
        codex_login::AuthCredentialsStoreMode::File,
        /*forced_chatgpt_workspace_id*/ None,
        /*chatgpt_base_url*/ None,
        codex_login::AuthKeyringBackendKind::default(),
        codex_login::test_support::transport_default_auth_route_config(),
    )
    .await;
    // The root prewarms first. The housekeeper's initial credential is then rejected.
    let unauthorized = json!({"type":"error","status":401,"error":{"type":"authentication_error","message":"expired"}});
    let server = start_websocket_server(vec![
        vec![vec![
            ev_response_created("prewarm"),
            ev_completed("prewarm"),
        ]],
        vec![vec![unauthorized.clone()]],
        vec![vec![
            ev_assistant_message(
                "route",
                r#"{"routes":[["release"]],"descriptions":{"release":"Release policy"}}"#,
            ),
            ev_completed("route"),
        ]],
    ])
    .await;
    let release_401 = server.gate_response_event(&unauthorized);
    let test = test_codex()
        .with_home(Arc::clone(&home))
        .with_auth_manager(auth)
        .with_history_mode(ThreadHistoryMode::Paginated)
        .with_model_info_override("gpt-5.6-luna", |_| {})
        .with_model_info_override("test-gv3-actor", |model| {
            model.auto_review_model_override = Some("test-gv3-router".to_owned());
        })
        .with_config(|config| {
            config.approvals_reviewer = ApprovalsReviewer::User;
        })
        .build_with_websocket_server(&server)
        .await?;
    let _prewarm = tokio::time::timeout(
        std::time::Duration::from_secs(15),
        server.wait_for_request(/*connection_index*/ 0, /*request_index*/ 0),
    )
    .await?;
    let root = test.session_configured.thread_id;
    let host = board_host(root);
    let board = local_board(test.config.sqlite_config().clone(), root.into())
        .read_write()
        .open(host.clone())
        .await?;
    board
        .upsert_channel(
            root,
            UpsertChannelRequest {
                channel_name: "release".to_owned(),
                description: "Release policy".to_owned().try_into()?,
            },
        )
        .await?;
    let host = Arc::new(PausedClockHost {
        inner: host,
        pause_next: AtomicBool::new(true),
        started: Notify::new(),
        release: Notify::new(),
    });
    let mut registry = ExtensionRegistryBuilder::default();
    codex_guardian_v3::install(
        &mut registry,
        Arc::downgrade(&test.thread_manager),
        test.config.sqlite_config().clone(),
        {
            let host = host.clone();
            move |_| host.clone()
        },
    );
    let registry = registry.build();
    let session = ExtensionData::new(root.to_string());
    let thread = test.codex.thread_extension_data();
    let lifecycle = &registry.thread_lifecycle_contributors()[0];
    lifecycle
        .on_thread_ready(ThreadReadyInput {
            config: &test.config,
            session_source: &SessionSource::Exec,
            history_initialization: codex_history::HistoryInitialization::New,
            history_mode: ThreadHistoryMode::Paginated,
            session_store: &session,
            thread_store: thread,
        })
        .await;
    assert!(!tokio::fs::try_exists(test.codex.rollout_path().expect("persistent root")).await?);
    let data = ResponseItemEnvelope {
        item: serde_json::from_value(
            json!({"type":"message","role":"user","content":[{"type":"input_text","text":"Stop the release."}]}),
        )?,
        metadata: Some(CodexHarnessMetadata {
            user_input_order: Some(1),
            ..Default::default()
        }),
    };
    test.thread_store
        .append_items(AppendThreadItemsParams {
            thread_id: root,
            items: vec![RolloutItem::ResponseItem(data)],
        })
        .await?;
    registry.turn_lifecycle_contributors()[0]
        .on_turn_abort(TurnAbortInput {
            reason: TurnAbortReason::Interrupted,
            session_store: &session,
            thread_store: thread,
            turn_store: &ExtensionData::new("turn"),
        })
        .await;
    tokio::time::timeout(
        std::time::Duration::from_secs(15),
        server.wait_for_request(/*connection_index*/ 1, /*request_index*/ 0),
    )
    .await?;
    write_credential("renewed-token")?;
    let _ = release_401.send(());
    tokio::time::timeout(std::time::Duration::from_secs(15), host.started.notified()).await?;
    let connections = server.connections();
    let [_, first_attempt, requests] = connections.as_slice() else {
        anyhow::bail!("expected the root prewarm, rejected housekeeper connection, and retry");
    };
    let [request] = requests.as_slice() else {
        anyhow::bail!("expected one housekeeper WebSocket request");
    };
    assert_eq!(first_attempt[0].body_json(), request.body_json());
    let handshakes = server.handshakes();
    assert_eq!(
        handshakes[1].header("authorization").as_deref(),
        Some("Bearer old-token")
    );
    assert_eq!(
        handshakes[2].header("authorization").as_deref(),
        Some("Bearer renewed-token")
    );
    assert_eq!(request.body_json()["model"], "gpt-5.6-luna");
    let housekeeper_id: ThreadId =
        serde_json::from_value(request.body_json()["client_metadata"]["thread_id"].take())?;
    assert_ne!(housekeeper_id, root);
    assert_eq!(
        request.body_json()["client_metadata"]["x-codex-parent-thread-id"],
        root.to_string()
    );
    let shutdown = lifecycle.on_thread_stop(ThreadStopInput {
        thread_store: thread,
        session_store: &session,
    });
    tokio::pin!(shutdown);
    // Poll shutdown through its first suspension. It must join the already accepted append.
    assert!(futures::poll!(shutdown.as_mut()).is_pending());
    host.release.notify_one();
    shutdown.await;
    let channels = board
        .list_channels(
            root,
            ChannelQuery {
                query: None,
                direction: SortDirection::OldestFirst,
                page: PageRequest::default(),
            },
        )
        .await?;
    assert!(
        channels
            .channels
            .results
            .iter()
            .any(|row| row.channel_name == "release" && row.message_count == 1)
    );
    assert_eq!(
        codex_core::find_thread_path_by_id_str(
            test.home.path(),
            &housekeeper_id.to_string(),
            /*state_db_ctx*/ None
        )
        .await?,
        None,
        "the housekeeper must not leave a private copy of the routed evidence"
    );
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

#[tokio::test]
async fn command_token_rotation_between_batches_reauthenticates_the_housekeeper()
-> anyhow::Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));
    let credentials = tempfile::tempdir()?;
    let token = credentials.path().join("token.txt");
    std::fs::write(&token, "first-token")?;
    let (command, args): (&str, &[&str]) = if cfg!(windows) {
        ("cmd.exe", &["/D", "/C", "type", "token.txt"])
    } else {
        ("cat", &["token.txt"])
    };
    let auth = serde_json::from_value(json!({
        "command": command, "args": args, "cwd": credentials.path(), "refresh_interval_ms": 1
    }))?;
    let route = vec![
        ev_assistant_message(
            "route",
            r#"{"routes":[["release"]],"descriptions":{"release":"Release policy"}}"#,
        ),
        ev_completed("route"),
    ];
    // Allow two root sockets (preconnect and prewarm) and two Guardian sockets in any order.
    // Each socket can serve both batches so only credential rotation forces a reconnect.
    let server = start_websocket_server(vec![vec![route.clone(), route]; 4]).await;
    let test = test_codex()
        .with_history_mode(ThreadHistoryMode::Paginated)
        .with_model_info_override("gpt-5.6-luna", |_| {})
        .with_config(move |config| {
            config.approvals_reviewer = ApprovalsReviewer::User;
            config.model_provider.requires_openai_auth = false;
            config.model_provider.auth = Some(auth);
        })
        .build_with_websocket_server(&server)
        .await?;
    let root = test.session_configured.thread_id;
    let store = test.thread_store.as_ref();
    store.persist_thread(root, PersistContext::Standard).await?;
    store.flush_thread(root).await?;
    let cursor = EvidenceCursor::from_now(store, root).await?;
    let board = local_board(test.config.sqlite_config().clone(), root.into())
        .read_write()
        .open(board_host(root))
        .await?;
    let mut housekeeper = Housekeeper::new(
        Arc::new(board),
        cursor,
        Arc::new(LiveDelivery::new(root)),
        &test.codex,
        &test.thread_manager,
    )
    .await?;
    for (index, text) in ["Hold the release.", "The release is approved."]
        .into_iter()
        .enumerate()
    {
        if index == 1 {
            std::fs::write(&token, "second-token")?;
            // The command provider uses a wall-clock TTL and can cache the previous token for 1ms.
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        }
        store
            .append_items(AppendThreadItemsParams {
                thread_id: root,
                items: vec![RolloutItem::ResponseItem(ResponseItemEnvelope {
                    item: user_message_item(text),
                    metadata: Some(CodexHarnessMetadata {
                        user_input_order: Some((index + 1) as u64),
                        ..Default::default()
                    }),
                })],
            })
            .await?;
        store.flush_thread(root).await?;
        let snapshot = housekeeper.cursor().capture(store).await?;
        assert_eq!(
            housekeeper
                .route_next(&snapshot, &CancellationToken::new())
                .await?,
            1
        );
    }
    let tokens: Vec<_> = server
        .handshakes()
        .into_iter()
        .filter(|handshake| handshake.header("x-openai-subagent").as_deref() == Some("guardian"))
        .map(|handshake| handshake.header("authorization"))
        .collect();
    assert_eq!(
        tokens,
        vec![
            Some("Bearer first-token".to_owned()),
            Some("Bearer second-token".to_owned())
        ]
    );
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

#[tokio::test]
async fn retains_only_reusable_reviewer_conversations() -> anyhow::Result<()> {
    use codex_core::guardian_review::prepare_review_prewarm;
    use codex_extension_api::ExtensionData;
    use codex_extension_api::ExtensionRegistryBuilder;
    use codex_extension_api::ThreadReadyInput;
    use codex_extension_api::ThreadStopInput;
    use codex_guardian_reviewer::SessionDisposition;
    use codex_protocol::protocol::SessionSource;
    use codex_protocol::protocol::SubAgentSource;

    core_test_support::skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let test = test_codex()
        .with_history_mode(ThreadHistoryMode::Paginated)
        .with_config(|config| config.approvals_reviewer = ApprovalsReviewer::User)
        .build_with_auto_env(&server)
        .await?;
    let actor_thread = &test.codex;
    let root = actor_thread.startup_metadata().thread_id;
    let mut registry = ExtensionRegistryBuilder::default();
    codex_guardian_v3::install(
        &mut registry,
        Arc::downgrade(&test.thread_manager),
        test.config.sqlite_config().clone(),
        |root| board_host(root),
    );
    let registry = registry.build();
    let session = ExtensionData::new(root.to_string());
    let thread = actor_thread.thread_extension_data();
    let lifecycle = &registry.thread_lifecycle_contributors()[0];
    let child = ExtensionData::new(codex_protocol::ThreadId::new().to_string());
    lifecycle
        .on_thread_ready(ThreadReadyInput {
            config: &test.config,
            session_source: &SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id: root,
                depth: 1,
                agent_path: None,
                agent_nickname: None,
                agent_role: None,
            }),
            history_initialization: codex_history::HistoryInitialization::New,
            history_mode: ThreadHistoryMode::Paginated,
            session_store: &session,
            thread_store: &child,
        })
        .await;
    assert!(child.get::<codex_guardian_v3::ActorReviewer>().is_none());
    lifecycle
        .on_thread_ready(ThreadReadyInput {
            config: &test.config,
            session_source: &SessionSource::Exec,
            history_initialization: codex_history::HistoryInitialization::New,
            history_mode: ThreadHistoryMode::Paginated,
            session_store: &session,
            thread_store: thread,
        })
        .await;
    let actor = thread
        .get::<codex_guardian_v3::ActorReviewer>()
        .expect("reviewer installed");
    let context = prepare_review_prewarm(actor_thread, codex_guardian_v3::BOARD_RETRIEVAL).await?;
    let cancel = CancellationToken::new();
    let first = actor.reserve(&context, &cancel).await?;
    let id = first.thread_id();
    first.finish(SessionDisposition::Reusable);
    let second = actor.reserve(&context, &cancel).await?;
    assert_eq!(id, second.thread_id());
    drop(second);
    let third = actor.reserve(&context, &cancel).await?;
    assert_ne!(id, third.thread_id());
    third.finish(SessionDisposition::Discard);
    lifecycle
        .on_thread_stop(ThreadStopInput {
            thread_store: thread,
            session_store: &session,
        })
        .await;
    test.codex.shutdown_and_wait().await?;
    Ok(())
}
