//! Verify persistent history, peer fallback, and bounded decompression.

#![cfg(test)]

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn response_header_selects_compression_or_plain_fallback() -> io::Result<()> {
    assert_eq!(
        WebSocketCompression::from_response_header(/*value*/ None)?,
        WebSocketCompression::Plain
    );
    assert_eq!(
        WebSocketCompression::from_response_header(Some(ZSTD_STREAM_PROTOCOL))?,
        WebSocketCompression::Zstd
    );
    assert!(WebSocketCompression::from_response_header(Some("unknown")).is_err());
    Ok(())
}

#[test]
fn messages_keep_history_and_have_independent_size_limits() -> io::Result<()> {
    let text = serde_json::to_string(
        &(0..1_000)
            .map(|index| format!("thread-{index}: individual message body {index}"))
            .collect::<Vec<_>>(),
    )?;
    let connection_id = ConnectionId(42);
    let (mut encoder, mut decoder) =
        zstd_codec_pair(text.len(), CompressionConnection::Listener(connection_id))?;
    assert_eq!(encoder.stats.as_ref().unwrap().saved_percent(), None);
    let first = encoder.encode(&text)?;
    let second = encoder.encode(&text)?;
    assert_eq!(decoder.decode(&first)?, text);
    assert_eq!(decoder.decode(&second)?, text);
    let tx = encoder.stats.as_ref().unwrap();
    let rx = decoder.stats.as_ref().unwrap();
    assert_eq!(
        (tx.connection_id, rx.connection_id),
        (connection_id.0, connection_id.0)
    );
    assert_eq!(
        (tx.uncompressed_bytes, tx.compressed_bytes, tx.messages),
        (
            (text.len() * 2) as u64,
            (first.len() + second.len()) as u64,
            2
        )
    );
    assert_eq!(
        (rx.uncompressed_bytes, rx.compressed_bytes, rx.messages),
        (tx.uncompressed_bytes, tx.compressed_bytes, tx.messages)
    );
    assert_eq!(
        tx.saved_percent(),
        Some(100.0 * (1.0 - (first.len() + second.len()) as f64 / (text.len() * 2) as f64))
    );
    assert!(second.len() < first.len());
    assert!(ZstdDecoder::new(text.len())?.decode(&second).is_err());
    Ok(())
}

#[test]
fn decoded_size_limit_covers_output_larger_than_scratch_buffer() -> io::Result<()> {
    let text = "abcdefgh".repeat(BUFFER_SIZE);
    let encoded = ZstdEncoder::new()?.encode(&text)?;
    assert_eq!(ZstdDecoder::new(text.len())?.decode(&encoded)?, text);
    let (_, mut decoder) = zstd_codec_pair(text.len() - 1, CompressionConnection::NativeClient)?;
    let error = decoder
        .decode(&encoded)
        .expect_err("oversize message must fail");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    let stats = decoder.stats.as_ref().unwrap();
    assert_eq!((stats.uncompressed_bytes, stats.compressed_bytes), (0, 0));
    Ok(())
}

#[test]
fn malformed_compressed_data_fails() -> io::Result<()> {
    assert!(
        ZstdDecoder::new(/*max_message_size*/ 1024)?
            .decode(b"not zstd")
            .is_err()
    );
    Ok(())
}

#[test]
fn larger_compression_window_is_rejected() -> io::Result<()> {
    let mut encoder = ZstdEncoder::new()?;
    encoder
        .encoder
        .set_parameter(CParameter::WindowLog(WINDOW_LOG + 1))?;
    let encoded = encoder.encode("a message with an unsupported compression window")?;
    assert!(
        ZstdDecoder::new(/*max_message_size*/ 1024)?
            .decode(&encoded)
            .is_err()
    );
    Ok(())
}
