use crate::{bits_to_rate, keccakf::KeccakF, Hasher, KeccakState, Xof};

/// The `SHAKE` extendable-output functions defined in [`FIPS-202`].
///
/// # Usage
///
/// ```toml
/// [dependencies]
/// tiny-keccak = { version = "2.0.0", features = ["shake"] }
/// ```
///
/// [`FIPS-202`]: https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.202.pdf
#[derive(Clone)]
pub struct Shake {
    state: KeccakState<KeccakF>,
}

impl Shake {
    const DELIM: u8 = 0x1f;

    /// Creates  new [`Shake`] hasher with a security level of 128 bits.
    ///
    /// [`Shake`]: struct.Shake.html
    pub fn v128() -> Shake {
        Shake::new(128)
    }

    /// Creates  new [`Shake`] hasher with a security level of 256 bits.
    ///
    /// [`Shake`]: struct.Shake.html
    pub fn v256() -> Shake {
        Shake::new(256)
    }

    pub(crate) fn new(bits: usize) -> Shake {
        Shake {
            state: KeccakState::new(bits_to_rate(bits), Self::DELIM),
        }
    }
}

impl Hasher for Shake {
    fn update(&mut self, input: &[u8]) {
        self.state.update(input);
    }

    fn finalize(self, output: &mut [u8]) {
        self.state.finalize(output);
    }
}

impl Xof for Shake {
    fn squeeze(&mut self, output: &mut [u8]) {
        self.state.squeeze(output)
    }
}
