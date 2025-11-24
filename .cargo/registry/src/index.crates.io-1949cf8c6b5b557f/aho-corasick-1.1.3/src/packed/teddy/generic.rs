use core::fmt::Debug;

use alloc::{
    boxed::Box, collections::BTreeMap, format, sync::Arc, vec, vec::Vec,
};

use crate::{
    packed::{
        ext::Pointer,
        pattern::Patterns,
        vector::{FatVector, Vector},
    },
    util::int::U32,
    PatternID,
};

/// A match type specialized to the Teddy implementations below.
///
/// Essentially, instead of representing a match at byte offsets, we use
/// raw pointers. This is because the implementations below operate on raw
/// pointers, and so this is a more natural return type based on how the
/// implementation works.
///
/// Also, the `PatternID` used here is a `u16`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Match {
    pid: PatternID,
    start: *const u8,
    end: *const u8,
}

impl Match {
    /// Returns the ID of the pattern that matched.
    pub(crate) fn pattern(&self) -> PatternID {
        self.pid
    }

    /// Returns a pointer into the haystack at which the match starts.
    pub(crate) fn start(&self) -> *const u8 {
        self.start
    }

    /// Returns a pointer into the haystack at which the match ends.
    pub(crate) fn end(&self) -> *const u8 {
        self.end
    }
}

/// A "slim" Teddy implementation that is generic over both the vector type
/// and the minimum length of the patterns being searched for.
///
/// Only 1, 2, 3 and 4 bytes are supported as minimum lengths.
#[derive(Clone, Debug)]
pub(crate) struct Slim<V, const BYTES: usize> {
    /// A generic data structure for doing "slim" Teddy verification.
    teddy: Teddy<8>,
    /// The masks used as inputs to the shuffle operation to generate
    /// candidates (which are fed into the verification routines).
    masks: [Mask<V>; BYTES],
}

impl<V: Vector, const BYTES: usize> Slim<V, BYTES> {
    /// Create a new "slim" Teddy searcher for the given patterns.
    ///
    /// # Panics
    ///
    /// This panics when `BYTES` is any value other than 1, 2, 3 or 4.
    ///
    /// # Safety
    ///
    /// Callers must ensure that this is okay to call in the current target for
    /// the current CPU.
    #[inline(always)]
    pub(crate) unsafe fn new(patterns: Arc<Patterns>) -> Slim<V, BYTES> {
        assert!(
            1 <= BYTES && BYTES <= 4,
            "only 1, 2, 3 or 4 bytes are supported"
        );
        let teddy = Teddy::new(patterns);
        let masks = SlimMaskBuilder::from_teddy(&teddy);
        Slim { teddy, masks }
    }

    /// Returns the approximate total amount of heap used by this type, in
    /// units of bytes.
    #[inline(always)]
    pub(crate) fn memory_usage(&self) -> usize {
        self.teddy.memory_usage()
    }

    /// Returns the minimum length, in bytes, that a haystack must be in order
    /// to use it with this searcher.
    #[inline(always)]
    pub(crate) fn minimum_len(&self) -> usize {
        V::BYTES + (BYTES - 1)
    }
}

impl<V: Vector> Slim<V, 1> {
    /// Look for an occurrences of the patterns in this finder in the haystack
    /// given by the `start` and `end` pointers.
    ///
    /// If no match could be found, then `None` is returned.
    ///
    /// # Safety
    ///
    /// The given pointers representing the haystack must be valid to read
    /// from. They must also point to a region of memory that is at least the
    /// minimum length required by this searcher.
    ///
    /// Callers must ensure that this is okay to call in the current target for
    /// the current CPU.
    #[inline(always)]
    pub(crate) unsafe fn find(
        &self,
        start: *const u8,
        end: *const u8,
    ) -> Option<Match> {
        let len = end.distance(start);
        debug_assert!(len >= self.minimum_len());
        let mut cur = start;
        while cur <= end.sub(V::BYTES) {
            if let Some(m) = self.find_one(cur, end) {
                return Some(m);
            }
            cur = cur.add(V::BYTES);
        }
        if cur < end {
            cur = end.sub(V::BYTES);
            if let Some(m) = self.find_one(cur, end) {
                return Some(m);
            }
        }
        None
    }

    /// Look for a match starting at the `V::BYTES` at and after `cur`. If
    /// there isn't one, then `None` is returned.
    ///
    /// # Safety
    ///
    /// The given pointers representing the haystack must be valid to read
    /// from. They must also point to a region of memory that is at least the
    /// minimum length required by this searcher.
    ///
    /// Callers must ensure that this is okay to call in the current target for
    /// the current CPU.
    #[inline(always)]
    unsafe fn find_one(
        &self,
        cur: *const u8,
        end: *const u8,
    ) -> Option<Match> {
        let c = self.candidate(cur);
        if !c.is_zero() {
            if let Some(m) = self.teddy.verify(cur, end, c) {
                return Some(m);
            }
        }
        None
    }

    /// Look for a candidate match (represented as a vector) starting at the
    /// `V::BYTES` at and after `cur`. If there isn't one, then a vector with
    /// all bits set to zero is returned.
    ///
    /// # Safety
    ///
    /// The given pointer representing the haystack must be valid to read
    /// from.
    ///
    /// Callers must ensure that this is okay to call in the current target for
    /// the current CPU.
    #[inline(always)]
    unsafe fn candidate(&self, cur: *const u8) -> V {
        let chunk = V::load_unaligned(cur);
        Mask::members1(chunk, self.masks)
    }
}

impl<V: Vector> Slim<V, 2> {
    /// See Slim<V, 1>::find.
    #[inline(always)]
    pub(crate) unsafe fn find(
        &self,
        start: *const u8,
        end: *const u8,
    ) -> Option<Match> {
        let len = end.distance(start);
        debug_assert!(len >= self.minimum_len());
        let mut cur = start.add(1);
        let mut prev0 = V::splat(0xFF);
        while cur <= end.sub(V::BYTES) {
            if let Some(m) = self.find_one(cur, end, &mut prev0) {
                return Some(m);
            }
            cur = cur.add(V::BYTES);
        }
        if cur < end {
            cur = end.sub(V::BYTES);
            prev0 = V::splat(0xFF);
            if let Some(m) = self.find_one(cur, end, &mut prev0) {
                return Some(m);
            }
        }
        None
    }

    /// See Slim<V, 1>::find_one.
    #[inline(always)]
    unsafe fn find_one(
        &self,
        cur: *const u8,
        end: *const u8,
        prev0: &mut V,
    ) -> Option<Match> {
        let c = self.candidate(cur, prev0);
        if !c.is_zero() {
            if let Some(m) = self.teddy.verify(cur.sub(1), end, c) {
                return Some(m);
            }
        }
        None
    }

    /// See Slim<V, 1>::candidate.
    #[inline(always)]
    unsafe fn candidate(&self, cur: *const u8, prev0: &mut V) -> V {
        let chunk = V::load_unaligned(cur);
        let (res0, res1) = Mask::members2(chunk, self.masks);
        let res0prev0 = res0.shift_in_one_byte(*prev0);
        let res = res0prev0.and(res1);
        *prev0 = res0;
        res
    }
}

impl<V: Vector> Slim<V, 3> {
    /// See Slim<V, 1>::find.
    #[inline(always)]
    pub(crate) unsafe fn find(
        &self,
        start: *const u8,
        end: *const u8,
    ) -> Option<Match> {
        let len = end.distance(start);
        debug_assert!(len >= self.minimum_len());
        let mut cur = start.add(2);
        let mut prev0 = V::splat(0xFF);
        let mut prev1 = V::splat(0xFF);
        while cur <= end.sub(V::BYTES) {
            if let Some(m) = self.find_one(cur, end, &mut prev0, &mut prev1) {
                return Some(m);
            }
            cur = cur.add(V::BYTES);
        }
        if cur < end {
            cur = end.sub(V::BYTES);
            prev0 = V::splat(0xFF);
            prev1 = V::splat(0xFF);
            if let Some(m) = self.find_one(cur, end, &mut prev0, &mut prev1) {
                return Some(m);
            }
        }
        None
    }

    /// See Slim<V, 1>::find_one.
    #[inline(always)]
    unsafe fn find_one(
        &self,
        cur: *const u8,
        end: *const u8,
        prev0: &mut V,
        prev1: &mut V,
    ) -> Option<Match> {
        let c = self.candidate(cur, prev0, prev1);
        if !c.is_zero() {
            if let Some(m) = self.teddy.verify(cur.sub(2), end, c) {
                return Some(m);
            }
        }
        None
    }

    /// See Slim<V, 1>::candidate.
    #[inline(always)]
    unsafe fn candidate(
        &self,
        cur: *const u8,
        prev0: &mut V,
        prev1: &mut V,
    ) -> V {
        let chunk = V::load_unaligned(cur);
        let (res0, res1, res2) = Mask::members3(chunk, self.masks);
        let res0prev0 = res0.shift_in_two_bytes(*prev0);
        let res1prev1 = res1.shift_in_one_byte(*prev1);
        let res = res0prev0.and(res1prev1).and(res2);
        *prev0 = res0;
        *prev1 = res1;
        res
    }
}

impl<V: Vector> Slim<V, 4> {
    /// See Slim<V, 1>::find.
    #[inline(always)]
    pub(crate) unsafe fn find(
        &self,
        start: *const u8,
        end: *const u8,
    ) -> Option<Match> {
        let len = end.distance(start);
        debug_assert!(len >= self.minimum_len());
        let mut cur = start.add(3);
        let mut prev0 = V::splat(0xFF);
        let mut prev1 = V::splat(0xFF);
        let mut prev2 = V::splat(0xFF);
        while cur <= end.sub(V::BYTES) {
            if let Some(m) =
                self.find_one(cur, end, &mut prev0, &mut prev1, &mut prev2)
            {
                return Some(m);
            }
            cur = cur.add(V::BYTES);
        }
        if cur < end {
            cur = end.sub(V::BYTES);
            prev0 = V::splat(0xFF);
            prev1 = V::splat(0xFF);
            prev2 = V::splat(0xFF);
            if let Some(m) =
                self.find_one(cur, end, &mut prev0, &mut prev1, &mut prev2)
            {
                return Some(m);
            }
        }
        None
    }

    /// See Slim<V, 1>::find_one.
    #[inline(always)]
    unsafe fn find_one(
        &self,
        cur: *const u8,
        end: *const u8,
        prev0: &mut V,
        prev1: &mut V,
        prev2: &mut V,
    ) -> Option<Match> {
        let c = self.candidate(cur, prev0, prev1, prev2);
        if !c.is_zero() {
            if let Some(m) = self.teddy.verify(cur.sub(3), end, c) {
                return Some(m);
            }
        }
        None
    }

    /// See Slim<V, 1>::candidate.
    #[inline(always)]
    unsafe fn candidate(
        &self,
        cur: *const u8,
        prev0: &mut V,
        prev1: &mut V,
        prev2: &mut V,
    ) -> V {
        let chunk = V::load_unaligned(cur);
        let (res0, res1, res2, res3) = Mask::members4(chunk, self.masks);
        let res0prev0 = res0.shift_in_three_bytes(*prev0);
        let res1prev1 = res1.shift_in_two_bytes(*prev1);
        let res2prev2 = res2.shift_in_one_byte(*prev2);
        let res = res0prev0.and(res1prev1).and(res2prev2).and(res3);
        *prev0 = res0;
        *prev1 = res1;
        *prev2 = res2;
        res
    }
}

/// A "fat" Teddy implementation that is generic over both the vector type
/// and the minimum length of the patterns being searched for.
///
/// Only 1, 2, 3 and 4 bytes are supported as minimum lengths.
#[derive(Clone, Debug)]
pub(crate) struct Fat<V, const BYTES: usize> {
    /// A generic data structure for doing "fat" Teddy verification.
    teddy: Teddy<16>,
    /// The masks used as inputs to the shuffle operation to generate
    /// candidates (which are fed into the verification routines).
    masks: [Mask<V>; BYTES],
}

impl<V: FatVector, const BYTES: usize> Fat<V, BYTES> {
    /// Create a new "fat" Teddy searcher for the given patterns.
    ///
    /// # Panics
    ///
    /// This panics when `BYTES` is any value other than 1, 2, 3 or 4.
    ///
    /// # Safety
    ///
    /// Callers must ensure that this is okay to call in the current target for
    /// the current CPU.
    #[inline(always)]
    pub(crate) unsafe fn new(patterns: Arc<Patterns>) -> Fat<V, BYTES> {
        assert!(
            1 <= BYTES && BYTES <= 4,
            "only 1, 2, 3 or 4 bytes are supported"
        );
        let teddy = Teddy::new(patterns);
        let masks = FatMaskBuilder::from_teddy(&teddy);
        Fat { teddy, masks }
    }

    /// Returns the approximate total amount of heap used by this type, in
    /// units of bytes.
    #[inline(always)]
    pub(crate) fn memory_usage(&self) -> usize {
        self.teddy.memory_usage()
    }

    /// Returns the minimum length, in bytes, that a haystack must be in order
    /// to use it with this searcher.
    #[inline(always)]
    pub(crate) fn minimum_len(&self) -> usize {
        V::Half::BYTES + (BYTES - 1)
    }
}

impl<V: FatVector> Fat<V, 1> {
    /// Look for an occurrences of the patterns in this finder in the haystack
    /// given by the `start` and `end` pointers.
    ///
    /// If no match could be found, then `None` is returned.
    ///
    /// # Safety
    ///
    /// The given pointers representing the haystack must be valid to read
    /// from. They must also point to a region of memory that is at least the
    /// minimum length required by this searcher.
    ///
    /// Callers must ensure that this is okay to call in the current target for
    /// the current CPU.
    #[inline(always)]
    pub(crate) unsafe fn find(
        &self,
        start: *const u8,
        end: *const u8,
    ) -> Option<Match> {
        let len = end.distance(start);
        debug_assert!(len >= self.minimum_len());
        let mut cur = start;
        while cur <= end.sub(V::Half::BYTES) {
            if let Some(m) = self.find_one(cur, end) {
                return Some(m);
            }
            cur = cur.add(V::Half::BYTES);
        }
        if cur < end {
            cur = end.sub(V::Half::BYTES);
            if let Some(m) = self.find_one(cur, end) {
                return Some(m);
            }
        }
        None
    }

    /// Look for a match starting at the `V::BYTES` at and after `cur`. If
    /// there isn't one, then `None` is returned.
    ///
    /// # Safety
    ///
    /// The given pointers representing the haystack must be valid to read
    /// from. They must also point to a region of memory that is at least the
    /// minimum length required by this searcher.
    ///
    /// Callers must ensure that this is okay to call in the current target for
    /// the current CPU.
    #[inline(always)]
    unsafe fn find_one(
        &self,
        cur: *const u8,
        end: *const u8,
    ) -> Option<Match> {
        let c = self.candidate(cur);
        if !c.is_zero() {
            if let Some(m) = self.teddy.verify(cur, end, c) {
                return Some(m);
            }
        }
        None
    }

    /// Look for a candidate match (represented as a vector) starting at the
    /// `V::BYTES` at and after `cur`. If there isn't one, then a vector with
    /// all bits set to zero is returned.
    ///
    /// # Safety
    ///
    /// The given pointer representing the haystack must be valid to read
    /// from.
    ///
    /// Callers must ensure that this is okay to call in the current target for
    /// the current CPU.
    #[inline(always)]
    unsafe fn candidate(&self, cur: *const u8) -> V {
        let chunk = V::load_half_unaligned(cur);
        Mask::members1(chunk, self.masks)
    }
}

impl<V: FatVector> Fat<V, 2> {
    /// See `Fat<V, 1>::find`.
    #[inline(always)]
    pub(crate) unsafe fn find(
        &self,
        start: *const u8,
        end: *const u8,
    ) -> Option<Match> {
        let len = end.distance(start);
        debug_assert!(len >= self.minimum_len());
        let mut cur = start.add(1);
        let mut prev0 = V::splat(0xFF);
        while cur <= end.sub(V::Half::BYTES) {
            if let Some(m) = self.find_one(cur, end, &mut prev0) {
                return Some(m);
            }
            cur = cur.add(V::Half::BYTES);
        }
        if cur < end {
            cur = end.sub(V::Half::BYTES);
            prev0 = V::splat(0xFF);
            if let Some(m) = self.find_one(cur, end, &mut prev0) {
                return Some(m);
            }
        }
        None
    }

    /// See `Fat<V, 1>::find_one`.
    #[inline(always)]
    unsafe fn find_one(
        &self,
        cur: *const u8,
        end: *const u8,
        prev0: &mut V,
    ) -> Option<Match> {
        let c = self.candidate(cur, prev0);
        if !c.is_zero() {
            if let Some(m) = self.teddy.verify(cur.sub(1), end, c) {
                return Some(m);
            }
        }
        None
    }

    /// See `Fat<V, 1>::candidate`.
    #[inline(always)]
    unsafe fn candidate(&self, cur: *const u8, prev0: &mut V) -> V {
        let chunk = V::load_half_unaligned(cur);
        let (res0, res1) = Mask::members2(chunk, self.masks);
        let res0prev0 = res0.half_shift_in_one_byte(*prev0);
        let res = res0prev0.and(res1);
        *prev0 = res0;
        res
    }
}

impl<V: FatVector> Fat<V, 3> {
    /// See `Fat<V, 1>::find`.
    #[inline(always)]
    pub(crate) unsafe fn find(
        &self,
        start: *const u8,
        end: *const u8,
    ) -> Option<Match> {
        let len = end.distance(start);
        debug_assert!(len >= self.minimum_len());
        let mut cur = start.add(2);
        let mut prev0 = V::splat(0xFF);
        let mut prev1 = V::splat(0xFF);
        while cur <= end.sub(V::Half::BYTES) {
            if let Some(m) = self.find_one(cur, end, &mut prev0, &mut prev1) {
                return Some(m);
            }
            cur = cur.add(V::Half::BYTES);
        }
        if cur < end {
            cur = end.sub(V::Half::BYTES);
            prev0 = V::splat(0xFF);
            prev1 = V::splat(0xFF);
            if let Some(m) = self.find_one(cur, end, &mut prev0, &mut prev1) {
                return Some(m);
            }
        }
        None
    }

    /// See `Fat<V, 1>::find_one`.
    #[inline(always)]
    unsafe fn find_one(
        &self,
        cur: *const u8,
        end: *const u8,
        prev0: &mut V,
        prev1: &mut V,
    ) -> Option<Match> {
        let c = self.candidate(cur, prev0, prev1);
        if !c.is_zero() {
            if let Some(m) = self.teddy.verify(cur.sub(2), end, c) {
                return Some(m);
            }
        }
        None
    }

    /// See `Fat<V, 1>::candidate`.
    #[inline(always)]
    unsafe fn candidate(
        &self,
        cur: *const u8,
        prev0: &mut V,
        prev1: &mut V,
    ) -> V {
        let chunk = V::load_half_unaligned(cur);
        let (res0, res1, res2) = Mask::members3(chunk, self.masks);
        let res0prev0 = res0.half_shift_in_two_bytes(*prev0);
        let res1prev1 = res1.half_shift_in_one_byte(*prev1);
        let res = res0prev0.and(res1prev1).and(res2);
        *prev0 = res0;
        *prev1 = res1;
        res
    }
}

impl<V: FatVector> Fat<V, 4> {
    /// See `Fat<V, 1>::find`.
    #[inline(always)]
    pub(crate) unsafe fn find(
        &self,
        start: *const u8,
        end: *const u8,
    ) -> Option<Match> {
        let len = end.distance(start);
        debug_assert!(len >= self.minimum_len());
        let mut cur = start.add(3);
        let mut prev0 = V::splat(0xFF);
        let mut prev1 = V::splat(0xFF);
        let mut prev2 = V::splat(0xFF);
        while cur <= end.sub(V::Half::BYTES) {
            if let Some(m) =
                self.find_one(cur, end, &mut prev0, &mut prev1, &mut prev2)
            {
                return Some(m);
            }
            cur = cur.add(V::Half::BYTES);
        }
        if cur < end {
            cur = end.sub(V::Half::BYTES);
            prev0 = V::splat(0xFF);
            prev1 = V::splat(0xFF);
            prev2 = V::splat(0xFF);
            if let Some(m) =
                self.find_one(cur, end, &mut prev0, &mut prev1, &mut prev2)
            {
                return Some(m);
            }
        }
        None
    }

    /// See `Fat<V, 1>::find_one`.
    #[inline(always)]
    unsafe fn find_one(
        &self,
        cur: *const u8,
        end: *const u8,
        prev0: &mut V,
        prev1: &mut V,
        prev2: &mut V,
    ) -> Option<Match> {
        let c = self.candidate(cur, prev0, prev1, prev2);
        if !c.is_zero() {
            if let Some(m) = self.teddy.verify(cur.sub(3), end, c) {
                return Some(m);
            }
        }
        None
    }

    /// See `Fat<V, 1>::candidate`.
    #[inline(always)]
    unsafe fn candidate(
        &self,
        cur: *const u8,
        prev0: &mut V,
        prev1: &mut V,
        prev2: &mut V,
    ) -> V {
        let chunk = V::load_half_unaligned(cur);
        let (res0, res1, res2, res3) = Mask::members4(chunk, self.masks);
        let res0prev0 = res0.half_shift_in_three_bytes(*prev0);
        let res1prev1 = res1.half_shift_in_two_bytes(*prev1);
        let res2prev2 = res2.half_shift_in_one_byte(*prev2);
        let res = res0prev0.and(res1prev1).and(res2prev2).and(res3);
        *prev0 = res0;
        *prev1 = res1;
        *prev2 = res2;
        res
    }
}

/// The common elements of all "slim" and "fat" Teddy search implementations.
///
/// Essentially, this contains the patterns and the buckets. Namely, it
/// contains enough to implement the verification step after candidates are
/// identified via the shuffle masks.
///
/// It is generic over the number of buckets used. In general, the number of
/// buckets is either 8 (for "slim" Teddy) or 16 (for "fat" Teddy). The generic
/// parameter isn't really meant to be instantiated for any value other than
/// 8 or 16, although it is technically possible. The main hiccup is that there
/// is some bit-shifting done in the critical part of verification that could
/// be quite expensive if `N` is not a multiple of 2.
#[derive(Clone, Debug)]
struct Teddy<const BUCKETS: usize> {
    /// The patterns we are searching for.
    ///
    /// A pattern string can be found by its `PatternID`.
    patterns: Arc<Patterns>,
    /// The allocation of patterns in buckets. This only contains the IDs of
    /// patterns. In order to do full verification, callers must provide the
    /// actual patterns when using Teddy.
    buckets: [Vec<PatternID>; BUCKETS],
    // N.B. The above representation is very simple, but it definitely results
    // in ping-ponging between different allocations during verification. I've
    // tried experimenting with other representations that flatten the pattern
    // strings into a single allocation, but it doesn't seem to help much.
    // Probably everything is small enough to fit into cache anyway, and so the
    // pointer chasing isn't a big deal?
    //
    // One other avenue I haven't explored is some kind of hashing trick
    // that let's us do another high-confidence check before launching into
    // `memcmp`.
}

impl<const BUCKETS: usize> Teddy<BUCKETS> {
    /// Create a new generic data structure for Teddy verification.
    fn new(patterns: Arc<Patterns>) -> Teddy<BUCKETS> {
        assert_ne!(0, patterns.len(), "Teddy requires at least one pattern");
        assert_ne!(
            0,
            patterns.minimum_len(),
            "Teddy does not support zero-length patterns"
        );
        assert!(
            BUCKETS == 8 || BUCKETS == 16,
            "Teddy only supports 8 or 16 buckets"
        );
        // MSRV(1.63): Use core::array::from_fn below instead of allocating a
        // superfluous outer Vec. Not a big deal (especially given the BTreeMap
        // allocation below), but nice to not do it.
        let buckets =
            <[Vec<PatternID>; BUCKETS]>::try_from(vec![vec![]; BUCKETS])
                .unwrap();
        let mut t = Teddy { patterns, buckets };

        let mut map: BTreeMap<Box<[u8]>, usize> = BTreeMap::new();
        for (id, pattern) in t.patterns.iter() {
            // We try to be slightly clever in how we assign patterns into
            // buckets. Generally speaking, we want patterns with the same
            // prefix to be in the same bucket, since it minimizes the amount
            // of time we spend churning through buckets in the verification
            // step.
            //
            // So we could assign patterns with the same N-prefix (where N is
            // the size of the mask, which is one of {1, 2, 3}) to the same
            // bucket. However, case insensitive searches are fairly common, so
            // we'd for example, ideally want to treat `abc` and `ABC` as if
            // they shared the same prefix. ASCII has the nice property that
            // the lower 4 bits of A and a are the same, so we therefore group
            // patterns with the same low-nybble-N-prefix into the same bucket.
            //
            // MOREOVER, this is actually necessary for correctness! In
            // particular, by grouping patterns with the same prefix into the
            // same bucket, we ensure that we preserve correct leftmost-first
            // and leftmost-longest match semantics. In addition to the fact
            // that `patterns.iter()` iterates in the correct order, this
            // guarantees that all possible ambiguous matches will occur in
            // the same bucket. The verification routine could be adjusted to
            // support correct leftmost match semantics regardless of bucket
            // allocation, but that results in a performance hit. It's much
            // nicer to be able to just stop as soon as a match is found.
            let lonybs = pattern.low_nybbles(t.mask_len());
            if let Some(&bucket) = map.get(&lonybs) {
                t.buckets[bucket].push(id);
            } else {
                // N.B. We assign buckets in reverse because it shouldn't have
                // any influence on performance, but it does make it harder to
                // get leftmost match semantics accidentally correct.
                let bucket = (BUCKETS - 1) - (id.as_usize() % BUCKETS);
                t.buckets[bucket].push(id);
                map.insert(lonybs, bucket);
            }
        }
        t
    }

    /// Verify whether there are any matches starting at or after `cur` in the
    /// haystack. The candidate chunk given should correspond to 8-bit bitsets
    /// for N buckets.
    ///
    /// # Safety
    ///
    /// The given pointers representing the haystack must be valid to read
    /// from.
    #[inline(always)]
    unsafe fn verify64(
        &self,
        cur: *const u8,
        end: *const u8,
        mut candidate_chunk: u64,
    ) -> Option<Match> {
        while candidate_chunk != 0 {
            let bit = candidate_chunk.trailing_zeros().as_usize();
            candidate_chunk &= !(1 << bit);

            let cur = cur.add(bit / BUCKETS);
            let bucket = bit % BUCKETS;
            if let Some(m) = self.verify_bucket(cur, end, bucket) {
                return Some(m);
            }
        }
        None
    }

    /// Verify whether there are any matches starting at `at` in the given
    /// `haystack` corresponding only to patterns in the given bucket.
    ///
    /// # Safety
    ///
    /// The given pointers representing the haystack must be valid to read
    /// from.
    ///
    /// The bucket index must be less than or equal to `self.buckets.len()`.
    #[inline(always)]
    unsafe fn verify_bucket(
        &self,
        cur: *const u8,
        end: *const u8,
        bucket: usize,
    ) -> Option<Match> {
        debug_assert!(bucket < self.buckets.len());
        // SAFETY: The caller must ensure that the bucket index is correct.
        for pid in self.buckets.get_unchecked(bucket).iter().copied() {
            // SAFETY: This is safe because we are guaranteed that every
            // index in a Teddy bucket is a valid index into `pats`, by
            // construction.
            debug_assert!(pid.as_usize() < self.patterns.len());
            let pat = self.patterns.get_unchecked(pid);
            if pat.is_prefix_raw(cur, end) {
                let start = cur;
                let end = start.add(pat.len());
                return Some(Match { pid, start, end });
            }
        }
        None
    }

    /// Returns the total number of masks required by the patterns in this
    /// Teddy searcher.
    ///
    /// Basically, the mask length corresponds to the type of Teddy searcher
    /// to use: a 1-byte, 2-byte, 3-byte or 4-byte searcher. The bigger the
    /// better, typically, since searching for longer substrings usually
    /// decreases the rate of false positives. Therefore, the number of masks
    /// needed is the length of the shortest pattern in this searcher. If the
    /// length of the shortest pattern (in bytes) is bigger than 4, then the
    /// mask length is 4 since there are no Teddy searchers for more than 4
    /// bytes.
    fn mask_len(&self) -> usize {
        core::cmp::min(4, self.patterns.minimum_len())
    }

    /// Returns the approximate total amount of heap used by this type, in
    /// units of bytes.
    fn memory_usage(&self) -> usize {
        // This is an upper bound rather than a precise accounting. No
        // particular reason, other than it's probably very close to actual
        // memory usage in practice.
        self.patterns.len() * core::mem::size_of::<PatternID>()
    }
}

impl Teddy<8> {
    /// Runs the verification routine for "slim" Teddy.
    ///
    /// The candidate given should be a collection of 8-bit bitsets (one bitset
    /// per lane), where the ith bit is set in the jth lane if and only if the
    /// byte occurring at `at + j` in `cur` is in the bucket `i`.
    ///
    /// # Safety
    ///
    /// Callers must ensure that this is okay to call in the current target for
    /// the current CPU.
    ///
    /// The given pointers must be valid to read from.
    #[inline(always)]
    unsafe fn verify<V: Vector>(
        &self,
        mut cur: *const u8,
        end: *const u8,
        candidate: V,
    ) -> Option<Match> {
        debug_assert!(!candidate.is_zero());
        // Convert the candidate into 64-bit chunks, and then verify each of
        // those chunks.
        candidate.for_each_64bit_lane(
            #[inline(always)]
            |_, chunk| {
                let result = self.verify64(cur, end, chunk);
                cur = cur.add(8);
                result
            },
        )
    }
}

impl Teddy<16> {
    /// Runs the verification routine for "fat" Teddy.
    ///
    /// The candidate given should be a collection of 8-bit bitsets (one bitset
    /// per lane), where the ith bit is set in the jth lane if and only if the
    /// byte occurring at `at + (j < 16 ? j : j - 16)` in `cur` is in the
    /// bucket `j < 16 ? i : i + 8`.
    ///
    /// # Safety
    ///
    /// Callers must ensure that this is okay to call in the current target for
    /// the current CPU.
    ///
    /// The given pointers must be valid to read from.
    #[inline(always)]
    unsafe fn verify<V: FatVector>(
        &self,
        mut cur: *const u8,
        end: *const u8,
        candidate: V,
    ) -> Option<Match> {
        // This is a bit tricky, but we basically want to convert our
        // candidate, which looks like this (assuming a 256-bit vector):
        //
        //     a31 a30 ... a17 a16 a15 a14 ... a01 a00
        //
        // where each a(i) is an 8-bit bitset corresponding to the activated
        // buckets, to this
        //
        //     a31 a15 a30 a14 a29 a13 ... a18 a02 a17 a01 a16 a00
        //
        // Namely, for Fat Teddy, the high 128-bits of the candidate correspond
        // to the same bytes in the haystack in the low 128-bits (so we only
        // scan 16 bytes at a time), but are for buckets 8-15 instead of 0-7.
        //
        // The verification routine wants to look at all potentially matching
        // buckets before moving on to the next lane. So for example, both
        // a16 and a00 both correspond to the first byte in our window; a00
        // contains buckets 0-7 and a16 contains buckets 8-15. Specifically,
        // a16 should be checked before a01. So the transformation shown above
        // allows us to use our normal verification procedure with one small
        // change: we treat each bitset as 16 bits instead of 8 bits.
        debug_assert!(!candidate.is_zero());

        // Swap the 128-bit lanes in the candidate vector.
        let swapped = candidate.swap_halves();
        // Interleave the bytes from the low 128-bit lanes, starting with
        // cand first.
        let r1 = candidate.interleave_low_8bit_lanes(swapped);
        // Interleave the bytes from the high 128-bit lanes, starting with
        // cand first.
        let r2 = candidate.interleave_high_8bit_lanes(swapped);
        // Now just take the 2 low 64-bit integers from both r1 and r2. We
        // can drop the high 64-bit integers because they are a mirror image
        // of the low 64-bit integers. All we care about are the low 128-bit
        // lanes of r1 and r2. Combined, they contain all our 16-bit bitsets
        // laid out in the desired order, as described above.
        r1.for_each_low_64bit_lane(
            r2,
            #[inline(always)]
            |_, chunk| {
                let result = self.verify64(cur, end, chunk);
                cur = cur.add(4);
                result
            },
        )
    }
}

/// A vector generic mask for the low and high nybbles in a set of patterns.
/// Each 8-bit lane `j` in a vector corresponds to a bitset where the `i`th bit
/// is set if and only if the nybble `j` is in the bucket `i` at a particular
/// position.
///
/// This is slightly tweaked dependending on whether Slim or Fat Teddy is being
/// used. For Slim Teddy, the bitsets in the lower half are the same as the
/// bitsets in the higher half, so that we can search `V::BYTES` bytes at a
/// time. (Remember, the nybbles in the haystack are used as indices into these
/// masks, and 256-bit shuffles only operate on 128-bit lanes.)
///
/// For Fat Teddy, the bitsets are not repeated, but instead, the high half
/// bits correspond to an addition 8 buckets. So that a bitset `00100010` has
/// buckets 1 and 5 set if it's in the lower half, but has buckets 9 and 13 set
/// if it's in the higher half.
#[derive(Clone, Copy, Debug)]
struct Mask<V> {
    lo: V,
    hi: V,
}

impl<V: Vector> Mask<V> {
    /// Return a candidate for Teddy (fat or slim) that is searching for 1-byte
    /// candidates.
    ///
    /// If a candidate is returned, it will be a collection of 8-bit bitsets
    /// (one bitset per lane), where the ith bit is set in the jth lane if and
    /// only if the byte occurring at the jth lane in `chunk` is in the bucket
    /// `i`. If no candidate is found, then the vector returned will have all
    /// lanes set to zero.
    ///
    /// `chunk` should correspond to a `V::BYTES` window of the haystack (where
    /// the least significant byte corresponds to the start of the window). For
    /// fat Teddy, the haystack window length should be `V::BYTES / 2`, with
    /// the window repeated in each half of the vector.
    ///
    /// `mask1` should correspond to a low/high mask for the first byte of all
    /// patterns that are being searched.
    #[inline(always)]
    unsafe fn members1(chunk: V, masks: [Mask<V>; 1]) -> V {
        let lomask = V::splat(0xF);
        let hlo = chunk.and(lomask);
        let hhi = chunk.shift_8bit_lane_right::<4>().and(lomask);
        let locand = masks[0].lo.shuffle_bytes(hlo);
        let hicand = masks[0].hi.shuffle_bytes(hhi);
        locand.and(hicand)
    }

    /// Return a candidate for Teddy (fat or slim) that is searching for 2-byte
    /// candidates.
    ///
    /// If candidates are returned, each will be a collection of 8-bit bitsets
    /// (one bitset per lane), where the ith bit is set in the jth lane if and
    /// only if the byte occurring at the jth lane in `chunk` is in the bucket
    /// `i`. Each candidate returned corresponds to the first and second bytes
    /// of the patterns being searched. If no candidate is found, then all of
    /// the lanes will be set to zero in at least one of the vectors returned.
    ///
    /// `chunk` should correspond to a `V::BYTES` window of the haystack (where
    /// the least significant byte corresponds to the start of the window). For
    /// fat Teddy, the haystack window length should be `V::BYTES / 2`, with
    /// the window repeated in each half of the vector.
    ///
    /// The masks should correspond to the masks computed for the first and
    /// second bytes of all patterns that are being searched.
    #[inline(always)]
    unsafe fn members2(chunk: V, masks: [Mask<V>; 2]) -> (V, V) {
        let lomask = V::splat(0xF);
        let hlo = chunk.and(lomask);
        let hhi = chunk.shift_8bit_lane_right::<4>().and(lomask);

        let locand1 = masks[0].lo.shuffle_bytes(hlo);
        let hicand1 = masks[0].hi.shuffle_bytes(hhi);
        let cand1 = locand1.and(hicand1);

        let locand2 = masks[1].lo.shuffle_bytes(hlo);
        let hicand2 = masks[1].hi.shuffle_bytes(hhi);
        let cand2 = locand2.and(hicand2);

        (cand1, cand2)
    }

    /// Return a candidate for Teddy (fat or slim) that is searching for 3-byte
    /// candidates.
    ///
    /// If candidates are returned, each will be a collection of 8-bit bitsets
    /// (one bitset per lane), where the ith bit is set in the jth lane if and
    /// only if the byte occurring at the jth lane in `chunk` is in the bucket
    /// `i`. Each candidate returned corresponds to the first, second and third
    /// bytes of the patterns being searched. If no candidate is found, then
    /// all of the lanes will be set to zero in at least one of the vectors
    /// returned.
    ///
    /// `chunk` should correspond to a `V::BYTES` window of the haystack (where
    /// the least significant byte corresponds to the start of the window). For
    /// fat Teddy, the haystack window length should be `V::BYTES / 2`, with
    /// the window repeated in each half of the vector.
    ///
    /// The masks should correspond to the masks computed for the first, second
    /// and third bytes of all patterns that are being searched.
    #[inline(always)]
    unsafe fn members3(chunk: V, masks: [Mask<V>; 3]) -> (V, V, V) {
        let lomask = V::splat(0xF);
        let hlo = chunk.and(lomask);
        let hhi = chunk.shift_8bit_lane_right::<4>().and(lomask);

        let locand1 = masks[0].lo.shuffle_bytes(hlo);
        let hicand1 = masks[0].hi.shuffle_bytes(hhi);
        let cand1 = locand1.and(hicand1);

        let locand2 = masks[1].lo.shuffle_bytes(hlo);
        let hicand2 = masks[1].hi.shuffle_bytes(hhi);
        let cand2 = locand2.and(hicand2);

        let locand3 = masks[2].lo.shuffle_bytes(hlo);
        let hicand3 = masks[2].hi.shuffle_bytes(hhi);
        let cand3 = locand3.and(hicand3);

        (cand1, cand2, cand3)
    }

    /// Return a candidate for Teddy (fat or slim) that is searching for 4-byte
    /// candidates.
    ///
    /// If candidates are returned, each will be a collection of 8-bit bitsets
    /// (one bitset per lane), where the ith bit is set in the jth lane if and
    /// only if the byte occurring at the jth lane in `chunk` is in the bucket
    /// `i`. Each candidate returned corresponds to the first, second, third
    /// and fourth bytes of the patterns being searched. If no candidate is
    /// found, then all of the lanes will be set to zero in at least one of the
    /// vectors returned.
    ///
    /// `chunk` should correspond to a `V::BYTES` window of the haystack (where
    /// the least significant byte corresponds to the start of the window). For
    /// fat Teddy, the haystack window length should be `V::BYTES / 2`, with
    /// the window repeated in each half of the vector.
    ///
    /// The masks should correspond to the masks computed for the first,
    /// second, third and fourth bytes of all patterns that are being searched.
    #[inline(always)]
    unsafe fn members4(chunk: V, masks: [Mask<V>; 4]) -> (V, V, V, V) {
        let lomask = V::splat(0xF);
        let hlo = chunk.and(lomask);
        let hhi = chunk.shift_8bit_lane_right::<4>().and(lomask);

        let locand1 = masks[0].lo.shuffle_bytes(hlo);
        let hicand1 = masks[0].hi.shuffle_bytes(hhi);
        let cand1 = locand1.and(hicand1);

        let locand2 = masks[1].lo.shuffle_bytes(hlo);
        let hicand2 = masks[1].hi.shuffle_bytes(hhi);
        let cand2 = locand2.and(hicand2);

        let locand3 = masks[2].lo.shuffle_bytes(hlo);
        let hicand3 = masks[2].hi.shuffle_bytes(hhi);
        let cand3 = locand3.and(hicand3);

        let locand4 = masks[3].lo.shuffle_bytes(hlo);
        let hicand4 = masks[3].hi.shuffle_bytes(hhi);
        let cand4 = locand4.and(hicand4);

        (cand1, cand2, cand3, cand4)
    }
}

/// Represents the low and high nybble masks that will be used during
/// search. Each mask is 32 bytes wide, although only the first 16 bytes are
/// used for 128-bit vectors.
///
/// Each byte in the mask corresponds to a 8-bit bitset, where bit `i` is set
/// if and only if the corresponding nybble is in the ith bucket. The index of
/// the byte (0-15, inclusive) corresponds to the nybble.
///
/// Each mask is used as the target of a shuffle, where the indices for the
/// shuffle are taken from the haystack. AND'ing the shuffles for both the
/// low and high masks together also results in 8-bit bitsets, but where bit
/// `i` is set if and only if the correspond *byte* is in the ith bucket.
#[derive(Clone, Default)]
struct SlimMaskBuilder {
    lo: [u8; 32],
    hi: [u8; 32],
}

impl SlimMaskBuilder {
    /// Update this mask by adding the given byte to the given bucket. The
    /// given bucket must be in the range 0-7.
    ///
    /// # Panics
    ///
    /// When `bucket >= 8`.
    fn add(&mut self, bucket: usize, byte: u8) {
        assert!(bucket < 8);

        let bucket = u8::try_from(bucket).unwrap();
        let byte_lo = usize::from(byte & 0xF);
        let byte_hi = usize::from((byte >> 4) & 0xF);
        // When using 256-bit vectors, we need to set this bucket assignment in
        // the low and high 128-bit portions of the mask. This allows us to
        // process 32 bytes at a time. Namely, AVX2 shuffles operate on each
        // of the 128-bit lanes, rather than the full 256-bit vector at once.
        self.lo[byte_lo] |= 1 << bucket;
        self.lo[byte_lo + 16] |= 1 << bucket;
        self.hi[byte_hi] |= 1 << bucket;
        self.hi[byte_hi + 16] |= 1 << bucket;
    }

    /// Turn this builder into a vector mask.
    ///
    /// # Panics
    ///
    /// When `V` represents a vector bigger than what `MaskBytes` can contain.
    ///
    /// # Safety
    ///
    /// Callers must ensure that this is okay to call in the current target for
    /// the current CPU.
    #[inline(always)]
    unsafe fn build<V: Vector>(&self) -> Mask<V> {
        assert!(V::BYTES <= self.lo.len());
        assert!(V::BYTES <= self.hi.len());
        Mask {
            lo: V::load_unaligned(self.lo[..].as_ptr()),
            hi: V::load_unaligned(self.hi[..].as_ptr()),
        }
    }

    /// A convenience function for building `N` vector masks from a slim
    /// `Teddy` value.
    ///
    /// # Panics
    ///
    /// When `V` represents a vector bigger than what `MaskBytes` can contain.
    ///
    /// # Safety
    ///
    /// Callers must ensure that this is okay to call in the current target for
    /// the current CPU.
    #[inline(always)]
    unsafe fn from_teddy<const BYTES: usize, V: Vector>(
        teddy: &Teddy<8>,
    ) -> [Mask<V>; BYTES] {
        // MSRV(1.63): Use core::array::from_fn to just build the array here
        // instead of creating a vector and turning it into an array.
        let mut mask_builders = vec![SlimMaskBuilder::default(); BYTES];
        for (bucket_index, bucket) in teddy.buckets.iter().enumerate() {
            for pid in bucket.iter().copied() {
                let pat = teddy.patterns.get(pid);
                for (i, builder) in mask_builders.iter_mut().enumerate() {
                    builder.add(bucket_index, pat.bytes()[i]);
                }
            }
        }
        let array =
            <[SlimMaskBuilder; BYTES]>::try_from(mask_builders).unwrap();
        array.map(|builder| builder.build())
    }
}

impl Debug for SlimMaskBuilder {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let (mut parts_lo, mut parts_hi) = (vec![], vec![]);
        for i in 0..32 {
            parts_lo.push(format!("{:02}: {:08b}", i, self.lo[i]));
            parts_hi.push(format!("{:02}: {:08b}", i, self.hi[i]));
        }
        f.debug_struct("SlimMaskBuilder")
            .field("lo", &parts_lo)
            .field("hi", &parts_hi)
            .finish()
    }
}

/// Represents the low and high nybble masks that will be used during "fat"
/// Teddy search.
///
/// Each mask is 32 bytes wide, and at the time of writing, only 256-bit vectors
/// support fat Teddy.
///
/// A fat Teddy mask is like a slim Teddy mask, except that instead of
/// repeating the bitsets in the high and low 128-bits in 256-bit vectors, the
/// high and low 128-bit halves each represent distinct buckets. (Bringing the
/// total to 16 instead of 8.) This permits spreading the patterns out a bit
/// more and thus putting less pressure on verification to be fast.
///
/// Each byte in the mask corresponds to a 8-bit bitset, where bit `i` is set
/// if and only if the corresponding nybble is in the ith bucket. The index of
/// the byte (0-15, inclusive) corresponds to the nybble.
#[derive(Clone, Copy, Default)]
struct FatMaskBuilder {
    lo: [u8; 32],
    hi: [u8; 32],
}

impl FatMaskBuilder {
    /// Update this mask by adding the given byte to the given bucket. The
    /// given bucket must be in the range 0-15.
    ///
    /// # Panics
    ///
    /// When `bucket >= 16`.
    fn add(&mut self, bucket: usize, byte: u8) {
        assert!(bucket < 16);

        let bucket = u8::try_from(bucket).unwrap();
        let byte_lo = usize::from(byte & 0xF);
        let byte_hi = usize::from((byte >> 4) & 0xF);
        // Unlike slim teddy, fat teddy only works with AVX2. For fat teddy,
        // the high 128 bits of our mask correspond to buckets 8-15, while the
        // low 128 bits correspond to buckets 0-7.
        if bucket < 8 {
            self.lo[byte_lo] |= 1 << bucket;
            self.hi[byte_hi] |= 1 << bucket;
        } else {
            self.lo[byte_lo + 16] |= 1 << (bucket % 8);
            self.hi[byte_hi + 16] |= 1 << (bucket % 8);
        }
    }

    /// Turn this builder into a vector mask.
    ///
    /// # Panics
    ///
    /// When `V` represents a vector bigger than what `MaskBytes` can contain.
    ///
    /// # Safety
    ///
    /// Callers must ensure that this is okay to call in the current target for
    /// the current CPU.
    #[inline(always)]
    unsafe fn build<V: Vector>(&self) -> Mask<V> {
        assert!(V::BYTES <= self.lo.len());
        assert!(V::BYTES <= self.hi.len());
        Mask {
            lo: V::load_unaligned(self.lo[..].as_ptr()),
            hi: V::load_unaligned(self.hi[..].as_ptr()),
        }
    }

    /// A convenience function for building `N` vector masks from a fat
    /// `Teddy` value.
    ///
    /// # Panics
    ///
    /// When `V` represents a vector bigger than what `MaskBytes` can contain.
    ///
    /// # Safety
    ///
    /// Callers must ensure that this is okay to call in the current target for
    /// the current CPU.
    #[inline(always)]
    unsafe fn from_teddy<const BYTES: usize, V: Vector>(
        teddy: &Teddy<16>,
    ) -> [Mask<V>; BYTES] {
        // MSRV(1.63): Use core::array::from_fn to just build the array here
        // instead of creating a vector and turning it into an array.
        let mut mask_builders = vec![FatMaskBuilder::default(); BYTES];
        for (bucket_index, bucket) in teddy.buckets.iter().enumerate() {
            for pid in bucket.iter().copied() {
                let pat = teddy.patterns.get(pid);
                for (i, builder) in mask_builders.iter_mut().enumerate() {
                    builder.add(bucket_index, pat.bytes()[i]);
                }
            }
        }
        let array =
            <[FatMaskBuilder; BYTES]>::try_from(mask_builders).unwrap();
        array.map(|builder| builder.build())
    }
}

impl Debug for FatMaskBuilder {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let (mut parts_lo, mut parts_hi) = (vec![], vec![]);
        for i in 0..32 {
            parts_lo.push(format!("{:02}: {:08b}", i, self.lo[i]));
            parts_hi.push(format!("{:02}: {:08b}", i, self.hi[i]));
        }
        f.debug_struct("FatMaskBuilder")
            .field("lo", &parts_lo)
            .field("hi", &parts_hi)
            .finish()
    }
}
