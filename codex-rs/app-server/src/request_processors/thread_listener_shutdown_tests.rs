#![cfg(test)]

//! Checks runtime release and notification delivery around listener shutdown.

use super::super::thread_goal_processor::ThreadGoalRequestProcessor;
use super::super::thread_processor::ThreadRequestProcessor;
use super::super::thread_processor::ThreadResumeTarget;
use super::*;
use crate::config_manager::ConfigManager;
use crate::outgoing_message::OutgoingEnvelope;
use crate::outgoing_message::OutgoingMessage;
use crate::thread_sample::SampleResponseRegistry;
use crate::thread_state::ConnectionCapabilities;
use crate::thread_state::resolve_server_request_on_thread_listener;
use anyhow::Context;
use anyhow::Result;
use codex_analytics::AnalyticsEventsClient;
use codex_app_server_protocol::ClientResponsePayload;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadUnsubscribeParams;
use codex_app_server_protocol::ThreadUnsubscribeResponse;
use codex_app_server_protocol::ThreadUnsubscribeStatus;
use codex_arg0::Arg0DispatchPaths;
use codex_goal_extension::GoalService;
use codex_login::AuthManager;
use codex_thread_store::PersistContext;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use std::collections::HashSet;
use test_case::test_case;
use tokio::sync::Semaphore;
use tokio::sync::mpsc;

#[test_case(true; "listener attached before shutdown")]
#[test_case(false; "listener attached after shutdown")]
#[tokio::test]
async fn listener_shutdown_releases_runtime_and_preserves_other_subscribers(
    attach_before_shutdown: bool,
) -> Result<()> {
    let server = wiremock::MockServer::start().await;
    let test = test_codex().build_with_auto_env(&server).await?;
    let thread_id = test.session_configured.thread_id;
    test.thread_store
        .persist_thread(thread_id, PersistContext::Standard)
        .await?;
    let thread = test.codex;
    let runtime = Arc::downgrade(&thread);
    let config = Arc::new(test.config.clone());
    let auth =
        AuthManager::shared_from_config(config.as_ref(), /*enable_codex_api_key_env*/ false)
            .await?;
    let config_manager =
        ConfigManager::without_managed_config_for_tests(config.codex_home.to_path_buf());
    let (outgoing_tx, mut outgoing_rx) = mpsc::channel(/*buffer*/ 16);
    let outgoing = Arc::new(OutgoingMessageSender::new(
        outgoing_tx,
        AnalyticsEventsClient::disabled(),
    ));
    let thread_state_manager = ThreadStateManager::new();
    let thread_watch_manager = ThreadWatchManager::new_with_outgoing(outgoing.clone());
    let skills_watcher = SkillsWatcher::new(
        test.thread_manager.skills_service(),
        &config.codex_home,
        outgoing.clone(),
    );
    let thread_goal_processor = ThreadGoalRequestProcessor::new(
        test.thread_manager.clone(),
        outgoing.clone(),
        config.clone(),
        thread_state_manager.clone(),
        /*state_db*/ None,
        Arc::new(GoalService::new()),
        config_manager.clone(),
    );
    let processor = ThreadRequestProcessor::new(
        auth,
        test.thread_manager.clone(),
        outgoing,
        Arg0DispatchPaths::default(),
        config,
        config_manager,
        test.thread_store.clone(),
        Default::default(),
        thread_state_manager.clone(),
        thread_watch_manager,
        Arc::new(Semaphore::new(/*permits*/ 1)),
        thread_goal_processor,
        /*state_db*/ None,
        /*log_db*/ None,
        skills_watcher.clone(),
        /*turn_cost_worker*/ None,
        Vec::new(),
        Arc::new(SampleResponseRegistry::default()),
    );
    let connection_id = ConnectionId(1);
    let disconnecting_connection_id = ConnectionId(2);
    let remaining_connection_id = ConnectionId(3);
    let connection_ids = vec![
        connection_id,
        disconnecting_connection_id,
        remaining_connection_id,
    ];
    for &connection_id in &connection_ids {
        processor
            .connection_initialized(connection_id, ConnectionCapabilities::default())
            .await;
    }
    if attach_before_shutdown {
        processor
            .try_attach_thread_listener(thread_id, Arc::clone(&thread), connection_ids.clone())
            .await;
    }
    thread.shutdown_and_wait().await?;
    let shutdown_result = tokio::time::timeout(Duration::from_secs(/*secs*/ 10), async {
        let resume = processor
            .thread_resume(
                ThreadResumeTarget::Client(ConnectionRequestId {
                    connection_id,
                    request_id: RequestId::Integer(1),
                }),
                ThreadResumeParams {
                    thread_id: thread_id.to_string(),
                    exclude_turns: true,
                    ..Default::default()
                },
                /*app_server_client_name*/ None,
                /*app_server_client_version*/ None,
                /*client_mcp_extensions*/ Default::default(),
            )
            .await;
        test.thread_manager.remove_thread(&thread_id).await;
        if !attach_before_shutdown {
            processor
                .try_attach_thread_listener(thread_id, Arc::clone(&thread), connection_ids)
                .await;
        }
        loop {
            match outgoing_rx.recv().await.expect("outgoing channel closed") {
                OutgoingEnvelope::Broadcast {
                    message: OutgoingMessage::AppServerNotification(notification),
                } if matches!(
                    &notification.notification,
                    ServerNotification::ThreadStatusChanged(status)
                        if status.status == ThreadStatus::NotLoaded
                ) =>
                {
                    break;
                }
                _ => {}
            }
        }
        drop(thread);
        while runtime.upgrade().is_some() {
            tokio::task::yield_now().await;
        }
        let Some(ClientResponsePayload::ThreadUnsubscribe(unsubscribe)) = processor
            .thread_unsubscribe(
                &ConnectionRequestId {
                    connection_id,
                    request_id: RequestId::Integer(2),
                },
                ThreadUnsubscribeParams {
                    thread_id: thread_id.to_string(),
                },
            )
            .await
            .expect("unsubscribe failed")
        else {
            panic!("unsubscribe response missing");
        };
        assert_eq!(
            unsubscribe,
            ThreadUnsubscribeResponse {
                status: ThreadUnsubscribeStatus::NotLoaded,
            },
        );
        assert_eq!(
            thread_state_manager
                .subscribed_connection_ids(thread_id)
                .await
                .into_iter()
                .collect::<HashSet<_>>(),
            HashSet::from([disconnecting_connection_id, remaining_connection_id]),
        );
        processor
            .connection_closed(disconnecting_connection_id)
            .await;
        assert_eq!(
            thread_state_manager
                .subscribed_connection_ids(thread_id)
                .await,
            vec![remaining_connection_id],
        );
        resolve_server_request_on_thread_listener(
            &thread_state_manager.thread_state(thread_id).await,
            RequestId::Integer(3),
        )
        .await;
        loop {
            if let OutgoingEnvelope::ToConnection {
                connection_id: recipient,
                message: OutgoingMessage::AppServerNotification(notification),
                ..
            } = outgoing_rx.recv().await.expect("outgoing channel closed")
                && recipient == remaining_connection_id
                && matches!(
                    &notification.notification,
                    ServerNotification::ServerRequestResolved(resolved)
                        if resolved.request_id == RequestId::Integer(3)
                )
            {
                break;
            }
        }
        resume
    })
    .await;
    thread_state_manager.clear_all_listeners().await;
    skills_watcher.shutdown();

    let resume = shutdown_result.context("terminal listener did not release or relay")?;
    assert_eq!(
        resume.expect_err("resume accepted a terminated runtime"),
        invalid_request(format!(
            "thread {thread_id} is closing; retry thread/resume after the thread is closed"
        )),
    );
    Ok(())
}
