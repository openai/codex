//! The foldhash implementation optimized for speed.

use core::hash::{BuildHasher, Hasher};

use crate::seed::{gen_per_hasher_seed, GlobalSeed, SharedSeed};
use crate::{folded_multiply, hash_bytes_long, hash_bytes_medium, rotate_right, ARBITRARY3};

/// A [`Hasher`] instance implementing foldhash, optimized for speed.
///
/// While you can create one directly with [`FoldHasher::with_seed`], you
/// most likely want to use [`RandomState`], [`SeedableRandomState`] or
/// [`FixedState`] to create [`FoldHasher`]s.
#[derive(Clone)]
pub struct FoldHasher {
    accumulator: u64,
    sponge: u128,
    sponge_len: u8,
    fold_seed: u64,
    expand_seed: u64,
    expand_seed2: u64,
    expand_seed3: u64,
}

impl FoldHasher {
    /// Initializes this [`FoldHasher`] with the given per-hasher seed and
    /// [`SharedSeed`].
    #[inline]
    pub fn with_seed(per_hasher_seed: u64, shared_seed: &SharedSeed) -> FoldHasher {
        FoldHasher {
            accumulator: per_hasher_seed,
            sponge: 0,
            sponge_len: 0,
            fold_seed: shared_seed.seeds[0],
            expand_seed: shared_seed.seeds[1],
            expand_seed2: shared_seed.seeds[2],
            expand_seed3: shared_seed.seeds[3],
        }
    }

    #[inline(always)]
    fn write_num<T: Into<u128>>(&mut self, x: T) {
        let bits: usize = 8 * core::mem::size_of::<T>();
        if self.sponge_len as usize + bits > 128 {
            let lo = self.sponge as u64;
            let hi = (self.sponge >> 64) as u64;
            self.accumulator = folded_multiply(lo ^ self.accumulator, hi ^ self.fold_seed);
            self.sponge = x.into();
            self.sponge_len = bits as u8;
        } else {
            self.sponge |= x.into() << self.sponge_len;
            self.sponge_len += bits as u8;
        }
    }
}

impl Hasher for FoldHasher {
    #[inline(always)]
    fn write(&mut self, bytes: &[u8]) {
        // We perform overlapping reads in the byte hash which could lead to
        // trivial length-extension attacks. These should be defeated by
        // adding a length-dependent rotation on our unpredictable seed
        // which costs only a single cycle (or none if executed with
        // instruction-level parallelism).
        let len = bytes.len();
        let base_seed = rotate_right(self.accumulator, len as u32);
        if len <= 16 {
            let mut s0 = base_seed;
            let mut s1 = self.expand_seed;
            // XOR the input into s0, s1, then multiply and fold.
            if len >= 8 {
                s0 ^= u64::from_ne_bytes(bytes[0..8].try_into().unwrap());
                s1 ^= u64::from_ne_bytes(bytes[len - 8..].try_into().unwrap());
            } else if len >= 4 {
                s0 ^= u32::from_ne_bytes(bytes[0..4].try_into().unwrap()) as u64;
                s1 ^= u32::from_ne_bytes(bytes[len - 4..].try_into().unwrap()) as u64;
            } else if len > 0 {
                let lo = bytes[0];
                let mid = bytes[len / 2];
                let hi = bytes[len - 1];
                s0 ^= lo as u64;
                s1 ^= ((hi as u64) << 8) | mid as u64;
            }
            self.accumulator = folded_multiply(s0, s1);
        } else if len < 256 {
            self.accumulator = hash_bytes_medium(
                bytes,
                base_seed,
                base_seed.wrapping_add(self.expand_seed),
                self.fold_seed,
            );
        } else {
            self.accumulator = hash_bytes_long(
                bytes,
                base_seed,
                base_seed.wrapping_add(self.expand_seed),
                base_seed.wrapping_add(self.expand_seed2),
                base_seed.wrapping_add(self.expand_seed3),
                self.fold_seed,
            );
        }
    }

    #[inline(always)]
    fn write_u8(&mut self, i: u8) {
        self.write_num(i);
    }

    #[inline(always)]
    fn write_u16(&mut self, i: u16) {
        self.write_num(i);
    }

    #[inline(always)]
    fn write_u32(&mut self, i: u32) {
        self.write_num(i);
    }

    #[inline(always)]
    fn write_u64(&mut self, i: u64) {
        self.write_num(i);
    }

    #[inline(always)]
    fn write_u128(&mut self, i: u128) {
        let lo = i as u64;
        let hi = (i >> 64) as u64;
        self.accumulator = folded_multiply(lo ^ self.accumulator, hi ^ self.fold_seed);
    }

    #[inline(always)]
    fn write_usize(&mut self, i: usize) {
        // u128 doesn't implement From<usize>.
        #[cfg(target_pointer_width = "32")]
        self.write_num(i as u32);
        #[cfg(target_pointer_width = "64")]
        self.write_num(i as u64);
    }

    #[inline(always)]
    fn finish(&self) -> u64 {
        if self.sponge_len > 0 {
            let lo = self.sponge as u64;
            let hi = (self.sponge >> 64) as u64;
            folded_multiply(lo ^ self.accumulator, hi ^ self.fold_seed)
        } else {
            self.accumulator
        }
    }
}

/// A [`BuildHasher`] for [`fast::FoldHasher`](FoldHasher) that is randomly initialized.
#[derive(Copy, Clone, Debug)]
pub struct RandomState {
    per_hasher_seed: u64,
    global_seed: GlobalSeed,
}

impl Default for RandomState {
    #[inline(always)]
    fn default() -> Self {
        Self {
            per_hasher_seed: gen_per_hasher_seed(),
            global_seed: GlobalSeed::new(),
        }
    }
}

impl BuildHasher for RandomState {
    type Hasher = FoldHasher;

    #[inline(always)]
    fn build_hasher(&self) -> FoldHasher {
        FoldHasher::with_seed(self.per_hasher_seed, self.global_seed.get())
    }
}

/// A [`BuildHasher`] for [`fast::FoldHasher`](FoldHasher) that is randomly
/// initialized by default, but can also be initialized with a specific seed.
///
/// This can be useful for e.g. testing, but the downside is that this type
/// has a size of 16 bytes rather than the 8 bytes [`RandomState`] is.
#[derive(Copy, Clone, Debug)]
pub struct SeedableRandomState {
    per_hasher_seed: u64,
    shared_seed: &'static SharedSeed,
}

impl Default for SeedableRandomState {
    #[inline(always)]
    fn default() -> Self {
        Self::random()
    }
}

impl SeedableRandomState {
    /// Generates a random [`SeedableRandomState`], similar to [`RandomState`].
    #[inline(always)]
    pub fn random() -> Self {
        Self {
            per_hasher_seed: gen_per_hasher_seed(),
            shared_seed: SharedSeed::global_random(),
        }
    }

    /// Generates a fixed [`SeedableRandomState`], similar to [`FixedState`].
    #[inline(always)]
    pub fn fixed() -> Self {
        Self {
            per_hasher_seed: ARBITRARY3,
            shared_seed: SharedSeed::global_fixed(),
        }
    }

    /// Generates a [`SeedableRandomState`] with the given per-hasher seed
    /// and [`SharedSeed`].
    #[inline(always)]
    pub fn with_seed(per_hasher_seed: u64, shared_seed: &'static SharedSeed) -> Self {
        // XOR with ARBITRARY3 such that with_seed(0) matches default.
        Self {
            per_hasher_seed: per_hasher_seed ^ ARBITRARY3,
            shared_seed,
        }
    }
}

impl BuildHasher for SeedableRandomState {
    type Hasher = FoldHasher;

    #[inline(always)]
    fn build_hasher(&self) -> FoldHasher {
        FoldHasher::with_seed(self.per_hasher_seed, self.shared_seed)
    }
}

/// A [`BuildHasher`] for [`fast::FoldHasher`](FoldHasher) that always has the same fixed seed.
///
/// Not recommended unless you absolutely need determinism.
#[derive(Copy, Clone, Debug)]
pub struct FixedState {
    per_hasher_seed: u64,
}

impl FixedState {
    /// Creates a [`FixedState`] with the given per-hasher-seed.
    #[inline(always)]
    pub const fn with_seed(per_hasher_seed: u64) -> Self {
        // XOR with ARBITRARY3 such that with_seed(0) matches default.
        Self {
            per_hasher_seed: per_hasher_seed ^ ARBITRARY3,
        }
    }
}

impl Default for FixedState {
    #[inline(always)]
    fn default() -> Self {
        Self {
            per_hasher_seed: ARBITRARY3,
        }
    }
}

impl BuildHasher for FixedState {
    type Hasher = FoldHasher;

    #[inline(always)]
    fn build_hasher(&self) -> FoldHasher {
        FoldHasher::with_seed(self.per_hasher_seed, SharedSeed::global_fixed())
    }
}
