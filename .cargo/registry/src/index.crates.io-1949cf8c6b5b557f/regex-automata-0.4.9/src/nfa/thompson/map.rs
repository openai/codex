// This module contains a couple simple and purpose built hash maps. The key
// trade off they make is that they serve as caches rather than true maps. That
// is, inserting a new entry may cause eviction of another entry. This gives
// us two things. First, there's less overhead associated with inserts and
// lookups. Secondly, it lets us control our memory usage.
//
// These maps are used in some fairly hot code when generating NFA states for
// large Unicode character classes.
//
// Instead of exposing a rich hashmap entry API, we just permit the caller to
// produce a hash of the key directly. The hash can then be reused for both
// lookups and insertions at the cost of leaking abstraction a bit. But these
// are for internal use only, so it's fine.
//
// The Utf8BoundedMap is used for Daciuk's algorithm for constructing a
// (almost) minimal DFA for large Unicode character classes in linear time.
// (Daciuk's algorithm is always used when compiling forward NFAs. For reverse
// NFAs, it's only used when the compiler is configured to 'shrink' the NFA,
// since there's a bit more expense in the reverse direction.)
//
// The Utf8SuffixMap is used when compiling large Unicode character classes for
// reverse NFAs when 'shrink' is disabled. Specifically, it augments the naive
// construction of UTF-8 automata by caching common suffixes. This doesn't
// get the same space savings as Daciuk's algorithm, but it's basically as
// fast as the naive approach and typically winds up using less memory (since
// it generates smaller NFAs) despite the presence of the cache.
//
// These maps effectively represent caching mechanisms for sparse and
// byte-range NFA states, respectively. The former represents a single NFA
// state with many transitions of equivalent priority while the latter
// represents a single NFA state with a single transition. (Neither state ever
// has or is an epsilon transition.) Thus, they have different key types. It's
// likely we could make one generic map, but the machinery didn't seem worth
// it. They are simple enough.

use alloc::{vec, vec::Vec};

use crate::{
    nfa::thompson::Transition,
    util::{
        int::{Usize, U64},
        primitives::StateID,
    },
};

// Basic FNV-1a hash constants as described in:
// https://en.wikipedia.org/wiki/Fowler%E2%80%93Noll%E2%80%93Vo_hash_function
const PRIME: u64 = 1099511628211;
const INIT: u64 = 14695981039346656037;

/// A bounded hash map where the key is a sequence of NFA transitions and the
/// value is a pre-existing NFA state ID.
///
/// std's hashmap can be used for this, however, this map has two important
/// advantages. Firstly, it has lower overhead. Secondly, it permits us to
/// control our memory usage by limited the number of slots. In general, the
/// cost here is that this map acts as a cache. That is, inserting a new entry
/// may remove an old entry. We are okay with this, since it does not impact
/// correctness in the cases where it is used. The only effect that dropping
/// states from the cache has is that the resulting NFA generated may be bigger
/// than it otherwise would be.
///
/// This improves benchmarks that compile large Unicode character classes,
/// since it makes the generation of (almost) minimal UTF-8 automaton faster.
/// Specifically, one could observe the difference with std's hashmap via
/// something like the following benchmark:
///
///   hyperfine "regex-cli debug thompson -qr --captures none '\w{90} ecurB'"
///
/// But to observe that difference, you'd have to modify the code to use
/// std's hashmap.
///
/// It is quite possible that there is a better way to approach this problem.
/// For example, if there happens to be a very common state that collides with
/// a lot of less frequent states, then we could wind up with very poor caching
/// behavior. Alas, the effectiveness of this cache has not been measured.
/// Instead, ad hoc experiments suggest that it is "good enough." Additional
/// smarts (such as an LRU eviction policy) have to be weighed against the
/// amount of extra time they cost.
#[derive(Clone, Debug)]
pub struct Utf8BoundedMap {
    /// The current version of this map. Only entries with matching versions
    /// are considered during lookups. If an entry is found with a mismatched
    /// version, then the map behaves as if the entry does not exist.
    ///
    /// This makes it possible to clear the map by simply incrementing the
    /// version number instead of actually deallocating any storage.
    version: u16,
    /// The total number of entries this map can store.
    capacity: usize,
    /// The actual entries, keyed by hash. Collisions between different states
    /// result in the old state being dropped.
    map: Vec<Utf8BoundedEntry>,
}

/// An entry in this map.
#[derive(Clone, Debug, Default)]
struct Utf8BoundedEntry {
    /// The version of the map used to produce this entry. If this entry's
    /// version does not match the current version of the map, then the map
    /// should behave as if this entry does not exist.
    version: u16,
    /// The key, which is a sorted sequence of non-overlapping NFA transitions.
    key: Vec<Transition>,
    /// The state ID corresponding to the state containing the transitions in
    /// this entry.
    val: StateID,
}

impl Utf8BoundedMap {
    /// Create a new bounded map with the given capacity. The map will never
    /// grow beyond the given size.
    ///
    /// Note that this does not allocate. Instead, callers must call `clear`
    /// before using this map. `clear` will allocate space if necessary.
    ///
    /// This avoids the need to pay for the allocation of this map when
    /// compiling regexes that lack large Unicode character classes.
    pub fn new(capacity: usize) -> Utf8BoundedMap {
        assert!(capacity > 0);
        Utf8BoundedMap { version: 0, capacity, map: vec![] }
    }

    /// Clear this map of all entries, but permit the reuse of allocation
    /// if possible.
    ///
    /// This must be called before the map can be used.
    pub fn clear(&mut self) {
        if self.map.is_empty() {
            self.map = vec![Utf8BoundedEntry::default(); self.capacity];
        } else {
            self.version = self.version.wrapping_add(1);
            // If we loop back to version 0, then we forcefully clear the
            // entire map. Otherwise, it might be possible to incorrectly
            // match entries used to generate other NFAs.
            if self.version == 0 {
                self.map = vec![Utf8BoundedEntry::default(); self.capacity];
            }
        }
    }

    /// Return a hash of the given transitions.
    pub fn hash(&self, key: &[Transition]) -> usize {
        let mut h = INIT;
        for t in key {
            h = (h ^ u64::from(t.start)).wrapping_mul(PRIME);
            h = (h ^ u64::from(t.end)).wrapping_mul(PRIME);
            h = (h ^ t.next.as_u64()).wrapping_mul(PRIME);
        }
        (h % self.map.len().as_u64()).as_usize()
    }

    /// Retrieve the cached state ID corresponding to the given key. The hash
    /// given must have been computed with `hash` using the same key value.
    ///
    /// If there is no cached state with the given transitions, then None is
    /// returned.
    pub fn get(&mut self, key: &[Transition], hash: usize) -> Option<StateID> {
        let entry = &self.map[hash];
        if entry.version != self.version {
            return None;
        }
        // There may be a hash collision, so we need to confirm real equality.
        if entry.key != key {
            return None;
        }
        Some(entry.val)
    }

    /// Add a cached state to this map with the given key. Callers should
    /// ensure that `state_id` points to a state that contains precisely the
    /// NFA transitions given.
    ///
    /// `hash` must have been computed using the `hash` method with the same
    /// key.
    pub fn set(
        &mut self,
        key: Vec<Transition>,
        hash: usize,
        state_id: StateID,
    ) {
        self.map[hash] =
            Utf8BoundedEntry { version: self.version, key, val: state_id };
    }
}

/// A cache of suffixes used to modestly compress UTF-8 automata for large
/// Unicode character classes.
#[derive(Clone, Debug)]
pub struct Utf8SuffixMap {
    /// The current version of this map. Only entries with matching versions
    /// are considered during lookups. If an entry is found with a mismatched
    /// version, then the map behaves as if the entry does not exist.
    version: u16,
    /// The total number of entries this map can store.
    capacity: usize,
    /// The actual entries, keyed by hash. Collisions between different states
    /// result in the old state being dropped.
    map: Vec<Utf8SuffixEntry>,
}

/// A key that uniquely identifies an NFA state. It is a triple that represents
/// a transition from one state for a particular byte range.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Utf8SuffixKey {
    pub from: StateID,
    pub start: u8,
    pub end: u8,
}

/// An entry in this map.
#[derive(Clone, Debug, Default)]
struct Utf8SuffixEntry {
    /// The version of the map used to produce this entry. If this entry's
    /// version does not match the current version of the map, then the map
    /// should behave as if this entry does not exist.
    version: u16,
    /// The key, which consists of a transition in a particular state.
    key: Utf8SuffixKey,
    /// The identifier that the transition in the key maps to.
    val: StateID,
}

impl Utf8SuffixMap {
    /// Create a new bounded map with the given capacity. The map will never
    /// grow beyond the given size.
    ///
    /// Note that this does not allocate. Instead, callers must call `clear`
    /// before using this map. `clear` will allocate space if necessary.
    ///
    /// This avoids the need to pay for the allocation of this map when
    /// compiling regexes that lack large Unicode character classes.
    pub fn new(capacity: usize) -> Utf8SuffixMap {
        assert!(capacity > 0);
        Utf8SuffixMap { version: 0, capacity, map: vec![] }
    }

    /// Clear this map of all entries, but permit the reuse of allocation
    /// if possible.
    ///
    /// This must be called before the map can be used.
    pub fn clear(&mut self) {
        if self.map.is_empty() {
            self.map = vec![Utf8SuffixEntry::default(); self.capacity];
        } else {
            self.version = self.version.wrapping_add(1);
            if self.version == 0 {
                self.map = vec![Utf8SuffixEntry::default(); self.capacity];
            }
        }
    }

    /// Return a hash of the given transition.
    pub fn hash(&self, key: &Utf8SuffixKey) -> usize {
        // Basic FNV-1a hash as described:
        // https://en.wikipedia.org/wiki/Fowler%E2%80%93Noll%E2%80%93Vo_hash_function
        const PRIME: u64 = 1099511628211;
        const INIT: u64 = 14695981039346656037;

        let mut h = INIT;
        h = (h ^ key.from.as_u64()).wrapping_mul(PRIME);
        h = (h ^ u64::from(key.start)).wrapping_mul(PRIME);
        h = (h ^ u64::from(key.end)).wrapping_mul(PRIME);
        (h % self.map.len().as_u64()).as_usize()
    }

    /// Retrieve the cached state ID corresponding to the given key. The hash
    /// given must have been computed with `hash` using the same key value.
    ///
    /// If there is no cached state with the given key, then None is returned.
    pub fn get(
        &mut self,
        key: &Utf8SuffixKey,
        hash: usize,
    ) -> Option<StateID> {
        let entry = &self.map[hash];
        if entry.version != self.version {
            return None;
        }
        if key != &entry.key {
            return None;
        }
        Some(entry.val)
    }

    /// Add a cached state to this map with the given key. Callers should
    /// ensure that `state_id` points to a state that contains precisely the
    /// NFA transition given.
    ///
    /// `hash` must have been computed using the `hash` method with the same
    /// key.
    pub fn set(&mut self, key: Utf8SuffixKey, hash: usize, state_id: StateID) {
        self.map[hash] =
            Utf8SuffixEntry { version: self.version, key, val: state_id };
    }
}
