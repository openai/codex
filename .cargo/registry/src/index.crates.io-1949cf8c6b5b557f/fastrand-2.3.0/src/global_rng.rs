//! A global, thread-local random number generator.

use crate::Rng;

use std::cell::Cell;
use std::ops::RangeBounds;
use std::vec::Vec;

// Chosen by fair roll of the dice.
const DEFAULT_RNG_SEED: u64 = 0xef6f79ed30ba75a;

impl Default for Rng {
    /// Initialize the `Rng` from the system's random number generator.
    ///
    /// This is equivalent to [`Rng::new()`].
    #[inline]
    fn default() -> Rng {
        Rng::new()
    }
}

impl Rng {
    /// Creates a new random number generator.
    #[inline]
    pub fn new() -> Rng {
        try_with_rng(Rng::fork).unwrap_or_else(|_| Rng::with_seed(0x4d595df4d0f33173))
    }
}

std::thread_local! {
    static RNG: Cell<Rng> = Cell::new(Rng(random_seed().unwrap_or(DEFAULT_RNG_SEED)));
}

/// Run an operation with the current thread-local generator.
#[inline]
fn with_rng<R>(f: impl FnOnce(&mut Rng) -> R) -> R {
    RNG.with(|rng| {
        let current = rng.replace(Rng(0));

        let mut restore = RestoreOnDrop { rng, current };

        f(&mut restore.current)
    })
}

/// Try to run an operation with the current thread-local generator.
#[inline]
fn try_with_rng<R>(f: impl FnOnce(&mut Rng) -> R) -> Result<R, std::thread::AccessError> {
    RNG.try_with(|rng| {
        let current = rng.replace(Rng(0));

        let mut restore = RestoreOnDrop { rng, current };

        f(&mut restore.current)
    })
}

/// Make sure the original RNG is restored even on panic.
struct RestoreOnDrop<'a> {
    rng: &'a Cell<Rng>,
    current: Rng,
}

impl Drop for RestoreOnDrop<'_> {
    fn drop(&mut self) {
        self.rng.set(Rng(self.current.0));
    }
}

/// Initializes the thread-local generator with the given seed.
#[inline]
pub fn seed(seed: u64) {
    with_rng(|r| r.seed(seed));
}

/// Gives back **current** seed that is being held by the thread-local generator.
#[inline]
pub fn get_seed() -> u64 {
    with_rng(|r| r.get_seed())
}

/// Generates a random `bool`.
#[inline]
pub fn bool() -> bool {
    with_rng(|r| r.bool())
}

/// Generates a random `char` in ranges a-z and A-Z.
#[inline]
pub fn alphabetic() -> char {
    with_rng(|r| r.alphabetic())
}

/// Generates a random `char` in ranges a-z, A-Z and 0-9.
#[inline]
pub fn alphanumeric() -> char {
    with_rng(|r| r.alphanumeric())
}

/// Generates a random `char` in range a-z.
#[inline]
pub fn lowercase() -> char {
    with_rng(|r| r.lowercase())
}

/// Generates a random `char` in range A-Z.
#[inline]
pub fn uppercase() -> char {
    with_rng(|r| r.uppercase())
}

/// Choose an item from an iterator at random.
///
/// This function may have an unexpected result if the `len()` property of the
/// iterator does not match the actual number of items in the iterator. If
/// the iterator is empty, this returns `None`.
#[inline]
pub fn choice<I>(iter: I) -> Option<I::Item>
where
    I: IntoIterator,
    I::IntoIter: ExactSizeIterator,
{
    with_rng(|r| r.choice(iter))
}

/// Generates a random digit in the given `base`.
///
/// Digits are represented by `char`s in ranges 0-9 and a-z.
///
/// Panics if the base is zero or greater than 36.
#[inline]
pub fn digit(base: u32) -> char {
    with_rng(|r| r.digit(base))
}

/// Shuffles a slice randomly.
#[inline]
pub fn shuffle<T>(slice: &mut [T]) {
    with_rng(|r| r.shuffle(slice))
}

/// Fill a byte slice with random data.
#[inline]
pub fn fill(slice: &mut [u8]) {
    with_rng(|r| r.fill(slice))
}

macro_rules! integer {
    ($t:tt, $doc:tt) => {
        #[doc = $doc]
        ///
        /// Panics if the range is empty.
        #[inline]
        pub fn $t(range: impl RangeBounds<$t>) -> $t {
            with_rng(|r| r.$t(range))
        }
    };
}

integer!(u8, "Generates a random `u8` in the given range.");
integer!(i8, "Generates a random `i8` in the given range.");
integer!(u16, "Generates a random `u16` in the given range.");
integer!(i16, "Generates a random `i16` in the given range.");
integer!(u32, "Generates a random `u32` in the given range.");
integer!(i32, "Generates a random `i32` in the given range.");
integer!(u64, "Generates a random `u64` in the given range.");
integer!(i64, "Generates a random `i64` in the given range.");
integer!(u128, "Generates a random `u128` in the given range.");
integer!(i128, "Generates a random `i128` in the given range.");
integer!(usize, "Generates a random `usize` in the given range.");
integer!(isize, "Generates a random `isize` in the given range.");
integer!(char, "Generates a random `char` in the given range.");

/// Generates a random `f32` in range `0..1`.
pub fn f32() -> f32 {
    with_rng(|r| r.f32())
}

/// Generates a random `f64` in range `0..1`.
pub fn f64() -> f64 {
    with_rng(|r| r.f64())
}

/// Collects `amount` values at random from the iterable into a vector.
pub fn choose_multiple<I: IntoIterator>(source: I, amount: usize) -> Vec<I::Item> {
    with_rng(|rng| rng.choose_multiple(source, amount))
}

#[cfg(not(all(
    any(target_arch = "wasm32", target_arch = "wasm64"),
    target_os = "unknown"
)))]
fn random_seed() -> Option<u64> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::thread;
    use std::time::Instant;

    let mut hasher = DefaultHasher::new();
    Instant::now().hash(&mut hasher);
    thread::current().id().hash(&mut hasher);
    Some(hasher.finish())
}

#[cfg(all(
    any(target_arch = "wasm32", target_arch = "wasm64"),
    target_os = "unknown",
    feature = "js"
))]
fn random_seed() -> Option<u64> {
    // TODO(notgull): Failures should be logged somewhere.
    let mut seed = [0u8; 8];
    getrandom::getrandom(&mut seed).ok()?;
    Some(u64::from_ne_bytes(seed))
}

#[cfg(all(
    any(target_arch = "wasm32", target_arch = "wasm64"),
    target_os = "unknown",
    not(feature = "js")
))]
fn random_seed() -> Option<u64> {
    None
}
