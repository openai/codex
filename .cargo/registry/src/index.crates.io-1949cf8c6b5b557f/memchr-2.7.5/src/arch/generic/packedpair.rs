/*!
Generic crate-internal routines for the "packed pair" SIMD algorithm.

The "packed pair" algorithm is based on the [generic SIMD] algorithm. The main
difference is that it (by default) uses a background distribution of byte
frequencies to heuristically select the pair of bytes to search for.

[generic SIMD]: http://0x80.pl/articles/simd-strfind.html#first-and-last
*/

use crate::{
    arch::all::{is_equal_raw, packedpair::Pair},
    ext::Pointer,
    vector::{MoveMask, Vector},
};

/// A generic architecture dependent "packed pair" finder.
///
/// This finder picks two bytes that it believes have high predictive power
/// for indicating an overall match of a needle. Depending on whether
/// `Finder::find` or `Finder::find_prefilter` is used, it reports offsets
/// where the needle matches or could match. In the prefilter case, candidates
/// are reported whenever the [`Pair`] of bytes given matches.
///
/// This is architecture dependent because it uses specific vector operations
/// to look for occurrences of the pair of bytes.
///
/// This type is not meant to be exported and is instead meant to be used as
/// the implementation for architecture specific facades. Why? Because it's a
/// bit of a quirky API that requires `inline(always)` annotations. And pretty
/// much everything has safety obligations due (at least) to the caller needing
/// to inline calls into routines marked with
/// `#[target_feature(enable = "...")]`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Finder<V> {
    pair: Pair,
    v1: V,
    v2: V,
    min_haystack_len: usize,
}

impl<V: Vector> Finder<V> {
    /// Create a new pair searcher. The searcher returned can either report
    /// exact matches of `needle` or act as a prefilter and report candidate
    /// positions of `needle`.
    ///
    /// # Safety
    ///
    /// Callers must ensure that whatever vector type this routine is called
    /// with is supported by the current environment.
    ///
    /// Callers must also ensure that `needle.len() >= 2`.
    #[inline(always)]
    pub(crate) unsafe fn new(needle: &[u8], pair: Pair) -> Finder<V> {
        let max_index = pair.index1().max(pair.index2());
        let min_haystack_len =
            core::cmp::max(needle.len(), usize::from(max_index) + V::BYTES);
        let v1 = V::splat(needle[usize::from(pair.index1())]);
        let v2 = V::splat(needle[usize::from(pair.index2())]);
        Finder { pair, v1, v2, min_haystack_len }
    }

    /// Searches the given haystack for the given needle. The needle given
    /// should be the same as the needle that this finder was initialized
    /// with.
    ///
    /// # Panics
    ///
    /// When `haystack.len()` is less than [`Finder::min_haystack_len`].
    ///
    /// # Safety
    ///
    /// Since this is meant to be used with vector functions, callers need to
    /// specialize this inside of a function with a `target_feature` attribute.
    /// Therefore, callers must ensure that whatever target feature is being
    /// used supports the vector functions that this function is specialized
    /// for. (For the specific vector functions used, see the Vector trait
    /// implementations.)
    #[inline(always)]
    pub(crate) unsafe fn find(
        &self,
        haystack: &[u8],
        needle: &[u8],
    ) -> Option<usize> {
        assert!(
            haystack.len() >= self.min_haystack_len,
            "haystack too small, should be at least {} but got {}",
            self.min_haystack_len,
            haystack.len(),
        );

        let all = V::Mask::all_zeros_except_least_significant(0);
        let start = haystack.as_ptr();
        let end = start.add(haystack.len());
        let max = end.sub(self.min_haystack_len);
        let mut cur = start;

        // N.B. I did experiment with unrolling the loop to deal with size(V)
        // bytes at a time and 2*size(V) bytes at a time. The double unroll
        // was marginally faster while the quadruple unroll was unambiguously
        // slower. In the end, I decided the complexity from unrolling wasn't
        // worth it. I used the memmem/krate/prebuilt/huge-en/ benchmarks to
        // compare.
        while cur <= max {
            if let Some(chunki) = self.find_in_chunk(needle, cur, end, all) {
                return Some(matched(start, cur, chunki));
            }
            cur = cur.add(V::BYTES);
        }
        if cur < end {
            let remaining = end.distance(cur);
            debug_assert!(
                remaining < self.min_haystack_len,
                "remaining bytes should be smaller than the minimum haystack \
                 length of {}, but there are {} bytes remaining",
                self.min_haystack_len,
                remaining,
            );
            if remaining < needle.len() {
                return None;
            }
            debug_assert!(
                max < cur,
                "after main loop, cur should have exceeded max",
            );
            let overlap = cur.distance(max);
            debug_assert!(
                overlap > 0,
                "overlap ({}) must always be non-zero",
                overlap,
            );
            debug_assert!(
                overlap < V::BYTES,
                "overlap ({}) cannot possibly be >= than a vector ({})",
                overlap,
                V::BYTES,
            );
            // The mask has all of its bits set except for the first N least
            // significant bits, where N=overlap. This way, any matches that
            // occur in find_in_chunk within the overlap are automatically
            // ignored.
            let mask = V::Mask::all_zeros_except_least_significant(overlap);
            cur = max;
            let m = self.find_in_chunk(needle, cur, end, mask);
            if let Some(chunki) = m {
                return Some(matched(start, cur, chunki));
            }
        }
        None
    }

    /// Searches the given haystack for offsets that represent candidate
    /// matches of the `needle` given to this finder's constructor. The offsets
    /// returned, if they are a match, correspond to the starting offset of
    /// `needle` in the given `haystack`.
    ///
    /// # Panics
    ///
    /// When `haystack.len()` is less than [`Finder::min_haystack_len`].
    ///
    /// # Safety
    ///
    /// Since this is meant to be used with vector functions, callers need to
    /// specialize this inside of a function with a `target_feature` attribute.
    /// Therefore, callers must ensure that whatever target feature is being
    /// used supports the vector functions that this function is specialized
    /// for. (For the specific vector functions used, see the Vector trait
    /// implementations.)
    #[inline(always)]
    pub(crate) unsafe fn find_prefilter(
        &self,
        haystack: &[u8],
    ) -> Option<usize> {
        assert!(
            haystack.len() >= self.min_haystack_len,
            "haystack too small, should be at least {} but got {}",
            self.min_haystack_len,
            haystack.len(),
        );

        let start = haystack.as_ptr();
        let end = start.add(haystack.len());
        let max = end.sub(self.min_haystack_len);
        let mut cur = start;

        // N.B. I did experiment with unrolling the loop to deal with size(V)
        // bytes at a time and 2*size(V) bytes at a time. The double unroll
        // was marginally faster while the quadruple unroll was unambiguously
        // slower. In the end, I decided the complexity from unrolling wasn't
        // worth it. I used the memmem/krate/prebuilt/huge-en/ benchmarks to
        // compare.
        while cur <= max {
            if let Some(chunki) = self.find_prefilter_in_chunk(cur) {
                return Some(matched(start, cur, chunki));
            }
            cur = cur.add(V::BYTES);
        }
        if cur < end {
            // This routine immediately quits if a candidate match is found.
            // That means that if we're here, no candidate matches have been
            // found at or before 'ptr'. Thus, we don't need to mask anything
            // out even though we might technically search part of the haystack
            // that we've already searched (because we know it can't match).
            cur = max;
            if let Some(chunki) = self.find_prefilter_in_chunk(cur) {
                return Some(matched(start, cur, chunki));
            }
        }
        None
    }

    /// Search for an occurrence of our byte pair from the needle in the chunk
    /// pointed to by cur, with the end of the haystack pointed to by end.
    /// When an occurrence is found, memcmp is run to check if a match occurs
    /// at the corresponding position.
    ///
    /// `mask` should have bits set corresponding the positions in the chunk
    /// in which matches are considered. This is only used for the last vector
    /// load where the beginning of the vector might have overlapped with the
    /// last load in the main loop. The mask lets us avoid visiting positions
    /// that have already been discarded as matches.
    ///
    /// # Safety
    ///
    /// It must be safe to do an unaligned read of size(V) bytes starting at
    /// both (cur + self.index1) and (cur + self.index2). It must also be safe
    /// to do unaligned loads on cur up to (end - needle.len()).
    #[inline(always)]
    unsafe fn find_in_chunk(
        &self,
        needle: &[u8],
        cur: *const u8,
        end: *const u8,
        mask: V::Mask,
    ) -> Option<usize> {
        let index1 = usize::from(self.pair.index1());
        let index2 = usize::from(self.pair.index2());
        let chunk1 = V::load_unaligned(cur.add(index1));
        let chunk2 = V::load_unaligned(cur.add(index2));
        let eq1 = chunk1.cmpeq(self.v1);
        let eq2 = chunk2.cmpeq(self.v2);

        let mut offsets = eq1.and(eq2).movemask().and(mask);
        while offsets.has_non_zero() {
            let offset = offsets.first_offset();
            let cur = cur.add(offset);
            if end.sub(needle.len()) < cur {
                return None;
            }
            if is_equal_raw(needle.as_ptr(), cur, needle.len()) {
                return Some(offset);
            }
            offsets = offsets.clear_least_significant_bit();
        }
        None
    }

    /// Search for an occurrence of our byte pair from the needle in the chunk
    /// pointed to by cur, with the end of the haystack pointed to by end.
    /// When an occurrence is found, memcmp is run to check if a match occurs
    /// at the corresponding position.
    ///
    /// # Safety
    ///
    /// It must be safe to do an unaligned read of size(V) bytes starting at
    /// both (cur + self.index1) and (cur + self.index2). It must also be safe
    /// to do unaligned reads on cur up to (end - needle.len()).
    #[inline(always)]
    unsafe fn find_prefilter_in_chunk(&self, cur: *const u8) -> Option<usize> {
        let index1 = usize::from(self.pair.index1());
        let index2 = usize::from(self.pair.index2());
        let chunk1 = V::load_unaligned(cur.add(index1));
        let chunk2 = V::load_unaligned(cur.add(index2));
        let eq1 = chunk1.cmpeq(self.v1);
        let eq2 = chunk2.cmpeq(self.v2);

        let offsets = eq1.and(eq2).movemask();
        if !offsets.has_non_zero() {
            return None;
        }
        Some(offsets.first_offset())
    }

    /// Returns the pair of offsets (into the needle) used to check as a
    /// predicate before confirming whether a needle exists at a particular
    /// position.
    #[inline]
    pub(crate) fn pair(&self) -> &Pair {
        &self.pair
    }

    /// Returns the minimum haystack length that this `Finder` can search.
    ///
    /// Providing a haystack to this `Finder` shorter than this length is
    /// guaranteed to result in a panic.
    #[inline(always)]
    pub(crate) fn min_haystack_len(&self) -> usize {
        self.min_haystack_len
    }
}

/// Accepts a chunk-relative offset and returns a haystack relative offset.
///
/// This used to be marked `#[cold]` and `#[inline(never)]`, but I couldn't
/// observe a consistent measureable difference between that and just inlining
/// it. So we go with inlining it.
///
/// # Safety
///
/// Same at `ptr::offset_from` in addition to `cur >= start`.
#[inline(always)]
unsafe fn matched(start: *const u8, cur: *const u8, chunki: usize) -> usize {
    cur.distance(start) + chunki
}

// If you're looking for tests, those are run for each instantiation of the
// above code. So for example, see arch::x86_64::sse2::packedpair.
