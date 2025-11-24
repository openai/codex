use crate::{framing_offset_size::FramingOffsetSize, Result};
use std::collections::VecDeque;

// Used internally for GVariant encoding and decoding.
//
// GVariant containers keeps framing offsets at the end and size of these offsets is dependent on
// the size of the container (which includes offsets themselves.

#[derive(Debug)]
pub(crate) struct FramingOffsets(VecDeque<usize>);

impl FramingOffsets {
    pub fn new() -> Self {
        // FIXME: Set some good default capacity
        Self(VecDeque::new())
    }

    pub fn from_encoded_array(container: &[u8]) -> Result<(Self, usize)> {
        let offset_size = FramingOffsetSize::for_encoded_container(container.len());

        // The last offset tells us the start of offsets.
        let offsets_start = offset_size.read_last_offset_from_buffer(container);
        if offsets_start > container.len() {
            return Err(serde::de::Error::invalid_length(
                offsets_start,
                &format!("< {}", container.len()).as_str(),
            ));
        }
        let mut i = offsets_start;
        let offsets_len = container.len() - i;
        let slice_len = offset_size as usize;
        let mut offsets = Self::new();
        while i < container.len() {
            let end = i + slice_len;
            if end > container.len() {
                return Err(serde::de::Error::invalid_length(
                    end,
                    &format!("< {}", container.len()).as_str(),
                ));
            }
            let offset = offset_size.read_last_offset_from_buffer(&container[i..end]);
            if offset > offsets_start {
                return Err(serde::de::Error::invalid_length(
                    offset,
                    &format!("< {offsets_start}").as_str(),
                ));
            }
            let offset = offset_size.read_last_offset_from_buffer(&container[i..end]);
            offsets.push(offset);

            i += slice_len;
        }

        Ok((offsets, offsets_len))
    }

    pub fn push(&mut self, offset: usize) {
        self.0.push_back(offset);
    }

    pub fn push_front(&mut self, offset: usize) {
        self.0.push_front(offset);
    }

    pub fn write_all<W>(self, writer: &mut W, container_len: usize) -> Result<()>
    where
        W: std::io::Write,
    {
        if self.is_empty() {
            return Ok(());
        }
        let offset_size = FramingOffsetSize::for_bare_container(container_len, self.0.len());

        for offset in self.0 {
            offset_size.write_offset(writer, offset)?;
        }

        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.0.len() == 0
    }

    pub fn pop(&mut self) -> Option<usize> {
        self.0.pop_front()
    }

    pub fn peek(&self) -> Option<usize> {
        self.0.front().cloned()
    }
}
