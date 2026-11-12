#![cfg(test)]

//! Ensures only a successfully attached thread can receive new turns.

use super::*;
use futures::stream;
use pretty_assertions::assert_eq;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

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
