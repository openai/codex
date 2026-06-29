use pretty_assertions::assert_eq;
use serde_json::json;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

use super::FramedReader;
use super::FramedWriter;
use super::MAX_FRAME_BYTES;

#[tokio::test]
async fn frame_wire_format_is_little_endian_length_prefixed_json() {
    let (writer, mut reader) = tokio::io::duplex(/*max_buf_size*/ 128);
    let write = tokio::spawn(async move {
        FramedWriter::new(writer)
            .write(&json!({"value": 1}))
            .await
            .expect("write frame");
    });

    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).await.expect("read bytes");
    write.await.expect("writer task");

    let payload = br#"{"value":1}"#;
    let mut expected = (payload.len() as u32).to_le_bytes().to_vec();
    expected.extend_from_slice(payload);
    assert_eq!(bytes, expected);
}

#[tokio::test]
async fn fragmented_frame_round_trips() {
    let value = json!({"type": "session/open", "sessionId": "session-1"});
    let payload = serde_json::to_vec(&value).expect("serialize");
    let mut bytes = (payload.len() as u32).to_le_bytes().to_vec();
    bytes.extend(payload);

    let (mut writer, reader) = tokio::io::duplex(/*max_buf_size*/ 128);
    let write = tokio::spawn(async move {
        for byte in bytes {
            writer.write_all(&[byte]).await.expect("write byte");
            tokio::task::yield_now().await;
        }
    });

    assert_eq!(
        FramedReader::new(reader)
            .read::<serde_json::Value>()
            .await
            .expect("read frame"),
        Some(value)
    );
    write.await.expect("writer task");
}

#[tokio::test]
async fn eof_is_clean_only_at_a_frame_boundary() {
    let (writer, reader) = tokio::io::duplex(/*max_buf_size*/ 16);
    drop(writer);
    assert_eq!(
        FramedReader::new(reader)
            .read::<serde_json::Value>()
            .await
            .expect("clean eof"),
        None
    );

    let (mut writer, reader) = tokio::io::duplex(/*max_buf_size*/ 16);
    writer
        .write_all(&[1, 0])
        .await
        .expect("write partial header");
    drop(writer);
    let err = FramedReader::new(reader)
        .read::<serde_json::Value>()
        .await
        .expect_err("truncated header");
    assert_eq!(err.kind(), std::io::ErrorKind::UnexpectedEof);
}

#[tokio::test]
async fn oversized_and_malformed_frames_are_rejected() {
    let (mut writer, reader) = tokio::io::duplex(/*max_buf_size*/ 16);
    writer
        .write_all(&((MAX_FRAME_BYTES as u32) + 1).to_le_bytes())
        .await
        .expect("write oversized header");
    let err = FramedReader::new(reader)
        .read::<serde_json::Value>()
        .await
        .expect_err("oversized frame");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);

    let (mut writer, reader) = tokio::io::duplex(/*max_buf_size*/ 16);
    writer
        .write_all(&(1_u32).to_le_bytes())
        .await
        .expect("write length");
    writer.write_all(b"{").await.expect("write malformed json");
    let err = FramedReader::new(reader)
        .read::<serde_json::Value>()
        .await
        .expect_err("malformed frame");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
}
