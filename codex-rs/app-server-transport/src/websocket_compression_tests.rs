//! Verify persistent history, peer fallback, and bounded decompression.

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
    let mut encoder = ZstdEncoder::new()?;
    let first = encoder.encode(&text)?;
    let second = encoder.encode(&text)?;
    let mut decoder = ZstdDecoder::new(text.len())?;
    assert_eq!(decoder.decode(&first)?, text);
    assert_eq!(decoder.decode(&second)?, text);
    assert!(second.len() < first.len());
    assert!(ZstdDecoder::new(text.len())?.decode(&second).is_err());
    Ok(())
}

#[test]
fn decoded_size_limit_covers_output_larger_than_scratch_buffer() -> io::Result<()> {
    let text = "abcdefgh".repeat(BUFFER_SIZE);
    let encoded = ZstdEncoder::new()?.encode(&text)?;
    assert_eq!(ZstdDecoder::new(text.len())?.decode(&encoded)?, text);
    let error = ZstdDecoder::new(text.len() - 1)?
        .decode(&encoded)
        .expect_err("oversize message must fail");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
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
