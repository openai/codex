/*!
An implementation of the [Shift-Or substring search algorithm][shiftor].

[shiftor]: https://en.wikipedia.org/wiki/Bitap_algorithm
*/

use alloc::boxed::Box;

/// The type of our mask.
///
/// While we don't expose anyway to configure this in the public API, if one
/// really needs less memory usage or support for longer needles, then it is
/// suggested to copy the code from this module and modify it to fit your
/// needs. The code below is written to be correct regardless of whether Mask
/// is a u8, u16, u32, u64 or u128.
type Mask = u16;

/// A forward substring searcher using the Shift-Or algorithm.
#[derive(Debug)]
pub struct Finder {
    masks: Box<[Mask; 256]>,
    needle_len: usize,
}

impl Finder {
    const MAX_NEEDLE_LEN: usize = (Mask::BITS - 1) as usize;

    /// Create a new Shift-Or forward searcher for the given `needle`.
    ///
    /// The needle may be empty. The empty needle matches at every byte offset.
    #[inline]
    pub fn new(needle: &[u8]) -> Option<Finder> {
        let needle_len = needle.len();
        if needle_len > Finder::MAX_NEEDLE_LEN {
            // A match is found when bit 7 is set in 'result' in the search
            // routine below. So our needle can't be bigger than 7. We could
            // permit bigger needles by using u16, u32 or u64 for our mask
            // entries. But this is all we need for this example.
            return None;
        }
        let mut searcher = Finder { masks: Box::from([!0; 256]), needle_len };
        for (i, &byte) in needle.iter().enumerate() {
            searcher.masks[usize::from(byte)] &= !(1 << i);
        }
        Some(searcher)
    }

    /// Return the first occurrence of the needle given to `Finder::new` in
    /// the `haystack` given. If no such occurrence exists, then `None` is
    /// returned.
    ///
    /// Unlike most other substring search implementations in this crate, this
    /// finder does not require passing the needle at search time. A match can
    /// be determined without the needle at all since the required information
    /// is already encoded into this finder at construction time.
    ///
    /// The maximum value this can return is `haystack.len()`, which can only
    /// occur when the needle and haystack both have length zero. Otherwise,
    /// for non-empty haystacks, the maximum value is `haystack.len() - 1`.
    #[inline]
    pub fn find(&self, haystack: &[u8]) -> Option<usize> {
        if self.needle_len == 0 {
            return Some(0);
        }
        let mut result = !1;
        for (i, &byte) in haystack.iter().enumerate() {
            result |= self.masks[usize::from(byte)];
            result <<= 1;
            if result & (1 << self.needle_len) == 0 {
                return Some(i + 1 - self.needle_len);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    define_substring_forward_quickcheck!(|h, n| Some(Finder::new(n)?.find(h)));

    #[test]
    fn forward() {
        crate::tests::substring::Runner::new()
            .fwd(|h, n| Some(Finder::new(n)?.find(h)))
            .run();
    }
}
