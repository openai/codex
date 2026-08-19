use crate::unified_exec::UNIFIED_EXEC_OUTPUT_MAX_BYTES;
use crate::unified_exec::format_output_omission_marker;
use std::collections::VecDeque;

/// A capped buffer that preserves a stable prefix ("head") and suffix ("tail"),
/// dropping the middle once it exceeds the configured maximum. The buffer is
/// symmetric meaning 50% of the capacity is allocated to the head and 50% is
/// allocated to the tail.
#[derive(Debug, Default)]
#[cfg_attr(test, derive(Eq, PartialEq))]
pub(crate) struct HeadTailBuffer<const MAX_BYTES: usize = UNIFIED_EXEC_OUTPUT_MAX_BYTES> {
    head: Vec<u8>,
    tail: VecDeque<u8>,
    omitted_bytes: usize,
}

impl<const MAX_BYTES: usize> HeadTailBuffer<MAX_BYTES> {
    const HEAD_BUDGET: usize = MAX_BYTES / 2;
    const TAIL_BUDGET: usize = MAX_BYTES.saturating_sub(Self::HEAD_BUDGET);

    // Used for tests.
    #[allow(dead_code)]
    /// Total bytes currently retained by the buffer (head + tail).
    pub(crate) fn retained_bytes(&self) -> usize {
        self.head.len().saturating_add(self.tail.len())
    }

    // Used for tests.
    #[allow(dead_code)]
    /// Total bytes that were dropped from the middle due to the size cap.
    pub(crate) fn omitted_bytes(&self) -> usize {
        self.omitted_bytes
    }

    /// Total bytes observed by the buffer, including bytes omitted by the cap.
    pub(crate) fn total_bytes(&self) -> usize {
        self.retained_bytes().saturating_add(self.omitted_bytes)
    }

    /// Append a chunk of bytes to the buffer.
    ///
    /// Bytes are first added to the head until the head budget is full; any
    /// remaining bytes are added to the tail, with older tail bytes being
    /// dropped to preserve the tail budget.
    pub(crate) fn push_chunk(&mut self, chunk: Vec<u8>) {
        if chunk.is_empty() {
            return;
        }
        if MAX_BYTES == 0 {
            self.omitted_bytes = self.omitted_bytes.saturating_add(chunk.len());
            return;
        }

        // Fill the head budget first, then keep a capped tail.
        let remaining_head = Self::HEAD_BUDGET.saturating_sub(self.head.len());
        let head_len = remaining_head.min(chunk.len());
        if head_len > 0 {
            self.head.extend_from_slice(&chunk[..head_len]);
        }
        self.push_to_tail(&chunk[head_len..]);
    }

    /// Return the retained output as a single byte vector.
    ///
    /// The output is formed by concatenating head chunks, then tail chunks.
    /// Omitted bytes are not represented in the returned value.
    pub(crate) fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.retained_bytes());
        out.extend_from_slice(&self.head);
        out.extend(self.tail.iter().copied());
        out
    }

    /// Return the retained output with an explicit marker between the head and
    /// tail when bytes were omitted.
    pub(crate) fn to_bytes_with_omission_marker(&self) -> Vec<u8> {
        if self.omitted_bytes == 0 {
            return self.to_bytes();
        }

        let marker = format_output_omission_marker(self.omitted_bytes);
        let marker_delimiter_bytes = 2;
        let mut out = Vec::with_capacity(
            self.retained_bytes()
                .saturating_add(marker.len())
                .saturating_add(marker_delimiter_bytes),
        );
        out.extend_from_slice(&self.head);
        out.push(b'\n');
        out.extend_from_slice(marker.as_bytes());
        out.push(b'\n');
        out.extend(self.tail.iter().copied());
        out
    }

    /// Append a later buffer with the same budget. This preserves the summary
    /// of the original concatenated output, including its omission count.
    pub(crate) fn push_buffer(&mut self, mut buffer: Self) {
        self.push_chunk(std::mem::take(&mut buffer.head));
        self.push_chunk(buffer.tail.drain(..).collect());
        self.omitted_bytes = self.omitted_bytes.saturating_add(buffer.omitted_bytes);
    }

    fn push_to_tail(&mut self, chunk: &[u8]) {
        if chunk.is_empty() {
            return;
        }
        if Self::TAIL_BUDGET == 0 {
            self.omitted_bytes = self.omitted_bytes.saturating_add(chunk.len());
            return;
        }

        if chunk.len() >= Self::TAIL_BUDGET {
            // This single chunk is larger than the whole tail budget. Keep only the last
            // tail_budget bytes and drop everything else.
            let start = chunk.len().saturating_sub(Self::TAIL_BUDGET);
            let kept = &chunk[start..];
            let dropped = chunk.len().saturating_sub(kept.len());
            self.omitted_bytes = self
                .omitted_bytes
                .saturating_add(self.tail.len())
                .saturating_add(dropped);
            self.tail.clear();
            self.tail.extend(kept);
            return;
        }

        self.tail.extend(chunk);
        self.trim_tail_to_budget();
    }

    fn trim_tail_to_budget(&mut self) {
        let excess = self.tail.len().saturating_sub(Self::TAIL_BUDGET);
        if excess > 0 {
            drop(self.tail.drain(..excess));
            self.omitted_bytes = self.omitted_bytes.saturating_add(excess);
        }
    }
}

#[cfg(test)]
#[path = "head_tail_buffer_tests.rs"]
mod tests;
