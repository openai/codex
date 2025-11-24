/*!
Generic crate-internal routines for the `memchr` family of functions.
*/

// What follows is a vector algorithm generic over the specific vector
// type to detect the position of one, two or three needles in a haystack.
// From what I know, this is a "classic" algorithm, although I don't
// believe it has been published in any peer reviewed journal. I believe
// it can be found in places like glibc and Go's standard library. It
// appears to be well known and is elaborated on in more detail here:
// https://gms.tf/stdfind-and-memchr-optimizations.html
//
// While the routine below is fairly long and perhaps intimidating, the basic
// idea is actually very simple and can be expressed straight-forwardly in
// pseudo code. The pseudo code below is written for 128 bit vectors, but the
// actual code below works for anything that implements the Vector trait.
//
//     needle = (n1 << 15) | (n1 << 14) | ... | (n1 << 1) | n1
//     // Note: shift amount is in bytes
//
//     while i <= haystack.len() - 16:
//       // A 16 byte vector. Each byte in chunk corresponds to a byte in
//       // the haystack.
//       chunk = haystack[i:i+16]
//       // Compare bytes in needle with bytes in chunk. The result is a 16
//       // byte chunk where each byte is 0xFF if the corresponding bytes
//       // in needle and chunk were equal, or 0x00 otherwise.
//       eqs = cmpeq(needle, chunk)
//       // Return a 32 bit integer where the most significant 16 bits
//       // are always 0 and the lower 16 bits correspond to whether the
//       // most significant bit in the correspond byte in `eqs` is set.
//       // In other words, `mask as u16` has bit i set if and only if
//       // needle[i] == chunk[i].
//       mask = movemask(eqs)
//
//       // Mask is 0 if there is no match, and non-zero otherwise.
//       if mask != 0:
//         // trailing_zeros tells us the position of the least significant
//         // bit that is set.
//         return i + trailing_zeros(mask)
//
//     // haystack length may not be a multiple of 16, so search the rest.
//     while i < haystack.len():
//       if haystack[i] == n1:
//         return i
//
//     // No match found.
//     return NULL
//
// In fact, we could loosely translate the above code to Rust line-for-line
// and it would be a pretty fast algorithm. But, we pull out all the stops
// to go as fast as possible:
//
// 1. We use aligned loads. That is, we do some finagling to make sure our
//    primary loop not only proceeds in increments of 16 bytes, but that
//    the address of haystack's pointer that we dereference is aligned to
//    16 bytes. 16 is a magic number here because it is the size of SSE2
//    128-bit vector. (For the AVX2 algorithm, 32 is the magic number.)
//    Therefore, to get aligned loads, our pointer's address must be evenly
//    divisible by 16.
// 2. Our primary loop proceeds 64 bytes at a time instead of 16. It's
//    kind of like loop unrolling, but we combine the equality comparisons
//    using a vector OR such that we only need to extract a single mask to
//    determine whether a match exists or not. If so, then we do some
//    book-keeping to determine the precise location but otherwise mush on.
// 3. We use our "chunk" comparison routine in as many places as possible,
//    even if it means using unaligned loads. In particular, if haystack
//    starts with an unaligned address, then we do an unaligned load to
//    search the first 16 bytes. We then start our primary loop at the
//    smallest subsequent aligned address, which will actually overlap with
//    previously searched bytes. But we're OK with that. We do a similar
//    dance at the end of our primary loop. Finally, to avoid a
//    byte-at-a-time loop at the end, we do a final 16 byte unaligned load
//    that may overlap with a previous load. This is OK because it converts
//    a loop into a small number of very fast vector instructions. The overlap
//    is OK because we know the place where the overlap occurs does not
//    contain a match.
//
// And that's pretty all there is to it. Note that since the below is
// generic and since it's meant to be inlined into routines with a
// `#[target_feature(enable = "...")]` annotation, we must mark all routines as
// both unsafe and `#[inline(always)]`.
//
// The fact that the code below is generic does somewhat inhibit us. For
// example, I've noticed that introducing an unlineable `#[cold]` function to
// handle the match case in the loop generates tighter assembly, but there is
// no way to do this in the generic code below because the generic code doesn't
// know what `target_feature` annotation to apply to the unlineable function.
// We could make such functions part of the `Vector` trait, but we instead live
// with the slightly sub-optimal codegen for now since it doesn't seem to have
// a noticeable perf difference.

use crate::{
    ext::Pointer,
    vector::{MoveMask, Vector},
};

/// Finds all occurrences of a single byte in a haystack.
#[derive(Clone, Copy, Debug)]
pub(crate) struct One<V> {
    s1: u8,
    v1: V,
}

impl<V: Vector> One<V> {
    /// The number of bytes we examine per each iteration of our search loop.
    const LOOP_SIZE: usize = 4 * V::BYTES;

    /// Create a new searcher that finds occurrences of the byte given.
    #[inline(always)]
    pub(crate) unsafe fn new(needle: u8) -> One<V> {
        One { s1: needle, v1: V::splat(needle) }
    }

    /// Returns the needle given to `One::new`.
    #[inline(always)]
    pub(crate) fn needle1(&self) -> u8 {
        self.s1
    }

    /// Return a pointer to the first occurrence of the needle in the given
    /// haystack. If no such occurrence exists, then `None` is returned.
    ///
    /// When a match is found, the pointer returned is guaranteed to be
    /// `>= start` and `< end`.
    ///
    /// # Safety
    ///
    /// * It must be the case that `start < end` and that the distance between
    /// them is at least equal to `V::BYTES`. That is, it must always be valid
    /// to do at least an unaligned load of `V` at `start`.
    /// * Both `start` and `end` must be valid for reads.
    /// * Both `start` and `end` must point to an initialized value.
    /// * Both `start` and `end` must point to the same allocated object and
    /// must either be in bounds or at most one byte past the end of the
    /// allocated object.
    /// * Both `start` and `end` must be _derived from_ a pointer to the same
    /// object.
    /// * The distance between `start` and `end` must not overflow `isize`.
    /// * The distance being in bounds must not rely on "wrapping around" the
    /// address space.
    #[inline(always)]
    pub(crate) unsafe fn find_raw(
        &self,
        start: *const u8,
        end: *const u8,
    ) -> Option<*const u8> {
        // If we want to support vectors bigger than 256 bits, we probably
        // need to move up to using a u64 for the masks used below. Currently
        // they are 32 bits, which means we're SOL for vectors that need masks
        // bigger than 32 bits. Overall unclear until there's a use case.
        debug_assert!(V::BYTES <= 32, "vector cannot be bigger than 32 bytes");

        let topos = V::Mask::first_offset;
        let len = end.distance(start);
        debug_assert!(
            len >= V::BYTES,
            "haystack has length {}, but must be at least {}",
            len,
            V::BYTES
        );

        // Search a possibly unaligned chunk at `start`. This covers any part
        // of the haystack prior to where aligned loads can start.
        if let Some(cur) = self.search_chunk(start, topos) {
            return Some(cur);
        }
        // Set `cur` to the first V-aligned pointer greater than `start`.
        let mut cur = start.add(V::BYTES - (start.as_usize() & V::ALIGN));
        debug_assert!(cur > start && end.sub(V::BYTES) >= start);
        if len >= Self::LOOP_SIZE {
            while cur <= end.sub(Self::LOOP_SIZE) {
                debug_assert_eq!(0, cur.as_usize() % V::BYTES);

                let a = V::load_aligned(cur);
                let b = V::load_aligned(cur.add(1 * V::BYTES));
                let c = V::load_aligned(cur.add(2 * V::BYTES));
                let d = V::load_aligned(cur.add(3 * V::BYTES));
                let eqa = self.v1.cmpeq(a);
                let eqb = self.v1.cmpeq(b);
                let eqc = self.v1.cmpeq(c);
                let eqd = self.v1.cmpeq(d);
                let or1 = eqa.or(eqb);
                let or2 = eqc.or(eqd);
                let or3 = or1.or(or2);
                if or3.movemask_will_have_non_zero() {
                    let mask = eqa.movemask();
                    if mask.has_non_zero() {
                        return Some(cur.add(topos(mask)));
                    }

                    let mask = eqb.movemask();
                    if mask.has_non_zero() {
                        return Some(cur.add(1 * V::BYTES).add(topos(mask)));
                    }

                    let mask = eqc.movemask();
                    if mask.has_non_zero() {
                        return Some(cur.add(2 * V::BYTES).add(topos(mask)));
                    }

                    let mask = eqd.movemask();
                    debug_assert!(mask.has_non_zero());
                    return Some(cur.add(3 * V::BYTES).add(topos(mask)));
                }
                cur = cur.add(Self::LOOP_SIZE);
            }
        }
        // Handle any leftovers after the aligned loop above. We use unaligned
        // loads here, but I believe we are guaranteed that they are aligned
        // since `cur` is aligned.
        while cur <= end.sub(V::BYTES) {
            debug_assert!(end.distance(cur) >= V::BYTES);
            if let Some(cur) = self.search_chunk(cur, topos) {
                return Some(cur);
            }
            cur = cur.add(V::BYTES);
        }
        // Finally handle any remaining bytes less than the size of V. In this
        // case, our pointer may indeed be unaligned and the load may overlap
        // with the previous one. But that's okay since we know the previous
        // load didn't lead to a match (otherwise we wouldn't be here).
        if cur < end {
            debug_assert!(end.distance(cur) < V::BYTES);
            cur = cur.sub(V::BYTES - end.distance(cur));
            debug_assert_eq!(end.distance(cur), V::BYTES);
            return self.search_chunk(cur, topos);
        }
        None
    }

    /// Return a pointer to the last occurrence of the needle in the given
    /// haystack. If no such occurrence exists, then `None` is returned.
    ///
    /// When a match is found, the pointer returned is guaranteed to be
    /// `>= start` and `< end`.
    ///
    /// # Safety
    ///
    /// * It must be the case that `start < end` and that the distance between
    /// them is at least equal to `V::BYTES`. That is, it must always be valid
    /// to do at least an unaligned load of `V` at `start`.
    /// * Both `start` and `end` must be valid for reads.
    /// * Both `start` and `end` must point to an initialized value.
    /// * Both `start` and `end` must point to the same allocated object and
    /// must either be in bounds or at most one byte past the end of the
    /// allocated object.
    /// * Both `start` and `end` must be _derived from_ a pointer to the same
    /// object.
    /// * The distance between `start` and `end` must not overflow `isize`.
    /// * The distance being in bounds must not rely on "wrapping around" the
    /// address space.
    #[inline(always)]
    pub(crate) unsafe fn rfind_raw(
        &self,
        start: *const u8,
        end: *const u8,
    ) -> Option<*const u8> {
        // If we want to support vectors bigger than 256 bits, we probably
        // need to move up to using a u64 for the masks used below. Currently
        // they are 32 bits, which means we're SOL for vectors that need masks
        // bigger than 32 bits. Overall unclear until there's a use case.
        debug_assert!(V::BYTES <= 32, "vector cannot be bigger than 32 bytes");

        let topos = V::Mask::last_offset;
        let len = end.distance(start);
        debug_assert!(
            len >= V::BYTES,
            "haystack has length {}, but must be at least {}",
            len,
            V::BYTES
        );

        if let Some(cur) = self.search_chunk(end.sub(V::BYTES), topos) {
            return Some(cur);
        }
        let mut cur = end.sub(end.as_usize() & V::ALIGN);
        debug_assert!(start <= cur && cur <= end);
        if len >= Self::LOOP_SIZE {
            while cur >= start.add(Self::LOOP_SIZE) {
                debug_assert_eq!(0, cur.as_usize() % V::BYTES);

                cur = cur.sub(Self::LOOP_SIZE);
                let a = V::load_aligned(cur);
                let b = V::load_aligned(cur.add(1 * V::BYTES));
                let c = V::load_aligned(cur.add(2 * V::BYTES));
                let d = V::load_aligned(cur.add(3 * V::BYTES));
                let eqa = self.v1.cmpeq(a);
                let eqb = self.v1.cmpeq(b);
                let eqc = self.v1.cmpeq(c);
                let eqd = self.v1.cmpeq(d);
                let or1 = eqa.or(eqb);
                let or2 = eqc.or(eqd);
                let or3 = or1.or(or2);
                if or3.movemask_will_have_non_zero() {
                    let mask = eqd.movemask();
                    if mask.has_non_zero() {
                        return Some(cur.add(3 * V::BYTES).add(topos(mask)));
                    }

                    let mask = eqc.movemask();
                    if mask.has_non_zero() {
                        return Some(cur.add(2 * V::BYTES).add(topos(mask)));
                    }

                    let mask = eqb.movemask();
                    if mask.has_non_zero() {
                        return Some(cur.add(1 * V::BYTES).add(topos(mask)));
                    }

                    let mask = eqa.movemask();
                    debug_assert!(mask.has_non_zero());
                    return Some(cur.add(topos(mask)));
                }
            }
        }
        while cur >= start.add(V::BYTES) {
            debug_assert!(cur.distance(start) >= V::BYTES);
            cur = cur.sub(V::BYTES);
            if let Some(cur) = self.search_chunk(cur, topos) {
                return Some(cur);
            }
        }
        if cur > start {
            debug_assert!(cur.distance(start) < V::BYTES);
            return self.search_chunk(start, topos);
        }
        None
    }

    /// Return a count of all matching bytes in the given haystack.
    ///
    /// # Safety
    ///
    /// * It must be the case that `start < end` and that the distance between
    /// them is at least equal to `V::BYTES`. That is, it must always be valid
    /// to do at least an unaligned load of `V` at `start`.
    /// * Both `start` and `end` must be valid for reads.
    /// * Both `start` and `end` must point to an initialized value.
    /// * Both `start` and `end` must point to the same allocated object and
    /// must either be in bounds or at most one byte past the end of the
    /// allocated object.
    /// * Both `start` and `end` must be _derived from_ a pointer to the same
    /// object.
    /// * The distance between `start` and `end` must not overflow `isize`.
    /// * The distance being in bounds must not rely on "wrapping around" the
    /// address space.
    #[inline(always)]
    pub(crate) unsafe fn count_raw(
        &self,
        start: *const u8,
        end: *const u8,
    ) -> usize {
        debug_assert!(V::BYTES <= 32, "vector cannot be bigger than 32 bytes");

        let confirm = |b| b == self.needle1();
        let len = end.distance(start);
        debug_assert!(
            len >= V::BYTES,
            "haystack has length {}, but must be at least {}",
            len,
            V::BYTES
        );

        // Set `cur` to the first V-aligned pointer greater than `start`.
        let mut cur = start.add(V::BYTES - (start.as_usize() & V::ALIGN));
        // Count any matching bytes before we start our aligned loop.
        let mut count = count_byte_by_byte(start, cur, confirm);
        debug_assert!(cur > start && end.sub(V::BYTES) >= start);
        if len >= Self::LOOP_SIZE {
            while cur <= end.sub(Self::LOOP_SIZE) {
                debug_assert_eq!(0, cur.as_usize() % V::BYTES);

                let a = V::load_aligned(cur);
                let b = V::load_aligned(cur.add(1 * V::BYTES));
                let c = V::load_aligned(cur.add(2 * V::BYTES));
                let d = V::load_aligned(cur.add(3 * V::BYTES));
                let eqa = self.v1.cmpeq(a);
                let eqb = self.v1.cmpeq(b);
                let eqc = self.v1.cmpeq(c);
                let eqd = self.v1.cmpeq(d);
                count += eqa.movemask().count_ones();
                count += eqb.movemask().count_ones();
                count += eqc.movemask().count_ones();
                count += eqd.movemask().count_ones();
                cur = cur.add(Self::LOOP_SIZE);
            }
        }
        // Handle any leftovers after the aligned loop above. We use unaligned
        // loads here, but I believe we are guaranteed that they are aligned
        // since `cur` is aligned.
        while cur <= end.sub(V::BYTES) {
            debug_assert!(end.distance(cur) >= V::BYTES);
            let chunk = V::load_unaligned(cur);
            count += self.v1.cmpeq(chunk).movemask().count_ones();
            cur = cur.add(V::BYTES);
        }
        // And finally count any leftovers that weren't caught above.
        count += count_byte_by_byte(cur, end, confirm);
        count
    }

    /// Search `V::BYTES` starting at `cur` via an unaligned load.
    ///
    /// `mask_to_offset` should be a function that converts a `movemask` to
    /// an offset such that `cur.add(offset)` corresponds to a pointer to the
    /// match location if one is found. Generally it is expected to use either
    /// `mask_to_first_offset` or `mask_to_last_offset`, depending on whether
    /// one is implementing a forward or reverse search, respectively.
    ///
    /// # Safety
    ///
    /// `cur` must be a valid pointer and it must be valid to do an unaligned
    /// load of size `V::BYTES` at `cur`.
    #[inline(always)]
    unsafe fn search_chunk(
        &self,
        cur: *const u8,
        mask_to_offset: impl Fn(V::Mask) -> usize,
    ) -> Option<*const u8> {
        let chunk = V::load_unaligned(cur);
        let mask = self.v1.cmpeq(chunk).movemask();
        if mask.has_non_zero() {
            Some(cur.add(mask_to_offset(mask)))
        } else {
            None
        }
    }
}

/// Finds all occurrences of two bytes in a haystack.
///
/// That is, this reports matches of one of two possible bytes. For example,
/// searching for `a` or `b` in `afoobar` would report matches at offsets `0`,
/// `4` and `5`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Two<V> {
    s1: u8,
    s2: u8,
    v1: V,
    v2: V,
}

impl<V: Vector> Two<V> {
    /// The number of bytes we examine per each iteration of our search loop.
    const LOOP_SIZE: usize = 2 * V::BYTES;

    /// Create a new searcher that finds occurrences of the byte given.
    #[inline(always)]
    pub(crate) unsafe fn new(needle1: u8, needle2: u8) -> Two<V> {
        Two {
            s1: needle1,
            s2: needle2,
            v1: V::splat(needle1),
            v2: V::splat(needle2),
        }
    }

    /// Returns the first needle given to `Two::new`.
    #[inline(always)]
    pub(crate) fn needle1(&self) -> u8 {
        self.s1
    }

    /// Returns the second needle given to `Two::new`.
    #[inline(always)]
    pub(crate) fn needle2(&self) -> u8 {
        self.s2
    }

    /// Return a pointer to the first occurrence of one of the needles in the
    /// given haystack. If no such occurrence exists, then `None` is returned.
    ///
    /// When a match is found, the pointer returned is guaranteed to be
    /// `>= start` and `< end`.
    ///
    /// # Safety
    ///
    /// * It must be the case that `start < end` and that the distance between
    /// them is at least equal to `V::BYTES`. That is, it must always be valid
    /// to do at least an unaligned load of `V` at `start`.
    /// * Both `start` and `end` must be valid for reads.
    /// * Both `start` and `end` must point to an initialized value.
    /// * Both `start` and `end` must point to the same allocated object and
    /// must either be in bounds or at most one byte past the end of the
    /// allocated object.
    /// * Both `start` and `end` must be _derived from_ a pointer to the same
    /// object.
    /// * The distance between `start` and `end` must not overflow `isize`.
    /// * The distance being in bounds must not rely on "wrapping around" the
    /// address space.
    #[inline(always)]
    pub(crate) unsafe fn find_raw(
        &self,
        start: *const u8,
        end: *const u8,
    ) -> Option<*const u8> {
        // If we want to support vectors bigger than 256 bits, we probably
        // need to move up to using a u64 for the masks used below. Currently
        // they are 32 bits, which means we're SOL for vectors that need masks
        // bigger than 32 bits. Overall unclear until there's a use case.
        debug_assert!(V::BYTES <= 32, "vector cannot be bigger than 32 bytes");

        let topos = V::Mask::first_offset;
        let len = end.distance(start);
        debug_assert!(
            len >= V::BYTES,
            "haystack has length {}, but must be at least {}",
            len,
            V::BYTES
        );

        // Search a possibly unaligned chunk at `start`. This covers any part
        // of the haystack prior to where aligned loads can start.
        if let Some(cur) = self.search_chunk(start, topos) {
            return Some(cur);
        }
        // Set `cur` to the first V-aligned pointer greater than `start`.
        let mut cur = start.add(V::BYTES - (start.as_usize() & V::ALIGN));
        debug_assert!(cur > start && end.sub(V::BYTES) >= start);
        if len >= Self::LOOP_SIZE {
            while cur <= end.sub(Self::LOOP_SIZE) {
                debug_assert_eq!(0, cur.as_usize() % V::BYTES);

                let a = V::load_aligned(cur);
                let b = V::load_aligned(cur.add(V::BYTES));
                let eqa1 = self.v1.cmpeq(a);
                let eqb1 = self.v1.cmpeq(b);
                let eqa2 = self.v2.cmpeq(a);
                let eqb2 = self.v2.cmpeq(b);
                let or1 = eqa1.or(eqb1);
                let or2 = eqa2.or(eqb2);
                let or3 = or1.or(or2);
                if or3.movemask_will_have_non_zero() {
                    let mask = eqa1.movemask().or(eqa2.movemask());
                    if mask.has_non_zero() {
                        return Some(cur.add(topos(mask)));
                    }

                    let mask = eqb1.movemask().or(eqb2.movemask());
                    debug_assert!(mask.has_non_zero());
                    return Some(cur.add(V::BYTES).add(topos(mask)));
                }
                cur = cur.add(Self::LOOP_SIZE);
            }
        }
        // Handle any leftovers after the aligned loop above. We use unaligned
        // loads here, but I believe we are guaranteed that they are aligned
        // since `cur` is aligned.
        while cur <= end.sub(V::BYTES) {
            debug_assert!(end.distance(cur) >= V::BYTES);
            if let Some(cur) = self.search_chunk(cur, topos) {
                return Some(cur);
            }
            cur = cur.add(V::BYTES);
        }
        // Finally handle any remaining bytes less than the size of V. In this
        // case, our pointer may indeed be unaligned and the load may overlap
        // with the previous one. But that's okay since we know the previous
        // load didn't lead to a match (otherwise we wouldn't be here).
        if cur < end {
            debug_assert!(end.distance(cur) < V::BYTES);
            cur = cur.sub(V::BYTES - end.distance(cur));
            debug_assert_eq!(end.distance(cur), V::BYTES);
            return self.search_chunk(cur, topos);
        }
        None
    }

    /// Return a pointer to the last occurrence of the needle in the given
    /// haystack. If no such occurrence exists, then `None` is returned.
    ///
    /// When a match is found, the pointer returned is guaranteed to be
    /// `>= start` and `< end`.
    ///
    /// # Safety
    ///
    /// * It must be the case that `start < end` and that the distance between
    /// them is at least equal to `V::BYTES`. That is, it must always be valid
    /// to do at least an unaligned load of `V` at `start`.
    /// * Both `start` and `end` must be valid for reads.
    /// * Both `start` and `end` must point to an initialized value.
    /// * Both `start` and `end` must point to the same allocated object and
    /// must either be in bounds or at most one byte past the end of the
    /// allocated object.
    /// * Both `start` and `end` must be _derived from_ a pointer to the same
    /// object.
    /// * The distance between `start` and `end` must not overflow `isize`.
    /// * The distance being in bounds must not rely on "wrapping around" the
    /// address space.
    #[inline(always)]
    pub(crate) unsafe fn rfind_raw(
        &self,
        start: *const u8,
        end: *const u8,
    ) -> Option<*const u8> {
        // If we want to support vectors bigger than 256 bits, we probably
        // need to move up to using a u64 for the masks used below. Currently
        // they are 32 bits, which means we're SOL for vectors that need masks
        // bigger than 32 bits. Overall unclear until there's a use case.
        debug_assert!(V::BYTES <= 32, "vector cannot be bigger than 32 bytes");

        let topos = V::Mask::last_offset;
        let len = end.distance(start);
        debug_assert!(
            len >= V::BYTES,
            "haystack has length {}, but must be at least {}",
            len,
            V::BYTES
        );

        if let Some(cur) = self.search_chunk(end.sub(V::BYTES), topos) {
            return Some(cur);
        }
        let mut cur = end.sub(end.as_usize() & V::ALIGN);
        debug_assert!(start <= cur && cur <= end);
        if len >= Self::LOOP_SIZE {
            while cur >= start.add(Self::LOOP_SIZE) {
                debug_assert_eq!(0, cur.as_usize() % V::BYTES);

                cur = cur.sub(Self::LOOP_SIZE);
                let a = V::load_aligned(cur);
                let b = V::load_aligned(cur.add(V::BYTES));
                let eqa1 = self.v1.cmpeq(a);
                let eqb1 = self.v1.cmpeq(b);
                let eqa2 = self.v2.cmpeq(a);
                let eqb2 = self.v2.cmpeq(b);
                let or1 = eqa1.or(eqb1);
                let or2 = eqa2.or(eqb2);
                let or3 = or1.or(or2);
                if or3.movemask_will_have_non_zero() {
                    let mask = eqb1.movemask().or(eqb2.movemask());
                    if mask.has_non_zero() {
                        return Some(cur.add(V::BYTES).add(topos(mask)));
                    }

                    let mask = eqa1.movemask().or(eqa2.movemask());
                    debug_assert!(mask.has_non_zero());
                    return Some(cur.add(topos(mask)));
                }
            }
        }
        while cur >= start.add(V::BYTES) {
            debug_assert!(cur.distance(start) >= V::BYTES);
            cur = cur.sub(V::BYTES);
            if let Some(cur) = self.search_chunk(cur, topos) {
                return Some(cur);
            }
        }
        if cur > start {
            debug_assert!(cur.distance(start) < V::BYTES);
            return self.search_chunk(start, topos);
        }
        None
    }

    /// Search `V::BYTES` starting at `cur` via an unaligned load.
    ///
    /// `mask_to_offset` should be a function that converts a `movemask` to
    /// an offset such that `cur.add(offset)` corresponds to a pointer to the
    /// match location if one is found. Generally it is expected to use either
    /// `mask_to_first_offset` or `mask_to_last_offset`, depending on whether
    /// one is implementing a forward or reverse search, respectively.
    ///
    /// # Safety
    ///
    /// `cur` must be a valid pointer and it must be valid to do an unaligned
    /// load of size `V::BYTES` at `cur`.
    #[inline(always)]
    unsafe fn search_chunk(
        &self,
        cur: *const u8,
        mask_to_offset: impl Fn(V::Mask) -> usize,
    ) -> Option<*const u8> {
        let chunk = V::load_unaligned(cur);
        let eq1 = self.v1.cmpeq(chunk);
        let eq2 = self.v2.cmpeq(chunk);
        let mask = eq1.or(eq2).movemask();
        if mask.has_non_zero() {
            let mask1 = eq1.movemask();
            let mask2 = eq2.movemask();
            Some(cur.add(mask_to_offset(mask1.or(mask2))))
        } else {
            None
        }
    }
}

/// Finds all occurrences of two bytes in a haystack.
///
/// That is, this reports matches of one of two possible bytes. For example,
/// searching for `a` or `b` in `afoobar` would report matches at offsets `0`,
/// `4` and `5`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Three<V> {
    s1: u8,
    s2: u8,
    s3: u8,
    v1: V,
    v2: V,
    v3: V,
}

impl<V: Vector> Three<V> {
    /// The number of bytes we examine per each iteration of our search loop.
    const LOOP_SIZE: usize = 2 * V::BYTES;

    /// Create a new searcher that finds occurrences of the byte given.
    #[inline(always)]
    pub(crate) unsafe fn new(
        needle1: u8,
        needle2: u8,
        needle3: u8,
    ) -> Three<V> {
        Three {
            s1: needle1,
            s2: needle2,
            s3: needle3,
            v1: V::splat(needle1),
            v2: V::splat(needle2),
            v3: V::splat(needle3),
        }
    }

    /// Returns the first needle given to `Three::new`.
    #[inline(always)]
    pub(crate) fn needle1(&self) -> u8 {
        self.s1
    }

    /// Returns the second needle given to `Three::new`.
    #[inline(always)]
    pub(crate) fn needle2(&self) -> u8 {
        self.s2
    }

    /// Returns the third needle given to `Three::new`.
    #[inline(always)]
    pub(crate) fn needle3(&self) -> u8 {
        self.s3
    }

    /// Return a pointer to the first occurrence of one of the needles in the
    /// given haystack. If no such occurrence exists, then `None` is returned.
    ///
    /// When a match is found, the pointer returned is guaranteed to be
    /// `>= start` and `< end`.
    ///
    /// # Safety
    ///
    /// * It must be the case that `start < end` and that the distance between
    /// them is at least equal to `V::BYTES`. That is, it must always be valid
    /// to do at least an unaligned load of `V` at `start`.
    /// * Both `start` and `end` must be valid for reads.
    /// * Both `start` and `end` must point to an initialized value.
    /// * Both `start` and `end` must point to the same allocated object and
    /// must either be in bounds or at most one byte past the end of the
    /// allocated object.
    /// * Both `start` and `end` must be _derived from_ a pointer to the same
    /// object.
    /// * The distance between `start` and `end` must not overflow `isize`.
    /// * The distance being in bounds must not rely on "wrapping around" the
    /// address space.
    #[inline(always)]
    pub(crate) unsafe fn find_raw(
        &self,
        start: *const u8,
        end: *const u8,
    ) -> Option<*const u8> {
        // If we want to support vectors bigger than 256 bits, we probably
        // need to move up to using a u64 for the masks used below. Currently
        // they are 32 bits, which means we're SOL for vectors that need masks
        // bigger than 32 bits. Overall unclear until there's a use case.
        debug_assert!(V::BYTES <= 32, "vector cannot be bigger than 32 bytes");

        let topos = V::Mask::first_offset;
        let len = end.distance(start);
        debug_assert!(
            len >= V::BYTES,
            "haystack has length {}, but must be at least {}",
            len,
            V::BYTES
        );

        // Search a possibly unaligned chunk at `start`. This covers any part
        // of the haystack prior to where aligned loads can start.
        if let Some(cur) = self.search_chunk(start, topos) {
            return Some(cur);
        }
        // Set `cur` to the first V-aligned pointer greater than `start`.
        let mut cur = start.add(V::BYTES - (start.as_usize() & V::ALIGN));
        debug_assert!(cur > start && end.sub(V::BYTES) >= start);
        if len >= Self::LOOP_SIZE {
            while cur <= end.sub(Self::LOOP_SIZE) {
                debug_assert_eq!(0, cur.as_usize() % V::BYTES);

                let a = V::load_aligned(cur);
                let b = V::load_aligned(cur.add(V::BYTES));
                let eqa1 = self.v1.cmpeq(a);
                let eqb1 = self.v1.cmpeq(b);
                let eqa2 = self.v2.cmpeq(a);
                let eqb2 = self.v2.cmpeq(b);
                let eqa3 = self.v3.cmpeq(a);
                let eqb3 = self.v3.cmpeq(b);
                let or1 = eqa1.or(eqb1);
                let or2 = eqa2.or(eqb2);
                let or3 = eqa3.or(eqb3);
                let or4 = or1.or(or2);
                let or5 = or3.or(or4);
                if or5.movemask_will_have_non_zero() {
                    let mask = eqa1
                        .movemask()
                        .or(eqa2.movemask())
                        .or(eqa3.movemask());
                    if mask.has_non_zero() {
                        return Some(cur.add(topos(mask)));
                    }

                    let mask = eqb1
                        .movemask()
                        .or(eqb2.movemask())
                        .or(eqb3.movemask());
                    debug_assert!(mask.has_non_zero());
                    return Some(cur.add(V::BYTES).add(topos(mask)));
                }
                cur = cur.add(Self::LOOP_SIZE);
            }
        }
        // Handle any leftovers after the aligned loop above. We use unaligned
        // loads here, but I believe we are guaranteed that they are aligned
        // since `cur` is aligned.
        while cur <= end.sub(V::BYTES) {
            debug_assert!(end.distance(cur) >= V::BYTES);
            if let Some(cur) = self.search_chunk(cur, topos) {
                return Some(cur);
            }
            cur = cur.add(V::BYTES);
        }
        // Finally handle any remaining bytes less than the size of V. In this
        // case, our pointer may indeed be unaligned and the load may overlap
        // with the previous one. But that's okay since we know the previous
        // load didn't lead to a match (otherwise we wouldn't be here).
        if cur < end {
            debug_assert!(end.distance(cur) < V::BYTES);
            cur = cur.sub(V::BYTES - end.distance(cur));
            debug_assert_eq!(end.distance(cur), V::BYTES);
            return self.search_chunk(cur, topos);
        }
        None
    }

    /// Return a pointer to the last occurrence of the needle in the given
    /// haystack. If no such occurrence exists, then `None` is returned.
    ///
    /// When a match is found, the pointer returned is guaranteed to be
    /// `>= start` and `< end`.
    ///
    /// # Safety
    ///
    /// * It must be the case that `start < end` and that the distance between
    /// them is at least equal to `V::BYTES`. That is, it must always be valid
    /// to do at least an unaligned load of `V` at `start`.
    /// * Both `start` and `end` must be valid for reads.
    /// * Both `start` and `end` must point to an initialized value.
    /// * Both `start` and `end` must point to the same allocated object and
    /// must either be in bounds or at most one byte past the end of the
    /// allocated object.
    /// * Both `start` and `end` must be _derived from_ a pointer to the same
    /// object.
    /// * The distance between `start` and `end` must not overflow `isize`.
    /// * The distance being in bounds must not rely on "wrapping around" the
    /// address space.
    #[inline(always)]
    pub(crate) unsafe fn rfind_raw(
        &self,
        start: *const u8,
        end: *const u8,
    ) -> Option<*const u8> {
        // If we want to support vectors bigger than 256 bits, we probably
        // need to move up to using a u64 for the masks used below. Currently
        // they are 32 bits, which means we're SOL for vectors that need masks
        // bigger than 32 bits. Overall unclear until there's a use case.
        debug_assert!(V::BYTES <= 32, "vector cannot be bigger than 32 bytes");

        let topos = V::Mask::last_offset;
        let len = end.distance(start);
        debug_assert!(
            len >= V::BYTES,
            "haystack has length {}, but must be at least {}",
            len,
            V::BYTES
        );

        if let Some(cur) = self.search_chunk(end.sub(V::BYTES), topos) {
            return Some(cur);
        }
        let mut cur = end.sub(end.as_usize() & V::ALIGN);
        debug_assert!(start <= cur && cur <= end);
        if len >= Self::LOOP_SIZE {
            while cur >= start.add(Self::LOOP_SIZE) {
                debug_assert_eq!(0, cur.as_usize() % V::BYTES);

                cur = cur.sub(Self::LOOP_SIZE);
                let a = V::load_aligned(cur);
                let b = V::load_aligned(cur.add(V::BYTES));
                let eqa1 = self.v1.cmpeq(a);
                let eqb1 = self.v1.cmpeq(b);
                let eqa2 = self.v2.cmpeq(a);
                let eqb2 = self.v2.cmpeq(b);
                let eqa3 = self.v3.cmpeq(a);
                let eqb3 = self.v3.cmpeq(b);
                let or1 = eqa1.or(eqb1);
                let or2 = eqa2.or(eqb2);
                let or3 = eqa3.or(eqb3);
                let or4 = or1.or(or2);
                let or5 = or3.or(or4);
                if or5.movemask_will_have_non_zero() {
                    let mask = eqb1
                        .movemask()
                        .or(eqb2.movemask())
                        .or(eqb3.movemask());
                    if mask.has_non_zero() {
                        return Some(cur.add(V::BYTES).add(topos(mask)));
                    }

                    let mask = eqa1
                        .movemask()
                        .or(eqa2.movemask())
                        .or(eqa3.movemask());
                    debug_assert!(mask.has_non_zero());
                    return Some(cur.add(topos(mask)));
                }
            }
        }
        while cur >= start.add(V::BYTES) {
            debug_assert!(cur.distance(start) >= V::BYTES);
            cur = cur.sub(V::BYTES);
            if let Some(cur) = self.search_chunk(cur, topos) {
                return Some(cur);
            }
        }
        if cur > start {
            debug_assert!(cur.distance(start) < V::BYTES);
            return self.search_chunk(start, topos);
        }
        None
    }

    /// Search `V::BYTES` starting at `cur` via an unaligned load.
    ///
    /// `mask_to_offset` should be a function that converts a `movemask` to
    /// an offset such that `cur.add(offset)` corresponds to a pointer to the
    /// match location if one is found. Generally it is expected to use either
    /// `mask_to_first_offset` or `mask_to_last_offset`, depending on whether
    /// one is implementing a forward or reverse search, respectively.
    ///
    /// # Safety
    ///
    /// `cur` must be a valid pointer and it must be valid to do an unaligned
    /// load of size `V::BYTES` at `cur`.
    #[inline(always)]
    unsafe fn search_chunk(
        &self,
        cur: *const u8,
        mask_to_offset: impl Fn(V::Mask) -> usize,
    ) -> Option<*const u8> {
        let chunk = V::load_unaligned(cur);
        let eq1 = self.v1.cmpeq(chunk);
        let eq2 = self.v2.cmpeq(chunk);
        let eq3 = self.v3.cmpeq(chunk);
        let mask = eq1.or(eq2).or(eq3).movemask();
        if mask.has_non_zero() {
            let mask1 = eq1.movemask();
            let mask2 = eq2.movemask();
            let mask3 = eq3.movemask();
            Some(cur.add(mask_to_offset(mask1.or(mask2).or(mask3))))
        } else {
            None
        }
    }
}

/// An iterator over all occurrences of a set of bytes in a haystack.
///
/// This iterator implements the routines necessary to provide a
/// `DoubleEndedIterator` impl, which means it can also be used to find
/// occurrences in reverse order.
///
/// The lifetime parameters are as follows:
///
/// * `'h` refers to the lifetime of the haystack being searched.
///
/// This type is intended to be used to implement all iterators for the
/// `memchr` family of functions. It handles a tiny bit of marginally tricky
/// raw pointer math, but otherwise expects the caller to provide `find_raw`
/// and `rfind_raw` routines for each call of `next` and `next_back`,
/// respectively.
#[derive(Clone, Debug)]
pub(crate) struct Iter<'h> {
    /// The original starting point into the haystack. We use this to convert
    /// pointers to offsets.
    original_start: *const u8,
    /// The current starting point into the haystack. That is, where the next
    /// search will begin.
    start: *const u8,
    /// The current ending point into the haystack. That is, where the next
    /// reverse search will begin.
    end: *const u8,
    /// A marker for tracking the lifetime of the start/cur_start/cur_end
    /// pointers above, which all point into the haystack.
    haystack: core::marker::PhantomData<&'h [u8]>,
}

// SAFETY: Iter contains no shared references to anything that performs any
// interior mutations. Also, the lifetime guarantees that Iter will not outlive
// the haystack.
unsafe impl<'h> Send for Iter<'h> {}

// SAFETY: Iter perform no interior mutations, therefore no explicit
// synchronization is necessary. Also, the lifetime guarantees that Iter will
// not outlive the haystack.
unsafe impl<'h> Sync for Iter<'h> {}

impl<'h> Iter<'h> {
    /// Create a new generic memchr iterator.
    #[inline(always)]
    pub(crate) fn new(haystack: &'h [u8]) -> Iter<'h> {
        Iter {
            original_start: haystack.as_ptr(),
            start: haystack.as_ptr(),
            end: haystack.as_ptr().wrapping_add(haystack.len()),
            haystack: core::marker::PhantomData,
        }
    }

    /// Returns the next occurrence in the forward direction.
    ///
    /// # Safety
    ///
    /// Callers must ensure that if a pointer is returned from the closure
    /// provided, then it must be greater than or equal to the start pointer
    /// and less than the end pointer.
    #[inline(always)]
    pub(crate) unsafe fn next(
        &mut self,
        mut find_raw: impl FnMut(*const u8, *const u8) -> Option<*const u8>,
    ) -> Option<usize> {
        // SAFETY: Pointers are derived directly from the same &[u8] haystack.
        // We only ever modify start/end corresponding to a matching offset
        // found between start and end. Thus all changes to start/end maintain
        // our safety requirements.
        //
        // The only other assumption we rely on is that the pointer returned
        // by `find_raw` satisfies `self.start <= found < self.end`, and that
        // safety contract is forwarded to the caller.
        let found = find_raw(self.start, self.end)?;
        let result = found.distance(self.original_start);
        self.start = found.add(1);
        Some(result)
    }

    /// Returns the number of remaining elements in this iterator.
    #[inline(always)]
    pub(crate) fn count(
        self,
        mut count_raw: impl FnMut(*const u8, *const u8) -> usize,
    ) -> usize {
        // SAFETY: Pointers are derived directly from the same &[u8] haystack.
        // We only ever modify start/end corresponding to a matching offset
        // found between start and end. Thus all changes to start/end maintain
        // our safety requirements.
        count_raw(self.start, self.end)
    }

    /// Returns the next occurrence in reverse.
    ///
    /// # Safety
    ///
    /// Callers must ensure that if a pointer is returned from the closure
    /// provided, then it must be greater than or equal to the start pointer
    /// and less than the end pointer.
    #[inline(always)]
    pub(crate) unsafe fn next_back(
        &mut self,
        mut rfind_raw: impl FnMut(*const u8, *const u8) -> Option<*const u8>,
    ) -> Option<usize> {
        // SAFETY: Pointers are derived directly from the same &[u8] haystack.
        // We only ever modify start/end corresponding to a matching offset
        // found between start and end. Thus all changes to start/end maintain
        // our safety requirements.
        //
        // The only other assumption we rely on is that the pointer returned
        // by `rfind_raw` satisfies `self.start <= found < self.end`, and that
        // safety contract is forwarded to the caller.
        let found = rfind_raw(self.start, self.end)?;
        let result = found.distance(self.original_start);
        self.end = found;
        Some(result)
    }

    /// Provides an implementation of `Iterator::size_hint`.
    #[inline(always)]
    pub(crate) fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.end.as_usize().saturating_sub(self.start.as_usize())))
    }
}

/// Search a slice using a function that operates on raw pointers.
///
/// Given a function to search a contiguous sequence of memory for the location
/// of a non-empty set of bytes, this will execute that search on a slice of
/// bytes. The pointer returned by the given function will be converted to an
/// offset relative to the starting point of the given slice. That is, if a
/// match is found, the offset returned by this routine is guaranteed to be a
/// valid index into `haystack`.
///
/// Callers may use this for a forward or reverse search.
///
/// # Safety
///
/// Callers must ensure that if a pointer is returned by `find_raw`, then the
/// pointer must be greater than or equal to the starting pointer and less than
/// the end pointer.
#[inline(always)]
pub(crate) unsafe fn search_slice_with_raw(
    haystack: &[u8],
    mut find_raw: impl FnMut(*const u8, *const u8) -> Option<*const u8>,
) -> Option<usize> {
    // SAFETY: We rely on `find_raw` to return a correct and valid pointer, but
    // otherwise, `start` and `end` are valid due to the guarantees provided by
    // a &[u8].
    let start = haystack.as_ptr();
    let end = start.add(haystack.len());
    let found = find_raw(start, end)?;
    Some(found.distance(start))
}

/// Performs a forward byte-at-a-time loop until either `ptr >= end_ptr` or
/// until `confirm(*ptr)` returns `true`. If the former occurs, then `None` is
/// returned. If the latter occurs, then the pointer at which `confirm` returns
/// `true` is returned.
///
/// # Safety
///
/// Callers must provide valid pointers and they must satisfy `start_ptr <=
/// ptr` and `ptr <= end_ptr`.
#[inline(always)]
pub(crate) unsafe fn fwd_byte_by_byte<F: Fn(u8) -> bool>(
    start: *const u8,
    end: *const u8,
    confirm: F,
) -> Option<*const u8> {
    debug_assert!(start <= end);
    let mut ptr = start;
    while ptr < end {
        if confirm(*ptr) {
            return Some(ptr);
        }
        ptr = ptr.offset(1);
    }
    None
}

/// Performs a reverse byte-at-a-time loop until either `ptr < start_ptr` or
/// until `confirm(*ptr)` returns `true`. If the former occurs, then `None` is
/// returned. If the latter occurs, then the pointer at which `confirm` returns
/// `true` is returned.
///
/// # Safety
///
/// Callers must provide valid pointers and they must satisfy `start_ptr <=
/// ptr` and `ptr <= end_ptr`.
#[inline(always)]
pub(crate) unsafe fn rev_byte_by_byte<F: Fn(u8) -> bool>(
    start: *const u8,
    end: *const u8,
    confirm: F,
) -> Option<*const u8> {
    debug_assert!(start <= end);

    let mut ptr = end;
    while ptr > start {
        ptr = ptr.offset(-1);
        if confirm(*ptr) {
            return Some(ptr);
        }
    }
    None
}

/// Performs a forward byte-at-a-time loop until `ptr >= end_ptr` and returns
/// the number of times `confirm(*ptr)` returns `true`.
///
/// # Safety
///
/// Callers must provide valid pointers and they must satisfy `start_ptr <=
/// ptr` and `ptr <= end_ptr`.
#[inline(always)]
pub(crate) unsafe fn count_byte_by_byte<F: Fn(u8) -> bool>(
    start: *const u8,
    end: *const u8,
    confirm: F,
) -> usize {
    debug_assert!(start <= end);
    let mut ptr = start;
    let mut count = 0;
    while ptr < end {
        if confirm(*ptr) {
            count += 1;
        }
        ptr = ptr.offset(1);
    }
    count
}
