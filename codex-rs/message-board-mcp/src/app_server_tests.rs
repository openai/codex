#![cfg(test)]
//! Exercise bounded, concurrent notification delivery and recovery without replay.

use super::AppServerNotifications;
use anyhow::Context;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_protocol::protocol::InterAgentCommunication;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

#[tokio::test]
async fn bounded_concurrent_notices_recover_without_replay() -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let (client, mut peer) = tokio::try_join!(
        AppServerNotifications::connect(format!("ws://{}", listener.local_addr()?).parse()?),
        accept_connection(&listener),
    )?;
    let recipient = ThreadId::new();
    // 1,007 text bytes + /root + /root/worker exactly fill the existing limit.
    let rejected = notice(&"x".repeat(1007));
    for oversized in [
        InterAgentCommunication {
            author: AgentPath::root().join("a").expect("valid author"),
            ..rejected.clone()
        },
        InterAgentCommunication {
            recipient: AgentPath::root()
                .join("worker_long")
                .expect("valid recipient"),
            ..rejected.clone()
        },
    ] {
        assert_eq!(
            client
                .inject(recipient, oversized)
                .await
                .unwrap_err()
                .to_string(),
            "remote board notice exceeds the context budget"
        );
    }
    let accepted = notice("parallel");
    let (rejection, result, served) = tokio::join!(biased;
        client.inject(recipient, rejected.clone()),
        client.inject(recipient, accepted.clone()),
        async {
            let first = receive_notice(&mut peer, recipient, &rejected).await?;
            // Neither call gets a reply until both have reached the same connection.
            let second = receive_notice(&mut peer, recipient, &accepted).await?;
            assert_ne!(first, second);
            send_json(&mut peer, json!({"id": second, "result": {}})).await?;
            send_json(&mut peer, json!({"id": first, "error": {"code": -32602, "message": "recipient unavailable"}})).await
        }
    );
    served?;
    result?;
    assert!(rejection.unwrap_err().to_string().contains("-32602"));
    // An RPC rejection must leave this socket usable for the next notice.
    let unanswered = notice("unanswered");
    let (result, received) = tokio::join!(
        client.inject(recipient, unanswered.clone()),
        receive_notice(&mut peer, recipient, &unanswered),
    );
    received?;
    assert_eq!(
        result.unwrap_err().to_string(),
        "app-server notification timed out"
    );
    assert!(matches!(
        peer.next().await.transpose()?,
        Some(Message::Close(_))
    ));

    let accepted = notice("accepted after timeout");
    let (result, served) = tokio::join!(client.inject(recipient, accepted.clone()), async {
        let mut replacement = accept_connection(&listener).await?;
        // The first request on the new connection must be the new notice.
        let id = receive_notice(&mut replacement, recipient, &accepted).await?;
        send_json(&mut replacement, json!({"id": id, "result": {}})).await?;
        Ok::<_, anyhow::Error>(replacement)
    });
    let mut replacement = served?;
    result?;
    drop(client);
    // Also catch any replay queued behind the successful request.
    assert!(
        replacement
            .next()
            .await
            .is_none_or(|frame| matches!(frame, Ok(Message::Close(_)) | Err(_)))
    );
    Ok(())
}

fn notice(text: &str) -> InterAgentCommunication {
    InterAgentCommunication::new(
        AgentPath::root(),
        AgentPath::root().join("worker").expect("valid recipient"),
        Vec::new(),
        text.into(),
        /*trigger_turn*/ false,
    )
}

async fn accept_connection(listener: &TcpListener) -> anyhow::Result<WebSocketStream<TcpStream>> {
    let (stream, _) = listener.accept().await?;
    let mut peer = accept_async(stream).await?;
    let initialize = read_json(&mut peer).await?;
    assert_eq!(initialize["method"], "initialize");
    send_json(&mut peer, json!({"id": initialize["id"], "result": {}})).await?;
    assert_eq!(
        read_json(&mut peer).await?,
        json!({"method": "initialized"})
    );
    Ok(peer)
}

async fn receive_notice(
    peer: &mut WebSocketStream<TcpStream>,
    recipient: ThreadId,
    expected: &InterAgentCommunication,
) -> anyhow::Result<Value> {
    let request = read_json(peer).await?;
    assert_eq!(
        request,
        json!({
            "id": request["id"],
            "method": "thread/inject_items",
            "params": {"threadId": recipient.to_string(), "items": [expected.to_model_input_item()]}
        })
    );
    Ok(request["id"].clone())
}

async fn read_json(peer: &mut WebSocketStream<TcpStream>) -> anyhow::Result<Value> {
    let frame = peer.next().await.context("expected a JSON-RPC message")??;
    Ok(serde_json::from_str(frame.to_text()?)?)
}

async fn send_json(peer: &mut WebSocketStream<TcpStream>, value: Value) -> anyhow::Result<()> {
    peer.send(Message::Text(value.to_string().into())).await?;
    Ok(())
}
