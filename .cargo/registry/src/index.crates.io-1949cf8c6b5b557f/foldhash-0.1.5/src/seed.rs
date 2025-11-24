// These constants may end up unused depending on platform support.
#[allow(unused)]
use crate::{ARBITRARY1, ARBITRARY9};

use super::{folded_multiply, ARBITRARY2, ARBITRARY4, ARBITRARY5, ARBITRARY6, ARBITRARY7};

/// Used for FixedState, and RandomState if atomics for dynamic init are unavailable.
const FIXED_GLOBAL_SEED: SharedSeed = SharedSeed {
    seeds: [ARBITRARY4, ARBITRARY5, ARBITRARY6, ARBITRARY7],
};

pub(crate) fn gen_per_hasher_seed() -> u64 {
    // We initialize the per-hasher seed with the stack pointer to ensure
    // different threads have different seeds, with as side benefit that
    // stack address randomization gives us further non-determinism.
    let mut per_hasher_seed = 0;
    let stack_ptr = core::ptr::addr_of!(per_hasher_seed) as u64;
    per_hasher_seed = stack_ptr;

    // If we have the standard library available we use a thread-local
    // state to ensure RandomStates are different with high probability,
    // even if the call stack is the same.
    #[cfg(feature = "std")]
    {
        use std::cell::Cell;
        thread_local! {
            static PER_HASHER_NONDETERMINISM: Cell<u64> = const { Cell::new(0) };
        }

        PER_HASHER_NONDETERMINISM.with(|cell| {
            let nondeterminism = cell.get();
            per_hasher_seed = folded_multiply(per_hasher_seed, ARBITRARY1 ^ nondeterminism);
            cell.set(per_hasher_seed);
        })
    };

    // If we don't have the standard library we instead use a global
    // atomic instead of a thread-local state.
    //
    // PER_HASHER_NONDETERMINISM is loaded and updated in a racy manner,
    // but this doesn't matter in practice - it is impossible that two
    // different threads have the same stack location, so they'll almost
    // surely generate different seeds, and provide a different possible
    // update for PER_HASHER_NONDETERMINISM. If we would use a proper
    // fetch_add atomic update then there is a larger chance of
    // problematic contention.
    //
    // We use usize instead of 64-bit atomics for best platform support.
    #[cfg(not(feature = "std"))]
    {
        use core::sync::atomic::{AtomicUsize, Ordering};
        static PER_HASHER_NONDETERMINISM: AtomicUsize = AtomicUsize::new(0);

        let nondeterminism = PER_HASHER_NONDETERMINISM.load(Ordering::Relaxed) as u64;
        per_hasher_seed = folded_multiply(per_hasher_seed, ARBITRARY1 ^ nondeterminism);
        PER_HASHER_NONDETERMINISM.store(per_hasher_seed as usize, Ordering::Relaxed);
    }

    // One extra mixing step to ensure good random bits.
    folded_multiply(per_hasher_seed, ARBITRARY2)
}

/// A random seed intended to be shared by many different foldhash instances.
///
/// This seed is consumed by [`FoldHasher::with_seed`](crate::fast::FoldHasher::with_seed),
/// and [`SeedableRandomState::with_seed`](crate::fast::SeedableRandomState::with_seed).
#[derive(Clone, Debug)]
pub struct SharedSeed {
    pub(crate) seeds: [u64; 4],
}

impl SharedSeed {
    /// Returns the globally shared randomly initialized [`SharedSeed`] as used
    /// by [`RandomState`](crate::fast::RandomState).
    #[inline(always)]
    pub fn global_random() -> &'static SharedSeed {
        global::GlobalSeed::new().get()
    }

    /// Returns the globally shared fixed [`SharedSeed`] as used
    /// by [`FixedState`](crate::fast::FixedState).
    #[inline(always)]
    pub const fn global_fixed() -> &'static SharedSeed {
        &FIXED_GLOBAL_SEED
    }

    /// Generates a new [`SharedSeed`] from a single 64-bit seed.
    ///
    /// Note that this is somewhat expensive so it is suggested to re-use the
    /// [`SharedSeed`] as much as possible, using the per-hasher seed to
    /// differentiate between hash instances.
    pub const fn from_u64(seed: u64) -> Self {
        macro_rules! mix {
            ($x: expr) => {
                folded_multiply($x, ARBITRARY9)
            };
        }

        let seed_a = mix!(mix!(mix!(seed)));
        let seed_b = mix!(mix!(mix!(seed_a)));
        let seed_c = mix!(mix!(mix!(seed_b)));
        let seed_d = mix!(mix!(mix!(seed_c)));

        // Zeroes form a weak-point for the multiply-mix, and zeroes tend to be
        // a common input. So we want our global seeds that are XOR'ed with the
        // input to always be non-zero. To also ensure there is always a good spread
        // of bits, we give up 3 bits of entropy and simply force some bits on.
        const FORCED_ONES: u64 = (1 << 63) | (1 << 31) | 1;
        Self {
            seeds: [
                seed_a | FORCED_ONES,
                seed_b | FORCED_ONES,
                seed_c | FORCED_ONES,
                seed_d | FORCED_ONES,
            ],
        }
    }
}

#[cfg(target_has_atomic = "8")]
mod global {
    use super::*;
    use core::cell::UnsafeCell;
    use core::sync::atomic::{AtomicU8, Ordering};

    fn generate_global_seed() -> SharedSeed {
        let mix = |seed: u64, x: u64| folded_multiply(seed ^ x, ARBITRARY9);

        // Use address space layout randomization as our main randomness source.
        // This isn't great, but we don't advertise HashDoS resistance in the first
        // place. This is a whole lot better than nothing, at near zero cost with
        // no dependencies.
        let mut seed = 0;
        let stack_ptr = &seed as *const _;
        let func_ptr = generate_global_seed;
        let static_ptr = &GLOBAL_SEED_STORAGE as *const _;
        seed = mix(seed, stack_ptr as usize as u64);
        seed = mix(seed, func_ptr as usize as u64);
        seed = mix(seed, static_ptr as usize as u64);

        // If we have the standard library available, augment entropy with the
        // current time and an address from the allocator.
        #[cfg(feature = "std")]
        {
            #[cfg(not(any(
                miri,
                all(target_family = "wasm", target_os = "unknown"),
                target_os = "zkvm"
            )))]
            if let Ok(duration) = std::time::UNIX_EPOCH.elapsed() {
                seed = mix(seed, duration.subsec_nanos() as u64);
                seed = mix(seed, duration.as_secs());
            }

            let box_ptr = &*Box::new(0u8) as *const _;
            seed = mix(seed, box_ptr as usize as u64);
        }

        SharedSeed::from_u64(seed)
    }

    // Now all the below code purely exists to cache the above seed as
    // efficiently as possible. Even if we weren't a no_std crate and had access to
    // OnceLock, we don't want to check whether the global is set each time we
    // hash an object, so we hand-roll a global storage where type safety allows us
    // to assume the storage is initialized after construction.
    struct GlobalSeedStorage {
        state: AtomicU8,
        seed: UnsafeCell<SharedSeed>,
    }

    const UNINIT: u8 = 0;
    const LOCKED: u8 = 1;
    const INIT: u8 = 2;

    // SAFETY: we only mutate the UnsafeCells when state is in the thread-exclusive
    // LOCKED state, and only read the UnsafeCells when state is in the
    // once-achieved-eternally-preserved state INIT.
    unsafe impl Sync for GlobalSeedStorage {}

    static GLOBAL_SEED_STORAGE: GlobalSeedStorage = GlobalSeedStorage {
        state: AtomicU8::new(UNINIT),
        seed: UnsafeCell::new(SharedSeed { seeds: [0; 4] }),
    };

    /// An object representing an initialized global seed.
    ///
    /// Does not actually store the seed inside itself, it is a zero-sized type.
    /// This prevents inflating the RandomState size and in turn HashMap's size.
    #[derive(Copy, Clone, Debug)]
    pub struct GlobalSeed {
        // So we can't accidentally type GlobalSeed { } within this crate.
        _no_accidental_unsafe_init: (),
    }

    impl GlobalSeed {
        #[inline(always)]
        pub fn new() -> Self {
            if GLOBAL_SEED_STORAGE.state.load(Ordering::Acquire) != INIT {
                Self::init_slow()
            }
            Self {
                _no_accidental_unsafe_init: (),
            }
        }

        #[cold]
        #[inline(never)]
        fn init_slow() {
            // Generate seed outside of critical section.
            let seed = generate_global_seed();

            loop {
                match GLOBAL_SEED_STORAGE.state.compare_exchange_weak(
                    UNINIT,
                    LOCKED,
                    Ordering::Acquire,
                    Ordering::Acquire,
                ) {
                    Ok(_) => unsafe {
                        // SAFETY: we just acquired an exclusive lock.
                        *GLOBAL_SEED_STORAGE.seed.get() = seed;
                        GLOBAL_SEED_STORAGE.state.store(INIT, Ordering::Release);
                        return;
                    },

                    Err(INIT) => return,

                    // Yes, it's a spin loop. We need to support no_std (so no easy
                    // access to proper locks), this is a one-time-per-program
                    // initialization, and the critical section is only a few
                    // store instructions, so it'll be fine.
                    _ => core::hint::spin_loop(),
                }
            }
        }

        #[inline(always)]
        pub fn get(self) -> &'static SharedSeed {
            // SAFETY: our constructor ensured we are in the INIT state and thus
            // this raw read does not race with any write.
            unsafe { &*GLOBAL_SEED_STORAGE.seed.get() }
        }
    }
}

#[cfg(not(target_has_atomic = "8"))]
mod global {
    use super::*;

    #[derive(Copy, Clone, Debug)]
    pub struct GlobalSeed {}

    impl GlobalSeed {
        #[inline(always)]
        pub fn new() -> Self {
            Self {}
        }

        #[inline(always)]
        pub fn get(self) -> &'static SharedSeed {
            &super::FIXED_GLOBAL_SEED
        }
    }
}

pub(crate) use global::GlobalSeed;
