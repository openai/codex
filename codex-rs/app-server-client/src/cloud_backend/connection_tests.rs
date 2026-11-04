#![cfg(test)]

//! Ensures only a successfully attached thread can receive new turns.

use super::*;
use futures::stream;
use pretty_assertions::assert_eq;

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
    let AppServerEvent::ServerNotification(notification) = notifications.try_recv()? else {
        panic!("expected a disconnect notification");
    };
    assert_eq!(
        serde_json::to_value(notification)?,
        serde_json::to_value(cloud_error(
            "new",
            "Cloud stream ended. Reopen the conversation to reconnect."
        ))?
    );
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
