use alloc::boxed::Box;

pub use crate::msgs::enums::HashAlgorithm;

/// Describes a single cryptographic hash function.
///
/// This interface can do both one-shot and incremental hashing, using
/// [`Hash::hash()`] and [`Hash::start()`] respectively.
pub trait Hash: Send + Sync {
    /// Start an incremental hash computation.
    fn start(&self) -> Box<dyn Context>;

    /// Return the output of this hash function with input `data`.
    fn hash(&self, data: &[u8]) -> Output;

    /// The length in bytes of this hash function's output.
    fn output_len(&self) -> usize;

    /// Which hash function this is, eg, `HashAlgorithm::SHA256`.
    fn algorithm(&self) -> HashAlgorithm;

    /// Return `true` if this is backed by a FIPS-approved implementation.
    fn fips(&self) -> bool {
        false
    }
}

/// A hash output, stored as a value.
pub struct Output {
    buf: [u8; Self::MAX_LEN],
    used: usize,
}

impl Output {
    /// Build a `hash::Output` from a slice of no more than `Output::MAX_LEN` bytes.
    pub fn new(bytes: &[u8]) -> Self {
        let mut output = Self {
            buf: [0u8; Self::MAX_LEN],
            used: bytes.len(),
        };
        debug_assert!(bytes.len() <= Self::MAX_LEN);
        output.buf[..bytes.len()].copy_from_slice(bytes);
        output
    }

    /// Maximum supported hash output size: supports up to SHA512.
    pub const MAX_LEN: usize = 64;
}

impl AsRef<[u8]> for Output {
    fn as_ref(&self) -> &[u8] {
        &self.buf[..self.used]
    }
}

/// How to incrementally compute a hash.
pub trait Context: Send + Sync {
    /// Finish the computation, returning the resulting output.
    ///
    /// The computation remains valid, and more data can be added later with
    /// [`Context::update()`].
    ///
    /// Compare with [`Context::finish()`] which consumes the computation
    /// and prevents any further data being added.  This can be more efficient
    /// because it avoids a hash context copy to apply Merkle-Damgård padding
    /// (if required).
    fn fork_finish(&self) -> Output;

    /// Fork the computation, producing another context that has the
    /// same prefix as this one.
    fn fork(&self) -> Box<dyn Context>;

    /// Terminate and finish the computation, returning the resulting output.
    ///
    /// Further data cannot be added after this, because the context is consumed.
    /// Compare [`Context::fork_finish()`].
    fn finish(self: Box<Self>) -> Output;

    /// Add `data` to computation.
    fn update(&mut self, data: &[u8]);
}
