// Copyright 2015-2019 Brian Smith.
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY
// SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION
// OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN
// CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

//! SHA-2 and the legacy SHA-1 digest algorithm.
//!
//! If all the data is available in a single contiguous slice then the `digest`
//! function should be used. Otherwise, the digest can be calculated in
//! multiple steps using `Context`.

use self::{
    dynstate::DynState,
    sha2::{SHA256_BLOCK_LEN, SHA512_BLOCK_LEN},
};
use crate::{
    bits::{BitLength, FromByteLen as _},
    cpu, debug, error,
    polyfill::{self, slice, sliceutil},
};
use core::num::Wrapping;

pub(crate) use self::finish_error::FinishError;

mod dynstate;
mod sha1;
mod sha2;

#[derive(Clone)]
pub(crate) struct BlockContext {
    state: DynState,

    // Note that SHA-512 has a 128-bit input bit counter, but this
    // implementation only supports up to 2^64-1 input bits for all algorithms,
    // so a 64-bit counter is more than sufficient.
    completed_bytes: u64,

    /// The context's algorithm.
    pub algorithm: &'static Algorithm,
}

impl BlockContext {
    pub(crate) fn new(algorithm: &'static Algorithm) -> Self {
        Self {
            state: algorithm.initial_state.clone(),
            completed_bytes: 0,
            algorithm,
        }
    }

    /// Processes all the full blocks in `input`, returning the partial block
    /// at the end, which may be empty.
    pub(crate) fn update<'i>(&mut self, input: &'i [u8], cpu_features: cpu::Features) -> &'i [u8] {
        let (completed_bytes, leftover) = self.block_data_order(input, cpu_features);
        // Using saturated addition here allows `update` to be infallible and
        // panic-free. If we were to reach the maximum value here then `finish`
        // will detect that we processed too much data when it converts this to
        // a bit length.
        self.completed_bytes = self
            .completed_bytes
            .saturating_add(polyfill::u64_from_usize(completed_bytes));
        leftover
    }

    // On input, `block[..num_pending]` is the (possibly-empty) last *partial*
    // chunk of input. It *must* be partial; that is, it is required that
    // `num_pending < self.algorithm.block_len`.
    //
    // `block` may be arbitrarily overwritten.
    pub(crate) fn try_finish(
        mut self,
        block: &mut [u8; MAX_BLOCK_LEN],
        num_pending: usize,
        cpu_features: cpu::Features,
    ) -> Result<Digest, FinishError> {
        let completed_bits = self
            .completed_bytes
            .checked_add(polyfill::u64_from_usize(num_pending))
            .ok_or_else(|| {
                // Choosing self.completed_bytes here is lossy & somewhat arbitrary.
                InputTooLongError::new(self.completed_bytes)
            })
            .and_then(BitLength::from_byte_len)
            .map_err(FinishError::input_too_long)?;

        let block_len = self.algorithm.block_len();
        let block = &mut block[..block_len];

        let padding = match block.get_mut(num_pending..) {
            Some([separator, padding @ ..]) => {
                *separator = 0x80;
                padding
            }
            // Precondition violated.
            unreachable => {
                return Err(FinishError::pending_not_a_partial_block(
                    unreachable.as_deref(),
                ));
            }
        };

        let padding = match padding
            .len()
            .checked_sub(self.algorithm.block_len.len_len())
        {
            Some(_) => padding,
            None => {
                padding.fill(0);
                let (completed_bytes, leftover) = self.block_data_order(block, cpu_features);
                debug_assert_eq!((completed_bytes, leftover.len()), (block_len, 0));
                // We don't increase |self.completed_bytes| because the padding
                // isn't data, and so it isn't included in the data length.
                &mut block[..]
            }
        };

        let (to_zero, len) = padding.split_at_mut(padding.len() - 8);
        to_zero.fill(0);
        len.copy_from_slice(&completed_bits.to_be_bytes());

        let (completed_bytes, leftover) = self.block_data_order(block, cpu_features);
        debug_assert_eq!((completed_bytes, leftover.len()), (block_len, 0));

        Ok(Digest {
            algorithm: self.algorithm,
            value: self.state.format_output(),
        })
    }

    #[must_use]
    fn block_data_order<'d>(
        &mut self,
        data: &'d [u8],
        cpu_features: cpu::Features,
    ) -> (usize, &'d [u8]) {
        (self.algorithm.block_data_order)(&mut self.state, data, cpu_features)
    }
}

pub(crate) type InputTooLongError = error::InputTooLongError<u64>;

cold_exhaustive_error! {
    enum finish_error::FinishError {
        input_too_long => InputTooLong(InputTooLongError),
        pending_not_a_partial_block_inner => PendingNotAPartialBlock(usize),
    }
}

impl FinishError {
    #[cold]
    #[inline(never)]
    fn pending_not_a_partial_block(padding: Option<&[u8]>) -> Self {
        match padding {
            None => Self::pending_not_a_partial_block_inner(0),
            Some(padding) => Self::pending_not_a_partial_block_inner(padding.len()),
        }
    }
}

/// A context for multi-step (Init-Update-Finish) digest calculations.
///
/// # Examples
///
/// ```
/// use ring::digest;
///
/// let one_shot = digest::digest(&digest::SHA384, b"hello, world");
///
/// let mut ctx = digest::Context::new(&digest::SHA384);
/// ctx.update(b"hello");
/// ctx.update(b", ");
/// ctx.update(b"world");
/// let multi_part = ctx.finish();
///
/// assert_eq!(&one_shot.as_ref(), &multi_part.as_ref());
/// ```
#[derive(Clone)]
pub struct Context {
    block: BlockContext,
    // TODO: More explicitly force 64-bit alignment for |pending|.
    pending: [u8; MAX_BLOCK_LEN],

    // Invariant: `self.num_pending < self.block.algorithm.block_len`.
    num_pending: usize,
}

impl Context {
    /// Constructs a new context.
    pub fn new(algorithm: &'static Algorithm) -> Self {
        Self {
            block: BlockContext::new(algorithm),
            pending: [0u8; MAX_BLOCK_LEN],
            num_pending: 0,
        }
    }

    pub(crate) fn clone_from(block: &BlockContext) -> Self {
        Self {
            block: block.clone(),
            pending: [0u8; MAX_BLOCK_LEN],
            num_pending: 0,
        }
    }

    /// Updates the digest with all the data in `data`.
    pub fn update(&mut self, data: &[u8]) {
        let cpu_features = cpu::features();

        let block_len = self.block.algorithm.block_len();
        let buffer = &mut self.pending[..block_len];

        let to_digest = if self.num_pending == 0 {
            data
        } else {
            let buffer_to_fill = match buffer.get_mut(self.num_pending..) {
                Some(buffer_to_fill) => buffer_to_fill,
                None => {
                    // Impossible because of the invariant.
                    unreachable!();
                }
            };
            sliceutil::overwrite_at_start(buffer_to_fill, data);
            match slice::split_at_checked(data, buffer_to_fill.len()) {
                Some((just_copied, to_digest)) => {
                    debug_assert_eq!(buffer_to_fill.len(), just_copied.len());
                    debug_assert_eq!(self.num_pending + just_copied.len(), block_len);
                    let leftover = self.block.update(buffer, cpu_features);
                    debug_assert_eq!(leftover.len(), 0);
                    self.num_pending = 0;
                    to_digest
                }
                None => {
                    self.num_pending += data.len();
                    // If `data` isn't enough to complete a block, buffer it and stop.
                    debug_assert!(self.num_pending < block_len);
                    return;
                }
            }
        };

        let leftover = self.block.update(to_digest, cpu_features);
        sliceutil::overwrite_at_start(buffer, leftover);
        self.num_pending = leftover.len();
        debug_assert!(self.num_pending < block_len);
    }

    /// Finalizes the digest calculation and returns the digest value.
    ///
    /// `finish` consumes the context so it cannot be (mis-)used after `finish`
    /// has been called.
    pub fn finish(self) -> Digest {
        let cpu = cpu::features();
        self.try_finish(cpu)
            .map_err(error::erase::<InputTooLongError>)
            .unwrap()
    }

    pub(crate) fn try_finish(
        mut self,
        cpu_features: cpu::Features,
    ) -> Result<Digest, InputTooLongError> {
        self.block
            .try_finish(&mut self.pending, self.num_pending, cpu_features)
            .map_err(|err| match err {
                FinishError::InputTooLong(i) => i,
                FinishError::PendingNotAPartialBlock(_) => {
                    // Due to invariant.
                    unreachable!()
                }
            })
    }

    /// The algorithm that this context is using.
    #[inline(always)]
    pub fn algorithm(&self) -> &'static Algorithm {
        self.block.algorithm
    }
}

/// Returns the digest of `data` using the given digest algorithm.
pub fn digest(algorithm: &'static Algorithm, data: &[u8]) -> Digest {
    let cpu = cpu::features();
    Digest::compute_from(algorithm, data, cpu)
        .map_err(error::erase::<InputTooLongError>)
        .unwrap()
}

/// A calculated digest value.
///
/// Use [`Self::as_ref`] to get the value as a `&[u8]`.
#[derive(Clone, Copy)]
pub struct Digest {
    value: Output,
    algorithm: &'static Algorithm,
}

impl Digest {
    pub(crate) fn compute_from(
        algorithm: &'static Algorithm,
        data: &[u8],
        cpu: cpu::Features,
    ) -> Result<Self, InputTooLongError> {
        let mut ctx = Context::new(algorithm);
        ctx.update(data);
        ctx.try_finish(cpu)
    }

    /// The algorithm that was used to calculate the digest value.
    #[inline(always)]
    pub fn algorithm(&self) -> &'static Algorithm {
        self.algorithm
    }
}

impl AsRef<[u8]> for Digest {
    #[inline(always)]
    fn as_ref(&self) -> &[u8] {
        &self.value.0[..self.algorithm.output_len()]
    }
}

impl core::fmt::Debug for Digest {
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(fmt, "{:?}:", self.algorithm)?;
        debug::write_hex_bytes(fmt, self.as_ref())
    }
}

/// A digest algorithm.
pub struct Algorithm {
    output_len: OutputLen,
    chaining_len: usize,
    block_len: BlockLen,

    /// `block_data_order` processes all the full blocks of data in `data`. It
    /// returns the number of bytes processed and the unprocessed data, which
    /// is guaranteed to be less than `block_len` bytes long.
    block_data_order: for<'d> fn(
        state: &mut DynState,
        data: &'d [u8],
        cpu_features: cpu::Features,
    ) -> (usize, &'d [u8]),

    initial_state: DynState,

    id: AlgorithmID,
}

#[derive(Debug, Eq, PartialEq)]
enum AlgorithmID {
    SHA1,
    SHA256,
    SHA384,
    SHA512,
    SHA512_256,
}

impl PartialEq for Algorithm {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Algorithm {}

derive_debug_via_id!(Algorithm);

impl Algorithm {
    /// The internal block length.
    pub fn block_len(&self) -> usize {
        self.block_len.into()
    }

    /// The size of the chaining value of the digest function, in bytes.
    ///
    /// For non-truncated algorithms (SHA-1, SHA-256, SHA-512), this is equal
    /// to [`Self::output_len()`]. For truncated algorithms (e.g. SHA-384,
    /// SHA-512/256), this is equal to the length before truncation. This is
    /// mostly helpful for determining the size of an HMAC key that is
    /// appropriate for the digest algorithm.
    pub fn chaining_len(&self) -> usize {
        self.chaining_len
    }

    /// The length of a finalized digest.
    pub fn output_len(&self) -> usize {
        self.output_len.into()
    }
}

/// SHA-1 as specified in [FIPS 180-4]. Deprecated.
///
/// [FIPS 180-4]: http://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.180-4.pdf
pub static SHA1_FOR_LEGACY_USE_ONLY: Algorithm = Algorithm {
    output_len: sha1::OUTPUT_LEN,
    chaining_len: sha1::CHAINING_LEN,
    block_len: sha1::BLOCK_LEN,
    block_data_order: dynstate::sha1_block_data_order,
    initial_state: DynState::new32([
        Wrapping(0x67452301u32),
        Wrapping(0xefcdab89u32),
        Wrapping(0x98badcfeu32),
        Wrapping(0x10325476u32),
        Wrapping(0xc3d2e1f0u32),
        Wrapping(0),
        Wrapping(0),
        Wrapping(0),
    ]),
    id: AlgorithmID::SHA1,
};

/// SHA-256 as specified in [FIPS 180-4].
///
/// [FIPS 180-4]: http://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.180-4.pdf
pub static SHA256: Algorithm = Algorithm {
    output_len: OutputLen::_256,
    chaining_len: SHA256_OUTPUT_LEN,
    block_len: SHA256_BLOCK_LEN,
    block_data_order: dynstate::sha256_block_data_order,
    initial_state: DynState::new32([
        Wrapping(0x6a09e667u32),
        Wrapping(0xbb67ae85u32),
        Wrapping(0x3c6ef372u32),
        Wrapping(0xa54ff53au32),
        Wrapping(0x510e527fu32),
        Wrapping(0x9b05688cu32),
        Wrapping(0x1f83d9abu32),
        Wrapping(0x5be0cd19u32),
    ]),
    id: AlgorithmID::SHA256,
};

/// SHA-384 as specified in [FIPS 180-4].
///
/// [FIPS 180-4]: http://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.180-4.pdf
pub static SHA384: Algorithm = Algorithm {
    output_len: OutputLen::_384,
    chaining_len: SHA512_OUTPUT_LEN,
    block_len: SHA512_BLOCK_LEN,
    block_data_order: dynstate::sha512_block_data_order,
    initial_state: DynState::new64([
        Wrapping(0xcbbb9d5dc1059ed8),
        Wrapping(0x629a292a367cd507),
        Wrapping(0x9159015a3070dd17),
        Wrapping(0x152fecd8f70e5939),
        Wrapping(0x67332667ffc00b31),
        Wrapping(0x8eb44a8768581511),
        Wrapping(0xdb0c2e0d64f98fa7),
        Wrapping(0x47b5481dbefa4fa4),
    ]),
    id: AlgorithmID::SHA384,
};

/// SHA-512 as specified in [FIPS 180-4].
///
/// [FIPS 180-4]: http://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.180-4.pdf
pub static SHA512: Algorithm = Algorithm {
    output_len: OutputLen::_512,
    chaining_len: SHA512_OUTPUT_LEN,
    block_len: SHA512_BLOCK_LEN,
    block_data_order: dynstate::sha512_block_data_order,
    initial_state: DynState::new64([
        Wrapping(0x6a09e667f3bcc908),
        Wrapping(0xbb67ae8584caa73b),
        Wrapping(0x3c6ef372fe94f82b),
        Wrapping(0xa54ff53a5f1d36f1),
        Wrapping(0x510e527fade682d1),
        Wrapping(0x9b05688c2b3e6c1f),
        Wrapping(0x1f83d9abfb41bd6b),
        Wrapping(0x5be0cd19137e2179),
    ]),
    id: AlgorithmID::SHA512,
};

/// SHA-512/256 as specified in [FIPS 180-4].
///
/// This is *not* the same as just truncating the output of SHA-512, as
/// SHA-512/256 has its own initial state distinct from SHA-512's initial
/// state.
///
/// [FIPS 180-4]: http://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.180-4.pdf
pub static SHA512_256: Algorithm = Algorithm {
    output_len: OutputLen::_256,
    chaining_len: SHA512_OUTPUT_LEN,
    block_len: SHA512_BLOCK_LEN,
    block_data_order: dynstate::sha512_block_data_order,
    initial_state: DynState::new64([
        Wrapping(0x22312194fc2bf72c),
        Wrapping(0x9f555fa3c84c64c2),
        Wrapping(0x2393b86b6f53b151),
        Wrapping(0x963877195940eabd),
        Wrapping(0x96283ee2a88effe3),
        Wrapping(0xbe5e1e2553863992),
        Wrapping(0x2b0199fc2c85b8aa),
        Wrapping(0x0eb72ddc81c52ca2),
    ]),
    id: AlgorithmID::SHA512_256,
};

#[derive(Clone, Copy)]
struct Output([u8; MAX_OUTPUT_LEN]);

/// The maximum block length ([`Algorithm::block_len()`]) of all the algorithms
/// in this module.
pub const MAX_BLOCK_LEN: usize = BlockLen::MAX.into();

/// The maximum output length ([`Algorithm::output_len()`]) of all the
/// algorithms in this module.
pub const MAX_OUTPUT_LEN: usize = OutputLen::MAX.into();

/// The maximum chaining length ([`Algorithm::chaining_len()`]) of all the
/// algorithms in this module.
pub const MAX_CHAINING_LEN: usize = MAX_OUTPUT_LEN;

#[inline]
fn format_output<T, F, const N: usize>(input: [Wrapping<T>; sha2::CHAINING_WORDS], f: F) -> Output
where
    F: Fn(T) -> [u8; N],
    T: Copy,
{
    let mut output = Output([0; MAX_OUTPUT_LEN]);
    output
        .0
        .chunks_mut(N)
        .zip(input.iter().copied().map(|Wrapping(w)| f(w)))
        .for_each(|(o, i)| {
            o.copy_from_slice(&i);
        });
    output
}

/// The length of the output of SHA-1, in bytes.
pub const SHA1_OUTPUT_LEN: usize = sha1::OUTPUT_LEN.into();

/// The length of the output of SHA-256, in bytes.
pub const SHA256_OUTPUT_LEN: usize = OutputLen::_256.into();

/// The length of the output of SHA-384, in bytes.
pub const SHA384_OUTPUT_LEN: usize = OutputLen::_384.into();

/// The length of the output of SHA-512, in bytes.
pub const SHA512_OUTPUT_LEN: usize = OutputLen::_512.into();

/// The length of the output of SHA-512/256, in bytes.
pub const SHA512_256_OUTPUT_LEN: usize = OutputLen::_256.into();

#[derive(Clone, Copy)]
enum BlockLen {
    _512 = 512 / 8,
    _1024 = 1024 / 8, // MAX
}

impl BlockLen {
    const MAX: Self = Self::_1024;
    #[inline(always)]
    const fn into(self) -> usize {
        self as usize
    }

    #[inline(always)]
    const fn len_len(self) -> usize {
        let len_len = match self {
            BlockLen::_512 => LenLen::_64,
            BlockLen::_1024 => LenLen::_128,
        };
        len_len as usize
    }
}

#[derive(Clone, Copy)]
enum LenLen {
    _64 = 64 / 8,
    _128 = 128 / 8,
}

#[derive(Clone, Copy)]
enum OutputLen {
    _160 = 160 / 8,
    _256 = 256 / 8,
    _384 = 384 / 8,
    _512 = 512 / 8, // MAX
}

impl OutputLen {
    const MAX: Self = Self::_512;

    #[inline(always)]
    const fn into(self) -> usize {
        self as usize
    }
}

#[cfg(test)]
mod tests {
    mod max_input {
        extern crate alloc;
        use super::super::super::digest;
        use crate::polyfill::u64_from_usize;
        use alloc::vec;

        macro_rules! max_input_tests {
            ( $algorithm_name:ident ) => {
                mod $algorithm_name {
                    use super::super::super::super::digest;

                    #[test]
                    fn max_input_test() {
                        super::max_input_test(&digest::$algorithm_name);
                    }

                    #[test]
                    #[should_panic]
                    fn too_long_input_test_block() {
                        super::too_long_input_test_block(&digest::$algorithm_name);
                    }

                    #[test]
                    #[should_panic]
                    fn too_long_input_test_byte() {
                        super::too_long_input_test_byte(&digest::$algorithm_name);
                    }
                }
            };
        }

        fn max_input_test(alg: &'static digest::Algorithm) {
            let mut context = nearly_full_context(alg);
            let next_input = vec![0u8; alg.block_len() - 1];
            context.update(&next_input);
            let _ = context.finish(); // no panic
        }

        fn too_long_input_test_block(alg: &'static digest::Algorithm) {
            let mut context = nearly_full_context(alg);
            let next_input = vec![0u8; alg.block_len()];
            context.update(&next_input);
            let _ = context.finish(); // should panic
        }

        fn too_long_input_test_byte(alg: &'static digest::Algorithm) {
            let mut context = nearly_full_context(alg);
            let next_input = vec![0u8; alg.block_len() - 1];
            context.update(&next_input);
            context.update(&[0]);
            let _ = context.finish(); // should panic
        }

        fn nearly_full_context(alg: &'static digest::Algorithm) -> digest::Context {
            // All implementations currently support up to 2^64-1 bits
            // of input; according to the spec, SHA-384 and SHA-512
            // support up to 2^128-1, but that's not implemented yet.
            let max_bytes = 1u64 << (64 - 3);
            let max_blocks = max_bytes / u64_from_usize(alg.block_len());
            let completed_bytes = (max_blocks - 1) * u64_from_usize(alg.block_len());
            digest::Context {
                block: digest::BlockContext {
                    state: alg.initial_state.clone(),
                    completed_bytes,
                    algorithm: alg,
                },
                pending: [0u8; digest::MAX_BLOCK_LEN],
                num_pending: 0,
            }
        }

        max_input_tests!(SHA1_FOR_LEGACY_USE_ONLY);
        max_input_tests!(SHA256);
        max_input_tests!(SHA384);
        max_input_tests!(SHA512);
    }
}
