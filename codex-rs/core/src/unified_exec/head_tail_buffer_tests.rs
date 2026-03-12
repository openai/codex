use super::HeadTailBuffer;

use pretty_assertions::assert_eq;

#[test]
fn keeps_prefix_and_suffix_when_over_budget() {
    let mut buf = HeadTailBuffer::new(10);

    buf.push_chunk(b"0123456789".to_vec());
    assert_eq!(buf.omitted_bytes(), 0);

    // Exceeds max by 2; we should keep head+tail and omit the middle.
    buf.push_chunk(b"ab".to_vec());
    assert!(buf.omitted_bytes() > 0);

    let rendered = String::from_utf8_lossy(&buf.to_bytes()).to_string();
    assert!(rendered.starts_with("01234"));
    assert!(rendered.ends_with("89ab"));
}

#[test]
fn max_bytes_zero_drops_everything() {
    let mut buf = HeadTailBuffer::new(0);
    buf.push_chunk(b"abc".to_vec());

    assert_eq!(buf.retained_bytes(), 0);
    assert_eq!(buf.omitted_bytes(), 3);
    assert_eq!(buf.to_bytes(), b"".to_vec());
    assert_eq!(buf.snapshot_chunks(), Vec::<Vec<u8>>::new());
}

#[test]
fn head_budget_zero_keeps_only_last_byte_in_tail() {
    let mut buf = HeadTailBuffer::new(1);
    buf.push_chunk(b"abc".to_vec());

    assert_eq!(buf.retained_bytes(), 1);
    assert_eq!(buf.omitted_bytes(), 2);
    assert_eq!(buf.to_bytes(), b"c".to_vec());
}

#[test]
fn draining_resets_state() {
    let mut buf = HeadTailBuffer::new(10);
    buf.push_chunk(b"0123456789".to_vec());
    buf.push_chunk(b"ab".to_vec());

    let drained = buf.drain_chunks();
    assert!(!drained.is_empty());

    assert_eq!(buf.retained_bytes(), 0);
    assert_eq!(buf.omitted_bytes(), 0);
    assert_eq!(buf.to_bytes(), b"".to_vec());
}

#[test]
fn chunk_larger_than_tail_budget_keeps_only_tail_end() {
    let mut buf = HeadTailBuffer::new(10);
    buf.push_chunk(b"0123456789".to_vec());

    // Tail budget is 5 bytes. This chunk should replace the tail and keep only its last 5 bytes.
    buf.push_chunk(b"ABCDEFGHIJK".to_vec());

    let out = String::from_utf8_lossy(&buf.to_bytes()).to_string();
    assert!(out.starts_with("01234"));
    assert!(out.ends_with("GHIJK"));
    assert!(buf.omitted_bytes() > 0);
}

#[test]
fn fills_head_then_tail_across_multiple_chunks() {
    let mut buf = HeadTailBuffer::new(10);

    // Fill the 5-byte head budget across multiple chunks.
    buf.push_chunk(b"01".to_vec());
    buf.push_chunk(b"234".to_vec());
    assert_eq!(buf.to_bytes(), b"01234".to_vec());

    // Then fill the 5-byte tail budget.
    buf.push_chunk(b"567".to_vec());
    buf.push_chunk(b"89".to_vec());
    assert_eq!(buf.to_bytes(), b"0123456789".to_vec());
    assert_eq!(buf.omitted_bytes(), 0);

    // One more byte causes the tail to drop its oldest byte.
    buf.push_chunk(b"a".to_vec());
    assert_eq!(buf.to_bytes(), b"012346789a".to_vec());
    assert_eq!(buf.omitted_bytes(), 1);
}
