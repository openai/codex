use crate::{bits_to_rate, left_encode, right_encode, CShake, Hasher, IntoXof, Xof};

/// The `KMAC` pseudo-random functions defined in [`SP800-185`].
///
/// The KECCAK Message Authentication Code (`KMAC`) algorithm is a `PRF` and keyed hash function based
/// on KECCAK. It provides variable-length output, and unlike [`SHAKE`] and [`cSHAKE`], altering the
/// requested output length generates a new, unrelated output. KMAC has two variants, [`KMAC128`] and
/// [`KMAC256`], built from [`cSHAKE128`] and [`cSHAKE256`], respectively. The two variants differ somewhat in
/// their technical security properties.
///
/// # Usage
///
/// ```toml
/// [dependencies]
/// tiny-keccak = { version = "2.0.0", features = ["kmac"] }
/// ```
///
/// [`SP800-185`]: https://nvlpubs.nist.gov/nistpubs/SpecialPublications/NIST.SP.800-185.pdf
/// [`KMAC128`]: struct.Kmac.html#method.v128
/// [`KMAC256`]: struct.Kmac.html#method.v256
/// [`SHAKE`]: struct.Shake.html
/// [`cSHAKE`]: struct.CShake.html
/// [`cSHAKE128`]: struct.CShake.html#method.v128
/// [`cSHAKE256`]: struct.CShake.html#method.v256
#[derive(Clone)]
pub struct Kmac {
    state: CShake,
}

impl Kmac {
    /// Creates  new [`Kmac`] hasher with a security level of 128 bits.
    ///
    /// [`Kmac`]: struct.Kmac.html
    pub fn v128(key: &[u8], custom_string: &[u8]) -> Kmac {
        Kmac::new(key, custom_string, 128)
    }

    /// Creates  new [`Kmac`] hasher with a security level of 256 bits.
    ///
    /// [`Kmac`]: struct.Kmac.html
    pub fn v256(key: &[u8], custom_string: &[u8]) -> Kmac {
        Kmac::new(key, custom_string, 256)
    }

    fn new(key: &[u8], custom_string: &[u8], bits: usize) -> Kmac {
        let rate = bits_to_rate(bits);
        let mut state = CShake::new(b"KMAC", custom_string, bits);
        state.update(left_encode(rate).value());
        state.update(left_encode(key.len() * 8).value());
        state.update(key);
        state.fill_block();
        Kmac { state }
    }
}

impl Hasher for Kmac {
    fn update(&mut self, input: &[u8]) {
        self.state.update(input)
    }

    fn finalize(mut self, output: &mut [u8]) {
        self.state.update(right_encode(output.len() * 8).value());
        self.state.finalize(output)
    }
}

/// The `KMACXOF` extendable-output functions defined in [`SP800-185`].
///
/// # Usage
///
/// ```toml
/// [dependencies]
/// tiny-keccak = { version = "2.0.0", features = ["kmac"] }
/// ```
///
/// # Example
///
/// ```
/// # use tiny_keccak::{Kmac, Xof, IntoXof, Hasher};
/// let input = b"hello world";
/// let mut output = [0u8; 64];
/// let mut kmac = Kmac::v256(b"", b"");
/// kmac.update(input);
/// let mut xof = kmac.into_xof();
/// xof.squeeze(&mut output[..32]);
/// xof.squeeze(&mut output[32..]);
/// ```
///
/// ---
///
/// [`KmacXof`] can be created only by using [`Kmac::IntoXof`] interface.
///
/// [`SP800-185`]: https://nvlpubs.nist.gov/nistpubs/SpecialPublications/NIST.SP.800-185.pdf
/// [`KmacXof`]: struct.KmacXof.html
/// [`Kmac::IntoXof`]: struct.Kmac.html#impl-IntoXof
#[derive(Clone)]
pub struct KmacXof {
    state: CShake,
}

impl IntoXof for Kmac {
    type Xof = KmacXof;

    fn into_xof(mut self) -> Self::Xof {
        self.state.update(right_encode(0).value());
        KmacXof { state: self.state }
    }
}

impl Xof for KmacXof {
    fn squeeze(&mut self, output: &mut [u8]) {
        self.state.squeeze(output)
    }
}
