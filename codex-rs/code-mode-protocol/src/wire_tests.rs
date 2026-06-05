use tokio::io::duplex;

use super::ClientMessage;
use super::HostRequest;
use super::read_frame;
use super::write_frame;

#[tokio::test]
async fn frame_round_trip_preserves_message() {
    let (mut client, mut server) = duplex(1024);
    let message = ClientMessage::Request {
        id: 7,
        request: HostRequest::CreateSession,
    };

    write_frame(&mut client, &message)
        .await
        .expect("write frame");
    let decoded = read_frame(&mut server).await.expect("read frame");

    assert!(matches!(
        decoded,
        Some(ClientMessage::Request {
            id: 7,
            request: HostRequest::CreateSession,
        })
    ));
}
