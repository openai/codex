/// A wrapper for the output slice used when decompressing.
///
/// Using this rather than `Cursor` lets us implement the writing methods directly on
/// the buffer and lets us use a usize rather than u64 for the position which helps with
/// performance on 32-bit systems.
pub struct OutputBuffer<'a> {
    slice: &'a mut [u8],
    position: usize,
}

impl<'a> OutputBuffer<'a> {
    #[inline]
    pub fn from_slice_and_pos(slice: &'a mut [u8], position: usize) -> OutputBuffer<'a> {
        OutputBuffer { slice, position }
    }

    #[inline(always)]
    pub const fn position(&self) -> usize {
        self.position
    }

    #[inline(always)]
    pub fn set_position(&mut self, position: usize) {
        self.position = position;
    }

    /// Write a byte to the current position and increment
    ///
    /// Assumes that there is space.
    #[inline]
    pub fn write_byte(&mut self, byte: u8) {
        self.slice[self.position] = byte;
        self.position += 1;
    }

    /// Write a slice to the current position and increment
    ///
    /// Assumes that there is space.
    #[inline]
    pub fn write_slice(&mut self, data: &[u8]) {
        let len = data.len();
        self.slice[self.position..self.position + len].copy_from_slice(data);
        self.position += data.len();
    }

    #[inline]
    pub const fn bytes_left(&self) -> usize {
        self.slice.len() - self.position
    }

    #[inline(always)]
    pub const fn get_ref(&self) -> &[u8] {
        self.slice
    }

    #[inline(always)]
    pub fn get_mut(&mut self) -> &mut [u8] {
        self.slice
    }
}

/// A wrapper for the output slice used when decompressing.
///
/// Using this rather than `Cursor` lets us implement the writing methods directly on
/// the buffer and lets us use a usize rather than u64 for the position which helps with
/// performance on 32-bit systems.
#[derive(Copy, Clone)]
pub struct InputWrapper<'a> {
    slice: &'a [u8],
}

impl<'a> InputWrapper<'a> {
    #[inline(always)]
    pub const fn as_slice(&self) -> &[u8] {
        self.slice
    }

    #[inline(always)]
    pub const fn from_slice(slice: &'a [u8]) -> InputWrapper<'a> {
        InputWrapper { slice }
    }

    #[inline(always)]
    pub fn advance(&mut self, steps: usize) {
        self.slice = &self.slice[steps..];
    }

    #[inline]
    pub fn read_byte(&mut self) -> Option<u8> {
        self.slice.first().map(|n| {
            self.advance(1);
            *n
        })
    }

    #[inline]
    #[cfg(target_pointer_width = "64")]
    pub fn read_u32_le(&mut self) -> u32 {
        let ret = {
            let four_bytes: [u8; 4] = self.slice[..4].try_into().unwrap_or_default();
            u32::from_le_bytes(four_bytes)
        };
        self.advance(4);
        ret
    }

    #[inline(always)]
    pub const fn bytes_left(&self) -> usize {
        self.slice.len()
    }
}
