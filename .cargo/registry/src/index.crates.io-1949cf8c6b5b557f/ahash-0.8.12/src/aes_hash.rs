use crate::convert::*;
use crate::operations::*;
use crate::random_state::PI;
use crate::RandomState;
use core::hash::Hasher;

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
    enc: u128,
    sum: u128,
    key: u128,
}

impl AHasher {
    /// Creates a new hasher keyed to the provided keys.
    ///
    /// Normally hashers are created via `AHasher::default()` for fixed keys or `RandomState::new()` for randomly
    /// generated keys and `RandomState::with_seeds(a,b)` for seeds that are set and can be reused. All of these work at
    /// map creation time (and hence don't have any overhead on a per-item bais).
    ///
    /// This method directly creates the hasher instance and performs no transformation on the provided seeds. This may
    /// be useful where a HashBuilder is not desired, such as for testing purposes.
    ///
    /// # Example
    ///
    /// ```no_build
    /// use std::hash::Hasher;
    /// use ahash::AHasher;
    ///
    /// let mut hasher = AHasher::new_with_keys(1234, 5678);
    ///
    /// hasher.write_u32(1989);
    /// hasher.write_u8(11);
    /// hasher.write_u8(9);
    /// hasher.write(b"Huh?");
    ///
    /// println!("Hash is {:x}!", hasher.finish());
    /// ```
    #[inline]
    pub(crate) fn new_with_keys(key1: u128, key2: u128) -> Self {
        let pi: [u128; 2] = PI.convert();
        let key1 = key1 ^ pi[0];
        let key2 = key2 ^ pi[1];
        Self {
            enc: key1,
            sum: key2,
            key: key1 ^ key2,
        }
    }

    #[allow(unused)] // False positive
    pub(crate) fn test_with_keys(key1: u128, key2: u128) -> Self {
        Self {
            enc: key1,
            sum: key2,
            key: key1 ^ key2,
        }
    }

    #[inline]
    pub(crate) fn from_random_state(rand_state: &RandomState) -> Self {
        let key1 = [rand_state.k0, rand_state.k1].convert();
        let key2 = [rand_state.k2, rand_state.k3].convert();
        Self {
            enc: key1,
            sum: key2,
            key: key1 ^ key2,
        }
    }

    #[inline(always)]
    fn hash_in(&mut self, new_value: u128) {
        self.enc = aesdec(self.enc, new_value);
        self.sum = shuffle_and_add(self.sum, new_value);
    }

    #[inline(always)]
    fn hash_in_2(&mut self, v1: u128, v2: u128) {
        self.enc = aesdec(self.enc, v1);
        self.sum = shuffle_and_add(self.sum, v1);
        self.enc = aesdec(self.enc, v2);
        self.sum = shuffle_and_add(self.sum, v2);
    }

    #[inline]
    #[cfg(specialize)]
    fn short_finish(&self) -> u64 {
        let combined = aesenc(self.sum, self.enc);
        let result: [u64; 2] = aesdec(combined, combined).convert();
        result[0]
    }
}

/// Provides [Hasher] methods to hash all of the primitive types.
///
/// [Hasher]: core::hash::Hasher
impl Hasher for AHasher {
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
    fn write_u128(&mut self, i: u128) {
        self.hash_in(i);
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
    fn write_u64(&mut self, i: u64) {
        self.write_u128(i as u128);
    }

    #[inline]
    #[allow(clippy::collapsible_if)]
    fn write(&mut self, input: &[u8]) {
        let mut data = input;
        let length = data.len();
        add_in_length(&mut self.enc, length as u64);

        //A 'binary search' on sizes reduces the number of comparisons.
        if data.len() <= 8 {
            let value = read_small(data);
            self.hash_in(value.convert());
        } else {
            if data.len() > 32 {
                if data.len() > 64 {
                    let tail = data.read_last_u128x4();
                    let mut current: [u128; 4] = [self.key; 4];
                    current[0] = aesenc(current[0], tail[0]);
                    current[1] = aesdec(current[1], tail[1]);
                    current[2] = aesenc(current[2], tail[2]);
                    current[3] = aesdec(current[3], tail[3]);
                    let mut sum: [u128; 2] = [self.key, !self.key];
                    sum[0] = add_by_64s(sum[0].convert(), tail[0].convert()).convert();
                    sum[1] = add_by_64s(sum[1].convert(), tail[1].convert()).convert();
                    sum[0] = shuffle_and_add(sum[0], tail[2]);
                    sum[1] = shuffle_and_add(sum[1], tail[3]);
                    while data.len() > 64 {
                        let (blocks, rest) = data.read_u128x4();
                        current[0] = aesdec(current[0], blocks[0]);
                        current[1] = aesdec(current[1], blocks[1]);
                        current[2] = aesdec(current[2], blocks[2]);
                        current[3] = aesdec(current[3], blocks[3]);
                        sum[0] = shuffle_and_add(sum[0], blocks[0]);
                        sum[1] = shuffle_and_add(sum[1], blocks[1]);
                        sum[0] = shuffle_and_add(sum[0], blocks[2]);
                        sum[1] = shuffle_and_add(sum[1], blocks[3]);
                        data = rest;
                    }
                    self.hash_in_2(current[0], current[1]);
                    self.hash_in_2(current[2], current[3]);
                    self.hash_in_2(sum[0], sum[1]);
                } else {
                    //len 33-64
                    let (head, _) = data.read_u128x2();
                    let tail = data.read_last_u128x2();
                    self.hash_in_2(head[0], head[1]);
                    self.hash_in_2(tail[0], tail[1]);
                }
            } else {
                if data.len() > 16 {
                    //len 17-32
                    self.hash_in_2(data.read_u128().0, data.read_last_u128());
                } else {
                    //len 9-16
                    let value: [u64; 2] = [data.read_u64().0, data.read_last_u64()];
                    self.hash_in(value.convert());
                }
            }
        }
    }
    #[inline]
    fn finish(&self) -> u64 {
        let combined = aesenc(self.sum, self.enc);
        let result: [u64; 2] = aesdec(aesdec(combined, self.key), combined).convert();
        result[0]
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

/// A specialized hasher for strings
/// Note that the other types don't panic because the hash impl for String tacks on an unneeded call. (As does vec)
#[cfg(specialize)]
impl Hasher for AHasherStr {
    #[inline]
    fn finish(&self) -> u64 {
        let result: [u64; 2] = self.0.enc.convert();
        result[0]
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        if bytes.len() > 8 {
            self.0.write(bytes);
            self.0.enc = aesenc(self.0.sum, self.0.enc);
            self.0.enc = aesdec(aesdec(self.0.enc, self.0.key), self.0.enc);
        } else {
            add_in_length(&mut self.0.enc, bytes.len() as u64);

            let value = read_small(bytes).convert();
            self.0.sum = shuffle_and_add(self.0.sum, value);
            self.0.enc = aesenc(self.0.sum, self.0.enc);
            self.0.enc = aesdec(aesdec(self.0.enc, self.0.key), self.0.enc);
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
    use super::*;
    use crate::convert::Convert;
    use crate::operations::aesenc;
    use crate::RandomState;
    use std::hash::{BuildHasher, Hasher};
    #[test]
    fn test_sanity() {
        let mut hasher = RandomState::with_seeds(1, 2, 3, 4).build_hasher();
        hasher.write_u64(0);
        let h1 = hasher.finish();
        hasher.write(&[1, 0, 0, 0, 0, 0, 0, 0]);
        let h2 = hasher.finish();
        assert_ne!(h1, h2);
    }

    #[cfg(feature = "compile-time-rng")]
    #[test]
    fn test_builder() {
        use std::collections::HashMap;
        use std::hash::BuildHasherDefault;

        let mut map = HashMap::<u32, u64, BuildHasherDefault<AHasher>>::default();
        map.insert(1, 3);
    }

    #[cfg(feature = "compile-time-rng")]
    #[test]
    fn test_default() {
        let hasher_a = AHasher::default();
        let a_enc: [u64; 2] = hasher_a.enc.convert();
        let a_sum: [u64; 2] = hasher_a.sum.convert();
        assert_ne!(0, a_enc[0]);
        assert_ne!(0, a_enc[1]);
        assert_ne!(0, a_sum[0]);
        assert_ne!(0, a_sum[1]);
        assert_ne!(a_enc[0], a_enc[1]);
        assert_ne!(a_sum[0], a_sum[1]);
        assert_ne!(a_enc[0], a_sum[0]);
        assert_ne!(a_enc[1], a_sum[1]);
        let hasher_b = AHasher::default();
        let b_enc: [u64; 2] = hasher_b.enc.convert();
        let b_sum: [u64; 2] = hasher_b.sum.convert();
        assert_eq!(a_enc[0], b_enc[0]);
        assert_eq!(a_enc[1], b_enc[1]);
        assert_eq!(a_sum[0], b_sum[0]);
        assert_eq!(a_sum[1], b_sum[1]);
    }

    #[test]
    fn test_hash() {
        let mut result: [u64; 2] = [0x6c62272e07bb0142, 0x62b821756295c58d];
        let value: [u64; 2] = [1 << 32, 0xFEDCBA9876543210];
        result = aesenc(value.convert(), result.convert()).convert();
        result = aesenc(result.convert(), result.convert()).convert();
        let mut result2: [u64; 2] = [0x6c62272e07bb0142, 0x62b821756295c58d];
        let value2: [u64; 2] = [1, 0xFEDCBA9876543210];
        result2 = aesenc(value2.convert(), result2.convert()).convert();
        result2 = aesenc(result2.convert(), result.convert()).convert();
        let result: [u8; 16] = result.convert();
        let result2: [u8; 16] = result2.convert();
        assert_ne!(hex::encode(result), hex::encode(result2));
    }
}
