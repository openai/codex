use alloc::boxed::Box;

use zeroize::Zeroize;

/// A concrete HMAC implementation, for a single cryptographic hash function.
///
/// You should have one object that implements this trait for HMAC-SHA256, another
/// for HMAC-SHA384, etc.
pub trait Hmac: Send + Sync {
    /// Prepare to use `key` as a HMAC key.
    fn with_key(&self, key: &[u8]) -> Box<dyn Key>;

    /// Give the length of the underlying hash function.  In RFC2104 terminology this is `L`.
    fn hash_output_len(&self) -> usize;

    /// Return `true` if this is backed by a FIPS-approved implementation.
    fn fips(&self) -> bool {
        false
    }
}

/// A HMAC tag, stored as a value.
#[derive(Clone)]
pub struct Tag {
    buf: [u8; Self::MAX_LEN],
    used: usize,
}

impl Tag {
    /// Build a tag by copying a byte slice.
    ///
    /// The slice can be up to [`Tag::MAX_LEN`] bytes in length.
    pub fn new(bytes: &[u8]) -> Self {
        let mut tag = Self {
            buf: [0u8; Self::MAX_LEN],
            used: bytes.len(),
        };
        tag.buf[..bytes.len()].copy_from_slice(bytes);
        tag
    }

    /// Maximum supported HMAC tag size: supports up to SHA512.
    pub const MAX_LEN: usize = 64;
}

impl Drop for Tag {
    fn drop(&mut self) {
        self.buf.zeroize();
    }
}

impl AsRef<[u8]> for Tag {
    fn as_ref(&self) -> &[u8] {
        &self.buf[..self.used]
    }
}

/// A HMAC key that is ready for use.
///
/// The algorithm used is implicit in the `Hmac` object that produced the key.
pub trait Key: Send + Sync {
    /// Calculates a tag over `data` -- a slice of byte slices.
    fn sign(&self, data: &[&[u8]]) -> Tag {
        self.sign_concat(&[], data, &[])
    }

    /// Calculates a tag over the concatenation of `first`, the items in `middle`, and `last`.
    fn sign_concat(&self, first: &[u8], middle: &[&[u8]], last: &[u8]) -> Tag;

    /// Returns the length of the tag returned by a computation using
    /// this key.
    fn tag_len(&self) -> usize;
}
