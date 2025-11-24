use std::collections::{HashMap, HashSet};

use crate::FxHasher;

/// Type alias for a hashmap using the `fx` hash algorithm with [`FxRandomState`].
pub type FxHashMapRand<K, V> = HashMap<K, V, FxRandomState>;

/// Type alias for a hashmap using the `fx` hash algorithm with [`FxRandomState`].
pub type FxHashSetRand<V> = HashSet<V, FxRandomState>;

/// `FxRandomState` is an alternative state for `HashMap` types.
///
/// A particular instance `FxRandomState` will create the same instances of
/// [`Hasher`], but the hashers created by two different `FxRandomState`
/// instances are unlikely to produce the same result for the same values.
#[derive(Clone)]
pub struct FxRandomState {
    seed: usize,
}

impl FxRandomState {
    /// Constructs a new `FxRandomState` that is initialized with random seed.
    pub fn new() -> FxRandomState {
        use rand::Rng;
        use std::{cell::Cell, thread_local};

        // This mirrors what `std::collections::hash_map::RandomState` does, as of 2024-01-14.
        //
        // Basically
        // 1. Cache result of the rng in a thread local, so repeatedly
        //    creating maps is cheaper
        // 2. Change the cached result on every creation, so maps created
        //    on the same thread don't have the same iteration order
        thread_local!(static SEED: Cell<usize> = {
            Cell::new(rand::thread_rng().gen())
        });

        SEED.with(|seed| {
            let s = seed.get();
            seed.set(s.wrapping_add(1));
            FxRandomState { seed: s }
        })
    }
}

impl core::hash::BuildHasher for FxRandomState {
    type Hasher = FxHasher;

    fn build_hasher(&self) -> Self::Hasher {
        FxHasher::with_seed(self.seed)
    }
}

impl Default for FxRandomState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::thread;

    use crate::FxHashMapRand;

    #[test]
    fn cloned_random_states_are_equal() {
        let a = FxHashMapRand::<&str, u32>::default();
        let b = a.clone();

        assert_eq!(a.hasher().seed, b.hasher().seed);
    }

    #[test]
    fn random_states_are_different() {
        let a = FxHashMapRand::<&str, u32>::default();
        let b = FxHashMapRand::<&str, u32>::default();

        // That's the whole point of them being random!
        //
        // N.B.: `FxRandomState` uses a thread-local set to a random value and then incremented,
        //       which means that this is *guaranteed* to pass :>
        assert_ne!(a.hasher().seed, b.hasher().seed);
    }

    #[test]
    fn random_states_are_different_cross_thread() {
        // This is similar to the test above, but uses two different threads, so they both get
        // completely random, unrelated values.
        //
        // This means that this test is technically flaky, but the probability of it failing is
        // `1 / 2.pow(bit_size_of::<usize>())`. Or 1/1.7e19 for 64 bit platforms or 1/4294967295
        // for 32 bit platforms. I suppose this is acceptable.
        let a = FxHashMapRand::<&str, u32>::default();
        let b = thread::spawn(|| FxHashMapRand::<&str, u32>::default())
            .join()
            .unwrap();

        assert_ne!(a.hasher().seed, b.hasher().seed);
    }
}
