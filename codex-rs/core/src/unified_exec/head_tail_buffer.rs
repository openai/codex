use crate::unified_exec::UNIFIED_EXEC_OUTPUT_MAX_BYTES;
use std::collections::VecDeque;

/// A capped buffer that preserves a stable prefix ("head") and suffix ("tail"),
/// dropping the middle once it exceeds the configured maximum. The buffer is
/// symmetric meaning 50% of the capacity is allocated to the head and 50% is
/// allocated to the tail.
#[derive(Debug)]
pub(crate) struct HeadTailBuffer {
    max_bytes: usize,
    head_budget: usize,
    tail_budget: usize,
    head: VecDeque<Vec<u8>>,
    tail: VecDeque<Vec<u8>>,
    head_bytes: usize,
    tail_bytes: usize,
    omitted_bytes: usize,
}

impl Default for HeadTailBuffer {
    fn default() -> Self {
        Self::new(UNIFIED_EXEC_OUTPUT_MAX_BYTES)
    }
}

impl HeadTailBuffer {
    /// Create a new buffer that retains at most `max_bytes` of output.
    ///
    /// The retained output is split across a prefix ("head") and suffix ("tail")
    /// budget, dropping bytes from the middle once the limit is exceeded.
    pub(crate) fn new(max_bytes: usize) -> Self {
        let head_budget = max_bytes / 2;
        let tail_budget = max_bytes.saturating_sub(head_budget);
        Self {
            max_bytes,
            head_budget,
            tail_budget,
            head: VecDeque::new(),
            tail: VecDeque::new(),
            head_bytes: 0,
            tail_bytes: 0,
            omitted_bytes: 0,
        }
    }

    // Used for tests.
    #[allow(dead_code)]
    /// Total bytes currently retained by the buffer (head + tail).
    pub(crate) fn retained_bytes(&self) -> usize {
        self.head_bytes.saturating_add(self.tail_bytes)
    }

    // Used for tests.
    #[allow(dead_code)]
    /// Total bytes that were dropped from the middle due to the size cap.
    pub(crate) fn omitted_bytes(&self) -> usize {
        self.omitted_bytes
    }

    /// Append a chunk of bytes to the buffer.
    ///
    /// Bytes are first added to the head until the head budget is full; any
    /// remaining bytes are added to the tail, with older tail bytes being
    /// dropped to preserve the tail budget.
    pub(crate) fn push_chunk(&mut self, chunk: Vec<u8>) {
        if self.max_bytes == 0 {
            self.omitted_bytes = self.omitted_bytes.saturating_add(chunk.len());
            return;
        }

        // Fill the head budget first, then keep a capped tail.
        if self.head_bytes < self.head_budget {
            let remaining_head = self.head_budget.saturating_sub(self.head_bytes);
            if chunk.len() <= remaining_head {
                self.head_bytes = self.head_bytes.saturating_add(chunk.len());
                self.head.push_back(chunk);
                return;
            }

            // Split the chunk: part goes to head, remainder goes to tail.
            let (head_part, tail_part) = chunk.split_at(remaining_head);
            if !head_part.is_empty() {
                self.head_bytes = self.head_bytes.saturating_add(head_part.len());
                self.head.push_back(head_part.to_vec());
            }
            self.push_to_tail(tail_part.to_vec());
            return;
        }

        self.push_to_tail(chunk);
    }

    /// Snapshot the retained output as a list of chunks.
    ///
    /// The returned chunks are ordered as: head chunks first, then tail chunks.
    /// Omitted bytes are not represented in the snapshot.
    pub(crate) fn snapshot_chunks(&self) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        out.extend(self.head.iter().cloned());
        out.extend(self.tail.iter().cloned());
        out
    }

    /// Return the retained output as a single byte vector.
    ///
    /// The output is formed by concatenating head chunks, then tail chunks.
    /// Omitted bytes are not represented in the returned value.
    pub(crate) fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.retained_bytes());
        for chunk in self.head.iter() {
            out.extend_from_slice(chunk);
        }
        for chunk in self.tail.iter() {
            out.extend_from_slice(chunk);
        }
        out
    }

    /// Drain all retained chunks from the buffer and reset its state.
    ///
    /// The drained chunks are returned in head-then-tail order. Omitted bytes
    /// are discarded along with the retained content.
    pub(crate) fn drain_chunks(&mut self) -> Vec<Vec<u8>> {
        let mut out: Vec<Vec<u8>> = self.head.drain(..).collect();
        out.extend(self.tail.drain(..));
        self.head_bytes = 0;
        self.tail_bytes = 0;
        self.omitted_bytes = 0;
        out
    }

    fn push_to_tail(&mut self, chunk: Vec<u8>) {
        if self.tail_budget == 0 {
            self.omitted_bytes = self.omitted_bytes.saturating_add(chunk.len());
            return;
        }

        if chunk.len() >= self.tail_budget {
            // This single chunk is larger than the whole tail budget. Keep only the last
            // tail_budget bytes and drop everything else.
            let start = chunk.len().saturating_sub(self.tail_budget);
            let kept = chunk[start..].to_vec();
            let dropped = chunk.len().saturating_sub(kept.len());
            self.omitted_bytes = self
                .omitted_bytes
                .saturating_add(self.tail_bytes)
                .saturating_add(dropped);
            self.tail.clear();
            self.tail_bytes = kept.len();
            self.tail.push_back(kept);
            return;
        }

        self.tail_bytes = self.tail_bytes.saturating_add(chunk.len());
        self.tail.push_back(chunk);
        self.trim_tail_to_budget();
    }

    fn trim_tail_to_budget(&mut self) {
        let mut excess = self.tail_bytes.saturating_sub(self.tail_budget);
        while excess > 0 {
            match self.tail.front_mut() {
                Some(front) if excess >= front.len() => {
                    excess -= front.len();
                    self.tail_bytes = self.tail_bytes.saturating_sub(front.len());
                    self.omitted_bytes = self.omitted_bytes.saturating_add(front.len());
                    self.tail.pop_front();
                }
                Some(front) => {
                    front.drain(..excess);
                    self.tail_bytes = self.tail_bytes.saturating_sub(excess);
                    self.omitted_bytes = self.omitted_bytes.saturating_add(excess);
                    break;
                }
                None => break,
            }
        }
    }
}

#[cfg(test)]
mod tests {
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
}
