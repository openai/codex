use core::num::NonZeroUsize;

use crate::error::Needed;
use crate::lib::std::iter::{Cloned, Enumerate};
use crate::lib::std::slice::Iter;
use crate::lib::std::{cmp::Ordering, fmt, ops};
use crate::stream::AsBytes;
use crate::stream::Checkpoint;
use crate::stream::Compare;
use crate::stream::CompareResult;
use crate::stream::FindSlice;
use crate::stream::Offset;
#[cfg(feature = "unstable-recover")]
#[cfg(feature = "std")]
use crate::stream::Recover;
use crate::stream::SliceLen;
use crate::stream::Stream;
use crate::stream::StreamIsPartial;
use crate::stream::UpdateSlice;

/// Improved `Debug` experience for `&[u8]` byte streams
#[allow(clippy::derived_hash_with_manual_eq)]
#[derive(Hash)]
#[repr(transparent)]
pub struct Bytes([u8]);

impl Bytes {
    /// Make a stream out of a byte slice-like.
    #[inline]
    pub fn new<B: ?Sized + AsRef<[u8]>>(bytes: &B) -> &Self {
        Self::from_bytes(bytes.as_ref())
    }

    #[inline]
    fn from_bytes(slice: &[u8]) -> &Self {
        unsafe { crate::lib::std::mem::transmute(slice) }
    }

    #[inline]
    fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl SliceLen for &Bytes {
    #[inline(always)]
    fn slice_len(&self) -> usize {
        self.len()
    }
}

impl<'i> Stream for &'i Bytes {
    type Token = u8;
    type Slice = &'i [u8];

    type IterOffsets = Enumerate<Cloned<Iter<'i, u8>>>;

    type Checkpoint = Checkpoint<Self, Self>;

    #[inline(always)]
    fn iter_offsets(&self) -> Self::IterOffsets {
        self.iter().cloned().enumerate()
    }
    #[inline(always)]
    fn eof_offset(&self) -> usize {
        self.len()
    }

    #[inline(always)]
    fn next_token(&mut self) -> Option<Self::Token> {
        if self.is_empty() {
            None
        } else {
            let token = self[0];
            *self = &self[1..];
            Some(token)
        }
    }

    #[inline(always)]
    fn peek_token(&self) -> Option<Self::Token> {
        if self.is_empty() {
            None
        } else {
            Some(self[0])
        }
    }

    #[inline(always)]
    fn offset_for<P>(&self, predicate: P) -> Option<usize>
    where
        P: Fn(Self::Token) -> bool,
    {
        self.iter().position(|b| predicate(*b))
    }
    #[inline(always)]
    fn offset_at(&self, tokens: usize) -> Result<usize, Needed> {
        if let Some(needed) = tokens.checked_sub(self.len()).and_then(NonZeroUsize::new) {
            Err(Needed::Size(needed))
        } else {
            Ok(tokens)
        }
    }
    #[inline(always)]
    fn next_slice(&mut self, offset: usize) -> Self::Slice {
        let (slice, next) = self.0.split_at(offset);
        *self = Bytes::from_bytes(next);
        slice
    }
    #[inline(always)]
    unsafe fn next_slice_unchecked(&mut self, offset: usize) -> Self::Slice {
        #[cfg(debug_assertions)]
        self.peek_slice(offset);

        // SAFETY: `Stream::next_slice_unchecked` requires `offset` to be in bounds
        let slice = unsafe { self.0.get_unchecked(..offset) };
        // SAFETY: `Stream::next_slice_unchecked` requires `offset` to be in bounds
        let next = unsafe { self.0.get_unchecked(offset..) };
        *self = Bytes::from_bytes(next);
        slice
    }
    #[inline(always)]
    fn peek_slice(&self, offset: usize) -> Self::Slice {
        &self[..offset]
    }
    #[inline(always)]
    unsafe fn peek_slice_unchecked(&self, offset: usize) -> Self::Slice {
        #[cfg(debug_assertions)]
        self.peek_slice(offset);

        // SAFETY: `Stream::next_slice_unchecked` requires `offset` to be in bounds
        let slice = unsafe { self.0.get_unchecked(..offset) };
        slice
    }

    #[inline(always)]
    fn checkpoint(&self) -> Self::Checkpoint {
        Checkpoint::<_, Self>::new(*self)
    }
    #[inline(always)]
    fn reset(&mut self, checkpoint: &Self::Checkpoint) {
        *self = checkpoint.inner;
    }

    #[inline(always)]
    fn raw(&self) -> &dyn crate::lib::std::fmt::Debug {
        self
    }
}

#[cfg(feature = "unstable-recover")]
#[cfg(feature = "std")]
impl<E> Recover<E> for &Bytes {
    #[inline(always)]
    fn record_err(
        &mut self,
        _token_start: &Self::Checkpoint,
        _err_start: &Self::Checkpoint,
        err: E,
    ) -> Result<(), E> {
        Err(err)
    }

    /// Report whether the [`Stream`] can save off errors for recovery
    #[inline(always)]
    fn is_recovery_supported() -> bool {
        false
    }
}

impl StreamIsPartial for &Bytes {
    type PartialState = ();

    #[inline]
    fn complete(&mut self) -> Self::PartialState {
        // Already complete
    }

    #[inline]
    fn restore_partial(&mut self, _state: Self::PartialState) {}

    #[inline(always)]
    fn is_partial_supported() -> bool {
        false
    }
}

impl Offset for &Bytes {
    #[inline(always)]
    fn offset_from(&self, start: &Self) -> usize {
        self.as_bytes().offset_from(&start.as_bytes())
    }
}

impl<'a> Offset<<&'a Bytes as Stream>::Checkpoint> for &'a Bytes {
    #[inline(always)]
    fn offset_from(&self, other: &<&'a Bytes as Stream>::Checkpoint) -> usize {
        self.checkpoint().offset_from(other)
    }
}

impl AsBytes for &Bytes {
    #[inline(always)]
    fn as_bytes(&self) -> &[u8] {
        (*self).as_bytes()
    }
}

impl<'a, T> Compare<T> for &'a Bytes
where
    &'a [u8]: Compare<T>,
{
    #[inline(always)]
    fn compare(&self, t: T) -> CompareResult {
        let bytes = (*self).as_bytes();
        bytes.compare(t)
    }
}

impl<'i, S> FindSlice<S> for &'i Bytes
where
    &'i [u8]: FindSlice<S>,
{
    #[inline(always)]
    fn find_slice(&self, substr: S) -> Option<crate::lib::std::ops::Range<usize>> {
        let bytes = (*self).as_bytes();
        let offset = bytes.find_slice(substr);
        offset
    }
}

impl UpdateSlice for &Bytes {
    #[inline(always)]
    fn update_slice(self, inner: Self::Slice) -> Self {
        Bytes::new(inner)
    }
}

impl fmt::Display for Bytes {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        <Self as fmt::UpperHex>::fmt(self, f)
    }
}

impl fmt::Debug for Bytes {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        <Self as fmt::UpperHex>::fmt(self, f)
    }
}

impl fmt::LowerHex for Bytes {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.as_bytes() {
            write!(f, "{byte:0>2x}")?;
        }
        Ok(())
    }
}

impl fmt::UpperHex for Bytes {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, byte) in self.as_bytes().iter().enumerate() {
            if 0 < i {
                let absolute = (self.as_bytes().as_ptr() as usize) + i;
                if f.alternate() && absolute != 0 && absolute % 4 == 0 {
                    write!(f, "_")?;
                }
            }
            write!(f, "{byte:0>2X}")?;
        }
        Ok(())
    }
}

impl ops::Deref for Bytes {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl ops::Index<usize> for Bytes {
    type Output = u8;

    #[inline]
    fn index(&self, idx: usize) -> &u8 {
        &self.as_bytes()[idx]
    }
}

impl ops::Index<ops::RangeFull> for Bytes {
    type Output = Bytes;

    #[inline]
    fn index(&self, _: ops::RangeFull) -> &Bytes {
        self
    }
}

impl ops::Index<ops::Range<usize>> for Bytes {
    type Output = Bytes;

    #[inline]
    fn index(&self, r: ops::Range<usize>) -> &Bytes {
        Bytes::new(&self.as_bytes()[r.start..r.end])
    }
}

impl ops::Index<ops::RangeInclusive<usize>> for Bytes {
    type Output = Bytes;

    #[inline]
    fn index(&self, r: ops::RangeInclusive<usize>) -> &Bytes {
        Bytes::new(&self.as_bytes()[*r.start()..=*r.end()])
    }
}

impl ops::Index<ops::RangeFrom<usize>> for Bytes {
    type Output = Bytes;

    #[inline]
    fn index(&self, r: ops::RangeFrom<usize>) -> &Bytes {
        Bytes::new(&self.as_bytes()[r.start..])
    }
}

impl ops::Index<ops::RangeTo<usize>> for Bytes {
    type Output = Bytes;

    #[inline]
    fn index(&self, r: ops::RangeTo<usize>) -> &Bytes {
        Bytes::new(&self.as_bytes()[..r.end])
    }
}

impl ops::Index<ops::RangeToInclusive<usize>> for Bytes {
    type Output = Bytes;

    #[inline]
    fn index(&self, r: ops::RangeToInclusive<usize>) -> &Bytes {
        Bytes::new(&self.as_bytes()[..=r.end])
    }
}

impl AsRef<[u8]> for Bytes {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl AsRef<Bytes> for [u8] {
    #[inline]
    fn as_ref(&self) -> &Bytes {
        Bytes::new(self)
    }
}

impl AsRef<Bytes> for str {
    #[inline]
    fn as_ref(&self) -> &Bytes {
        Bytes::new(self)
    }
}

#[cfg(feature = "alloc")]
impl crate::lib::std::borrow::ToOwned for Bytes {
    type Owned = crate::lib::std::vec::Vec<u8>;

    #[inline]
    fn to_owned(&self) -> Self::Owned {
        crate::lib::std::vec::Vec::from(self.as_bytes())
    }
}

#[cfg(feature = "alloc")]
impl crate::lib::std::borrow::Borrow<Bytes> for crate::lib::std::vec::Vec<u8> {
    #[inline]
    fn borrow(&self) -> &Bytes {
        Bytes::from_bytes(self.as_slice())
    }
}

impl<'a> Default for &'a Bytes {
    fn default() -> &'a Bytes {
        Bytes::new(b"")
    }
}

impl<'a> From<&'a [u8]> for &'a Bytes {
    #[inline]
    fn from(s: &'a [u8]) -> &'a Bytes {
        Bytes::new(s)
    }
}

impl<'a> From<&'a Bytes> for &'a [u8] {
    #[inline]
    fn from(s: &'a Bytes) -> &'a [u8] {
        Bytes::as_bytes(s)
    }
}

impl<'a> From<&'a str> for &'a Bytes {
    #[inline]
    fn from(s: &'a str) -> &'a Bytes {
        Bytes::new(s.as_bytes())
    }
}

impl Eq for Bytes {}

impl PartialEq<Bytes> for Bytes {
    #[inline]
    fn eq(&self, other: &Bytes) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl_partial_eq!(Bytes, [u8]);
impl_partial_eq!(Bytes, &'a [u8]);
impl_partial_eq!(Bytes, str);
impl_partial_eq!(Bytes, &'a str);

impl PartialOrd for Bytes {
    #[inline]
    fn partial_cmp(&self, other: &Bytes) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Bytes {
    #[inline]
    fn cmp(&self, other: &Bytes) -> Ordering {
        Ord::cmp(self.as_bytes(), other.as_bytes())
    }
}

impl_partial_ord!(Bytes, [u8]);
impl_partial_ord!(Bytes, &'a [u8]);
impl_partial_ord!(Bytes, str);
impl_partial_ord!(Bytes, &'a str);

#[cfg(test)]
mod test {
    use crate::stream::Bytes;

    #[test]
    fn partial_eq_bytes_byte_slice() {
        let input = b"foo".as_slice();
        let actual = Bytes::new(input);
        assert!(actual == input);
    }

    #[test]
    fn partial_eq_byte_slice_bytes() {
        let input = b"foo".as_slice();
        let actual = Bytes::new(input);
        assert!(input == actual);
    }

    #[test]
    fn partial_eq_bytes_str() {
        let input = "foo";
        let actual = Bytes::new(input);
        assert!(actual == input);
    }

    #[test]
    fn partial_eq_str_bytes() {
        let input = "foo";
        let actual = Bytes::new(input);
        assert!(input == actual);
    }

    #[test]
    fn partial_ord_bytes_byte_slice() {
        let input = b"foo".as_slice();
        let actual = Bytes::new(input);
        assert!(actual.partial_cmp(input) == Some(core::cmp::Ordering::Equal));
    }

    #[test]
    fn partial_ord_byte_slice_bytes() {
        let input = b"foo".as_slice();
        let actual = Bytes::new(input);
        assert!(input.partial_cmp(actual) == Some(core::cmp::Ordering::Equal));
    }

    #[test]
    fn partial_ord_bytes_str() {
        let input = "foo";
        let actual = Bytes::new(input);
        assert!(actual.partial_cmp(input) == Some(core::cmp::Ordering::Equal));
    }

    #[test]
    fn partial_ord_str_bytes() {
        let input = "foo";
        let actual = Bytes::new(input);
        assert!(input.partial_cmp(actual) == Some(core::cmp::Ordering::Equal));
    }
}

#[cfg(all(test, feature = "std"))]
mod display {
    use crate::stream::Bytes;

    #[test]
    fn clean() {
        assert_eq!(&format!("{}", Bytes::new(b"abc")), "616263");
        assert_eq!(&format!("{}", Bytes::new(b"\xf0\x28\x8c\xbc")), "F0288CBC");
    }
}

#[cfg(all(test, feature = "std"))]
mod debug {
    use crate::stream::Bytes;
    use crate::stream::Stream as _;
    use snapbox::assert_data_eq;
    use snapbox::str;

    #[test]
    fn test_debug() {
        let input = Bytes::new(b"\0\0\0 ftypisom\0\0\x02\0isomiso2avc1mp");
        let expected = str!["000000206674797069736F6D0000020069736F6D69736F32617663316D70"];
        assert_data_eq!(&format!("{input:?}"), expected);
    }

    #[test]
    fn test_pretty_debug() {
        let input = Bytes::new(b"\0\0\0 ftypisom\0\0\x02\0isomiso2avc1mp");
        let expected = str!["000000206674797069736F6D0000020069736F6D69736F32617663316D70"];
        assert_data_eq!(&format!("{input:#?}").replace('_', ""), expected);
    }

    #[test]
    fn test_trace() {
        let input = Bytes::new(b"\0\0\0 ftypisom\0\0\x02\0isomiso2avc1mp");
        let expected = str!["000000206674797069736F6D0000020069736F6D69736F32617663316D70"];
        assert_data_eq!(
            crate::util::from_fn(|f| input.trace(f))
                .to_string()
                .replace('_', ""),
            expected
        );
    }

    #[test]
    fn test_sliced() {
        // Output can change from run-to-run
        let total = Bytes::new(b"12345678901234567890");
        let _ = format!("{total:#?}");
        let _ = format!("{:#?}", &total[1..]);
        let _ = format!("{:#?}", &total[10..]);
    }
}
