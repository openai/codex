/// A wrapper around a slice that exposes no functions that can panic.
///
/// Intentionally avoids implementing `Debug`, `Eq`, and `PartialEq` to avoid
/// creating a side channel that would leak information about the value.
#[derive(Clone, Copy)]
pub struct Slice<'a> {
    bytes: &'a [u8],
}

impl<'a> Slice<'a> {
    #[inline]
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    #[inline]
    pub fn get(&self, i: usize) -> Option<&u8> {
        self.bytes.get(i)
    }

    #[inline]
    pub fn subslice(&self, r: core::ops::Range<usize>) -> Option<Self> {
        self.bytes.get(r).map(|bytes| Self { bytes })
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    #[inline]
    pub fn as_slice_less_safe(&self) -> &'a [u8] {
        self.bytes
    }
}
