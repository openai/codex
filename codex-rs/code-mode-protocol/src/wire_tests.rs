use tokio::io::AsyncWriteExt;
use tokio::io::duplex;

use super::ClientMessage;
use super::HostRequest;
use super::MAX_FRAME_BYTES;
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

#[tokio::test]
async fn frame_round_trip_supports_payloads_larger_than_sixteen_mib() {
    let (mut client, mut server) = duplex(64 * 1024);
    let payload = "x".repeat(17 * 1024 * 1024);

    let (write_result, read_result) = tokio::join!(
        write_frame(&mut client, &payload),
        read_frame::<_, String>(&mut server),
    );

    write_result.expect("write frame");
    assert_eq!(read_result.expect("read frame"), Some(payload));
}

#[tokio::test]
async fn read_frame_rejects_lengths_above_allocation_limit() {
    let (mut client, mut server) = duplex(16);
    client
        .write_u32(u32::try_from(MAX_FRAME_BYTES + 1).expect("frame limit fits in u32"))
        .await
        .expect("write frame length");

    let error = read_frame::<_, String>(&mut server)
        .await
        .expect_err("reject oversized frame");

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
}
