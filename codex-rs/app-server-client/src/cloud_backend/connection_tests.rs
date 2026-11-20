#![cfg(test)]

//! Exercises native worker progress and reply attachment.

use super::*;
use crate::AppServerRequestHandle;
use futures::stream;
use pretty_assertions::assert_eq;
use std::sync::Mutex;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_bytes;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test]
async fn rejected_rename_preserves_loaded_cloud_thread_name() -> Result<()> {
    let server = MockServer::start().await;
    let home = tempfile::tempdir()?;
    let config = crate::legacy_core::config::ConfigBuilder::default()
        .codex_home(home.path().to_owned())
        .fallback_cwd(Some(home.path().to_owned()))
        .without_project_context()
        .build()
        .await?;
    let mut requests = requests::Requests {
        client: std::sync::Arc::new(cloud::Client::new(
            &config.http_client_factory(),
            &server.uri(),
            cloud::Credentials::new("synthetic-bearer", "synthetic-account")?,
        )?),
        account: protocol::GetAccountResponse {
            user_id: None,
            account: None,
            requires_openai_auth: true,
            workspace_routing: None,
        },
        loaded: Some(Box::new(serde_json::from_value(serde_json::json!({
            "thread": {
                "id": "t", "sessionId": "t", "preview": "", "ephemeral": false,
                "modelProvider": "openai", "createdAt": 0, "updatedAt": 0,
                "status": {"type": "idle"}, "cwd": home.path(), "cliVersion": "",
                "source": "cli", "turns": [], "name": "Old",
            },
            "model": "test", "modelProvider": "openai", "cwd": home.path(),
            "approvalPolicy": "never", "approvalsReviewer": "user",
            "sandbox": {"type": "readOnly"},
        }))?)),
        warned_partial_history: None,
    };
    Mock::given(method("POST"))
        .and(path(
            "/oaiproto.codex_app_server_backend.ThreadService/SetThreadName",
        ))
        .and(body_bytes(b"\x00\x00\x00\x00\x08\x0a\x01t\x12\x03New"))
        .respond_with(ResponseTemplate::new(503))
        .expect(1)
        .mount(&server)
        .await;

    let (output, mut notifications) = mpsc::unbounded_channel();
    let mut replies = Replies::default();
    let read = protocol::ThreadReadParams {
        thread_id: "t".into(),
        include_turns: false,
    };
    let rejected = handle_request(
        protocol::ClientRequest::ThreadSetName {
            request_id: protocol::RequestId::Integer(1),
            params: protocol::ThreadSetNameParams {
                thread_id: "t".into(),
                name: " New ".into(),
            },
        },
        cloud::SubmissionMode::StartIfIdle,
        /*environment*/ None,
        &mut requests,
        &mut replies,
        &output,
    )
    .await;
    assert!(rejected.unwrap_err().is::<cloud::MutationError>());
    assert_eq!(
        requests.read_thread(&read, /*status*/ None)?["thread"]["name"],
        "Old"
    );
    assert!(notifications.try_recv().is_err());
    Ok(())
}

#[tokio::test]
async fn failed_archive_loaded_list_tracks_attachment_not_cached_metadata() -> Result<()> {
    let server = MockServer::start().await;
    let home = tempfile::tempdir()?;
    let mut config = crate::legacy_core::config::ConfigBuilder::default()
        .codex_home(home.path().to_owned())
        .fallback_cwd(Some(home.path().to_owned()))
        .without_project_context()
        .build()
        .await?;
    config.cli_auth_credentials_store_mode = codex_login::AuthCredentialsStoreMode::File;
    let storage = config.auth_config();
    let token = "e30.e30.sig";
    let account = "synthetic-account";
    let auth = serde_json::from_value(serde_json::json!({
        "auth_mode": "chatgpt",
        "tokens": {
            "id_token": token, "access_token": token, "refresh_token": "test", "account_id": account,
        },
        "last_refresh": "2026-01-01T00:00:00Z",
    }))?;
    codex_login::save_auth(
        home.path(),
        &auth,
        storage.auth_credentials_store_mode,
        storage.keyring_backend_kind,
    )?;

    let cached = Box::new(serde_json::from_value::<protocol::ThreadResumeResponse>(
        serde_json::json!({
            "thread": {
                "id": "t", "sessionId": "t", "preview": "", "ephemeral": false,
                "modelProvider": "openai", "createdAt": 0, "updatedAt": 0,
                "status": {"type": "idle"}, "cwd": home.path(), "cliVersion": "",
                "source": "cli", "turns": [],
            },
            "model": "test", "modelProvider": "openai", "cwd": home.path(),
            "approvalPolicy": "never", "approvalsReviewer": "user",
            "sandbox": {"type": "readOnly"},
        }),
    )?);
    let mut requests = requests::Requests {
        client: std::sync::Arc::new(cloud::Client::new(
            &config.http_client_factory(),
            &server.uri(),
            cloud::Credentials::new(token, account)?,
        )?),
        account: protocol::GetAccountResponse {
            user_id: None,
            account: None,
            requires_openai_auth: true,
            workspace_routing: None,
        },
        loaded: Some(cached.clone()),
        warned_partial_history: None,
    };
    let (output, mut notifications) = mpsc::unbounded_channel();
    let (sender, stream) = futures::channel::mpsc::unbounded();
    let mut replies = Replies::default();
    replies
        .replace("t", async { Ok(stream.boxed()) }, &output)
        .await?;
    Mock::given(method("POST"))
        .and(path(
            "/oaiproto.codex_app_server_backend.ThreadService/ArchiveThread",
        ))
        .respond_with(ResponseTemplate::new(503))
        .expect(2)
        .mount(&server)
        .await;

    // A rejection before shutdown must preserve both this attachment and an
    // unrelated attachment when the rejected archive targets another thread.
    for id in ["t", "other"] {
        assert!(
            handle_request(
                protocol::ClientRequest::ThreadArchive {
                    request_id: protocol::RequestId::Integer(1),
                    params: protocol::ThreadArchiveParams {
                        thread_id: id.into(),
                    },
                },
                cloud::SubmissionMode::StartIfIdle,
                /*environment*/ None,
                &mut requests,
                &mut replies,
                &output,
            )
            .await
            .is_err()
        );
        let loaded = handle_request(
            protocol::ClientRequest::ThreadLoadedList {
                request_id: protocol::RequestId::Integer(2),
                params: Default::default(),
            },
            cloud::SubmissionMode::StartIfIdle,
            /*environment*/ None,
            &mut requests,
            &mut replies,
            &output,
        )
        .await?;
        assert_eq!(
            loaded,
            serde_json::json!({"data": ["t"], "nextCursor": null})
        );
        replies.admit_turn("t", &output)?;
        assert!(notifications.try_recv().is_err());
    }

    // Model a late archive failure winning while_listening's biased select
    // when Attach's EOF is also ready. No worker poll has observed the EOF yet.
    drop(sender);
    let failed: Result<()> = replies
        .while_listening(
            async { Err(eyre!("archive failed after shutdown")) },
            &output,
        )
        .await;
    assert!(failed.is_err());
    assert!(replies.active.is_some());
    // The first query must notice EOF itself; the second sees an already
    // observed disconnect. Neither may use the cached resume response.
    for _ in 0..2 {
        let loaded = handle_request(
            protocol::ClientRequest::ThreadLoadedList {
                request_id: protocol::RequestId::Integer(3),
                params: Default::default(),
            },
            cloud::SubmissionMode::StartIfIdle,
            /*environment*/ None,
            &mut requests,
            &mut replies,
            &output,
        )
        .await?;
        assert_eq!(loaded, serde_json::json!({"data": [], "nextCursor": null}));
        assert_eq!(requests.loaded.as_ref(), Some(&cached));
    }
    let AppServerEvent::ThreadStreamDisconnected(error) = notifications.try_recv()? else {
        panic!("expected the existing stream-loss recovery notification");
    };
    assert_eq!(
        *error,
        cloud_error(
            "t",
            "Cloud stream ended. Reopen the conversation to reconnect."
        )
    );
    assert!(notifications.try_recv().is_err());
    let login = idle::Login {
        storage,
        mode: codex_protocol::auth::AuthMode::Chatgpt,
        token: token.into(),
        account: account.into(),
        permit: config
            .http_client_factory()
            .network_policy()
            .acquire(&server.uri().parse()?)?,
    };
    // A live replacement installs and forwards held and buffered events.
    let (ack, installed) = oneshot::channel();
    let (state, _) = tokio::sync::watch::channel(crate::cloud::IdleReconnect::Inactive);
    let mut recovery = idle::Recovery::new(state, Arc::clone(&requests.client), login.clone());
    recovery.held = Some((
        "t".into(),
        tokio::time::Instant::now() + std::time::Duration::from_secs(8),
        Box::pin(async move {
            let idle: &[u8] = b"\x1a\x09\x12\x07\x0a\x02\x12\x00\x12\x01t";
            Some(idle::Opened {
                stream: stream::iter([Ok(cloud::Event::Notification(idle.into()))])
                    .chain(stream::pending())
                    .boxed(),
                first: Some(cloud::Event::ServerRequest(Default::default())),
                ack,
            })
        }),
    ));
    replies.idle = Some(recovery);
    requests.warned_partial_history = Some("t".into());
    replies.next_event(&output).await;
    assert_eq!(installed.await?, Ok(()));
    let AppServerEvent::ServerNotification(notification) = notifications.try_recv()? else {
        panic!("expected held first event");
    };
    assert!(matches!(
        *notification,
        protocol::ServerNotification::Warning(_)
    ));
    replies.next_event(&output).await;
    assert_eq!(replies.thread_status("t"), Some(&cloud::ThreadStatus::Idle));
    // Installation must not erase a warning already emitted by the refreshed history.
    let (commands, receiver) = mpsc::channel(1);
    drop(commands);
    run_worker(receiver, &mut requests, &mut replies, &output).await;
    assert_eq!(requests.warned_partial_history.as_deref(), Some("t"));

    // A quiet Attach cannot prove that no whole turn was missed while disconnected.
    let attach_path = "/oaiproto.codex_app_server_backend.ThreadService/Attach";
    for (idle, statuses, outcome) in [
        (true, &[][..], "pat"),
        (true, &["quiet"][..], "install"),
        (true, &["7"][..], "install"),
        (false, &["14", "7"][..], "install"),
        (false, &["14", "busy"][..], "install"),
        (true, &["busy"][..], "install"),
        (true, &["busy"][..], "cancel"),
        (true, &["busy", "busy"][..], "retry"),
        (true, &["busy"][..], "slow-history"),
        (true, &["busy"][..], "auth-change"),
    ] {
        server.reset().await;
        let attempt = std::sync::atomic::AtomicUsize::new(0);
        Mock::given(method("POST"))
            .and(path(attach_path))
            .and(header("authorization", format!("Bearer {token}")))
            .respond_with(move |_: &wiremock::Request| {
                let index = attempt.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let response =
                    ResponseTemplate::new(200).insert_header("content-type", "application/grpc");
                match statuses[index] {
                    "quiet" => response.set_body_bytes(b"\x00\x00\x00\x00\x02\x0a\x00".to_vec()),
                    "busy" => response.set_body_bytes(vec![0, 0, 0, 0, 2, 18, 0]),
                    status => response
                        .insert_header("grpc-status", status)
                        .insert_header("grpc-message", "private backend details"),
                }
            })
            .expect(statuses.len() as u64)
            .mount(&server)
            .await;
        let (client, commands, events) = CloudClient::channel(/*capacity*/ 4);
        let mut client = AppServerClient::Cloud(client);
        let requests_handle = client.request_handle();
        let AppServerRequestHandle::Cloud(handle) = &requests_handle else {
            unreachable!();
        };
        let mut recovery = idle::Recovery::new(
            handle.reconnect.clone(),
            Arc::clone(&requests.client),
            if outcome == "pat" {
                idle::Login {
                    mode: codex_protocol::auth::AuthMode::PersonalAccessToken,
                    ..login.clone()
                }
            } else {
                login.clone()
            },
        );
        recovery.observed_idle = Some(idle);
        let mut worker_replies = Replies {
            idle: Some(recovery),
            ..Default::default()
        };
        let mut worker_requests = requests::Requests {
            client: Arc::clone(&requests.client),
            account: requests.account.clone(),
            loaded: Some(cached.clone()),
            warned_partial_history: Some("t".into()),
        };
        let worker = tokio::spawn(async move {
            run_worker(commands, &mut worker_requests, &mut worker_replies, &events).await;
        });
        let budget = std::time::Duration::from_secs(if outcome == "slow-history" { 2 } else { 8 });
        let result = handle
            .resume_disconnected_thread(
                "t".into(),
                /*require_history*/ false,
                tokio::time::Instant::now() + budget,
            )
            .await;
        if outcome == "pat" || statuses.last() == Some(&"7") {
            let Err(crate::TypedRequestError::Server { source, .. }) = result else {
                panic!("expected recovery to reject this authentication mode or access denial");
            };
            assert_eq!(
                source,
                idle::recovery_error(if outcome == "pat" {
                    "Reopen this cloud conversation to reconnect with your personal access token."
                } else {
                    "Your account no longer has access to this cloud conversation."
                })
            );
        } else {
            let (settings, release) = result?;
            assert_eq!(settings.as_ref(), Some(cached.as_ref()));
            assert!(client.next_event().now_or_never().is_none());
            let loaded: protocol::ThreadLoadedListResponse = requests_handle
                .request_typed(protocol::ClientRequest::ThreadLoadedList {
                    request_id: protocol::RequestId::Integer(4),
                    params: Default::default(),
                })
                .await?;
            assert!(loaded.data.is_empty());

            let auth_path = home.path().join("auth.json");
            Mock::given(method("POST"))
                .and(wiremock::matchers::path_regex(
                    "/oaiproto.codex_app_server_backend.ThreadService/List(Turns|Items)",
                ))
                .respond_with(move |request: &wiremock::Request| {
                    if outcome == "auth-change" {
                        std::fs::remove_file(&auth_path).unwrap();
                    }
                    let body: &[u8] = if request.url.path().ends_with("ListItems") {
                        b"\0\0\0\0\x0f\x0a\x0a\x0a\x08\x0a\x01u\x12\x03\xa2\x01\0\x12\x01x"
                    } else {
                        b"\x00\x00\x00\x00\x02\x0a\x00"
                    };
                    let response = ResponseTemplate::new(200)
                        .insert_header("content-type", "application/grpc")
                        .set_body_bytes(body);
                    if outcome == "slow-history" {
                        response.set_delay(std::time::Duration::from_secs(10))
                    } else {
                        response
                    }
                })
                .expect(if matches!(outcome, "auth-change" | "slow-history") {
                    1
                } else {
                    3
                })
                .mount(&server)
                .await;
            let history: Result<protocol::ThreadTurnsListResponse, _> = tokio::time::timeout(
                std::time::Duration::from_secs(if outcome == "slow-history" { 3 } else { 2 }),
                requests_handle.request_typed(protocol::ClientRequest::ThreadTurnsList {
                    request_id: protocol::RequestId::Integer(5),
                    params: protocol::ThreadTurnsListParams {
                        thread_id: "t".into(),
                        cursor: None,
                        limit: Some(5),
                        sort_direction: Some(protocol::SortDirection::Desc),
                        items_view: Some(protocol::TurnItemsView::NotLoaded),
                    },
                }),
            )
            .await?;
            if outcome == "auth-change" {
                let Err(crate::TypedRequestError::Transport { source, .. }) = history else {
                    panic!("expected changed sign-in to withhold the fetched page");
                };
                assert!(source.get_ref().unwrap().is::<CloudRecoverySignInChanged>());
                assert_eq!(source.to_string(), idle::SIGN_IN_AGAIN);
                let items = requests_handle
                    .request(protocol::ClientRequest::ThreadItemsList {
                        request_id: protocol::RequestId::Integer(7),
                        params: serde_json::from_value(serde_json::json!({"threadId":"t"}))?,
                    })
                    .await
                    .expect_err("the changed sign-in must reject the next page before RPC");
                assert!(items.get_ref().unwrap().is::<CloudRecoverySignInChanged>());
                assert!(
                    server
                        .received_requests()
                        .await
                        .unwrap()
                        .iter()
                        .all(|request| !request.url.path().ends_with("/ListItems"))
                );
            } else if outcome == "slow-history" {
                let Err(crate::TypedRequestError::Server { source, .. }) = history else {
                    panic!("expected the recovery deadline to withhold the page");
                };
                assert_eq!(source, idle::recovery_error(idle::RECOVERY_TIMEOUT));
            } else {
                assert_eq!(
                    history?,
                    protocol::ThreadTurnsListResponse {
                        data: Vec::new(),
                        next_cursor: None,
                        backwards_cursor: None,
                    }
                );
                for page in 0..2 {
                    let _: protocol::ThreadItemsListResponse = requests_handle
                        .request_typed(protocol::ClientRequest::ThreadItemsList {
                            request_id: protocol::RequestId::Integer(7 + page),
                            params: serde_json::from_value(serde_json::json!({"threadId":"t"}))?,
                        })
                        .await?;
                    if page == 0 {
                        assert!(matches!(client.next_event().now_or_never().flatten(),
                            Some(AppServerEvent::ServerNotification(notification))
                                if matches!(*notification, protocol::ServerNotification::Warning(_))));
                    } else {
                        assert!(client.next_event().now_or_never().is_none());
                    }
                }
            }
            if outcome != "install" {
                drop(release);
                if outcome == "retry" {
                    // No inventory read or yield may retire the canceled hold before this retry.
                    let (settings, release) = handle
                        .resume_disconnected_thread(
                            "t".into(),
                            /*require_history*/ true,
                            tokio::time::Instant::now() + std::time::Duration::from_secs(8),
                        )
                        .await?;
                    assert_eq!(settings.as_ref(), Some(cached.as_ref()));
                    drop(release);
                }
                let loaded: protocol::ThreadLoadedListResponse = requests_handle
                    .request_typed(protocol::ClientRequest::ThreadLoadedList {
                        request_id: protocol::RequestId::Integer(6),
                        params: Default::default(),
                    })
                    .await?;
                assert!(loaded.data.is_empty());
                assert!(client.next_event().now_or_never().is_none());
            } else {
                // The wiremock response is finite: it must never ACK a live installation.
                assert!(release.install().await.is_err());
                assert!(client.next_event().now_or_never().is_none());
            }
        }
        client.shutdown().await?;
        worker.await?;
        server.verify().await;
    }
    Ok(())
}

#[tokio::test]
async fn dropping_an_in_flight_read_unblocks_the_native_worker() -> Result<()> {
    for (rpc, request) in [
        (
            "ListThreads",
            protocol::ClientRequest::ThreadList {
                request_id: protocol::RequestId::Integer(1),
                params: serde_json::from_value(serde_json::json!({"limit": 10}))?,
            },
        ),
        (
            "ListTurns",
            protocol::ClientRequest::ThreadTurnsList {
                request_id: protocol::RequestId::Integer(1),
                params: serde_json::from_value(serde_json::json!({
                    "threadId": "t",
                    "itemsView": "notLoaded",
                }))?,
            },
        ),
        (
            "ListItems",
            protocol::ClientRequest::ThreadItemsList {
                request_id: protocol::RequestId::Integer(1),
                params: serde_json::from_value(serde_json::json!({"threadId": "t"}))?,
            },
        ),
    ] {
        assert_abandoned_request_unblocks_worker(rpc, CloudOperation::Request(Box::new(request)))
            .await?;
    }
    Ok(())
}

#[tokio::test]
async fn dropping_an_in_flight_start_unblocks_the_native_worker() -> Result<()> {
    for operation in [
        CloudOperation::Request(Box::new(protocol::ClientRequest::ThreadStart {
            request_id: protocol::RequestId::Integer(1),
            params: Default::default(),
        })),
        CloudOperation::StartThreadWithEnvironment {
            request_id: protocol::RequestId::Integer(1),
            params: Box::default(),
            environment: cloud::EnvironmentSelection::Project(cloud::ProjectEnvironment {
                name: "project".into(),
                origin_urls: vec!["https://github.com/example/project.git".into()],
                cwd: None,
            }),
        },
    ] {
        assert_abandoned_request_unblocks_worker("Prewarm", operation).await?;
    }
    Ok(())
}

async fn assert_abandoned_request_unblocks_worker(
    rpc: &str,
    operation: CloudOperation,
) -> Result<()> {
    let server = MockServer::start().await;
    let home = tempfile::tempdir()?;
    let config = crate::legacy_core::config::ConfigBuilder::default()
        .codex_home(home.path().to_owned())
        .fallback_cwd(Some(home.path().to_owned()))
        .without_project_context()
        .build()
        .await?;
    let native = cloud::Client::new(
        &config.http_client_factory(),
        &server.uri(),
        cloud::Credentials::new("synthetic-bearer", "synthetic-account")?,
    )?;
    let account = protocol::GetAccountResponse {
        user_id: None,
        account: None,
        requires_openai_auth: true,
        workspace_routing: None,
    };
    let (client, commands, output) = CloudClient::channel(/*capacity*/ 32);
    let handle = client.request_handle();
    let client = AppServerClient::Cloud(client);
    let mut requests = requests::Requests {
        client: Arc::new(native),
        account: account.clone(),
        loaded: None,
        warned_partial_history: None,
    };
    tokio::spawn(async move {
        run_worker(commands, &mut requests, &mut Replies::default(), &output).await;
    });
    let (request_arrived, mut arrival) = mpsc::unbounded_channel();
    let (release_request, held) = std::sync::mpsc::channel::<()>();
    let held = Mutex::new(held);
    Mock::given(method("POST"))
        .and(path(format!(
            "/oaiproto.codex_app_server_backend.ThreadService/{rpc}"
        )))
        .and(header("authorization", "Bearer synthetic-bearer"))
        .respond_with(move |_: &wiremock::Request| {
            let _ = request_arrived.send(());
            // Wiremock serves on a dedicated thread. Hold the RPC until the sender drops.
            let _ = held.lock().unwrap().recv();
            ResponseTemplate::new(503)
        })
        .expect(1)
        .mount(&server)
        .await;

    let mut pending = Box::pin(async {
        match operation {
            CloudOperation::Request(request) => client.request(*request).await,
            CloudOperation::StartThreadWithEnvironment {
                request_id,
                params,
                environment,
            } => {
                handle
                    .start_thread_with_environment(request_id, *params, environment)
                    .await
            }
            _ => unreachable!("fixture supports Cloud lists and starts"),
        }
    });
    tokio::select! {
        seen = tokio::time::timeout(crate::SHUTDOWN_TIMEOUT, arrival.recv()) => {
            assert_eq!(seen?, Some(()));
        }
        _ = &mut pending => panic!("request finished before the fixture held it"),
    }
    drop(pending);
    let account_response = tokio::time::timeout(
        crate::SHUTDOWN_TIMEOUT,
        client.request_typed::<protocol::GetAccountResponse>(protocol::ClientRequest::GetAccount {
            request_id: protocol::RequestId::Integer(2),
            params: protocol::GetAccountParams {
                refresh_token: false,
            },
        }),
    )
    .await;
    drop(release_request);
    assert_eq!(account_response??, account);
    client.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn failed_replacement_keeps_old_thread_until_new_attachment_succeeds() -> Result<()> {
    let (output, mut notifications) = mpsc::unbounded_channel();
    let mut replies = Replies::default();
    replies
        .replace("old", async { Ok(stream::pending().boxed()) }, &output)
        .await?;

    for failure in [
        Err(cloud::Error::InvalidResponse),
        Ok(stream::empty().boxed()),
        Ok(stream::iter([Err(cloud::Error::InvalidResponse)]).boxed()),
    ] {
        assert!(
            replies
                .replace("new", async { failure }, &output)
                .await
                .is_err()
        );
        replies.admit_turn("old", &output)?;
        assert!(replies.admit_turn("new", &output).is_err());
    }

    let (close, closed) = tokio::sync::oneshot::channel::<()>();
    let stream = stream::once(async move {
        let _ = closed.await;
        Err(cloud::Error::InvalidResponse)
    })
    .boxed();
    replies
        .replace("new", async { Ok(stream) }, &output)
        .await?;
    replies.detach("old");
    assert!(replies.admit_turn("old", &output).is_err());
    replies
        .replace(
            "new",
            async { panic!("an already subscribed thread must reuse its reply stream") },
            &output,
        )
        .await?;
    replies.admit_turn("new", &output)?;

    let _ = close.send(());
    assert!(replies.admit_turn("new", &output).is_err());
    let AppServerEvent::ThreadStreamDisconnected(notification) = notifications.try_recv()? else {
        panic!("expected a disconnect notification");
    };
    assert_eq!(
        serde_json::to_value(protocol::ServerNotification::Error(*notification))?,
        serde_json::to_value(protocol::ServerNotification::Error(cloud_error(
            "new",
            "Cloud stream ended. Reopen the conversation to reconnect."
        )))?
    );
    Ok(())
}

#[tokio::test]
async fn prepared_resume_keeps_the_previous_stream_until_handoff() -> Result<()> {
    let server = MockServer::start().await;
    let home = tempfile::tempdir()?;
    let settings = |id: &str| {
        serde_json::from_value(serde_json::json!({
            "thread": {
                "id": id, "sessionId": id, "preview": "", "ephemeral": false,
                "modelProvider": "openai", "createdAt": 0, "updatedAt": 0,
                "status": {"type": "idle"}, "cwd": home.path(), "cliVersion": "",
                "source": "cli", "turns": [],
            },
            "model": "test", "modelProvider": "openai", "cwd": home.path(),
            "approvalPolicy": "never", "approvalsReviewer": "user", "sandbox": {"type": "readOnly"},
        }))
    };
    let native = Arc::new(cloud::Client::new(
        &codex_http_client::HttpClientFactory::new(
            codex_http_client::OutboundProxyPolicy::ReqwestDefault,
        ),
        &server.uri(),
        cloud::Credentials::new("synthetic-bearer", "synthetic-account")?,
    )?);
    // One opaque history item with a continuation. A's mutation deliberately loses its receipt.
    let partial_page: &[u8] = b"\0\0\0\0\x0f\x0a\x0a\x0a\x08\x0a\x01u\x12\x03\xa2\x01\0\x12\x01x";
    let list = ResponseTemplate::new(200)
        .insert_header("grpc-status", "0")
        .set_body_raw(partial_page, "application/grpc");
    let add = ResponseTemplate::new(503);
    for (rpc, response, count) in [("ListItems", list, 8), ("AddUserMessage", add, 4)] {
        let path_name = format!("/oaiproto.codex_app_server_backend.ThreadService/{rpc}");
        Mock::given(method("POST"))
            .and(path(path_name))
            .respond_with(response)
            .expect(count)
            .mount(&server)
            .await;
    }
    let waiting = b"\x1a\x0d\x12\x0b\x0a\x06\x22\x04\x0a\x02\x08\x01\x12\x01b";
    let idle = b"\x1a\x09\x12\x07\x0a\x02\x12\x00\x12\x01b";
    for outcome in ["eof", "burst", "shutdown", "abandon", "reused", "stale"] {
        let (mut client, commands, output) = CloudClient::channel(/*capacity*/ 1);
        let (scoped, history, _) = CloudClient::channel(/*capacity*/ 1);
        let mut requests = requests::Requests {
            client: Arc::clone(&native),
            account: serde_json::from_value(serde_json::json!({"requiresOpenaiAuth": true}))?,
            loaded: Some(settings("a")?),
            warned_partial_history: Some("a".into()),
        };
        let mut replies = Replies::default();
        replies
            .replace("a", async { Ok(stream::pending().boxed()) }, &output)
            .await?;
        let (incoming, stream) = mpsc::unbounded_channel();
        incoming.send(Ok(cloud::Event::Notification(idle.as_slice().into())))?;
        if outcome == "burst" {
            incoming.send(Ok(cloud::Event::Notification(waiting.as_slice().into())))?;
            incoming.send(Ok(cloud::Event::Notification(idle.as_slice().into())))?;
            for _ in 0..PREPARED_EVENT_LIMIT {
                incoming.send(Ok(cloud::Event::ServerRequest(Default::default())))?;
            }
        }
        let stream = stream::unfold(stream, |mut rx| async {
            rx.recv().await.map(|event| (event, rx))
        });
        let reused = matches!(outcome, "reused" | "stale");
        let first = cloud::Event::ServerRequest(Default::default());
        let candidate = PreparedResume {
            settings: settings(if reused { "a" } else { "b" })?,
            opened: (!reused).then(|| (stream.boxed(), Some(first))),
            history,
            generation: replies.generation - u64::from(outcome == "stale"),
            canceled_recovery: None,
            warning: None,
        };
        // The EOF case closes after a second, distinguishable buffered event.
        let incoming = (outcome != "eof").then_some(incoming);
        let drive = async {
            let handle = scoped.request_handle();
            let page = |cursor: Option<&str>| {
                handle.request(protocol::ClientRequest::ThreadItemsList {
                    request_id: protocol::RequestId::Integer(1),
                    params: serde_json::from_value(
                        serde_json::json!({"threadId": "b", "cursor": cursor}),
                    )
                    .unwrap(),
                })
            };
            if !reused {
                assert_eq!(page(None).await?.unwrap()["nextCursor"], "x");
                let params = serde_json::from_value(serde_json::json!({
                    "threadId": "a", "clientUserMessageId": "send",
                    "input": [{"type": "text", "text": "hello"}],
                }))?;
                let sent = client.request_handle().submit_user_message(params).await;
                assert!(
                    matches!(sent, Err(crate::TypedRequestError::Server { source, .. })
                    if source.code == -32000 && source.message.contains("mutation outcome unknown"))
                );
                assert!(page(Some("x")).await?.is_ok());
                assert!(client.next_event().now_or_never().is_none());
            }
            if outcome == "abandon" {
                drop((scoped, handle));
                incoming.as_ref().unwrap().closed().await;
            } else if outcome != "shutdown" {
                let commit = handle.commit_prepared_resume().await;
                assert_eq!(commit.is_ok(), matches!(outcome, "burst" | "reused"));
                if outcome == "eof" {
                    assert!(client.next_event().now_or_never().is_none());
                }
            }
            if outcome == "burst" {
                // One partial-history warning plus the entire live burst, including its unread tail.
                for _ in 0..PREPARED_EVENT_LIMIT + 2 {
                    let mut event = client.next_event().await;
                    while matches!(event, Some(AppServerEvent::CloudInventoryChanged(Some(_)))) {
                        event = client.next_event().await;
                    }
                    assert!(
                        matches!(event, Some(AppServerEvent::ServerNotification(notification))
                        if matches!(*notification, protocol::ServerNotification::Warning(_)))
                    );
                }
            }
            client.shutdown().await?;
            Ok::<(), color_eyre::Report>(())
        };
        let worker = run_worker_with_prepared(
            commands,
            &mut requests,
            &mut replies,
            &output,
            Some(candidate),
        );
        let ((), observed) = tokio::time::timeout(crate::SHUTDOWN_TIMEOUT, async {
            tokio::join!(worker, drive)
        })
        .await?;
        observed?;
        let committed = outcome == "burst";
        assert_eq!(
            requests.loaded.unwrap().thread.id,
            if committed { "b" } else { "a" }
        );
        if committed {
            assert_eq!(replies.thread_status("b"), Some(&cloud::ThreadStatus::Idle));
        }
        assert_eq!(replies.generation, if committed { 2 } else { 1 });
        assert_eq!(
            requests.warned_partial_history.as_deref(),
            match outcome {
                "burst" => Some("b"),
                "reused" => None,
                _ => Some("a"),
            }
        );
    }
    Ok(())
}

#[tokio::test]
async fn successful_reopen_supersedes_only_earlier_losses_for_that_thread() -> Result<()> {
    let (mut client, _commands, output) = CloudClient::channel(/*capacity*/ 1);
    let mut replies = Replies {
        stream_losses: client.stream_losses.clone(),
        ..Default::default()
    };

    for id in ["reopened", "different", "reopened"] {
        let (sender, stream) = futures::channel::mpsc::unbounded();
        replies
            .replace(id, async { Ok(stream.boxed()) }, &output)
            .await?;
        drop(sender);
        replies.forward_next(&output).await;
    }
    assert!(
        replies
            .replace("reopened", async { Ok(stream::empty().boxed()) }, &output)
            .await
            .is_err()
    );

    // The first EOF was followed by a successful reopen. A different thread's
    // EOF and the reopened thread's newer EOF must still be delivered, in order.
    drop(output);
    for expected in ["different", "reopened"] {
        let Some(AppServerEvent::ThreadStreamDisconnected(error)) = client.next_event().await
        else {
            panic!("expected a thread stream disconnect");
        };
        assert_eq!(
            *error,
            cloud_error(
                expected,
                "Cloud stream ended. Reopen the conversation to reconnect."
            )
        );
    }
    assert!(client.next_event().await.is_none());
    Ok(())
}

#[tokio::test]
async fn accepted_idle_reconnect_reports_changed_login() -> Result<()> {
    let server = MockServer::start().await;
    let home = tempfile::tempdir()?;
    let config = crate::legacy_core::config::ConfigBuilder::default()
        .codex_home(home.path().to_owned())
        .fallback_cwd(Some(home.path().to_owned()))
        .without_project_context()
        .build()
        .await?;
    let factory = config.http_client_factory();
    let (mut client, _commands, output) = CloudClient::channel(/*capacity*/ 1);
    let handle = client.request_handle();
    let mut recovery = idle::Recovery::new(
        handle.reconnect.clone(),
        Arc::new(cloud::Client::new(
            &factory,
            &server.uri(),
            cloud::Credentials::new("synthetic-bearer", "synthetic-account")?,
        )?),
        idle::Login {
            storage: config.auth_config(),
            mode: codex_protocol::auth::AuthMode::Chatgpt,
            token: "synthetic-bearer".into(),
            account: "synthetic-account".into(),
            permit: factory.network_policy().acquire(&server.uri().parse()?)?,
        },
    );
    recovery.observed_idle = Some(true);
    let mut replies = Replies {
        idle: Some(recovery),
        active: Some(Attachment {
            thread_id: "t".into(),
            stream: stream::empty().boxed(),
        }),
        stream_losses: client.stream_losses.clone(),
        ..Default::default()
    };
    replies.forward_next(&output).await;
    assert!(matches!(
        client.next_event().await,
        Some(AppServerEvent::ThreadStreamDisconnected(_))
    ));
    assert!(handle.reconnect_idle_thread("t"));

    replies.forward_next(&output).await;
    let Some(AppServerEvent::ThreadStreamDisconnected(error)) = client.next_event().await else {
        panic!("expected a disconnect notification after the accepted reconnect failed");
    };
    assert_eq!(
        *error,
        cloud_error(
            "t",
            "Your cloud sign-in changed or expired. Sign in again to access this conversation."
        )
    );
    Ok(())
}

#[tokio::test]
async fn attachment_tracks_its_status_and_reports_account_inventory_changes() -> Result<()> {
    let (output, mut events) = mpsc::unbounded_channel();
    let (client, _commands, _events) = CloudClient::channel(/*capacity*/ 1);
    let handle = client.request_handle();
    let mut replies = Replies {
        attached: handle.attached.clone(),
        ..Default::default()
    };
    let notifications: [&[u8]; 6] = [
        b"\x1a\x0d\x12\x0b\x0a\x06\x22\x04\x0a\x02\x08\x01\x12\x01t", // t is waiting
        b"\xaa\x01\x0c\x12\x0a\x0a\x01x\x12\x05\x22\x01u\x40\x04",    // x starts a turn
        b"\x1a\x09\x12\x07\x0a\x02\x12\x00\x12\x01x",                 // x is idle
        b"\x22\x05\x12\x03\x0a\x01x",                                 // x is archived
        b"\x32\x05\x12\x03\x0a\x01x",                                 // x is unarchived
        b"\x1a\x09\x12\x07\x0a\x02\x12\x00\x12\x01t",                 // t is idle
    ];
    let stream = stream::iter(
        notifications
            .map(|bytes| Ok(cloud::Event::Notification(bytes.into())))
            .into_iter()
            .chain([Err(cloud::Error::Rpc(cloud::RpcStatus {
                code: 7,
                message: "private backend details".into(),
                details: Default::default(),
            }))]),
    );
    replies
        .replace("t", async { Ok(stream.boxed()) }, &output)
        .await?;

    let waiting = cloud::ThreadStatus::Active {
        active_flags: vec![cloud::ThreadActiveFlag::WaitingOnApproval],
    };
    let waiting = serde_json::json!(waiting);
    let status_change = |status: &Value| serde_json::json!({"method": "thread/status/changed", "params": {"threadId": "t", "status": status}});
    for expected in [
        status_change(&waiting),
        Value::Null,
        Value::Null,
        serde_json::json!({"method": "thread/archived", "params": {"threadId": "x"}}),
        serde_json::json!({"method": "thread/unarchived", "params": {"threadId": "x"}}),
        status_change(&serde_json::json!(cloud::ThreadStatus::Idle)),
    ] {
        let AppServerEvent::CloudInventoryChanged(lifecycle) = events.try_recv()? else {
            panic!("expected inventory change");
        };
        assert_eq!(serde_json::to_value(lifecycle)?, expected);
        let status = expected.pointer("/params/status").unwrap_or(&waiting);
        assert_eq!(&serde_json::to_value(replies.thread_status("t"))?, status);
        assert!(handle.has_attachment());
        replies.forward_next(&output).await;
    }

    assert!(!handle.has_attachment());
    assert_eq!(replies.thread_status("t"), Some(&cloud::ThreadStatus::Idle));
    let AppServerEvent::ThreadStreamDisconnected(error) = events.try_recv()? else {
        panic!("expected a Cloud stream loss");
    };
    assert_eq!(
        *error,
        cloud_error(
            "t",
            "Your account no longer has access to this cloud conversation."
        )
    );

    replies
        .replace("t", async { Ok(stream::pending().boxed()) }, &output)
        .await?;
    assert_eq!(replies.thread_status("t"), None);
    assert!(handle.has_attachment());
    replies.detach("t");
    assert!(!handle.has_attachment());
    Ok(())
}
