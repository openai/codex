#[cfg(test)]
mod test;

use smallvec::{Array, SmallVec};

use std::convert::{From, Into};
use std::fmt::{self, Debug, Formatter};
use std::iter::FromIterator;

/// A `NibbleVec` backed by a `SmallVec` with 64 inline element slots.
/// This will not allocate until more than 64 elements are added.
pub type Nibblet = NibbleVec<[u8; 64]>;

/// A data-structure for storing a sequence of 4-bit values.
///
/// Values are stored in a `Vec<u8>`, with two values per byte.
///
/// Values at even indices are stored in the most-significant half of their byte,
/// while values at odd indices are stored in the least-significant half.
///
/// Imagine a vector of [MSB][msb-wiki] first bytes, and you'll be right.
///
/// n = [_ _ | _ _ | _ _]
///
/// [msb-wiki]: http://en.wikipedia.org/wiki/Most_significant_bit
#[derive(Clone, Default)]
pub struct NibbleVec<A: Array<Item = u8>> {
    length: usize,
    data: SmallVec<A>,
}

impl<A: Array<Item = u8>> NibbleVec<A> {
    /// Create an empty nibble vector.
    pub fn new() -> NibbleVec<A> {
        NibbleVec {
            length: 0,
            data: SmallVec::new(),
        }
    }

    /// Create a nibble vector from a vector of bytes.
    ///
    /// Each byte is split into two 4-bit entries (MSB, LSB).
    #[inline]
    pub fn from_byte_vec(vec: Vec<u8>) -> NibbleVec<A> {
        let length = 2 * vec.len();
        NibbleVec {
            length,
            data: SmallVec::from_iter(vec),
        }
    }

    /// Returns a byte slice of the nibble vector's contents.
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.data[..]
    }

    /// Converts a nibble vector into a byte vector.
    ///
    /// This consumes the nibble vector, so we do not need to copy its contents.
    #[inline]
    pub fn into_bytes(self) -> Vec<u8> {
        self.data.to_vec()
    }

    /// Get the number of elements stored in the vector.
    #[inline]
    pub fn len(&self) -> usize {
        self.length
    }

    /// Returns `true` if the nibble vector has a length of 0.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Fetch a single entry from the vector.
    ///
    /// Guaranteed to be a value in the interval [0, 15].
    ///
    /// **Panics** if `idx >= self.len()`.
    #[inline]
    pub fn get(&self, idx: usize) -> u8 {
        if idx >= self.length {
            panic!(
                "NibbleVec index out of bounds: len is {}, index is {}",
                self.length, idx
            );
        }
        let vec_idx = idx / 2;
        match idx % 2 {
            // If the index is even, take the first (most significant) half of the stored byte.
            0 => self.data[vec_idx] >> 4,
            // If the index is odd, take the second (least significant) half.
            _ => self.data[vec_idx] & 0x0F,
        }
    }

    /// Add a single nibble to the vector.
    ///
    /// Only the 4 least-significant bits of the value are used.
    #[inline]
    pub fn push(&mut self, val: u8) {
        if self.length % 2 == 0 {
            self.data.push(val << 4);
        } else {
            let vec_len = self.data.len();

            // Zero the second half of the last byte just to be safe.
            self.data[vec_len - 1] &= 0xF0;

            // Write the new value.
            self.data[vec_len - 1] |= val & 0x0F;
        }
        self.length += 1;
    }

    /// Split the vector into two parts.
    ///
    /// All elements at or following the given index are returned in a new `NibbleVec`,
    /// with exactly `idx` elements remaining in this vector.
    ///
    /// **Panics** if `idx > self.len()`.
    pub fn split(&mut self, idx: usize) -> NibbleVec<A> {
        // assert! is a few percent slower surprisingly
        if idx > self.length {
            panic!(
                "attempted to split past vector end. len is {}, index is {}",
                self.length, idx
            );
        } else if idx == self.length {
            NibbleVec::new()
        } else if idx % 2 == 0 {
            self.split_even(idx)
        } else {
            self.split_odd(idx)
        }
    }

    /// Split function for odd *indices*.
    #[inline]
    fn split_odd(&mut self, idx: usize) -> NibbleVec<A> {
        let mut tail = NibbleVec::new();

        // Perform an overlap copy, copying the last nibble of the original vector only if
        // the length of the new tail is *odd*.
        let tail_length = self.length - idx;
        let take_last = tail_length % 2 == 1;
        self.overlap_copy(
            idx / 2,
            self.data.len(),
            &mut tail.data,
            &mut tail.length,
            take_last,
        );

        // Remove the copied bytes, being careful to skip the idx byte.
        for _ in (idx / 2 + 1)..self.data.len() {
            self.data.pop();
        }

        // Zero the second half of the index byte so as to maintain the last-nibble invariant.
        self.data[idx / 2] &= 0xF0;

        // Update the length of the first NibbleVec.
        self.length = idx;

        tail
    }

    /// Split function for even *indices*.
    #[inline]
    fn split_even(&mut self, idx: usize) -> NibbleVec<A> {
        // Avoid allocating a temporary vector by copying all the bytes in order, then popping them.

        // Possible to prove: l_d - ⌊i / 2⌋ = ⌊(l_v - i + 1) / 2⌋
        //  where l_d = self.data.len()
        //        l_v = self.length

        let half_idx = idx / 2;
        let mut tail = NibbleVec::new();

        // Copy the bytes.
        for i in half_idx..self.data.len() {
            tail.data.push(self.data[i]);
        }

        // Pop the same bytes.
        for _ in half_idx..self.data.len() {
            self.data.pop();
        }

        // Update lengths.
        tail.length = self.length - idx;
        self.length = idx;

        tail
    }

    /// Copy data between the second half of self.data[start] and
    /// self.data[end - 1]. The second half of the last entry is included
    /// if include_last is true.
    #[inline]
    fn overlap_copy(
        &self,
        start: usize,
        end: usize,
        vec: &mut SmallVec<A>,
        length: &mut usize,
        include_last: bool,
    ) {
        // Copy up to the first half of the last byte.
        for i in start..(end - 1) {
            // The first half is the second half of the old entry.
            let first_half = self.data[i] & 0x0f;

            // The second half is the first half of the next entry.
            let second_half = self.data[i + 1] >> 4;

            vec.push((first_half << 4) | second_half);
            *length += 2;
        }

        if include_last {
            let last = self.data[end - 1] & 0x0f;
            vec.push(last << 4);
            *length += 1;
        }
    }

    /// Append another nibble vector whilst consuming this vector.
    #[inline]
    pub fn join(mut self, other: &NibbleVec<A>) -> NibbleVec<A> {
        // If the length is even, we can append directly.
        if self.length % 2 == 0 {
            self.length += other.length;
            self.data.extend_from_slice(&other.data);
            return self;
        }

        // If the other vector is empty, bail out.
        if other.is_empty() {
            return self;
        }

        // If the length is odd, we have to perform an overlap copy.
        // Copy the first half of the first element, to make the vector an even length.
        self.push(other.get(0));

        // Copy the rest of the vector using an overlap copy.
        let take_last = other.len() % 2 == 0;
        other.overlap_copy(
            0,
            other.data.len(),
            &mut self.data,
            &mut self.length,
            take_last,
        );

        self
    }
}

impl<A: Array<Item = u8>> PartialEq<NibbleVec<A>> for NibbleVec<A> {
    #[inline]
    fn eq(&self, other: &NibbleVec<A>) -> bool {
        self.length == other.length && self.data == other.data
    }
}

impl<A: Array<Item = u8>> Eq for NibbleVec<A> {}

/// Compare a `NibbleVec` and a slice of bytes *element-by-element*.
/// Bytes are **not** interpreted as two `NibbleVec` entries.
impl<A: Array<Item = u8>> PartialEq<[u8]> for NibbleVec<A> {
    #[inline]
    fn eq(&self, other: &[u8]) -> bool {
        if other.len() != self.len() {
            return false;
        }

        for (i, x) in other.iter().enumerate() {
            if self.get(i) != *x {
                return false;
            }
        }
        true
    }
}

impl<A: Array<Item = u8>> Debug for NibbleVec<A> {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        write!(fmt, "NibbleVec [")?;

        if !self.is_empty() {
            write!(fmt, "{}", self.get(0))?;
        }

        for i in 1..self.len() {
            write!(fmt, ", {}", self.get(i))?;
        }
        write!(fmt, "]")
    }
}

impl<A: Array<Item = u8>> From<Vec<u8>> for NibbleVec<A> {
    #[inline]
    fn from(v: Vec<u8>) -> NibbleVec<A> {
        NibbleVec::from_byte_vec(v)
    }
}

impl<'a, A: Array<Item = u8>> From<&'a [u8]> for NibbleVec<A> {
    #[inline]
    fn from(v: &[u8]) -> NibbleVec<A> {
        NibbleVec::from_byte_vec(v.into())
    }
}

impl<A: Array<Item = u8>> Into<Vec<u8>> for NibbleVec<A> {
    #[inline]
    fn into(self) -> Vec<u8> {
        self.data.to_vec()
    }
}

impl<'a, A: Array<Item = u8>> Into<Vec<u8>> for &'a NibbleVec<A> {
    #[inline]
    fn into(self) -> Vec<u8> {
        self.data.to_vec()
    }
}
