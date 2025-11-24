use crate::{
    arithmetic::{bigint, montgomery::RR},
    bits::{self, FromByteLen as _},
    cpu,
    error::{self, InputTooLongError},
    rsa::N,
};
use core::ops::RangeInclusive;

/// The modulus (n) of an RSA public key.
pub struct PublicModulus {
    value: bigint::OwnedModulus<N>,
    oneRR: bigint::One<N, RR>,
}

impl Clone for PublicModulus {
    fn clone(&self) -> Self {
        let PublicModulus { value, oneRR } = self;
        let value = value.clone();

        // XXX: Shouldn't really be needed just to call `alloc_zero()`,
        // but not worth optimizing away.
        let cpu = cpu::features();
        let n = value.modulus(cpu);
        let oneRR = oneRR.clone_into(n.alloc_zero());

        Self { value, oneRR }
    }
}

/*
impl core::fmt::Debug for PublicModulus {
    fn fmt(&self, fmt: &mut ::core::fmt::Formatter) -> Result<(), ::core::fmt::Error> {
        self.value.fmt(fmt)
    }
}*/

impl PublicModulus {
    pub(super) fn from_be_bytes(
        n: untrusted::Input,
        allowed_bit_lengths: RangeInclusive<bits::BitLength>,
        cpu_features: cpu::Features,
    ) -> Result<Self, error::KeyRejected> {
        // See `PublicKey::from_modulus_and_exponent` for background on the step
        // numbering.

        let min_bits = *allowed_bit_lengths.start();
        let max_bits = *allowed_bit_lengths.end();

        // `pkcs1_encode` depends on this not being small. Otherwise,
        // `pkcs1_encode` would generate padding that is invalid (too few 0xFF
        // bytes) for very small keys.
        const MIN_BITS: bits::BitLength = bits::BitLength::from_bits(1024);

        // Step 3 / Step c for `n` (out of order).
        let value = bigint::OwnedModulusValue::from_be_bytes(n)?;
        let bits = value.len_bits();

        // Step 1 / Step a. XXX: SP800-56Br1 and SP800-89 require the length of
        // the public modulus to be exactly 2048 or 3072 bits, but we are more
        // flexible to be compatible with other commonly-used crypto libraries.
        assert!(min_bits >= MIN_BITS);
        let bits_rounded_up = bits::BitLength::from_byte_len(bits.as_usize_bytes_rounded_up())
            .map_err(error::erase::<InputTooLongError>)
            .unwrap(); // TODO: safe?
        if bits_rounded_up < min_bits {
            return Err(error::KeyRejected::too_small());
        }
        if bits > max_bits {
            return Err(error::KeyRejected::too_large());
        }
        let value = bigint::OwnedModulus::from(value);
        let m = value.modulus(cpu_features);
        let oneRR = bigint::One::newRR(m.alloc_zero(), &m);

        Ok(Self { value, oneRR })
    }

    /// The big-endian encoding of the modulus.
    ///
    /// There are no leading zeros.
    pub fn be_bytes(&self) -> impl ExactSizeIterator<Item = u8> + Clone + '_ {
        self.value.be_bytes()
    }

    /// The length of the modulus in bits.
    pub fn len_bits(&self) -> bits::BitLength {
        self.value.len_bits()
    }

    pub(super) fn value(&self, cpu_features: cpu::Features) -> bigint::Modulus<N> {
        self.value.modulus(cpu_features)
    }

    pub(super) fn oneRR(&self) -> &bigint::Elem<N, RR> {
        self.oneRR.as_ref()
    }
}
