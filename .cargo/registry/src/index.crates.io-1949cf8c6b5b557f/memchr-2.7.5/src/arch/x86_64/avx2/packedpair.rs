/*!
A 256-bit vector implementation of the "packed pair" SIMD algorithm.

The "packed pair" algorithm is based on the [generic SIMD] algorithm. The main
difference is that it (by default) uses a background distribution of byte
frequencies to heuristically select the pair of bytes to search for.

[generic SIMD]: http://0x80.pl/articles/simd-strfind.html#first-and-last
*/

use core::arch::x86_64::{__m128i, __m256i};

use crate::arch::{all::packedpair::Pair, generic::packedpair};

/// A "packed pair" finder that uses 256-bit vector operations.
///
/// This finder picks two bytes that it believes have high predictive power
/// for indicating an overall match of a needle. Depending on whether
/// `Finder::find` or `Finder::find_prefilter` is used, it reports offsets
/// where the needle matches or could match. In the prefilter case, candidates
/// are reported whenever the [`Pair`] of bytes given matches.
#[derive(Clone, Copy, Debug)]
pub struct Finder {
    sse2: packedpair::Finder<__m128i>,
    avx2: packedpair::Finder<__m256i>,
}

impl Finder {
    /// Create a new pair searcher. The searcher returned can either report
    /// exact matches of `needle` or act as a prefilter and report candidate
    /// positions of `needle`.
    ///
    /// If AVX2 is unavailable in the current environment or if a [`Pair`]
    /// could not be constructed from the needle given, then `None` is
    /// returned.
    #[inline]
    pub fn new(needle: &[u8]) -> Option<Finder> {
        Finder::with_pair(needle, Pair::new(needle)?)
    }

    /// Create a new "packed pair" finder using the pair of bytes given.
    ///
    /// This constructor permits callers to control precisely which pair of
    /// bytes is used as a predicate.
    ///
    /// If AVX2 is unavailable in the current environment, then `None` is
    /// returned.
    #[inline]
    pub fn with_pair(needle: &[u8], pair: Pair) -> Option<Finder> {
        if Finder::is_available() {
            // SAFETY: we check that sse2/avx2 is available above. We are also
            // guaranteed to have needle.len() > 1 because we have a valid
            // Pair.
            unsafe { Some(Finder::with_pair_impl(needle, pair)) }
        } else {
            None
        }
    }

    /// Create a new `Finder` specific to SSE2 vectors and routines.
    ///
    /// # Safety
    ///
    /// Same as the safety for `packedpair::Finder::new`, and callers must also
    /// ensure that both SSE2 and AVX2 are available.
    #[target_feature(enable = "sse2", enable = "avx2")]
    #[inline]
    unsafe fn with_pair_impl(needle: &[u8], pair: Pair) -> Finder {
        let sse2 = packedpair::Finder::<__m128i>::new(needle, pair);
        let avx2 = packedpair::Finder::<__m256i>::new(needle, pair);
        Finder { sse2, avx2 }
    }

    /// Returns true when this implementation is available in the current
    /// environment.
    ///
    /// When this is true, it is guaranteed that [`Finder::with_pair`] will
    /// return a `Some` value. Similarly, when it is false, it is guaranteed
    /// that `Finder::with_pair` will return a `None` value. Notice that this
    /// does not guarantee that [`Finder::new`] will return a `Finder`. Namely,
    /// even when `Finder::is_available` is true, it is not guaranteed that a
    /// valid [`Pair`] can be found from the needle given.
    ///
    /// Note also that for the lifetime of a single program, if this returns
    /// true then it will always return true.
    #[inline]
    pub fn is_available() -> bool {
        #[cfg(not(target_feature = "sse2"))]
        {
            false
        }
        #[cfg(target_feature = "sse2")]
        {
            #[cfg(target_feature = "avx2")]
            {
                true
            }
            #[cfg(not(target_feature = "avx2"))]
            {
                #[cfg(feature = "std")]
                {
                    std::is_x86_feature_detected!("avx2")
                }
                #[cfg(not(feature = "std"))]
                {
                    false
                }
            }
        }
    }

    /// Execute a search using AVX2 vectors and routines.
    ///
    /// # Panics
    ///
    /// When `haystack.len()` is less than [`Finder::min_haystack_len`].
    #[inline]
    pub fn find(&self, haystack: &[u8], needle: &[u8]) -> Option<usize> {
        // SAFETY: Building a `Finder` means it's safe to call 'sse2' routines.
        unsafe { self.find_impl(haystack, needle) }
    }

    /// Run this finder on the given haystack as a prefilter.
    ///
    /// If a candidate match is found, then an offset where the needle *could*
    /// begin in the haystack is returned.
    ///
    /// # Panics
    ///
    /// When `haystack.len()` is less than [`Finder::min_haystack_len`].
    #[inline]
    pub fn find_prefilter(&self, haystack: &[u8]) -> Option<usize> {
        // SAFETY: Building a `Finder` means it's safe to call 'sse2' routines.
        unsafe { self.find_prefilter_impl(haystack) }
    }

    /// Execute a search using AVX2 vectors and routines.
    ///
    /// # Panics
    ///
    /// When `haystack.len()` is less than [`Finder::min_haystack_len`].
    ///
    /// # Safety
    ///
    /// (The target feature safety obligation is automatically fulfilled by
    /// virtue of being a method on `Finder`, which can only be constructed
    /// when it is safe to call `sse2` and `avx2` routines.)
    #[target_feature(enable = "sse2", enable = "avx2")]
    #[inline]
    unsafe fn find_impl(
        &self,
        haystack: &[u8],
        needle: &[u8],
    ) -> Option<usize> {
        if haystack.len() < self.avx2.min_haystack_len() {
            self.sse2.find(haystack, needle)
        } else {
            self.avx2.find(haystack, needle)
        }
    }

    /// Execute a prefilter search using AVX2 vectors and routines.
    ///
    /// # Panics
    ///
    /// When `haystack.len()` is less than [`Finder::min_haystack_len`].
    ///
    /// # Safety
    ///
    /// (The target feature safety obligation is automatically fulfilled by
    /// virtue of being a method on `Finder`, which can only be constructed
    /// when it is safe to call `sse2` and `avx2` routines.)
    #[target_feature(enable = "sse2", enable = "avx2")]
    #[inline]
    unsafe fn find_prefilter_impl(&self, haystack: &[u8]) -> Option<usize> {
        if haystack.len() < self.avx2.min_haystack_len() {
            self.sse2.find_prefilter(haystack)
        } else {
            self.avx2.find_prefilter(haystack)
        }
    }

    /// Returns the pair of offsets (into the needle) used to check as a
    /// predicate before confirming whether a needle exists at a particular
    /// position.
    #[inline]
    pub fn pair(&self) -> &Pair {
        self.avx2.pair()
    }

    /// Returns the minimum haystack length that this `Finder` can search.
    ///
    /// Using a haystack with length smaller than this in a search will result
    /// in a panic. The reason for this restriction is that this finder is
    /// meant to be a low-level component that is part of a larger substring
    /// strategy. In that sense, it avoids trying to handle all cases and
    /// instead only handles the cases that it can handle very well.
    #[inline]
    pub fn min_haystack_len(&self) -> usize {
        // The caller doesn't need to care about AVX2's min_haystack_len
        // since this implementation will automatically switch to the SSE2
        // implementation if the haystack is too short for AVX2. Therefore, the
        // caller only needs to care about SSE2's min_haystack_len.
        //
        // This does assume that SSE2's min_haystack_len is less than or
        // equal to AVX2's min_haystack_len. In practice, this is true and
        // there is no way it could be false based on how this Finder is
        // implemented. Namely, both SSE2 and AVX2 use the same `Pair`. If
        // they used different pairs, then it's possible (although perhaps
        // pathological) for SSE2's min_haystack_len to be bigger than AVX2's.
        self.sse2.min_haystack_len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find(haystack: &[u8], needle: &[u8]) -> Option<Option<usize>> {
        let f = Finder::new(needle)?;
        if haystack.len() < f.min_haystack_len() {
            return None;
        }
        Some(f.find(haystack, needle))
    }

    define_substring_forward_quickcheck!(find);

    #[test]
    fn forward_substring() {
        crate::tests::substring::Runner::new().fwd(find).run()
    }

    #[test]
    fn forward_packedpair() {
        fn find(
            haystack: &[u8],
            needle: &[u8],
            index1: u8,
            index2: u8,
        ) -> Option<Option<usize>> {
            let pair = Pair::with_indices(needle, index1, index2)?;
            let f = Finder::with_pair(needle, pair)?;
            if haystack.len() < f.min_haystack_len() {
                return None;
            }
            Some(f.find(haystack, needle))
        }
        crate::tests::packedpair::Runner::new().fwd(find).run()
    }

    #[test]
    fn forward_packedpair_prefilter() {
        fn find(
            haystack: &[u8],
            needle: &[u8],
            index1: u8,
            index2: u8,
        ) -> Option<Option<usize>> {
            if !cfg!(target_feature = "sse2") {
                return None;
            }
            let pair = Pair::with_indices(needle, index1, index2)?;
            let f = Finder::with_pair(needle, pair)?;
            if haystack.len() < f.min_haystack_len() {
                return None;
            }
            Some(f.find_prefilter(haystack))
        }
        crate::tests::packedpair::Runner::new().fwd(find).run()
    }
}
