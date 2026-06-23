use codex_code_mode_protocol::host::CapabilitySet;
use codex_code_mode_protocol::host::ClientHello;
use codex_code_mode_protocol::host::ClientToHost;
use codex_code_mode_protocol::host::HostHello;
use codex_code_mode_protocol::host::HostToClient;
use codex_code_mode_protocol::host::ProtocolVersion;
use codex_code_mode_protocol::host::SessionId;
use codex_code_mode_protocol::host::SupportedProtocolVersions;
use pretty_assertions::assert_eq;
use serde_json::json;

use super::DirectionController;
use super::TransportError;
use super::in_memory_connection;

fn session_id() -> SessionId {
    SessionId::new("session-1").expect("valid session ID")
}

fn supported_versions() -> SupportedProtocolVersions {
    SupportedProtocolVersions::try_new([ProtocolVersion::V1]).expect("supported version")
}

async fn deliver_next_frame(direction: &mut DirectionController) {
    let pending = direction.next_pending_frame().await.expect("pending frame");
    direction.deliver(pending).expect("deliver frame");
    direction
        .next_received_frame()
        .await
        .expect("receiver progress")
        .release()
        .expect("release receiver");
}

#[tokio::test]
async fn handshake_and_session_lifecycle_run_end_to_end() {
    let (mut client, mut host, mut controller) = in_memory_connection();
    let client_hello = ClientToHost::ClientHello(
        ClientHello::new(
            supported_versions(),
            CapabilitySet::empty(),
            CapabilitySet::empty(),
        )
        .expect("valid client hello"),
    );
    client.send(&client_hello).expect("queue client hello");
    let (received, ()) = tokio::join!(
        host.receive(),
        deliver_next_frame(&mut controller.client_to_host)
    );
    assert_eq!(received.expect("receive client hello"), Some(client_hello));

    let host_hello =
        HostToClient::HostHello(HostHello::new(ProtocolVersion::V1, CapabilitySet::empty()));
    host.send(&host_hello).expect("queue host hello");
    let (received, ()) = tokio::join!(
        client.receive(),
        deliver_next_frame(&mut controller.host_to_client)
    );
    assert_eq!(received.expect("receive host hello"), Some(host_hello));

    let open = ClientToHost::OpenSession {
        session_id: session_id(),
    };
    client.send(&open).expect("queue session open");
    let (received, ()) = tokio::join!(
        host.receive(),
        deliver_next_frame(&mut controller.client_to_host)
    );
    assert_eq!(received.expect("receive session open"), Some(open));

    let ready = HostToClient::SessionReady {
        session_id: session_id(),
    };
    host.send(&ready).expect("queue session ready");
    let (received, ()) = tokio::join!(
        client.receive(),
        deliver_next_frame(&mut controller.host_to_client)
    );
    assert_eq!(received.expect("receive session ready"), Some(ready));

    let close = ClientToHost::CloseSession {
        session_id: session_id(),
    };
    client.send(&close).expect("queue session close");
    let (received, ()) = tokio::join!(
        host.receive(),
        deliver_next_frame(&mut controller.client_to_host)
    );
    assert_eq!(received.expect("receive session close"), Some(close));

    let closed = HostToClient::SessionClosed {
        session_id: session_id(),
    };
    host.send(&closed).expect("queue session closed");
    let (received, ()) = tokio::join!(
        client.receive(),
        deliver_next_frame(&mut controller.host_to_client)
    );
    assert_eq!(received.expect("receive session closed"), Some(closed));
}

#[tokio::test]
async fn controller_gates_frames_before_delivery_and_after_receive() {
    let (client, mut host, mut controller) = in_memory_connection();
    let message = ClientToHost::OpenSession {
        session_id: session_id(),
    };
    client.send(&message).expect("queue client frame");

    let pending = controller
        .client_to_host
        .next_pending_frame()
        .await
        .expect("pending client frame");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(pending.as_bytes()).expect("valid JSON"),
        json!({ "type": "session/open", "sessionId": "session-1" })
    );

    let receive = tokio::spawn(async move { host.receive().await });
    assert!(!receive.is_finished());
    controller
        .client_to_host
        .deliver(pending)
        .expect("deliver client frame");

    let received = controller
        .client_to_host
        .next_received_frame()
        .await
        .expect("receiver progress");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(received.as_bytes()).expect("valid JSON"),
        json!({ "type": "session/open", "sessionId": "session-1" })
    );
    assert!(!receive.is_finished());
    received.release().expect("release receiver");

    assert_eq!(
        receive.await.expect("receive task").expect("receive frame"),
        Some(message)
    );
}

#[tokio::test]
async fn directions_preserve_order_independently() {
    let (mut client, mut host, mut controller) = in_memory_connection();
    let open = ClientToHost::OpenSession {
        session_id: session_id(),
    };
    let close = ClientToHost::CloseSession {
        session_id: session_id(),
    };
    let ready = HostToClient::SessionReady {
        session_id: session_id(),
    };
    let closed = HostToClient::SessionClosed {
        session_id: session_id(),
    };

    client.send(&open).expect("queue open");
    client.send(&close).expect("queue close");
    host.send(&ready).expect("queue ready");
    host.send(&closed).expect("queue closed");

    let host_receive = tokio::spawn(async move {
        [
            host.receive().await.expect("receive open"),
            host.receive().await.expect("receive close"),
        ]
    });
    let client_receive = tokio::spawn(async move {
        [
            client.receive().await.expect("receive ready"),
            client.receive().await.expect("receive closed"),
        ]
    });

    for direction in [
        &mut controller.client_to_host,
        &mut controller.host_to_client,
    ] {
        for _ in 0..2 {
            let pending = direction.next_pending_frame().await.expect("pending frame");
            direction.deliver(pending).expect("deliver frame");
            direction
                .next_received_frame()
                .await
                .expect("receiver progress")
                .release()
                .expect("release receiver");
        }
    }

    assert_eq!(
        host_receive.await.expect("host receive task"),
        [Some(open), Some(close)]
    );
    assert_eq!(
        client_receive.await.expect("client receive task"),
        [Some(ready), Some(closed)]
    );
}

#[tokio::test]
async fn malformed_complete_frame_fails_decode_after_receive_barrier() {
    let (_client, mut host, mut controller) = in_memory_connection();
    let receive = tokio::spawn(async move { host.receive().await });

    controller
        .client_to_host
        .deliver_bytes(br#"{"type":"session/open","sessionId":"#.to_vec())
        .expect("inject malformed frame");
    let received = controller
        .client_to_host
        .next_received_frame()
        .await
        .expect("receiver progress");
    assert!(!receive.is_finished());
    received.release().expect("release receiver");

    assert!(matches!(
        receive.await.expect("receive task"),
        Err(TransportError::Decode(_))
    ));
}

#[tokio::test]
async fn read_and_write_failures_are_injected_per_direction() {
    let (client, mut host, mut controller) = in_memory_connection();
    controller.client_to_host.fail_writes();
    assert!(matches!(
        client.send(&ClientToHost::OpenSession {
            session_id: session_id(),
        }),
        Err(TransportError::WriteClosed)
    ));

    controller
        .client_to_host
        .inject_read_failure("injected read failure")
        .expect("inject read failure");
    assert!(matches!(
        host.receive().await,
        Err(TransportError::ReadFailed(message)) if message == "injected read failure"
    ));

    let response = HostToClient::SessionClosed {
        session_id: session_id(),
    };
    host.send(&response)
        .expect("opposite direction remains open");
    assert!(
        controller
            .host_to_client
            .next_pending_frame()
            .await
            .is_some()
    );
}

#[tokio::test]
async fn closing_one_direction_produces_eof_without_closing_the_other() {
    let (mut client, mut host, mut controller) = in_memory_connection();
    controller.client_to_host.close();

    assert_eq!(host.receive().await.expect("orderly EOF"), None);
    assert!(matches!(
        client.send(&ClientToHost::CloseSession {
            session_id: session_id(),
        }),
        Err(TransportError::WriteClosed)
    ));

    let response = HostToClient::SessionClosed {
        session_id: session_id(),
    };
    host.send(&response).expect("queue reverse frame");
    let pending = controller
        .host_to_client
        .next_pending_frame()
        .await
        .expect("pending reverse frame");
    controller
        .host_to_client
        .deliver(pending)
        .expect("deliver reverse frame");

    let receive = tokio::spawn(async move { client.receive().await });
    controller
        .host_to_client
        .next_received_frame()
        .await
        .expect("reverse receiver progress")
        .release()
        .expect("release reverse receiver");
    assert_eq!(
        receive
            .await
            .expect("client receive task")
            .expect("receive reverse frame"),
        Some(response)
    );
}
