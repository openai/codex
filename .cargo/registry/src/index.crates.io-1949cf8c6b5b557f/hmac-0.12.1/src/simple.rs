use super::{get_der_key, IPAD, OPAD};
use core::fmt;
use digest::{
    crypto_common::{Block, BlockSizeUser, InvalidLength, Key, KeySizeUser},
    Digest, FixedOutput, KeyInit, MacMarker, Output, OutputSizeUser, Update,
};
#[cfg(feature = "reset")]
use digest::{FixedOutputReset, Reset};

/// Simplified HMAC instance able to operate over hash functions
/// which do not expose block-level API and hash functions which
/// process blocks lazily (e.g. BLAKE2).
#[derive(Clone)]
pub struct SimpleHmac<D: Digest + BlockSizeUser> {
    digest: D,
    opad_key: Block<D>,
    #[cfg(feature = "reset")]
    ipad_key: Block<D>,
}

impl<D: Digest + BlockSizeUser> KeySizeUser for SimpleHmac<D> {
    type KeySize = D::BlockSize;
}

impl<D: Digest + BlockSizeUser> MacMarker for SimpleHmac<D> {}

impl<D: Digest + BlockSizeUser> KeyInit for SimpleHmac<D> {
    fn new(key: &Key<Self>) -> Self {
        Self::new_from_slice(key.as_slice()).unwrap()
    }

    #[inline]
    fn new_from_slice(key: &[u8]) -> Result<Self, InvalidLength> {
        let der_key = get_der_key::<D>(key);
        let mut ipad_key = der_key.clone();
        for b in ipad_key.iter_mut() {
            *b ^= IPAD;
        }
        let mut digest = D::new();
        digest.update(&ipad_key);

        let mut opad_key = der_key;
        for b in opad_key.iter_mut() {
            *b ^= OPAD;
        }

        Ok(Self {
            digest,
            opad_key,
            #[cfg(feature = "reset")]
            ipad_key,
        })
    }
}

impl<D: Digest + BlockSizeUser> Update for SimpleHmac<D> {
    #[inline(always)]
    fn update(&mut self, data: &[u8]) {
        self.digest.update(data);
    }
}

impl<D: Digest + BlockSizeUser> OutputSizeUser for SimpleHmac<D> {
    type OutputSize = D::OutputSize;
}

impl<D: Digest + BlockSizeUser> FixedOutput for SimpleHmac<D> {
    fn finalize_into(self, out: &mut Output<Self>) {
        let mut h = D::new();
        h.update(&self.opad_key);
        h.update(&self.digest.finalize());
        h.finalize_into(out);
    }
}

impl<D: Digest + BlockSizeUser + fmt::Debug> fmt::Debug for SimpleHmac<D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SimpleHmac")
            .field("digest", &self.digest)
            // TODO: replace with `finish_non_exhaustive` on MSRV
            // bump to 1.53
            .field("..", &"..")
            .finish()
    }
}

#[cfg(feature = "reset")]
#[cfg_attr(docsrs, doc(cfg(feature = "reset")))]
impl<D: Digest + BlockSizeUser + Reset> Reset for SimpleHmac<D> {
    fn reset(&mut self) {
        Reset::reset(&mut self.digest);
        self.digest.update(&self.ipad_key);
    }
}

#[cfg(feature = "reset")]
#[cfg_attr(docsrs, doc(cfg(feature = "reset")))]
impl<D: Digest + BlockSizeUser + FixedOutputReset> FixedOutputReset for SimpleHmac<D> {
    fn finalize_into_reset(&mut self, out: &mut Output<Self>) {
        let mut h = D::new();
        Update::update(&mut h, &self.opad_key);
        Update::update(&mut h, &self.digest.finalize_reset());
        Update::update(&mut self.digest, &self.ipad_key);
        Digest::finalize_into(h, out);
    }
}
