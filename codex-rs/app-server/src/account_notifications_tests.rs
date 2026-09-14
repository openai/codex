//! Exercises auth changes while notification delivery waits for queue capacity.

use super::*;
use pretty_assertions::assert_eq;
use test_case::test_case;

#[test_case(0; "same_owner_refresh")]
#[test_case(1; "owner_change")]
#[test_case(2; "coalesced_owner_changes")]
#[tokio::test]
async fn queued_notifications_follow_auth_owner_changes(owner_generation: u64) {
    for login in [false, true] {
        let (tx, mut rx) = mpsc::channel(/*buffer*/ 1);
        let outgoing = OutgoingMessageSender::new(
            tx.clone(),
            codex_analytics::AnalyticsEventsClient::disabled(),
        );
        let (changes, auth_changes) = watch::channel(AuthChangeState::default());
        let updated = AccountUpdatedNotification {
            auth_mode: None,
            plan_type: None,
        };
        tx.send(OutgoingEnvelope::Broadcast {
            message: timestamped_server_notification(ServerNotification::AccountUpdated(
                updated.clone(),
            )),
        })
        .await
        .unwrap();
        let payload = AccountLoginCompletedNotification {
            login_id: Some("queued-login".into()),
            success: true,
            error: None,
            onboarding_entrypoint: None,
        };
        let notification = if login {
            AccountNotification::LoginCompleted(payload.clone())
        } else {
            AccountNotification::Updated(updated.clone())
        };
        let send = outgoing.send_account_notification(
            Some(ConnectionId(7)),
            &auth_changes,
            /*owner_generation*/ 0,
            notification,
        );
        tokio::pin!(send);
        // Poll delivery with a full queue before changing authentication.
        assert!(futures::poll!(&mut send).is_pending());
        changes.send_replace(AuthChangeState {
            generation: 2,
            owner_generation,
        });
        rx.recv().await.unwrap();
        send.await;
        if login {
            let OutgoingEnvelope::ToConnection {
                connection_id,
                message: OutgoingMessage::AppServerNotification(envelope),
                ..
            } = rx.try_recv().unwrap()
            else {
                panic!("expected targeted login completion");
            };
            assert_eq!(connection_id, ConnectionId(7));
            let ServerNotification::AccountLoginCompleted(actual) = envelope.notification else {
                panic!("expected login completion");
            };
            assert_eq!(
                actual,
                if owner_generation == 0 {
                    payload
                } else {
                    AccountLoginCompletedNotification {
                        success: false,
                        error: Some("account changed before sign-in completed".into()),
                        ..payload
                    }
                }
            );
        } else if owner_generation == 0 {
            let OutgoingEnvelope::ToConnection {
                connection_id,
                message: OutgoingMessage::AppServerNotification(envelope),
                ..
            } = rx.try_recv().unwrap()
            else {
                panic!("expected targeted account update");
            };
            assert_eq!(connection_id, ConnectionId(7));
            let ServerNotification::AccountUpdated(actual) = envelope.notification else {
                panic!("expected account update");
            };
            assert_eq!(actual, updated);
        } else {
            assert!(rx.try_recv().is_err());
        }
    }
}
