//! A capped byte buffer that preserves a stable prefix and the most recent suffix.
//! Retained bytes never exceed the caller's const-generic budget.

use std::collections::VecDeque;

/// A capped buffer that preserves a stable prefix ("head") and suffix ("tail"),
/// dropping the middle once it exceeds the configured maximum. The buffer is
/// symmetric meaning 50% of the capacity is allocated to the head and 50% is
/// allocated to the tail.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct HeadTailBuffer<const MAX_BYTES: usize> {
    head: Vec<u8>,
    tail: VecDeque<u8>,
    omitted_bytes: usize,
}

impl<const MAX_BYTES: usize> HeadTailBuffer<MAX_BYTES> {
    const HEAD_BUDGET: usize = MAX_BYTES / 2;
    const TAIL_BUDGET: usize = MAX_BYTES.saturating_sub(Self::HEAD_BUDGET);

    /// Total bytes currently retained by the buffer (head + tail).
    pub fn retained_bytes(&self) -> usize {
        self.head.len().saturating_add(self.tail.len())
    }

    /// Total bytes that were dropped from the middle due to the size cap.
    pub fn omitted_bytes(&self) -> usize {
        self.omitted_bytes
    }

    /// Total bytes observed by the buffer, including bytes omitted by the cap.
    pub fn total_bytes(&self) -> usize {
        self.retained_bytes().saturating_add(self.omitted_bytes)
    }

    /// Append a chunk of bytes to the buffer.
    ///
    /// Bytes are first added to the head until the head budget is full; any
    /// remaining bytes are added to the tail, with older tail bytes being
    /// dropped to preserve the tail budget.
    pub fn push_chunk(&mut self, chunk: &[u8]) {
        let chunk = self.fill_head(chunk);
        self.push_tail(chunk);
    }

    /// Fill the stable prefix and return the bytes that did not fit.
    fn fill_head<'a>(&mut self, chunk: &'a [u8]) -> &'a [u8] {
        let Self {
            head,
            tail: _,
            omitted_bytes: _,
        } = self;

        let remaining_head = Self::HEAD_BUDGET.saturating_sub(head.len());
        // A shorter chunk fits entirely in the head.
        let (chunk_head, chunk_tail) = chunk
            .split_at_checked(remaining_head)
            .unwrap_or((chunk, &[]));
        head.extend_from_slice(chunk_head);
        chunk_tail
    }

    /// Append bytes known not to belong in the head, keeping the newest tail bytes.
    fn push_tail(&mut self, chunk: &[u8]) {
        let Self {
            head: _,
            tail,
            omitted_bytes,
        } = self;

        let remaining_tail = Self::TAIL_BUDGET.saturating_sub(tail.len());
        let excess_tail = chunk.len().saturating_sub(remaining_tail);
        *omitted_bytes = omitted_bytes.saturating_add(excess_tail);

        // Discard old tail bytes first, then skip any excess incoming bytes.
        let chunk = match excess_tail.checked_sub(tail.len()) {
            None => {
                tail.drain(..excess_tail);
                chunk
            }
            Some(skip) => {
                tail.clear();
                &chunk[skip..]
            }
        };
        tail.extend(chunk);
    }

    /// Return the retained output as a single byte vector.
    ///
    /// The output is formed by concatenating head chunks, then tail chunks.
    /// Omitted bytes are not represented in the returned value.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.retained_bytes());
        out.extend_from_slice(&self.head);
        out.extend(self.tail.iter().copied());
        out
    }

    /// Return the retained output with an explicit marker between the head and
    /// tail when bytes were omitted.
    pub fn to_bytes_with_omission_marker(&self) -> Vec<u8> {
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
    pub fn push_buffer(&mut self, buffer: Self) {
        let Self {
            head,
            tail,
            omitted_bytes,
        } = buffer;

        self.omitted_bytes = self.omitted_bytes.saturating_add(omitted_bytes);

        // Preserve an existing prefix; otherwise reuse the source head.
        let overflow = if self.head.is_empty() {
            self.head = head;
            &[]
        } else {
            self.fill_head(&head)
        };

        // A full source tail displaces both the old tail and the unused source head.
        if tail.len() == Self::TAIL_BUDGET {
            self.omitted_bytes = self
                .omitted_bytes
                .saturating_add(self.tail.len())
                .saturating_add(overflow.len());
            self.tail = tail;
        } else {
            self.push_tail(overflow);
            // An empty destination can take a partial source tail without copying it.
            if self.tail.is_empty() {
                self.tail = tail;
            } else {
                // A nonempty source tail means its head, and now ours, is full.
                let (first, second) = tail.as_slices();
                self.push_tail(first);
                self.push_tail(second);
            }
        }
    }
}

/// Render the byte omission notice used by callers of the buffer.
pub fn format_output_omission_marker(omitted_bytes: usize) -> String {
    format!("... {omitted_bytes} bytes omitted ...")
}

#[cfg(test)]
#[path = "head_tail_buffer_tests.rs"]
mod tests;
