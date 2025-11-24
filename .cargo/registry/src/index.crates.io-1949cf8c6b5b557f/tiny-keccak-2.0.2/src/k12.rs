//! The `KangarooTwelve` hash function defined [`here`].
//!
//! [`here`]: https://eprint.iacr.org/2016/770.pdf

use crate::{bits_to_rate, keccakp::KeccakP, EncodedLen, Hasher, IntoXof, KeccakState, Xof};

fn encode_len(len: usize) -> EncodedLen {
    let len_view = (len as u64).to_be_bytes();
    let offset = len_view.iter().position(|i| *i != 0).unwrap_or(8);
    let mut buffer = [0u8; 9];
    buffer[..8].copy_from_slice(&len_view);
    buffer[8] = 8 - offset as u8;

    EncodedLen { offset, buffer }
}

/// The `KangarooTwelve` hash function defined [`here`].
///
/// # Usage
///
/// ```toml
/// [dependencies]
/// tiny-keccak = { version = "2.0.0", features = ["k12"] }
/// ```
///
/// [`here`]: https://eprint.iacr.org/2016/770.pdf
#[derive(Clone)]
pub struct KangarooTwelve<T> {
    state: KeccakState<KeccakP>,
    current_chunk: KeccakState<KeccakP>,
    custom_string: Option<T>,
    written: usize,
    chunks: usize,
}

impl<T> KangarooTwelve<T> {
    const MAX_CHUNK_SIZE: usize = 8192;

    /// Creates  new [`KangarooTwelve`] hasher with a security level of 128 bits.
    ///
    /// [`KangarooTwelve`]: struct.KangarooTwelve.html
    pub fn new(custom_string: T) -> Self {
        let rate = bits_to_rate(128);
        KangarooTwelve {
            state: KeccakState::new(rate, 0),
            current_chunk: KeccakState::new(rate, 0x0b),
            custom_string: Some(custom_string),
            written: 0,
            chunks: 0,
        }
    }
}

impl<T: AsRef<[u8]>> Hasher for KangarooTwelve<T> {
    fn update(&mut self, input: &[u8]) {
        let mut to_absorb = input;
        if self.chunks == 0 {
            let todo = core::cmp::min(Self::MAX_CHUNK_SIZE - self.written, to_absorb.len());
            self.state.update(&to_absorb[..todo]);
            self.written += todo;
            to_absorb = &to_absorb[todo..];

            if to_absorb.len() > 0 && self.written == Self::MAX_CHUNK_SIZE {
                self.state.update(&[0x03, 0, 0, 0, 0, 0, 0, 0]);
                self.written = 0;
                self.chunks += 1;
            }
        }

        while to_absorb.len() > 0 {
            if self.written == Self::MAX_CHUNK_SIZE {
                let mut chunk_hash = [0u8; 32];
                let current_chunk = self.current_chunk.clone();
                self.current_chunk.reset();
                current_chunk.finalize(&mut chunk_hash);
                self.state.update(&chunk_hash);
                self.written = 0;
                self.chunks += 1;
            }

            let todo = core::cmp::min(Self::MAX_CHUNK_SIZE - self.written, to_absorb.len());
            self.current_chunk.update(&to_absorb[..todo]);
            self.written += todo;
            to_absorb = &to_absorb[todo..];
        }
    }

    fn finalize(self, output: &mut [u8]) {
        let mut xof = self.into_xof();
        xof.squeeze(output);
    }
}

/// The `KangarooTwelve` extendable-output function defined [`here`].
///
/// # Usage
///
/// ```toml
/// [dependencies]
/// tiny-keccak = { version = "2.0.0", features = ["k12"] }
/// ```
///
/// # Example
///
/// ```
/// # use tiny_keccak::{KangarooTwelve, Xof, IntoXof, Hasher};
/// let input = b"hello world";
/// let mut output = [0u8; 64];
/// let mut hasher = KangarooTwelve::new(b"");
/// hasher.update(input);
/// let mut xof = hasher.into_xof();
/// xof.squeeze(&mut output[..32]);
/// xof.squeeze(&mut output[32..]);
/// ```
///
/// ---
///
/// [`KangarooTwelveXof`] can be created only by using [`KangarooTwelve::IntoXof`] interface.
///
/// [`here`]: https://eprint.iacr.org/2016/770.pdf
/// [`KangarooTwelveXof`]: struct.KangarooTwelveXof.html
/// [`KangarooTwelve::IntoXof`]: struct.KangarooTwelve.html#impl-IntoXof
#[derive(Clone)]
pub struct KangarooTwelveXof {
    state: KeccakState<KeccakP>,
}

impl<T: AsRef<[u8]>> IntoXof for KangarooTwelve<T> {
    type Xof = KangarooTwelveXof;

    fn into_xof(mut self) -> KangarooTwelveXof {
        let custom_string = self
            .custom_string
            .take()
            .expect("KangarooTwelve cannot be initialized without custom_string; qed");
        let encoded_len = encode_len(custom_string.as_ref().len());
        self.update(custom_string.as_ref());
        self.update(encoded_len.value());

        if self.chunks == 0 {
            self.state.delim = 0x07;
        } else {
            let encoded_chunks = encode_len(self.chunks);
            let mut tmp_chunk = [0u8; 32];
            self.current_chunk.finalize(&mut tmp_chunk);
            self.state.update(&tmp_chunk);
            self.state.update(encoded_chunks.value());
            self.state.update(&[0xff, 0xff]);
            self.state.delim = 0x06;
        }

        KangarooTwelveXof { state: self.state }
    }
}

impl Xof for KangarooTwelveXof {
    fn squeeze(&mut self, output: &mut [u8]) {
        self.state.squeeze(output);
    }
}
