//! The foldhash implementation optimized for quality.

use core::hash::{BuildHasher, Hasher};

use crate::seed::SharedSeed;

use crate::{fast, folded_multiply, ARBITRARY0, ARBITRARY8};

/// A [`Hasher`] instance implementing foldhash, optimized for quality.
///
/// While you can create one directly with [`FoldHasher::with_seed`], you
/// most likely want to use [`RandomState`], [`SeedableRandomState`] or
/// [`FixedState`] to create [`FoldHasher`]s.
#[derive(Clone)]
pub struct FoldHasher {
    pub(crate) inner: fast::FoldHasher,
}

impl FoldHasher {
    /// Initializes this [`FoldHasher`] with the given per-hasher seed and
    /// [`SharedSeed`].
    #[inline(always)]
    pub fn with_seed(per_hasher_seed: u64, shared_seed: &SharedSeed) -> FoldHasher {
        FoldHasher {
            inner: fast::FoldHasher::with_seed(per_hasher_seed, shared_seed),
        }
    }
}

impl Hasher for FoldHasher {
    #[inline(always)]
    fn write(&mut self, bytes: &[u8]) {
        self.inner.write(bytes);
    }

    #[inline(always)]
    fn write_u8(&mut self, i: u8) {
        self.inner.write_u8(i);
    }

    #[inline(always)]
    fn write_u16(&mut self, i: u16) {
        self.inner.write_u16(i);
    }

    #[inline(always)]
    fn write_u32(&mut self, i: u32) {
        self.inner.write_u32(i);
    }

    #[inline(always)]
    fn write_u64(&mut self, i: u64) {
        self.inner.write_u64(i);
    }

    #[inline(always)]
    fn write_u128(&mut self, i: u128) {
        self.inner.write_u128(i);
    }

    #[inline(always)]
    fn write_usize(&mut self, i: usize) {
        self.inner.write_usize(i);
    }

    #[inline(always)]
    fn finish(&self) -> u64 {
        folded_multiply(self.inner.finish(), ARBITRARY0)
    }
}

/// A [`BuildHasher`] for [`quality::FoldHasher`](FoldHasher) that is randomly initialized.
#[derive(Copy, Clone, Default, Debug)]
pub struct RandomState {
    inner: fast::RandomState,
}

impl BuildHasher for RandomState {
    type Hasher = FoldHasher;

    #[inline(always)]
    fn build_hasher(&self) -> FoldHasher {
        FoldHasher {
            inner: self.inner.build_hasher(),
        }
    }
}

/// A [`BuildHasher`] for [`quality::FoldHasher`](FoldHasher) that is randomly
/// initialized by default, but can also be initialized with a specific seed.
///
/// This can be useful for e.g. testing, but the downside is that this type
/// has a size of 16 bytes rather than the 8 bytes [`RandomState`] is.
#[derive(Copy, Clone, Default, Debug)]
pub struct SeedableRandomState {
    inner: fast::SeedableRandomState,
}

impl SeedableRandomState {
    /// Generates a random [`SeedableRandomState`], similar to [`RandomState`].
    #[inline(always)]
    pub fn random() -> Self {
        Self {
            inner: fast::SeedableRandomState::random(),
        }
    }

    /// Generates a fixed [`SeedableRandomState`], similar to [`FixedState`].
    #[inline(always)]
    pub fn fixed() -> Self {
        Self {
            inner: fast::SeedableRandomState::fixed(),
        }
    }

    /// Generates a [`SeedableRandomState`] with the given per-hasher seed
    /// and [`SharedSeed`].
    #[inline(always)]
    pub fn with_seed(per_hasher_seed: u64, shared_seed: &'static SharedSeed) -> Self {
        Self {
            // We do an additional folded multiply with the seed here for
            // the quality hash to ensure better independence between seed
            // and hash.
            inner: fast::SeedableRandomState::with_seed(
                folded_multiply(per_hasher_seed, ARBITRARY8),
                shared_seed,
            ),
        }
    }
}

impl BuildHasher for SeedableRandomState {
    type Hasher = FoldHasher;

    #[inline(always)]
    fn build_hasher(&self) -> FoldHasher {
        FoldHasher {
            inner: self.inner.build_hasher(),
        }
    }
}

/// A [`BuildHasher`] for [`quality::FoldHasher`](FoldHasher) that always has the same fixed seed.
///
/// Not recommended unless you absolutely need determinism.
#[derive(Copy, Clone, Default, Debug)]
pub struct FixedState {
    inner: fast::FixedState,
}

impl FixedState {
    /// Creates a [`FixedState`] with the given per-hasher seed.
    #[inline(always)]
    pub const fn with_seed(per_hasher_seed: u64) -> Self {
        Self {
            // We do an additional folded multiply with the seed here for
            // the quality hash to ensure better independence between seed
            // and hash. If the seed is zero the folded multiply is zero,
            // preserving with_seed(0) == default().
            inner: fast::FixedState::with_seed(folded_multiply(per_hasher_seed, ARBITRARY8)),
        }
    }
}

impl BuildHasher for FixedState {
    type Hasher = FoldHasher;

    #[inline(always)]
    fn build_hasher(&self) -> FoldHasher {
        FoldHasher {
            inner: self.inner.build_hasher(),
        }
    }
}
