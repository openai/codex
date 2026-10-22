//! Exercise write ordering independently of rmcp's send-task scheduling.

use super::OrderedByteTransport;
use rmcp::RoleClient;
use rmcp::RoleServer;
use rmcp::model::CallToolRequest;
use rmcp::model::CallToolRequestParams;
use rmcp::model::CancelledNotification;
use rmcp::model::CancelledNotificationParam;
use rmcp::model::ClientJsonRpcMessage;
use rmcp::model::ClientNotification;
use rmcp::model::ClientRequest;
use rmcp::model::RequestId;
use rmcp::transport::IntoTransport;
use rmcp::transport::Transport;
use std::time::Duration;
use tokio::time::timeout;

#[tokio::test]
async fn cancellation_cannot_overtake_request() -> anyhow::Result<()> {
    let (client_io, server_io) = tokio::io::duplex(/*max_buf_size*/ 4096);
    let mut client =
        OrderedByteTransport::new(IntoTransport::<RoleClient, _, _>::into_transport(client_io));
    let mut server = IntoTransport::<RoleServer, _, _>::into_transport(server_io);
    let request = ClientJsonRpcMessage::request(
        ClientRequest::CallToolRequest(CallToolRequest::new(CallToolRequestParams::new("wait"))),
        RequestId::Number(1),
    );
    let cancellation = ClientJsonRpcMessage::notification(
        ClientNotification::CancelledNotification(CancelledNotification::new(
            CancelledNotificationParam::new(Some(RequestId::Number(1)), None),
        )),
    );

    // Exhaust the task budget: reserving write order must still happen synchronously.
    while tokio::task::coop::has_budget_remaining() {
        tokio::task::consume_budget().await;
    }
    let first = client.send(request.clone());
    let discarded = client.send(request.clone());
    let mut second = Box::pin(client.send(cancellation.clone()));
    tokio::task::yield_now().await;
    assert!(futures::poll!(second.as_mut()).is_pending());
    drop(discarded);
    assert!(futures::poll!(second.as_mut()).is_pending());

    timeout(Duration::from_secs(5), first).await??;
    timeout(Duration::from_secs(5), second).await??;
    let received = [
        timeout(Duration::from_secs(5), server.receive()).await?,
        timeout(Duration::from_secs(5), server.receive()).await?,
    ];
    pretty_assertions::assert_eq!(
        serde_json::to_value(received)?,
        serde_json::to_value([Some(request), Some(cancellation)])?
    );
    Ok(())
}
