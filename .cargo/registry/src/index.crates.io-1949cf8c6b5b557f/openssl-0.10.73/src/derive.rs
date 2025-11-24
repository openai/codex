//! Shared secret derivation.
//!
//! # Example
//!
//! The following example implements [ECDH] using `NIST P-384` keys:
//!
//! ```
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # use std::convert::TryInto;
//! use openssl::bn::BigNumContext;
//! use openssl::pkey::PKey;
//! use openssl::derive::Deriver;
//! use openssl::ec::{EcGroup, EcKey, EcPoint, PointConversionForm};
//! use openssl::nid::Nid;
//!
//! let group = EcGroup::from_curve_name(Nid::SECP384R1)?;
//!
//! let first: PKey<_> = EcKey::generate(&group)?.try_into()?;
//!
//! // second party generates an ephemeral key and derives
//! // a shared secret using first party's public key
//! let shared_key = EcKey::generate(&group)?;
//! // shared_public is sent to first party
//! let mut ctx = BigNumContext::new()?;
//! let shared_public = shared_key.public_key().to_bytes(
//!        &group,
//!        PointConversionForm::COMPRESSED,
//!        &mut ctx,
//!    )?;
//!
//! let shared_key: PKey<_> = shared_key.try_into()?;
//! let mut deriver = Deriver::new(&shared_key)?;
//! deriver.set_peer(&first)?;
//! // secret can be used e.g. as a symmetric encryption key
//! let secret = deriver.derive_to_vec()?;
//! # drop(deriver);
//!
//! // first party derives the same shared secret using
//! // shared_public
//! let point = EcPoint::from_bytes(&group, &shared_public, &mut ctx)?;
//! let recipient_key: PKey<_> = EcKey::from_public_key(&group, &point)?.try_into()?;
//! let mut deriver = Deriver::new(&first)?;
//! deriver.set_peer(&recipient_key)?;
//! let first_secret = deriver.derive_to_vec()?;
//!
//! assert_eq!(secret, first_secret);
//! # Ok(()) }
//! ```
//!
//! [ECDH]: https://wiki.openssl.org/index.php/Elliptic_Curve_Diffie_Hellman

use foreign_types::ForeignTypeRef;
use std::marker::PhantomData;
use std::ptr;

use crate::error::ErrorStack;
use crate::pkey::{HasPrivate, HasPublic, PKeyRef};
use crate::{cvt, cvt_p};
use openssl_macros::corresponds;

/// A type used to derive a shared secret between two keys.
pub struct Deriver<'a>(*mut ffi::EVP_PKEY_CTX, PhantomData<&'a ()>);

unsafe impl Sync for Deriver<'_> {}
unsafe impl Send for Deriver<'_> {}

#[allow(clippy::len_without_is_empty)]
impl<'a> Deriver<'a> {
    /// Creates a new `Deriver` using the provided private key.
    ///
    /// This corresponds to [`EVP_PKEY_derive_init`].
    ///
    /// [`EVP_PKEY_derive_init`]: https://www.openssl.org/docs/manmaster/crypto/EVP_PKEY_derive_init.html
    pub fn new<T>(key: &'a PKeyRef<T>) -> Result<Deriver<'a>, ErrorStack>
    where
        T: HasPrivate,
    {
        unsafe {
            cvt_p(ffi::EVP_PKEY_CTX_new(key.as_ptr(), ptr::null_mut()))
                .map(|p| Deriver(p, PhantomData))
                .and_then(|ctx| cvt(ffi::EVP_PKEY_derive_init(ctx.0)).map(|_| ctx))
        }
    }

    /// Sets the peer key used for secret derivation.
    #[corresponds(EVP_PKEY_derive_set_peer)]
    pub fn set_peer<T>(&mut self, key: &'a PKeyRef<T>) -> Result<(), ErrorStack>
    where
        T: HasPublic,
    {
        unsafe { cvt(ffi::EVP_PKEY_derive_set_peer(self.0, key.as_ptr())).map(|_| ()) }
    }

    /// Sets the peer key used for secret derivation along with optionally validating the peer public key.
    ///
    /// Requires OpenSSL 3.0.0 or newer.
    #[corresponds(EVP_PKEY_derive_set_peer_ex)]
    #[cfg(ossl300)]
    pub fn set_peer_ex<T>(
        &mut self,
        key: &'a PKeyRef<T>,
        validate_peer: bool,
    ) -> Result<(), ErrorStack>
    where
        T: HasPublic,
    {
        unsafe {
            cvt(ffi::EVP_PKEY_derive_set_peer_ex(
                self.0,
                key.as_ptr(),
                validate_peer as i32,
            ))
            .map(|_| ())
        }
    }

    /// Returns the size of the shared secret.
    ///
    /// It can be used to size the buffer passed to [`Deriver::derive`].
    ///
    /// This corresponds to [`EVP_PKEY_derive`].
    ///
    /// [`Deriver::derive`]: #method.derive
    /// [`EVP_PKEY_derive`]: https://www.openssl.org/docs/manmaster/crypto/EVP_PKEY_derive_init.html
    pub fn len(&mut self) -> Result<usize, ErrorStack> {
        unsafe {
            let mut len = 0;
            cvt(ffi::EVP_PKEY_derive(self.0, ptr::null_mut(), &mut len)).map(|_| len)
        }
    }

    /// Derives a shared secret between the two keys, writing it into the buffer.
    ///
    /// Returns the number of bytes written.
    ///
    /// This corresponds to [`EVP_PKEY_derive`].
    ///
    /// [`EVP_PKEY_derive`]: https://www.openssl.org/docs/manmaster/crypto/EVP_PKEY_derive_init.html
    pub fn derive(&mut self, buf: &mut [u8]) -> Result<usize, ErrorStack> {
        let mut len = buf.len();
        unsafe {
            cvt(ffi::EVP_PKEY_derive(
                self.0,
                buf.as_mut_ptr() as *mut _,
                &mut len,
            ))
            .map(|_| len)
        }
    }

    /// A convenience function which derives a shared secret and returns it in a new buffer.
    ///
    /// This simply wraps [`Deriver::len`] and [`Deriver::derive`].
    ///
    /// [`Deriver::len`]: #method.len
    /// [`Deriver::derive`]: #method.derive
    pub fn derive_to_vec(&mut self) -> Result<Vec<u8>, ErrorStack> {
        let len = self.len()?;
        let mut buf = vec![0; len];
        let len = self.derive(&mut buf)?;
        buf.truncate(len);
        Ok(buf)
    }
}

impl Drop for Deriver<'_> {
    fn drop(&mut self) {
        unsafe {
            ffi::EVP_PKEY_CTX_free(self.0);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::ec::{EcGroup, EcKey};
    use crate::nid::Nid;
    use crate::pkey::PKey;

    #[test]
    fn derive_without_peer() {
        let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
        let ec_key = EcKey::generate(&group).unwrap();
        let pkey = PKey::from_ec_key(ec_key).unwrap();
        let mut deriver = Deriver::new(&pkey).unwrap();
        deriver.derive_to_vec().unwrap_err();
    }

    #[test]
    fn test_ec_key_derive() {
        let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
        let ec_key = EcKey::generate(&group).unwrap();
        let ec_key2 = EcKey::generate(&group).unwrap();
        let pkey = PKey::from_ec_key(ec_key).unwrap();
        let pkey2 = PKey::from_ec_key(ec_key2).unwrap();
        let mut deriver = Deriver::new(&pkey).unwrap();
        deriver.set_peer(&pkey2).unwrap();
        let shared = deriver.derive_to_vec().unwrap();
        assert!(!shared.is_empty());
    }

    #[test]
    #[cfg(ossl300)]
    fn test_ec_key_derive_ex() {
        let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
        let ec_key = EcKey::generate(&group).unwrap();
        let ec_key2 = EcKey::generate(&group).unwrap();
        let pkey = PKey::from_ec_key(ec_key).unwrap();
        let pkey2 = PKey::from_ec_key(ec_key2).unwrap();
        let mut deriver = Deriver::new(&pkey).unwrap();
        deriver.set_peer_ex(&pkey2, true).unwrap();
        let shared = deriver.derive_to_vec().unwrap();
        assert!(!shared.is_empty());
    }
}
