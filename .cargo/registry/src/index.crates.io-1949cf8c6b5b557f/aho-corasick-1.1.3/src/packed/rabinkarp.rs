use alloc::{sync::Arc, vec, vec::Vec};

use crate::{packed::pattern::Patterns, util::search::Match, PatternID};

/// The type of the rolling hash used in the Rabin-Karp algorithm.
type Hash = usize;

/// The number of buckets to store our patterns in. We don't want this to be
/// too big in order to avoid wasting memory, but we don't want it to be too
/// small either to avoid spending too much time confirming literals.
///
/// The number of buckets MUST be a power of two. Otherwise, determining the
/// bucket from a hash will slow down the code considerably. Using a power
/// of two means `hash % NUM_BUCKETS` can compile down to a simple `and`
/// instruction.
const NUM_BUCKETS: usize = 64;

/// An implementation of the Rabin-Karp algorithm. The main idea of this
/// algorithm is to maintain a rolling hash as it moves through the input, and
/// then check whether that hash corresponds to the same hash for any of the
/// patterns we're looking for.
///
/// A draw back of naively scaling Rabin-Karp to multiple patterns is that
/// it requires all of the patterns to be the same length, which in turn
/// corresponds to the number of bytes to hash. We adapt this to work for
/// multiple patterns of varying size by fixing the number of bytes to hash
/// to be the length of the smallest pattern. We also split the patterns into
/// several buckets to hopefully make the confirmation step faster.
///
/// Wikipedia has a decent explanation, if a bit heavy on the theory:
/// https://en.wikipedia.org/wiki/Rabin%E2%80%93Karp_algorithm
///
/// But ESMAJ provides something a bit more concrete:
/// https://www-igm.univ-mlv.fr/~lecroq/string/node5.html
#[derive(Clone, Debug)]
pub(crate) struct RabinKarp {
    /// The patterns we're searching for.
    patterns: Arc<Patterns>,
    /// The order of patterns in each bucket is significant. Namely, they are
    /// arranged such that the first one to match is the correct match. This
    /// may not necessarily correspond to the order provided by the caller.
    /// For example, if leftmost-longest semantics are used, then the patterns
    /// are sorted by their length in descending order. If leftmost-first
    /// semantics are used, then the patterns are sorted by their pattern ID
    /// in ascending order (which corresponds to the caller's order).
    buckets: Vec<Vec<(Hash, PatternID)>>,
    /// The length of the hashing window. Generally, this corresponds to the
    /// length of the smallest pattern.
    hash_len: usize,
    /// The factor to subtract out of a hash before updating it with a new
    /// byte.
    hash_2pow: usize,
}

impl RabinKarp {
    /// Compile a new Rabin-Karp matcher from the patterns given.
    ///
    /// This panics if any of the patterns in the collection are empty, or if
    /// the collection is itself empty.
    pub(crate) fn new(patterns: &Arc<Patterns>) -> RabinKarp {
        assert!(patterns.len() >= 1);
        let hash_len = patterns.minimum_len();
        assert!(hash_len >= 1);

        let mut hash_2pow = 1usize;
        for _ in 1..hash_len {
            hash_2pow = hash_2pow.wrapping_shl(1);
        }

        let mut rk = RabinKarp {
            patterns: Arc::clone(patterns),
            buckets: vec![vec![]; NUM_BUCKETS],
            hash_len,
            hash_2pow,
        };
        for (id, pat) in patterns.iter() {
            let hash = rk.hash(&pat.bytes()[..rk.hash_len]);
            let bucket = hash % NUM_BUCKETS;
            rk.buckets[bucket].push((hash, id));
        }
        rk
    }

    /// Return the first matching pattern in the given haystack, begining the
    /// search at `at`.
    pub(crate) fn find_at(
        &self,
        haystack: &[u8],
        mut at: usize,
    ) -> Option<Match> {
        assert_eq!(NUM_BUCKETS, self.buckets.len());

        if at + self.hash_len > haystack.len() {
            return None;
        }
        let mut hash = self.hash(&haystack[at..at + self.hash_len]);
        loop {
            let bucket = &self.buckets[hash % NUM_BUCKETS];
            for &(phash, pid) in bucket {
                if phash == hash {
                    if let Some(c) = self.verify(pid, haystack, at) {
                        return Some(c);
                    }
                }
            }
            if at + self.hash_len >= haystack.len() {
                return None;
            }
            hash = self.update_hash(
                hash,
                haystack[at],
                haystack[at + self.hash_len],
            );
            at += 1;
        }
    }

    /// Returns the approximate total amount of heap used by this searcher, in
    /// units of bytes.
    pub(crate) fn memory_usage(&self) -> usize {
        self.buckets.len() * core::mem::size_of::<Vec<(Hash, PatternID)>>()
            + self.patterns.len() * core::mem::size_of::<(Hash, PatternID)>()
    }

    /// Verify whether the pattern with the given id matches at
    /// `haystack[at..]`.
    ///
    /// We tag this function as `cold` because it helps improve codegen.
    /// Intuitively, it would seem like inlining it would be better. However,
    /// the only time this is called and a match is not found is when there
    /// there is a hash collision, or when a prefix of a pattern matches but
    /// the entire pattern doesn't match. This is hopefully fairly rare, and
    /// if it does occur a lot, it's going to be slow no matter what we do.
    #[cold]
    fn verify(
        &self,
        id: PatternID,
        haystack: &[u8],
        at: usize,
    ) -> Option<Match> {
        let pat = self.patterns.get(id);
        if pat.is_prefix(&haystack[at..]) {
            Some(Match::new(id, at..at + pat.len()))
        } else {
            None
        }
    }

    /// Hash the given bytes.
    fn hash(&self, bytes: &[u8]) -> Hash {
        assert_eq!(self.hash_len, bytes.len());

        let mut hash = 0usize;
        for &b in bytes {
            hash = hash.wrapping_shl(1).wrapping_add(b as usize);
        }
        hash
    }

    /// Update the hash given based on removing `old_byte` at the beginning
    /// of some byte string, and appending `new_byte` to the end of that same
    /// byte string.
    fn update_hash(&self, prev: Hash, old_byte: u8, new_byte: u8) -> Hash {
        prev.wrapping_sub((old_byte as usize).wrapping_mul(self.hash_2pow))
            .wrapping_shl(1)
            .wrapping_add(new_byte as usize)
    }
}
