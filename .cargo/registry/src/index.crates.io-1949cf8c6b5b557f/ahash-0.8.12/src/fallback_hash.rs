use crate::convert::*;
use crate::operations::folded_multiply;
use crate::operations::read_small;
use crate::operations::MULTIPLE;
use crate::random_state::PI;
use crate::RandomState;
use core::hash::Hasher;

const ROT: u32 = 23; //17

/// A `Hasher` for hashing an arbitrary stream of bytes.
///
/// Instances of [`AHasher`] represent state that is updated while hashing data.
///
/// Each method updates the internal state based on the new data provided. Once
/// all of the data has been provided, the resulting hash can be obtained by calling
/// `finish()`
///
/// [Clone] is also provided in case you wish to calculate hashes for two different items that
/// start with the same data.
///
#[derive(Debug, Clone)]
pub struct AHasher {
    buffer: u64,
    pad: u64,
    extra_keys: [u64; 2],
}

impl AHasher {
    /// Creates a new hasher keyed to the provided key.
    #[inline]
    #[allow(dead_code)] // Is not called if non-fallback hash is used.
    pub(crate) fn new_with_keys(key1: u128, key2: u128) -> AHasher {
        let pi: [u128; 2] = PI.convert();
        let key1: [u64; 2] = (key1 ^ pi[0]).convert();
        let key2: [u64; 2] = (key2 ^ pi[1]).convert();
        AHasher {
            buffer: key1[0],
            pad: key1[1],
            extra_keys: key2,
        }
    }

    #[allow(unused)] // False positive
    pub(crate) fn test_with_keys(key1: u128, key2: u128) -> Self {
        let key1: [u64; 2] = key1.convert();
        let key2: [u64; 2] = key2.convert();
        Self {
            buffer: key1[0],
            pad: key1[1],
            extra_keys: key2,
        }
    }

    #[inline]
    #[allow(dead_code)] // Is not called if non-fallback hash is used.
    pub(crate) fn from_random_state(rand_state: &RandomState) -> AHasher {
        AHasher {
            buffer: rand_state.k1,
            pad: rand_state.k0,
            extra_keys: [rand_state.k2, rand_state.k3],
        }
    }

    /// This update function has the goal of updating the buffer with a single multiply
    /// FxHash does this but is vulnerable to attack. To avoid this input needs to be masked to with an
    /// unpredictable value. Other hashes such as murmurhash have taken this approach but were found vulnerable
    /// to attack. The attack was based on the idea of reversing the pre-mixing (Which is necessarily
    /// reversible otherwise bits would be lost) then placing a difference in the highest bit before the
    /// multiply used to mix the data. Because a multiply can never affect the bits to the right of it, a
    /// subsequent update that also differed in this bit could result in a predictable collision.
    ///
    /// This version avoids this vulnerability while still only using a single multiply. It takes advantage
    /// of the fact that when a 64 bit multiply is performed the upper 64 bits are usually computed and thrown
    /// away. Instead it creates two 128 bit values where the upper 64 bits are zeros and multiplies them.
    /// (The compiler is smart enough to turn this into a 64 bit multiplication in the assembly)
    /// Then the upper bits are xored with the lower bits to produce a single 64 bit result.
    ///
    /// To understand why this is a good scrambling function it helps to understand multiply-with-carry PRNGs:
    /// https://en.wikipedia.org/wiki/Multiply-with-carry_pseudorandom_number_generator
    /// If the multiple is chosen well, this creates a long period, decent quality PRNG.
    /// Notice that this function is equivalent to this except the `buffer`/`state` is being xored with each
    /// new block of data. In the event that data is all zeros, it is exactly equivalent to a MWC PRNG.
    ///
    /// This is impervious to attack because every bit buffer at the end is dependent on every bit in
    /// `new_data ^ buffer`. For example suppose two inputs differed in only the 5th bit. Then when the
    /// multiplication is performed the `result` will differ in bits 5-69. More specifically it will differ by
    /// 2^5 * MULTIPLE. However in the next step bits 65-128 are turned into a separate 64 bit value. So the
    /// differing bits will be in the lower 6 bits of this value. The two intermediate values that differ in
    /// bits 5-63 and in bits 0-5 respectively get added together. Producing an output that differs in every
    /// bit. The addition carries in the multiplication and at the end additionally mean that the even if an
    /// attacker somehow knew part of (but not all) the contents of the buffer before hand,
    /// they would not be able to predict any of the bits in the buffer at the end.
    #[inline(always)]
    fn update(&mut self, new_data: u64) {
        self.buffer = folded_multiply(new_data ^ self.buffer, MULTIPLE);
    }

    /// Similar to the above this function performs an update using a "folded multiply".
    /// However it takes in 128 bits of data instead of 64. Both halves must be masked.
    ///
    /// This makes it impossible for an attacker to place a single bit difference between
    /// two blocks so as to cancel each other.
    ///
    /// However this is not sufficient. to prevent (a,b) from hashing the same as (b,a) the buffer itself must
    /// be updated between calls in a way that does not commute. To achieve this XOR and Rotate are used.
    /// Add followed by xor is not the same as xor followed by add, and rotate ensures that the same out bits
    /// can't be changed by the same set of input bits. To cancel this sequence with subsequent input would require
    /// knowing the keys.
    #[inline(always)]
    fn large_update(&mut self, new_data: u128) {
        let block: [u64; 2] = new_data.convert();
        let combined = folded_multiply(block[0] ^ self.extra_keys[0], block[1] ^ self.extra_keys[1]);
        self.buffer = (self.buffer.wrapping_add(self.pad) ^ combined).rotate_left(ROT);
    }

    #[inline]
    #[cfg(specialize)]
    fn short_finish(&self) -> u64 {
        folded_multiply(self.buffer, self.pad)
    }
}

/// Provides [Hasher] methods to hash all of the primitive types.
///
/// [Hasher]: core::hash::Hasher
impl Hasher for AHasher {
    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.update(i as u64);
    }

    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.update(i as u64);
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.update(i as u64);
    }

    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.update(i as u64);
    }

    #[inline]
    fn write_u128(&mut self, i: u128) {
        self.large_update(i);
    }

    #[inline]
    #[cfg(any(
        target_pointer_width = "64",
        target_pointer_width = "32",
        target_pointer_width = "16"
    ))]
    fn write_usize(&mut self, i: usize) {
        self.write_u64(i as u64);
    }

    #[inline]
    #[cfg(target_pointer_width = "128")]
    fn write_usize(&mut self, i: usize) {
        self.write_u128(i as u128);
    }

    #[inline]
    #[allow(clippy::collapsible_if)]
    fn write(&mut self, input: &[u8]) {
        let mut data = input;
        let length = data.len() as u64;
        //Needs to be an add rather than an xor because otherwise it could be canceled with carefully formed input.
        self.buffer = self.buffer.wrapping_add(length).wrapping_mul(MULTIPLE);
        //A 'binary search' on sizes reduces the number of comparisons.
        if data.len() > 8 {
            if data.len() > 16 {
                let tail = data.read_last_u128();
                self.large_update(tail);
                while data.len() > 16 {
                    let (block, rest) = data.read_u128();
                    self.large_update(block);
                    data = rest;
                }
            } else {
                self.large_update([data.read_u64().0, data.read_last_u64()].convert());
            }
        } else {
            let value = read_small(data);
            self.large_update(value.convert());
        }
    }

    #[inline]
    fn finish(&self) -> u64 {
        let rot = (self.buffer & 63) as u32;
        folded_multiply(self.buffer, self.pad).rotate_left(rot)
    }
}

#[cfg(specialize)]
pub(crate) struct AHasherU64 {
    pub(crate) buffer: u64,
    pub(crate) pad: u64,
}

/// A specialized hasher for only primitives under 64 bits.
#[cfg(specialize)]
impl Hasher for AHasherU64 {
    #[inline]
    fn finish(&self) -> u64 {
        folded_multiply(self.buffer, self.pad)
        //self.buffer
    }

    #[inline]
    fn write(&mut self, _bytes: &[u8]) {
        unreachable!("Specialized hasher was called with a different type of object")
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.write_u64(i as u64);
    }

    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.write_u64(i as u64);
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.write_u64(i as u64);
    }

    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.buffer = folded_multiply(i ^ self.buffer, MULTIPLE);
    }

    #[inline]
    fn write_u128(&mut self, _i: u128) {
        unreachable!("Specialized hasher was called with a different type of object")
    }

    #[inline]
    fn write_usize(&mut self, _i: usize) {
        unreachable!("Specialized hasher was called with a different type of object")
    }
}

#[cfg(specialize)]
pub(crate) struct AHasherFixed(pub AHasher);

/// A specialized hasher for fixed size primitives larger than 64 bits.
#[cfg(specialize)]
impl Hasher for AHasherFixed {
    #[inline]
    fn finish(&self) -> u64 {
        self.0.short_finish()
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        self.0.write(bytes)
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.write_u64(i as u64);
    }

    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.write_u64(i as u64);
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.write_u64(i as u64);
    }

    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.0.write_u64(i);
    }

    #[inline]
    fn write_u128(&mut self, i: u128) {
        self.0.write_u128(i);
    }

    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.0.write_usize(i);
    }
}

#[cfg(specialize)]
pub(crate) struct AHasherStr(pub AHasher);

/// A specialized hasher for a single string
/// Note that the other types don't panic because the hash impl for String tacks on an unneeded call. (As does vec)
#[cfg(specialize)]
impl Hasher for AHasherStr {
    #[inline]
    fn finish(&self) -> u64 {
        self.0.finish()
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        if bytes.len() > 8 {
            self.0.write(bytes)
        } else {
            let value = read_small(bytes);
            self.0.buffer = folded_multiply(value[0] ^ self.0.buffer, value[1] ^ self.0.extra_keys[1]);
            self.0.pad = self.0.pad.wrapping_add(bytes.len() as u64);
        }
    }

    #[inline]
    fn write_u8(&mut self, _i: u8) {}

    #[inline]
    fn write_u16(&mut self, _i: u16) {}

    #[inline]
    fn write_u32(&mut self, _i: u32) {}

    #[inline]
    fn write_u64(&mut self, _i: u64) {}

    #[inline]
    fn write_u128(&mut self, _i: u128) {}

    #[inline]
    fn write_usize(&mut self, _i: usize) {}
}

#[cfg(test)]
mod tests {
    use crate::fallback_hash::*;

    #[test]
    fn test_hash() {
        let mut hasher = AHasher::new_with_keys(0, 0);
        let value: u64 = 1 << 32;
        hasher.update(value);
        let result = hasher.buffer;
        let mut hasher = AHasher::new_with_keys(0, 0);
        let value2: u64 = 1;
        hasher.update(value2);
        let result2 = hasher.buffer;
        let result: [u8; 8] = result.convert();
        let result2: [u8; 8] = result2.convert();
        assert_ne!(hex::encode(result), hex::encode(result2));
    }
}
