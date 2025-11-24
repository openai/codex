use core::{cmp, fmt, mem, u16, usize};

use alloc::{boxed::Box, string::String, vec, vec::Vec};

use crate::{
    packed::{api::MatchKind, ext::Pointer},
    PatternID,
};

/// A non-empty collection of non-empty patterns to search for.
///
/// This collection of patterns is what is passed around to both execute
/// searches and to construct the searchers themselves. Namely, this permits
/// searches to avoid copying all of the patterns, and allows us to keep only
/// one copy throughout all packed searchers.
///
/// Note that this collection is not a set. The same pattern can appear more
/// than once.
#[derive(Clone, Debug)]
pub(crate) struct Patterns {
    /// The match semantics supported by this collection of patterns.
    ///
    /// The match semantics determines the order of the iterator over patterns.
    /// For leftmost-first, patterns are provided in the same order as were
    /// provided by the caller. For leftmost-longest, patterns are provided in
    /// descending order of length, with ties broken by the order in which they
    /// were provided by the caller.
    kind: MatchKind,
    /// The collection of patterns, indexed by their identifier.
    by_id: Vec<Vec<u8>>,
    /// The order of patterns defined for iteration, given by pattern
    /// identifiers. The order of `by_id` and `order` is always the same for
    /// leftmost-first semantics, but may be different for leftmost-longest
    /// semantics.
    order: Vec<PatternID>,
    /// The length of the smallest pattern, in bytes.
    minimum_len: usize,
    /// The total number of pattern bytes across the entire collection. This
    /// is used for reporting total heap usage in constant time.
    total_pattern_bytes: usize,
}

// BREADCRUMBS: I think we want to experiment with a different bucket
// representation. Basically, each bucket is just a Range<usize> to a single
// contiguous allocation? Maybe length-prefixed patterns or something? The
// idea is to try to get rid of the pointer chasing in verification. I don't
// know that that is the issue, but I suspect it is.

impl Patterns {
    /// Create a new collection of patterns for the given match semantics. The
    /// ID of each pattern is the index of the pattern at which it occurs in
    /// the `by_id` slice.
    ///
    /// If any of the patterns in the slice given are empty, then this panics.
    /// Similarly, if the number of patterns given is zero, then this also
    /// panics.
    pub(crate) fn new() -> Patterns {
        Patterns {
            kind: MatchKind::default(),
            by_id: vec![],
            order: vec![],
            minimum_len: usize::MAX,
            total_pattern_bytes: 0,
        }
    }

    /// Add a pattern to this collection.
    ///
    /// This panics if the pattern given is empty.
    pub(crate) fn add(&mut self, bytes: &[u8]) {
        assert!(!bytes.is_empty());
        assert!(self.by_id.len() <= u16::MAX as usize);

        let id = PatternID::new(self.by_id.len()).unwrap();
        self.order.push(id);
        self.by_id.push(bytes.to_vec());
        self.minimum_len = cmp::min(self.minimum_len, bytes.len());
        self.total_pattern_bytes += bytes.len();
    }

    /// Set the match kind semantics for this collection of patterns.
    ///
    /// If the kind is not set, then the default is leftmost-first.
    pub(crate) fn set_match_kind(&mut self, kind: MatchKind) {
        self.kind = kind;
        match self.kind {
            MatchKind::LeftmostFirst => {
                self.order.sort();
            }
            MatchKind::LeftmostLongest => {
                let (order, by_id) = (&mut self.order, &mut self.by_id);
                order.sort_by(|&id1, &id2| {
                    by_id[id1].len().cmp(&by_id[id2].len()).reverse()
                });
            }
        }
    }

    /// Return the number of patterns in this collection.
    ///
    /// This is guaranteed to be greater than zero.
    pub(crate) fn len(&self) -> usize {
        self.by_id.len()
    }

    /// Returns true if and only if this collection of patterns is empty.
    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the approximate total amount of heap used by these patterns, in
    /// units of bytes.
    pub(crate) fn memory_usage(&self) -> usize {
        self.order.len() * mem::size_of::<PatternID>()
            + self.by_id.len() * mem::size_of::<Vec<u8>>()
            + self.total_pattern_bytes
    }

    /// Clears all heap memory associated with this collection of patterns and
    /// resets all state such that it is a valid empty collection.
    pub(crate) fn reset(&mut self) {
        self.kind = MatchKind::default();
        self.by_id.clear();
        self.order.clear();
        self.minimum_len = usize::MAX;
    }

    /// Returns the length, in bytes, of the smallest pattern.
    ///
    /// This is guaranteed to be at least one.
    pub(crate) fn minimum_len(&self) -> usize {
        self.minimum_len
    }

    /// Returns the match semantics used by these patterns.
    pub(crate) fn match_kind(&self) -> &MatchKind {
        &self.kind
    }

    /// Return the pattern with the given identifier. If such a pattern does
    /// not exist, then this panics.
    pub(crate) fn get(&self, id: PatternID) -> Pattern<'_> {
        Pattern(&self.by_id[id])
    }

    /// Return the pattern with the given identifier without performing bounds
    /// checks.
    ///
    /// # Safety
    ///
    /// Callers must ensure that a pattern with the given identifier exists
    /// before using this method.
    pub(crate) unsafe fn get_unchecked(&self, id: PatternID) -> Pattern<'_> {
        Pattern(self.by_id.get_unchecked(id.as_usize()))
    }

    /// Return an iterator over all the patterns in this collection, in the
    /// order in which they should be matched.
    ///
    /// Specifically, in a naive multi-pattern matcher, the following is
    /// guaranteed to satisfy the match semantics of this collection of
    /// patterns:
    ///
    /// ```ignore
    /// for i in 0..haystack.len():
    ///   for p in patterns.iter():
    ///     if haystack[i..].starts_with(p.bytes()):
    ///       return Match(p.id(), i, i + p.bytes().len())
    /// ```
    ///
    /// Namely, among the patterns in a collection, if they are matched in
    /// the order provided by this iterator, then the result is guaranteed
    /// to satisfy the correct match semantics. (Either leftmost-first or
    /// leftmost-longest.)
    pub(crate) fn iter(&self) -> PatternIter<'_> {
        PatternIter { patterns: self, i: 0 }
    }
}

/// An iterator over the patterns in the `Patterns` collection.
///
/// The order of the patterns provided by this iterator is consistent with the
/// match semantics of the originating collection of patterns.
///
/// The lifetime `'p` corresponds to the lifetime of the collection of patterns
/// this is iterating over.
#[derive(Debug)]
pub(crate) struct PatternIter<'p> {
    patterns: &'p Patterns,
    i: usize,
}

impl<'p> Iterator for PatternIter<'p> {
    type Item = (PatternID, Pattern<'p>);

    fn next(&mut self) -> Option<(PatternID, Pattern<'p>)> {
        if self.i >= self.patterns.len() {
            return None;
        }
        let id = self.patterns.order[self.i];
        let p = self.patterns.get(id);
        self.i += 1;
        Some((id, p))
    }
}

/// A pattern that is used in packed searching.
#[derive(Clone)]
pub(crate) struct Pattern<'a>(&'a [u8]);

impl<'a> fmt::Debug for Pattern<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Pattern")
            .field("lit", &String::from_utf8_lossy(&self.0))
            .finish()
    }
}

impl<'p> Pattern<'p> {
    /// Returns the length of this pattern, in bytes.
    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns the bytes of this pattern.
    pub(crate) fn bytes(&self) -> &[u8] {
        &self.0
    }

    /// Returns the first `len` low nybbles from this pattern. If this pattern
    /// is shorter than `len`, then this panics.
    pub(crate) fn low_nybbles(&self, len: usize) -> Box<[u8]> {
        let mut nybs = vec![0; len].into_boxed_slice();
        for (i, byte) in self.bytes().iter().take(len).enumerate() {
            nybs[i] = byte & 0xF;
        }
        nybs
    }

    /// Returns true if this pattern is a prefix of the given bytes.
    #[inline(always)]
    pub(crate) fn is_prefix(&self, bytes: &[u8]) -> bool {
        is_prefix(bytes, self.bytes())
    }

    /// Returns true if this pattern is a prefix of the haystack given by the
    /// raw `start` and `end` pointers.
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
    pub(crate) unsafe fn is_prefix_raw(
        &self,
        start: *const u8,
        end: *const u8,
    ) -> bool {
        let patlen = self.bytes().len();
        let haylen = end.distance(start);
        if patlen > haylen {
            return false;
        }
        // SAFETY: We've checked that the haystack has length at least equal
        // to this pattern. All other safety concerns are the responsibility
        // of the caller.
        is_equal_raw(start, self.bytes().as_ptr(), patlen)
    }
}

/// Returns true if and only if `needle` is a prefix of `haystack`.
///
/// This uses a latency optimized variant of `memcmp` internally which *might*
/// make this faster for very short strings.
///
/// # Inlining
///
/// This routine is marked `inline(always)`. If you want to call this function
/// in a way that is not always inlined, you'll need to wrap a call to it in
/// another function that is marked as `inline(never)` or just `inline`.
#[inline(always)]
fn is_prefix(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.len() > haystack.len() {
        return false;
    }
    // SAFETY: Our pointers are derived directly from borrowed slices which
    // uphold all of our safety guarantees except for length. We account for
    // length with the check above.
    unsafe { is_equal_raw(haystack.as_ptr(), needle.as_ptr(), needle.len()) }
}

/// Compare corresponding bytes in `x` and `y` for equality.
///
/// That is, this returns true if and only if `x.len() == y.len()` and
/// `x[i] == y[i]` for all `0 <= i < x.len()`.
///
/// Note that this isn't used. We only use it in tests as a convenient way
/// of testing `is_equal_raw`.
///
/// # Inlining
///
/// This routine is marked `inline(always)`. If you want to call this function
/// in a way that is not always inlined, you'll need to wrap a call to it in
/// another function that is marked as `inline(never)` or just `inline`.
///
/// # Motivation
///
/// Why not use slice equality instead? Well, slice equality usually results in
/// a call out to the current platform's `libc` which might not be inlineable
/// or have other overhead. This routine isn't guaranteed to be a win, but it
/// might be in some cases.
#[cfg(test)]
#[inline(always)]
fn is_equal(x: &[u8], y: &[u8]) -> bool {
    if x.len() != y.len() {
        return false;
    }
    // SAFETY: Our pointers are derived directly from borrowed slices which
    // uphold all of our safety guarantees except for length. We account for
    // length with the check above.
    unsafe { is_equal_raw(x.as_ptr(), y.as_ptr(), x.len()) }
}

/// Compare `n` bytes at the given pointers for equality.
///
/// This returns true if and only if `*x.add(i) == *y.add(i)` for all
/// `0 <= i < n`.
///
/// # Inlining
///
/// This routine is marked `inline(always)`. If you want to call this function
/// in a way that is not always inlined, you'll need to wrap a call to it in
/// another function that is marked as `inline(never)` or just `inline`.
///
/// # Motivation
///
/// Why not use slice equality instead? Well, slice equality usually results in
/// a call out to the current platform's `libc` which might not be inlineable
/// or have other overhead. This routine isn't guaranteed to be a win, but it
/// might be in some cases.
///
/// # Safety
///
/// * Both `x` and `y` must be valid for reads of up to `n` bytes.
/// * Both `x` and `y` must point to an initialized value.
/// * Both `x` and `y` must each point to an allocated object and
/// must either be in bounds or at most one byte past the end of the
/// allocated object. `x` and `y` do not need to point to the same allocated
/// object, but they may.
/// * Both `x` and `y` must be _derived from_ a pointer to their respective
/// allocated objects.
/// * The distance between `x` and `x+n` must not overflow `isize`. Similarly
/// for `y` and `y+n`.
/// * The distance being in bounds must not rely on "wrapping around" the
/// address space.
#[inline(always)]
unsafe fn is_equal_raw(mut x: *const u8, mut y: *const u8, n: usize) -> bool {
    // If we don't have enough bytes to do 4-byte at a time loads, then
    // handle each possible length specially. Note that I used to have a
    // byte-at-a-time loop here and that turned out to be quite a bit slower
    // for the memmem/pathological/defeat-simple-vector-alphabet benchmark.
    if n < 4 {
        return match n {
            0 => true,
            1 => x.read() == y.read(),
            2 => {
                x.cast::<u16>().read_unaligned()
                    == y.cast::<u16>().read_unaligned()
            }
            // I also tried copy_nonoverlapping here and it looks like the
            // codegen is the same.
            3 => x.cast::<[u8; 3]>().read() == y.cast::<[u8; 3]>().read(),
            _ => unreachable!(),
        };
    }
    // When we have 4 or more bytes to compare, then proceed in chunks of 4 at
    // a time using unaligned loads.
    //
    // Also, why do 4 byte loads instead of, say, 8 byte loads? The reason is
    // that this particular version of memcmp is likely to be called with tiny
    // needles. That means that if we do 8 byte loads, then a higher proportion
    // of memcmp calls will use the slower variant above. With that said, this
    // is a hypothesis and is only loosely supported by benchmarks. There's
    // likely some improvement that could be made here. The main thing here
    // though is to optimize for latency, not throughput.

    // SAFETY: The caller is responsible for ensuring the pointers we get are
    // valid and readable for at least `n` bytes. We also do unaligned loads,
    // so there's no need to ensure we're aligned. (This is justified by this
    // routine being specifically for short strings.)
    let xend = x.add(n.wrapping_sub(4));
    let yend = y.add(n.wrapping_sub(4));
    while x < xend {
        let vx = x.cast::<u32>().read_unaligned();
        let vy = y.cast::<u32>().read_unaligned();
        if vx != vy {
            return false;
        }
        x = x.add(4);
        y = y.add(4);
    }
    let vx = xend.cast::<u32>().read_unaligned();
    let vy = yend.cast::<u32>().read_unaligned();
    vx == vy
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equals_different_lengths() {
        assert!(!is_equal(b"", b"a"));
        assert!(!is_equal(b"a", b""));
        assert!(!is_equal(b"ab", b"a"));
        assert!(!is_equal(b"a", b"ab"));
    }

    #[test]
    fn equals_mismatch() {
        let one_mismatch = [
            (&b"a"[..], &b"x"[..]),
            (&b"ab"[..], &b"ax"[..]),
            (&b"abc"[..], &b"abx"[..]),
            (&b"abcd"[..], &b"abcx"[..]),
            (&b"abcde"[..], &b"abcdx"[..]),
            (&b"abcdef"[..], &b"abcdex"[..]),
            (&b"abcdefg"[..], &b"abcdefx"[..]),
            (&b"abcdefgh"[..], &b"abcdefgx"[..]),
            (&b"abcdefghi"[..], &b"abcdefghx"[..]),
            (&b"abcdefghij"[..], &b"abcdefghix"[..]),
            (&b"abcdefghijk"[..], &b"abcdefghijx"[..]),
            (&b"abcdefghijkl"[..], &b"abcdefghijkx"[..]),
            (&b"abcdefghijklm"[..], &b"abcdefghijklx"[..]),
            (&b"abcdefghijklmn"[..], &b"abcdefghijklmx"[..]),
        ];
        for (x, y) in one_mismatch {
            assert_eq!(x.len(), y.len(), "lengths should match");
            assert!(!is_equal(x, y));
            assert!(!is_equal(y, x));
        }
    }

    #[test]
    fn equals_yes() {
        assert!(is_equal(b"", b""));
        assert!(is_equal(b"a", b"a"));
        assert!(is_equal(b"ab", b"ab"));
        assert!(is_equal(b"abc", b"abc"));
        assert!(is_equal(b"abcd", b"abcd"));
        assert!(is_equal(b"abcde", b"abcde"));
        assert!(is_equal(b"abcdef", b"abcdef"));
        assert!(is_equal(b"abcdefg", b"abcdefg"));
        assert!(is_equal(b"abcdefgh", b"abcdefgh"));
        assert!(is_equal(b"abcdefghi", b"abcdefghi"));
    }

    #[test]
    fn prefix() {
        assert!(is_prefix(b"", b""));
        assert!(is_prefix(b"a", b""));
        assert!(is_prefix(b"ab", b""));
        assert!(is_prefix(b"foo", b"foo"));
        assert!(is_prefix(b"foobar", b"foo"));

        assert!(!is_prefix(b"foo", b"fob"));
        assert!(!is_prefix(b"foobar", b"fob"));
    }
}
