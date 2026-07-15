use super::CONTEXT_APPEND_MAX_BYTES;
use super::context_append_chunks;

#[test]
fn context_append_chunks_preserve_text_within_wire_limit() {
    for text in ["a".repeat(1_201), "🙂".repeat(200)] {
        let chunks = context_append_chunks(&text);
        assert_eq!(chunks.concat(), text);
        assert!(
            chunks
                .iter()
                .all(|chunk| chunk.len() <= CONTEXT_APPEND_MAX_BYTES)
        );
    }
}
