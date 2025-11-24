use crate::error;
use crate::polyfill::{unwrap_const, ArrayFlatMap, LeadingZerosStripped};
use core::num::NonZeroU64;

/// The exponent `e` of an RSA public key.
#[derive(Clone, Copy)]
pub struct PublicExponent(NonZeroU64);

impl PublicExponent {
    #[cfg(test)]
    const ALL_CONSTANTS: [Self; 3] = [Self::_3, Self::_65537, Self::MAX];

    pub(super) const _3: Self = Self(unwrap_const(NonZeroU64::new(3)));
    pub(super) const _65537: Self = Self(unwrap_const(NonZeroU64::new(65537)));

    // This limit was chosen to bound the performance of the simple
    // exponentiation-by-squaring implementation in `elem_exp_vartime`. In
    // particular, it helps mitigate theoretical resource exhaustion attacks. 33
    // bits was chosen as the limit based on the recommendations in [1] and
    // [2]. Windows CryptoAPI (at least older versions) doesn't support values
    // larger than 32 bits [3], so it is unlikely that exponents larger than 32
    // bits are being used for anything Windows commonly does.
    //
    // [1] https://www.imperialviolet.org/2012/03/16/rsae.html
    // [2] https://www.imperialviolet.org/2012/03/17/rsados.html
    // [3] https://msdn.microsoft.com/en-us/library/aa387685(VS.85).aspx
    const MAX: Self = Self(unwrap_const(NonZeroU64::new((1u64 << 33) - 1)));

    pub(super) fn from_be_bytes(
        input: untrusted::Input,
        min_value: Self,
    ) -> Result<Self, error::KeyRejected> {
        // See `PublicKey::from_modulus_and_exponent` for background on the step
        // numbering.

        if input.len() > 5 {
            return Err(error::KeyRejected::too_large());
        }
        let value = input.read_all(error::KeyRejected::invalid_encoding(), |input| {
            // The exponent can't be zero and it can't be prefixed with
            // zero-valued bytes.
            if input.peek(0) {
                return Err(error::KeyRejected::invalid_encoding());
            }
            let mut value = 0u64;
            loop {
                let byte = input
                    .read_byte()
                    .map_err(|untrusted::EndOfInput| error::KeyRejected::invalid_encoding())?;
                value = (value << 8) | u64::from(byte);
                if input.at_end() {
                    return Ok(value);
                }
            }
        })?;

        // Step 2 / Step b. NIST SP800-89 defers to FIPS 186-3, which requires
        // `e >= 65537`. We enforce this when signing, but are more flexible in
        // verification, for compatibility. Only small public exponents are
        // supported.
        let value = NonZeroU64::new(value).ok_or_else(error::KeyRejected::too_small)?;
        if value < min_value.0 {
            return Err(error::KeyRejected::too_small());
        }
        if value > Self::MAX.0 {
            return Err(error::KeyRejected::too_large());
        }

        // Step 3 / Step c.
        if value.get() & 1 != 1 {
            return Err(error::KeyRejected::invalid_component());
        }

        Ok(Self(value))
    }

    /// The big-endian encoding of the exponent.
    ///
    /// There are no leading zeros.
    pub fn be_bytes(&self) -> impl ExactSizeIterator<Item = u8> + Clone + '_ {
        // The `unwrap()` won't fail as `self.0` is only a few bytes long.
        let bytes = ArrayFlatMap::new(core::iter::once(self.0.get()), u64::to_be_bytes).unwrap();
        LeadingZerosStripped::new(bytes)
    }

    pub(super) fn value(self) -> NonZeroU64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_public_exponent_constants() {
        for value in PublicExponent::ALL_CONSTANTS.iter() {
            let value: u64 = value.0.into();
            assert_eq!(value & 1, 1);
            assert!(value >= PublicExponent::_3.0.into()); // The absolute minimum.
            assert!(value <= PublicExponent::MAX.0.into());
        }
    }
}
