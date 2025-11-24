use std::{
    cell::Cell,
    collections::hash_map::DefaultHasher,
    hash::Hasher,
    num::Wrapping,
    sync::atomic::{AtomicUsize, Ordering},
};

// Based on [Fisher–Yates shuffle].
//
// [Fisher–Yates shuffle]: https://en.wikipedia.org/wiki/Fisher–Yates_shuffle
#[doc(hidden)]
pub fn shuffle<T>(slice: &mut [T]) {
    for i in (1..slice.len()).rev() {
        slice.swap(i, gen_index(i + 1));
    }
}

/// Return a value from `0..n`.
fn gen_index(n: usize) -> usize {
    (random() % n as u64) as usize
}

/// Pseudorandom number generator based on [xorshift*].
///
/// [xorshift*]: https://en.wikipedia.org/wiki/Xorshift#xorshift*
fn random() -> u64 {
    std::thread_local! {
        static RNG: Cell<Wrapping<u64>> = Cell::new(Wrapping(prng_seed()));
    }

    fn prng_seed() -> u64 {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);

        // Any non-zero seed will do
        let mut seed = 0;
        while seed == 0 {
            let mut hasher = DefaultHasher::new();
            hasher.write_usize(COUNTER.fetch_add(1, Ordering::Relaxed));
            seed = hasher.finish();
        }
        seed
    }

    RNG.with(|rng| {
        let mut x = rng.get();
        debug_assert_ne!(x.0, 0);
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        rng.set(x);
        x.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    })
}
