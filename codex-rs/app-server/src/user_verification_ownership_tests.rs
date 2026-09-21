use super::*;
use app_test_support::ChatGptAuthFixture;
use app_test_support::write_chatgpt_auth;
use codex_app_server_protocol::McpServerElicitationRequest;
use codex_app_server_protocol::McpServerElicitationRequestParams;
use codex_config::types::AuthCredentialsStoreMode;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::future::Future;
use std::task::Context;
use std::task::Waker;

fn request(thread_id: ThreadId) -> ServerRequestPayload {
    ServerRequestPayload::McpServerElicitationRequest(McpServerElicitationRequestParams {
        thread_id: thread_id.to_string(),
        turn_id: Some("turn-1".into()),
        server_name: "plugin-service".into(),
        request: McpServerElicitationRequest::UserVerification {
            meta: None,
            title: "Approve".into(),
            description: String::new(),
            challenge: "AQID".into(),
        },
    })
}

#[tokio::test]
async fn verification_is_delivered_to_one_owner_and_other_connections_cannot_resolve_it() {
    let (sender, mut messages) = mpsc::channel(4);
    let outgoing = OutgoingMessageSender::new(sender, AnalyticsEventsClient::disabled());
    outgoing
        .enable_user_verification_connection(ConnectionId(1))
        .await;
    let thread_id = ThreadId::new();
    let (id, response) = outgoing
        .send_request_to_connections(
            Some(&[ConnectionId(2), ConnectionId(1)]),
            request(thread_id),
            Some(thread_id),
        )
        .await;
    assert!(matches!(
        messages.recv().await,
        Some(OutgoingEnvelope::ToConnection {
            connection_id: ConnectionId(1),
            ..
        })
    ));
    assert!(messages.try_recv().is_err());
    let proof = json!({"action": "accept", "content": {"credentialId": "AQID", "signature": "BAUG"}, "_meta": null});
    outgoing
        .notify_client_response(ConnectionId(2), id.clone(), proof.clone())
        .await;
    outgoing
        .notify_client_error(ConnectionId(2), id.clone(), internal_error("not the owner"))
        .await;
    assert!(
        outgoing
            .request_id_to_callback
            .lock()
            .await
            .contains_key(&id)
    );
    outgoing
        .replay_requests_to_connection_for_thread(ConnectionId(2), thread_id)
        .await;
    assert!(messages.try_recv().is_err());
    outgoing
        .notify_client_response(ConnectionId(1), id.clone(), proof.clone())
        .await;
    assert_eq!(response.await.unwrap(), Ok(proof));
    assert!(
        !outgoing
            .request_id_to_callback
            .lock()
            .await
            .contains_key(&id)
    );
}

#[tokio::test]
async fn verification_cancels_when_its_owner_disconnects() {
    let (sender, mut messages) = mpsc::channel(4);
    let outgoing = OutgoingMessageSender::new(sender, AnalyticsEventsClient::disabled());
    outgoing
        .enable_user_verification_connection(ConnectionId(1))
        .await;
    let thread_id = ThreadId::new();
    let (id, response) = outgoing
        .send_request_to_connections(
            Some(&[ConnectionId(1)]),
            request(thread_id),
            Some(thread_id),
        )
        .await;
    messages.recv().await.unwrap();
    outgoing.connection_closed(ConnectionId(2)).await;
    assert!(
        outgoing
            .request_id_to_callback
            .lock()
            .await
            .contains_key(&id)
    );
    outgoing.connection_closed(ConnectionId(1)).await;
    assert!(response.await.is_err());
    assert!(
        !outgoing
            .request_id_to_callback
            .lock()
            .await
            .contains_key(&id)
    );
}

#[tokio::test]
async fn verification_disconnect_during_registration_leaves_no_callback_or_request() {
    let (sender, mut messages) = mpsc::channel(4);
    let outgoing = OutgoingMessageSender::new(sender, AnalyticsEventsClient::disabled());
    outgoing
        .enable_user_verification_connection(ConnectionId(1))
        .await;
    let thread_id = ThreadId::new();
    let connection_ids = [ConnectionId(1)];
    let mut registration = std::pin::pin!(outgoing.send_request_to_connections(
        Some(&connection_ids),
        request(thread_id),
        Some(thread_id),
    ));
    let mut disconnect = std::pin::pin!(outgoing.connection_closed(ConnectionId(1)));
    {
        // Suspend registration at the callback lock, then let disconnect remove eligibility.
        let _callbacks = outgoing.request_id_to_callback.lock().await;
        let mut context = Context::from_waker(Waker::noop());
        assert!(registration.as_mut().poll(&mut context).is_pending());
        assert!(disconnect.as_mut().poll(&mut context).is_pending());
        assert!(
            outgoing
                .verification_connections
                .try_lock()
                .expect("registration must release eligibility before waiting for callbacks")
                .is_empty()
        );
    }
    let ((id, response), ()) = tokio::join!(registration, disconnect);
    assert!(response.await.is_err());
    assert!(messages.try_recv().is_err());
    assert!(
        !outgoing
            .request_id_to_callback
            .lock()
            .await
            .contains_key(&id)
    );
}

#[tokio::test]
async fn verification_without_a_connected_app_has_no_pending_callback() {
    let (sender, mut messages) = mpsc::channel(4);
    let outgoing = OutgoingMessageSender::new(sender, AnalyticsEventsClient::disabled());
    outgoing
        .enable_user_verification_connection(ConnectionId(1))
        .await;
    let thread_id = ThreadId::new();
    let (_, response) = outgoing
        .send_request_to_connections(Some(&[]), request(thread_id), Some(thread_id))
        .await;
    assert!(response.await.is_err());
    assert!(messages.try_recv().is_err());
    assert!(outgoing.request_id_to_callback.lock().await.is_empty());
}

async fn auth_manager(home: &std::path::Path) -> Arc<AuthManager> {
    let auth = AuthManager::from_auth_for_testing_with_home(
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        home.to_path_buf(),
    );
    write_auth(home, "token-1", "account-1");
    reload_auth(&auth).await;
    auth
}

async fn reload_auth(auth: &AuthManager) {
    let changes = auth.auth_change_receiver();
    let revision = *changes.borrow();
    // reload's boolean compares auth modes; the watch revision also detects token/account changes.
    auth.reload().await;
    assert_eq!(*changes.borrow(), revision + 1);
}

fn write_auth(home: &std::path::Path, token: &str, account: &str) {
    write_chatgpt_auth(
        home,
        ChatGptAuthFixture::new(token)
            .chatgpt_user_id("user-1")
            .account_id(account)
            .chatgpt_account_id(account),
        AuthCredentialsStoreMode::File,
    )
    .unwrap();
}

#[tokio::test]
async fn auth_changes_cancel_user_verification() {
    let home = tempfile::tempdir().unwrap();
    let auth = auth_manager(home.path()).await;
    let (sender, mut messages) = mpsc::channel(4);
    let outgoing = Arc::new(OutgoingMessageSender::new(
        sender,
        AnalyticsEventsClient::disabled(),
    ));
    outgoing.watch_user_verification_auth(Arc::clone(&auth));
    outgoing
        .enable_user_verification_connection(ConnectionId(1))
        .await;
    let thread_id = ThreadId::new();
    for (token, account) in [("token-2", "account-1"), ("token-3", "account-2")] {
        let (_, response) = outgoing
            .send_request_to_connections(
                Some(&[ConnectionId(1)]),
                request(thread_id),
                Some(thread_id),
            )
            .await;
        messages.recv().await.unwrap();
        write_auth(home.path(), token, account);
        reload_auth(&auth).await;
        assert!(
            tokio::time::timeout(std::time::Duration::from_secs(5), response)
                .await
                .unwrap()
                .is_err()
        );
    }
}

#[tokio::test]
async fn account_switch_back_rejects_proof_before_the_auth_watcher_runs() {
    let home = tempfile::tempdir().unwrap();
    let auth = auth_manager(home.path()).await;
    let (sender, mut messages) = mpsc::channel(4);
    let outgoing = OutgoingMessageSender::new(sender, AnalyticsEventsClient::disabled());
    // Leave the watcher stopped to exercise response validation before a coalesced notification.
    assert!(outgoing.verification_auth.set(Arc::clone(&auth)).is_ok());
    outgoing
        .enable_user_verification_connection(ConnectionId(1))
        .await;
    let thread_id = ThreadId::new();
    let (id, response) = outgoing
        .send_request_to_connections(
            Some(&[ConnectionId(1)]),
            request(thread_id),
            Some(thread_id),
        )
        .await;
    messages.recv().await.unwrap();
    for (token, account) in [("token-2", "account-2"), ("token-3", "account-1")] {
        write_auth(home.path(), token, account);
        reload_auth(&auth).await;
    }
    let proof = json!({"action": "accept", "content": {"credentialId": "AQID", "signature": "BAUG"}, "_meta": null});
    outgoing
        .notify_client_response(ConnectionId(1), id, proof)
        .await;
    assert!(response.await.is_err());
}

#[tokio::test]
async fn account_switch_during_eligibility_lock_wait_does_not_reassign_verification() {
    let home = tempfile::tempdir().unwrap();
    let auth = auth_manager(home.path()).await;
    let (sender, mut messages) = mpsc::channel(4);
    let outgoing = OutgoingMessageSender::new(sender, AnalyticsEventsClient::disabled());
    assert!(outgoing.verification_auth.set(Arc::clone(&auth)).is_ok());
    outgoing
        .enable_user_verification_connection(ConnectionId(1))
        .await;
    let thread_id = ThreadId::new();
    let connection_ids = [ConnectionId(1)];

    let mut registration = std::pin::pin!(outgoing.send_request_to_connections(
        Some(&connection_ids),
        request(thread_id),
        Some(thread_id),
    ));
    {
        let eligible = outgoing.verification_connections.lock().await;
        let mut context = Context::from_waker(Waker::noop());
        assert!(registration.as_mut().poll(&mut context).is_pending());
        write_auth(home.path(), "token-2", "account-2");
        drop(eligible);
    }
    reload_auth(&auth).await;

    let (id, response) = registration.await;
    assert!(response.await.is_err());
    assert!(messages.try_recv().is_err());
    assert!(
        !outgoing
            .request_id_to_callback
            .lock()
            .await
            .contains_key(&id)
    );
}
