use crate::{Error, Result, WriteBytes, LE};

// Used internally for GVariant encoding and decoding.
//
// GVariant containers keeps framing offsets at the end and size of these offsets is dependent on
// the size of the container (which includes offsets themselves).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(usize)]
pub(crate) enum FramingOffsetSize {
    U8 = 1,
    U16 = 2,
    U32 = 4,
    #[cfg(not(target_pointer_width = "32"))]
    U64 = 8,
}

impl FramingOffsetSize {
    pub(crate) fn for_bare_container(container_len: usize, num_offsets: usize) -> Self {
        let mut offset_size = FramingOffsetSize::U8;

        loop {
            if container_len + num_offsets * (offset_size as usize) <= offset_size.max() {
                return offset_size;
            }

            offset_size = offset_size
                .bump_up()
                .expect("Can't handle container too large for a 64-bit pointer");
        }
    }

    pub(crate) fn for_encoded_container(container_len: usize) -> Self {
        Self::for_bare_container(container_len, 0)
    }

    pub(crate) fn write_offset<W>(self, writer: &mut W, offset: usize) -> Result<()>
    where
        W: std::io::Write,
    {
        match self {
            FramingOffsetSize::U8 => writer.write_u8(LE, offset as u8),
            FramingOffsetSize::U16 => writer.write_u16(LE, offset as u16),
            FramingOffsetSize::U32 => writer.write_u32(LE, offset as u32),
            #[cfg(not(target_pointer_width = "32"))]
            FramingOffsetSize::U64 => writer.write_u64(LE, offset as u64),
        }
        .map_err(|e| Error::InputOutput(e.into()))
    }

    pub fn read_last_offset_from_buffer(self, buffer: &[u8]) -> usize {
        if buffer.is_empty() {
            return 0;
        }

        let end = buffer.len();
        match self {
            FramingOffsetSize::U8 => buffer[end - 1] as usize,
            FramingOffsetSize::U16 => LE.read_u16(&buffer[end - 2..end]) as usize,
            FramingOffsetSize::U32 => LE.read_u32(&buffer[end - 4..end]) as usize,
            #[cfg(not(target_pointer_width = "32"))]
            FramingOffsetSize::U64 => LE.read_u64(&buffer[end - 8..end]) as usize,
        }
    }

    fn max(self) -> usize {
        match self {
            FramingOffsetSize::U8 => std::u8::MAX as usize,
            FramingOffsetSize::U16 => std::u16::MAX as usize,
            FramingOffsetSize::U32 => std::u32::MAX as usize,
            #[cfg(not(target_pointer_width = "32"))]
            FramingOffsetSize::U64 => std::u64::MAX as usize,
        }
    }

    fn bump_up(self) -> Option<Self> {
        match self {
            FramingOffsetSize::U8 => Some(FramingOffsetSize::U16),
            FramingOffsetSize::U16 => Some(FramingOffsetSize::U32),
            #[cfg(not(target_pointer_width = "32"))]
            FramingOffsetSize::U32 => Some(FramingOffsetSize::U64),
            #[cfg(not(target_pointer_width = "32"))]
            FramingOffsetSize::U64 => None,
            #[cfg(target_pointer_width = "32")]
            FramingOffsetSize::U32 => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::framing_offset_size::FramingOffsetSize;

    #[test]
    fn framing_offset_size_bump() {
        assert_eq!(
            FramingOffsetSize::for_bare_container(std::u8::MAX as usize - 3, 3),
            FramingOffsetSize::U8
        );
        assert_eq!(
            FramingOffsetSize::for_bare_container(std::u8::MAX as usize - 1, 2),
            FramingOffsetSize::U16
        );
        assert_eq!(
            FramingOffsetSize::for_bare_container(std::u16::MAX as usize - 4, 2),
            FramingOffsetSize::U16
        );
        assert_eq!(
            FramingOffsetSize::for_bare_container(std::u16::MAX as usize - 3, 2),
            FramingOffsetSize::U32
        );
        assert_eq!(
            FramingOffsetSize::for_bare_container(std::u32::MAX as usize - 12, 3),
            FramingOffsetSize::U32
        );
        #[cfg(not(target_pointer_width = "32"))]
        assert_eq!(
            FramingOffsetSize::for_bare_container(std::u32::MAX as usize - 11, 3),
            FramingOffsetSize::U64
        );
    }
}
