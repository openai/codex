/*!
An implementation of the [Rabin-Karp substring search algorithm][rabinkarp].

Rabin-Karp works by creating a hash of the needle provided and then computing
a rolling hash for each needle sized window in the haystack. When the rolling
hash matches the hash of the needle, a byte-wise comparison is done to check
if a match exists. The worst case time complexity of Rabin-Karp is `O(m *
n)` where `m ~ len(needle)` and `n ~ len(haystack)`. Its worst case space
complexity is constant.

The main utility of Rabin-Karp is that the searcher can be constructed very
quickly with very little memory. This makes it especially useful when searching
for small needles in small haystacks, as it might finish its search before a
beefier algorithm (like Two-Way) even starts.

[rabinkarp]: https://en.wikipedia.org/wiki/Rabin%E2%80%93Karp_algorithm
*/

/*
(This was the comment I wrote for this module originally when it was not
exposed. The comment still looks useful, but it's a bit in the weeds, so it's
not public itself.)

This module implements the classical Rabin-Karp substring search algorithm,
with no extra frills. While its use would seem to break our time complexity
guarantee of O(m+n) (RK's time complexity is O(mn)), we are careful to only
ever use RK on a constant subset of haystacks. The main point here is that
RK has good latency properties for small needles/haystacks. It's very quick
to compute a needle hash and zip through the haystack when compared to
initializing Two-Way, for example. And this is especially useful for cases
where the haystack is just too short for vector instructions to do much good.

The hashing function used here is the same one recommended by ESMAJ.

Another choice instead of Rabin-Karp would be Shift-Or. But its latency
isn't quite as good since its preprocessing time is a bit more expensive
(both in practice and in theory). However, perhaps Shift-Or has a place
somewhere else for short patterns. I think the main problem is that it
requires space proportional to the alphabet and the needle. If we, for
example, supported needles up to length 16, then the total table size would be
len(alphabet)*size_of::<u16>()==512 bytes. Which isn't exactly small, and it's
probably bad to put that on the stack. So ideally, we'd throw it on the heap,
but we'd really like to write as much code without using alloc/std as possible.
But maybe it's worth the special casing. It's a TODO to benchmark.

Wikipedia has a decent explanation, if a bit heavy on the theory:
https://en.wikipedia.org/wiki/Rabin%E2%80%93Karp_algorithm

But ESMAJ provides something a bit more concrete:
http://www-igm.univ-mlv.fr/~lecroq/string/node5.html

Finally, aho-corasick uses Rabin-Karp for multiple pattern match in some cases:
https://github.com/BurntSushi/aho-corasick/blob/3852632f10587db0ff72ef29e88d58bf305a0946/src/packed/rabinkarp.rs
*/

use crate::ext::Pointer;

/// A forward substring searcher using the Rabin-Karp algorithm.
///
/// Note that, as a lower level API, a `Finder` does not have access to the
/// needle it was constructed with. For this reason, executing a search
/// with a `Finder` requires passing both the needle and the haystack,
/// where the needle is exactly equivalent to the one given to the `Finder`
/// at construction time. This design was chosen so that callers can have
/// more precise control over where and how many times a needle is stored.
/// For example, in cases where Rabin-Karp is just one of several possible
/// substring search algorithms.
#[derive(Clone, Debug)]
pub struct Finder {
    /// The actual hash.
    hash: Hash,
    /// The factor needed to multiply a byte by in order to subtract it from
    /// the hash. It is defined to be 2^(n-1) (using wrapping exponentiation),
    /// where n is the length of the needle. This is how we "remove" a byte
    /// from the hash once the hash window rolls past it.
    hash_2pow: u32,
}

impl Finder {
    /// Create a new Rabin-Karp forward searcher for the given `needle`.
    ///
    /// The needle may be empty. The empty needle matches at every byte offset.
    ///
    /// Note that callers must pass the same needle to all search calls using
    /// this `Finder`.
    #[inline]
    pub fn new(needle: &[u8]) -> Finder {
        let mut s = Finder { hash: Hash::new(), hash_2pow: 1 };
        let first_byte = match needle.get(0) {
            None => return s,
            Some(&first_byte) => first_byte,
        };
        s.hash.add(first_byte);
        for b in needle.iter().copied().skip(1) {
            s.hash.add(b);
            s.hash_2pow = s.hash_2pow.wrapping_shl(1);
        }
        s
    }

    /// Return the first occurrence of the `needle` in the `haystack`
    /// given. If no such occurrence exists, then `None` is returned.
    ///
    /// The `needle` provided must match the needle given to this finder at
    /// construction time.
    ///
    /// The maximum value this can return is `haystack.len()`, which can only
    /// occur when the needle and haystack both have length zero. Otherwise,
    /// for non-empty haystacks, the maximum value is `haystack.len() - 1`.
    #[inline]
    pub fn find(&self, haystack: &[u8], needle: &[u8]) -> Option<usize> {
        unsafe {
            let hstart = haystack.as_ptr();
            let hend = hstart.add(haystack.len());
            let nstart = needle.as_ptr();
            let nend = nstart.add(needle.len());
            let found = self.find_raw(hstart, hend, nstart, nend)?;
            Some(found.distance(hstart))
        }
    }

    /// Like `find`, but accepts and returns raw pointers.
    ///
    /// When a match is found, the pointer returned is guaranteed to be
    /// `>= start` and `<= end`. The pointer returned is only ever equivalent
    /// to `end` when both the needle and haystack are empty. (That is, the
    /// empty string matches the empty string.)
    ///
    /// This routine is useful if you're already using raw pointers and would
    /// like to avoid converting back to a slice before executing a search.
    ///
    /// # Safety
    ///
    /// Note that `start` and `end` below refer to both pairs of pointers given
    /// to this routine. That is, the conditions apply to both `hstart`/`hend`
    /// and `nstart`/`nend`.
    ///
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
    /// * It must be the case that `start <= end`.
    #[inline]
    pub unsafe fn find_raw(
        &self,
        hstart: *const u8,
        hend: *const u8,
        nstart: *const u8,
        nend: *const u8,
    ) -> Option<*const u8> {
        let hlen = hend.distance(hstart);
        let nlen = nend.distance(nstart);
        if nlen > hlen {
            return None;
        }
        let mut cur = hstart;
        let end = hend.sub(nlen);
        let mut hash = Hash::forward(cur, cur.add(nlen));
        loop {
            if self.hash == hash && is_equal_raw(cur, nstart, nlen) {
                return Some(cur);
            }
            if cur >= end {
                return None;
            }
            hash.roll(self, cur.read(), cur.add(nlen).read());
            cur = cur.add(1);
        }
    }
}

/// A reverse substring searcher using the Rabin-Karp algorithm.
#[derive(Clone, Debug)]
pub struct FinderRev(Finder);

impl FinderRev {
    /// Create a new Rabin-Karp reverse searcher for the given `needle`.
    #[inline]
    pub fn new(needle: &[u8]) -> FinderRev {
        let mut s = FinderRev(Finder { hash: Hash::new(), hash_2pow: 1 });
        let last_byte = match needle.last() {
            None => return s,
            Some(&last_byte) => last_byte,
        };
        s.0.hash.add(last_byte);
        for b in needle.iter().rev().copied().skip(1) {
            s.0.hash.add(b);
            s.0.hash_2pow = s.0.hash_2pow.wrapping_shl(1);
        }
        s
    }

    /// Return the last occurrence of the `needle` in the `haystack`
    /// given. If no such occurrence exists, then `None` is returned.
    ///
    /// The `needle` provided must match the needle given to this finder at
    /// construction time.
    ///
    /// The maximum value this can return is `haystack.len()`, which can only
    /// occur when the needle and haystack both have length zero. Otherwise,
    /// for non-empty haystacks, the maximum value is `haystack.len() - 1`.
    #[inline]
    pub fn rfind(&self, haystack: &[u8], needle: &[u8]) -> Option<usize> {
        unsafe {
            let hstart = haystack.as_ptr();
            let hend = hstart.add(haystack.len());
            let nstart = needle.as_ptr();
            let nend = nstart.add(needle.len());
            let found = self.rfind_raw(hstart, hend, nstart, nend)?;
            Some(found.distance(hstart))
        }
    }

    /// Like `rfind`, but accepts and returns raw pointers.
    ///
    /// When a match is found, the pointer returned is guaranteed to be
    /// `>= start` and `<= end`. The pointer returned is only ever equivalent
    /// to `end` when both the needle and haystack are empty. (That is, the
    /// empty string matches the empty string.)
    ///
    /// This routine is useful if you're already using raw pointers and would
    /// like to avoid converting back to a slice before executing a search.
    ///
    /// # Safety
    ///
    /// Note that `start` and `end` below refer to both pairs of pointers given
    /// to this routine. That is, the conditions apply to both `hstart`/`hend`
    /// and `nstart`/`nend`.
    ///
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
    /// * It must be the case that `start <= end`.
    #[inline]
    pub unsafe fn rfind_raw(
        &self,
        hstart: *const u8,
        hend: *const u8,
        nstart: *const u8,
        nend: *const u8,
    ) -> Option<*const u8> {
        let hlen = hend.distance(hstart);
        let nlen = nend.distance(nstart);
        if nlen > hlen {
            return None;
        }
        let mut cur = hend.sub(nlen);
        let start = hstart;
        let mut hash = Hash::reverse(cur, cur.add(nlen));
        loop {
            if self.0.hash == hash && is_equal_raw(cur, nstart, nlen) {
                return Some(cur);
            }
            if cur <= start {
                return None;
            }
            cur = cur.sub(1);
            hash.roll(&self.0, cur.add(nlen).read(), cur.read());
        }
    }
}

/// Whether RK is believed to be very fast for the given needle/haystack.
#[inline]
pub(crate) fn is_fast(haystack: &[u8], _needle: &[u8]) -> bool {
    haystack.len() < 16
}

/// A Rabin-Karp hash. This might represent the hash of a needle, or the hash
/// of a rolling window in the haystack.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Hash(u32);

impl Hash {
    /// Create a new hash that represents the empty string.
    #[inline(always)]
    fn new() -> Hash {
        Hash(0)
    }

    /// Create a new hash from the bytes given for use in forward searches.
    ///
    /// # Safety
    ///
    /// The given pointers must be valid to read from within their range.
    #[inline(always)]
    unsafe fn forward(mut start: *const u8, end: *const u8) -> Hash {
        let mut hash = Hash::new();
        while start < end {
            hash.add(start.read());
            start = start.add(1);
        }
        hash
    }

    /// Create a new hash from the bytes given for use in reverse searches.
    ///
    /// # Safety
    ///
    /// The given pointers must be valid to read from within their range.
    #[inline(always)]
    unsafe fn reverse(start: *const u8, mut end: *const u8) -> Hash {
        let mut hash = Hash::new();
        while start < end {
            end = end.sub(1);
            hash.add(end.read());
        }
        hash
    }

    /// Add 'new' and remove 'old' from this hash. The given needle hash should
    /// correspond to the hash computed for the needle being searched for.
    ///
    /// This is meant to be used when the rolling window of the haystack is
    /// advanced.
    #[inline(always)]
    fn roll(&mut self, finder: &Finder, old: u8, new: u8) {
        self.del(finder, old);
        self.add(new);
    }

    /// Add a byte to this hash.
    #[inline(always)]
    fn add(&mut self, byte: u8) {
        self.0 = self.0.wrapping_shl(1).wrapping_add(u32::from(byte));
    }

    /// Remove a byte from this hash. The given needle hash should correspond
    /// to the hash computed for the needle being searched for.
    #[inline(always)]
    fn del(&mut self, finder: &Finder, byte: u8) {
        let factor = finder.hash_2pow;
        self.0 = self.0.wrapping_sub(u32::from(byte).wrapping_mul(factor));
    }
}

/// Returns true when `x[i] == y[i]` for all `0 <= i < n`.
///
/// We forcefully don't inline this to hint at the compiler that it is unlikely
/// to be called. This causes the inner rabinkarp loop above to be a bit
/// tighter and leads to some performance improvement. See the
/// memmem/krate/prebuilt/sliceslice-words/words benchmark.
///
/// # Safety
///
/// Same as `crate::arch::all::is_equal_raw`.
#[cold]
#[inline(never)]
unsafe fn is_equal_raw(x: *const u8, y: *const u8, n: usize) -> bool {
    crate::arch::all::is_equal_raw(x, y, n)
}

#[cfg(test)]
mod tests {
    use super::*;

    define_substring_forward_quickcheck!(|h, n| Some(
        Finder::new(n).find(h, n)
    ));
    define_substring_reverse_quickcheck!(|h, n| Some(
        FinderRev::new(n).rfind(h, n)
    ));

    #[test]
    fn forward() {
        crate::tests::substring::Runner::new()
            .fwd(|h, n| Some(Finder::new(n).find(h, n)))
            .run();
    }

    #[test]
    fn reverse() {
        crate::tests::substring::Runner::new()
            .rev(|h, n| Some(FinderRev::new(n).rfind(h, n)))
            .run();
    }
}
