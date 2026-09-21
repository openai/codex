//! Covers the controller's RPC boundary, cancellation generations, and proof redaction.

use super::*;
use codex_app_server_protocol::McpServerElicitationRequest;
use codex_app_server_protocol::McpServerElicitationRequestParams;
use codex_app_server_protocol::ServerRequest;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;

fn verification_request() -> ServerRequest {
    ServerRequest::McpServerElicitationRequest {
        request_id: RequestId::Integer(7),
        params: McpServerElicitationRequestParams {
            thread_id: ThreadId::new().to_string(),
            turn_id: None,
            server_name: "deployments".to_string(),
            request: McpServerElicitationRequest::UserVerification {
                meta: None,
                title: "Approve deployment?".to_string(),
                description: "Deploy the reviewed change.".to_string(),
                challenge: "AQID".to_string(),
            },
        },
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RpcScenario {
    Success,
    MethodNotFound,
    CancelWhilePending,
    RemoteWorkspace,
}

#[tokio::test]
async fn user_verification_controller_uses_the_local_binary_rpc_and_original_response()
-> color_eyre::Result<()> {
    run_verification_rpc_scenario(RpcScenario::Success).await
}

#[tokio::test]
async fn user_verification_controller_cancels_for_an_older_binary() -> color_eyre::Result<()> {
    run_verification_rpc_scenario(RpcScenario::MethodNotFound).await
}

#[tokio::test]
async fn user_verification_controller_suppresses_a_late_proof_after_cancellation()
-> color_eyre::Result<()> {
    run_verification_rpc_scenario(RpcScenario::CancelWhilePending).await
}

#[tokio::test]
async fn user_verification_remote_workspace_dismisses_the_waiting_prompt() -> color_eyre::Result<()>
{
    run_verification_rpc_scenario(RpcScenario::RemoteWorkspace).await
}

async fn run_verification_rpc_scenario(scenario: RpcScenario) -> color_eyre::Result<()> {
    use codex_app_server_protocol::JSONRPCMessage;
    use tokio_tungstenite::tungstenite::Message;

    let (mut app, mut event_rx, _op_rx) = crate::app::tests::make_test_app_with_channels().await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let foreground_thread_id = ThreadId::new();
    app.active_thread_id = Some(foreground_thread_id);
    app.primary_thread_id = Some(foreground_thread_id);
    let request = verification_request();
    let ServerRequest::McpServerElicitationRequest { request_id, params } = &request else {
        unreachable!()
    };
    let thread_id = ThreadId::from_string(&params.thread_id)?;
    app.pending_app_server_requests
        .note_server_request(&request);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = crate::resolve_remote_addr(&format!("ws://{}", listener.local_addr()?))?;
    let expected_params = serde_json::json!({ "title": "Approve deployment?", "description": "Deploy the reviewed change.", "challenge": "AQID" });
    let proof =
        serde_json::json!({ "credentialId": "issued-credential", "signature": "issued-signature" });
    let (verify_started_tx, verify_started_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await?;
        let mut socket = tokio_tungstenite::accept_async(stream).await?;
        let mut verify_started_tx = Some(verify_started_tx);
        let mut pending_verification_id = None;
        let mut response_received = false;
        while let Some(frame) = socket.next().await {
            let Message::Text(text) = frame? else {
                continue;
            };
            match serde_json::from_str::<JSONRPCMessage>(&text)? {
                JSONRPCMessage::Request(request) => {
                    let response = match request.method.as_str() {
                        "initialize" => {
                            serde_json::json!({ "id": request.id, "result": { "userAgent": "codex-tui-test", "codexHome": std::env::temp_dir(), "platformFamily": std::env::consts::FAMILY, "platformOs": std::env::consts::OS } })
                        }
                        "userVerification/verify" => {
                            assert_eq!(request.params, Some(expected_params.clone()));
                            let _ = verify_started_tx
                                .take()
                                .expect("one verification attempt")
                                .send(());
                            if scenario == RpcScenario::CancelWhilePending {
                                pending_verification_id = Some(request.id);
                                continue;
                            }
                            if scenario == RpcScenario::MethodNotFound {
                                serde_json::json!({ "id": request.id, "error": { "code": -32601, "message": "unknown method: private server diagnostic" } })
                            } else {
                                serde_json::json!({ "id": request.id, "result": { "proof": proof } })
                            }
                        }
                        "userVerification/status" => {
                            assert!(response_received);
                            socket.send(Message::Text(serde_json::json!({ "id": request.id, "result": { "unavailableReason": null, "unavailableMessage": null, "credentialId": null } }).to_string().into())).await?;
                            return Ok::<_, color_eyre::Report>(());
                        }
                        method => panic!("unexpected RPC: {method}"),
                    };
                    socket
                        .send(Message::Text(response.to_string().into()))
                        .await?;
                }
                JSONRPCMessage::Response(response) => {
                    assert!(
                        !response_received,
                        "a cancelled verification must never send a later acceptance"
                    );
                    response_received = true;
                    assert_eq!(response.id, RequestId::Integer(7));
                    let expected = if scenario == RpcScenario::Success {
                        serde_json::json!({ "action": "accept", "content": proof, "_meta": null })
                    } else {
                        serde_json::json!({ "action": "cancel", "content": null, "_meta": null })
                    };
                    assert_eq!(response.result, expected);
                    if let Some(id) = pending_verification_id.take() {
                        socket
                            .send(Message::Text(
                                serde_json::json!({ "id": id, "result": { "proof": proof } })
                                    .to_string()
                                    .into(),
                            ))
                            .await?;
                        continue;
                    }
                    return Ok::<_, color_eyre::Report>(());
                }
                JSONRPCMessage::Notification(_) => {}
                JSONRPCMessage::Error(error) => {
                    panic!("unexpected RPC error: {}", error.error.code)
                }
            }
        }
        color_eyre::eyre::bail!("connection closed before the elicitation response")
    });
    let client = crate::connect_remote_app_server(endpoint).await?;
    let mode = if scenario == RpcScenario::RemoteWorkspace {
        app.chat_widget
            .handle_elicitation_request_now(request_id.clone(), params.clone());
        app.chat_widget
            .handle_key_event(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Enter,
                crossterm::event::KeyModifiers::NONE,
            ));
        assert!(
            crate::chatwidget::tests::helpers::render_bottom_popup(
                &app.chat_widget,
                /*width*/ 80
            )
            .contains("Waiting for verification")
        );
        crate::app_server_session::ThreadParamsMode::Remote
    } else {
        crate::app_server_session::ThreadParamsMode::Embedded
    };
    let mut session = AppServerSession::new(client, mode);
    let approval = if scenario == RpcScenario::RemoteWorkspace {
        event_rx.recv().await.expect("approval event")
    } else {
        AppEvent::UserVerificationApproved {
            thread_id,
            server_name: "deployments".to_string(),
            request_id: request_id.clone(),
        }
    };
    app.handle_event(&mut tui, &mut session, approval).await?;
    if scenario == RpcScenario::RemoteWorkspace {
        assert!(
            !crate::chatwidget::tests::helpers::render_bottom_popup(
                &app.chat_widget,
                /*width*/ 80
            )
            .contains("Waiting for verification")
        );
        let AppEvent::SubmitThreadOp { thread_id, op } =
            event_rx.recv().await.expect("cancel request")
        else {
            panic!("expected cancel response");
        };
        app.submit_thread_op(&mut session, thread_id, op).await?;
    }
    if scenario == RpcScenario::CancelWhilePending {
        tokio::time::timeout(
            std::time::Duration::from_secs(/*secs*/ 5),
            verify_started_rx,
        )
        .await??;
        app.submit_thread_op(
            &mut session,
            thread_id,
            AppCommand::resolve_user_verification(
                "deployments".to_string(),
                request_id.clone(),
                UserVerificationResponse::Cancel,
            ),
        )
        .await?;
    }
    if matches!(scenario, RpcScenario::Success | RpcScenario::MethodNotFound) {
        let completion =
            tokio::time::timeout(std::time::Duration::from_secs(/*secs*/ 5), event_rx.recv())
                .await?
                .expect("controller result");
        assert!(matches!(
            &completion,
            AppEvent::UserVerificationFinished { .. }
        ));
        app.handle_event(&mut tui, &mut session, completion).await?;
    }
    if scenario == RpcScenario::CancelWhilePending {
        // Flush the client command queue after processing the late proof. The mock rejects any
        // second elicitation response before acknowledging this harmless read.
        session
            .request_handle()
            .request_typed::<codex_app_server_protocol::UserVerificationStatusResponse>(
                ClientRequest::UserVerificationStatus {
                    request_id: RequestId::String("after-cancel".to_string()),
                    params: codex_app_server_protocol::UserVerificationStatusParams {},
                },
            )
            .await?;
        assert!(
            event_rx.try_recv().is_err(),
            "cancelled verification must drop its RPC future"
        );
    }
    if matches!(
        scenario,
        RpcScenario::MethodNotFound | RpcScenario::RemoteWorkspace
    ) {
        // A failure for the background request must not enter the foreground transcript.
        assert!(event_rx.try_recv().is_err());
        let snapshot = app.thread_event_channels[&thread_id]
            .store
            .lock()
            .await
            .snapshot();
        assert_eq!(snapshot.events.len(), 1);
        let crate::app::ThreadBufferedEvent::Notification(notification) = &snapshot.events[0]
        else {
            panic!("expected verification warning");
        };
        let message = if scenario == RpcScenario::RemoteWorkspace {
            "User verification is unavailable for remote workspaces."
        } else {
            "The local Codex binary could not complete user verification."
        };
        let ServerNotification::Warning(warning) = notification.as_ref() else {
            panic!("expected warning notification");
        };
        assert_eq!(
            warning,
            &WarningNotification {
                thread_id: Some(thread_id.to_string()),
                message: message.to_string(),
            }
        );
        // Render the buffered warning as it appears when the requesting thread is displayed.
        app.chat_widget
            .handle_server_notification(*notification.clone(), /*replay_kind*/ None);
        let AppEvent::InsertHistoryCell(cell) = event_rx.recv().await.expect("warning history")
        else {
            panic!("expected warning history cell");
        };
        let rendered = cell
            .transcript_lines(/*width*/ 80)
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        if scenario == RpcScenario::RemoteWorkspace {
            insta::assert_snapshot!("remote_workspace_verification_warning", rendered);
        } else {
            insta::assert_snapshot!("older_binary_verification_warning", rendered);
        }
    }
    tokio::time::timeout(std::time::Duration::from_secs(/*secs*/ 5), server).await???;
    session.shutdown().await?;
    Ok(())
}
