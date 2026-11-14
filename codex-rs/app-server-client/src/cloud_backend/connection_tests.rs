#![cfg(test)]

//! Exercises native worker progress and reply attachment.

use super::*;
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
        /*project*/ None,
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
    let config = crate::legacy_core::config::ConfigBuilder::default()
        .codex_home(home.path().to_owned())
        .fallback_cwd(Some(home.path().to_owned()))
        .without_project_context()
        .build()
        .await?;
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
            cloud::Credentials::new("synthetic-bearer", "synthetic-account")?,
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
                /*project*/ None,
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
            /*project*/ None,
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
            /*project*/ None,
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
    Ok(())
}

#[tokio::test]
async fn dropping_an_in_flight_list_unblocks_the_native_worker() -> Result<()> {
    assert_abandoned_request_unblocks_worker(
        "ListThreads",
        CloudOperation::Request(Box::new(protocol::ClientRequest::ThreadList {
            request_id: protocol::RequestId::Integer(1),
            params: serde_json::from_value(serde_json::json!({"limit": 10}))?,
        })),
    )
    .await
}

#[tokio::test]
async fn dropping_an_in_flight_start_unblocks_the_native_worker() -> Result<()> {
    for operation in [
        CloudOperation::Request(Box::new(protocol::ClientRequest::ThreadStart {
            request_id: protocol::RequestId::Integer(1),
            params: Default::default(),
        })),
        CloudOperation::StartProjectThread {
            request_id: protocol::RequestId::Integer(1),
            params: Box::default(),
            project: cloud::ProjectEnvironment {
                name: "project".into(),
                origin_urls: vec!["https://github.com/example/project.git".into()],
                cwd: None,
            },
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
            CloudOperation::StartProjectThread {
                request_id,
                params,
                project,
            } => {
                handle
                    .start_project_thread(request_id, *params, project)
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
                    assert!(
                        matches!(client.next_event().await, Some(AppServerEvent::ServerNotification(notification))
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
async fn attachment_status_follows_its_server_notifications() -> Result<()> {
    let (output, _notifications) = mpsc::unbounded_channel();
    let mut replies = Replies::default();
    let notifications: [&[u8]; 3] = [
        b"\x1a\x0d\x12\x0b\x0a\x06\x22\x04\x0a\x02\x08\x01\x12\x01t", // t is waiting
        b"\x1a\x09\x12\x07\x0a\x02\x12\x00\x12\x01x",                 // x is idle
        b"\x1a\x09\x12\x07\x0a\x02\x12\x00\x12\x01t",                 // t is idle
    ];
    let stream =
        stream::iter(notifications.map(|bytes| Ok(cloud::Event::Notification(bytes.into()))));
    replies
        .replace("t", async { Ok(stream.boxed()) }, &output)
        .await?;

    let waiting = cloud::ThreadStatus::Active {
        active_flags: vec![cloud::ThreadActiveFlag::WaitingOnApproval],
    };
    assert_eq!(replies.thread_status("t"), Some(&waiting));
    replies.forward_next(&output).await;
    assert_eq!(replies.thread_status("t"), Some(&waiting));

    replies.forward_next(&output).await;
    assert_eq!(replies.thread_status("t"), Some(&cloud::ThreadStatus::Idle));
    replies.forward_next(&output).await;
    assert_eq!(replies.thread_status("t"), Some(&cloud::ThreadStatus::Idle));

    replies
        .replace("t", async { Ok(stream::pending().boxed()) }, &output)
        .await?;
    assert_eq!(replies.thread_status("t"), None);
    Ok(())
}
