//! Low level Elliptic Curve Digital Signature Algorithm (ECDSA) functions.

use cfg_if::cfg_if;
use foreign_types::{ForeignType, ForeignTypeRef};
use libc::c_int;
use std::mem;
use std::ptr;

use crate::bn::{BigNum, BigNumRef};
use crate::ec::EcKeyRef;
use crate::error::ErrorStack;
use crate::pkey::{HasPrivate, HasPublic};
use crate::util::ForeignTypeRefExt;
use crate::{cvt_n, cvt_p, LenType};
use openssl_macros::corresponds;

foreign_type_and_impl_send_sync! {
    type CType = ffi::ECDSA_SIG;
    fn drop = ffi::ECDSA_SIG_free;

    /// A low level interface to ECDSA.
    pub struct EcdsaSig;
    /// A reference to an [`EcdsaSig`].
    pub struct EcdsaSigRef;
}

impl EcdsaSig {
    /// Computes a digital signature of the hash value `data` using the private EC key eckey.
    #[corresponds(ECDSA_do_sign)]
    pub fn sign<T>(data: &[u8], eckey: &EcKeyRef<T>) -> Result<EcdsaSig, ErrorStack>
    where
        T: HasPrivate,
    {
        unsafe {
            assert!(data.len() <= c_int::MAX as usize);
            let sig = cvt_p(ffi::ECDSA_do_sign(
                data.as_ptr(),
                data.len() as LenType,
                eckey.as_ptr(),
            ))?;
            Ok(EcdsaSig::from_ptr(sig))
        }
    }

    /// Returns a new `EcdsaSig` by setting the `r` and `s` values associated with an ECDSA signature.
    #[corresponds(ECDSA_SIG_set0)]
    pub fn from_private_components(r: BigNum, s: BigNum) -> Result<EcdsaSig, ErrorStack> {
        unsafe {
            let sig = cvt_p(ffi::ECDSA_SIG_new())?;
            ECDSA_SIG_set0(sig, r.as_ptr(), s.as_ptr());
            mem::forget((r, s));
            Ok(EcdsaSig::from_ptr(sig))
        }
    }

    from_der! {
        /// Decodes a DER-encoded ECDSA signature.
        #[corresponds(d2i_ECDSA_SIG)]
        from_der,
        EcdsaSig,
        ffi::d2i_ECDSA_SIG
    }
}

impl EcdsaSigRef {
    to_der! {
        /// Serializes the ECDSA signature into a DER-encoded ECDSASignature structure.
        #[corresponds(i2d_ECDSA_SIG)]
        to_der,
        ffi::i2d_ECDSA_SIG
    }

    /// Verifies if the signature is a valid ECDSA signature using the given public key.
    #[corresponds(ECDSA_do_verify)]
    pub fn verify<T>(&self, data: &[u8], eckey: &EcKeyRef<T>) -> Result<bool, ErrorStack>
    where
        T: HasPublic,
    {
        unsafe {
            assert!(data.len() <= c_int::MAX as usize);
            cvt_n(ffi::ECDSA_do_verify(
                data.as_ptr(),
                data.len() as LenType,
                self.as_ptr(),
                eckey.as_ptr(),
            ))
            .map(|x| x == 1)
        }
    }

    /// Returns internal component: `r` of an `EcdsaSig`. (See X9.62 or FIPS 186-2)
    #[corresponds(ECDSA_SIG_get0)]
    pub fn r(&self) -> &BigNumRef {
        unsafe {
            let mut r = ptr::null();
            ECDSA_SIG_get0(self.as_ptr(), &mut r, ptr::null_mut());
            BigNumRef::from_const_ptr(r)
        }
    }

    /// Returns internal components: `s` of an `EcdsaSig`. (See X9.62 or FIPS 186-2)
    #[corresponds(ECDSA_SIG_get0)]
    pub fn s(&self) -> &BigNumRef {
        unsafe {
            let mut s = ptr::null();
            ECDSA_SIG_get0(self.as_ptr(), ptr::null_mut(), &mut s);
            BigNumRef::from_const_ptr(s)
        }
    }
}

cfg_if! {
    if #[cfg(any(ossl110, libressl273, boringssl, awslc))] {
        use ffi::{ECDSA_SIG_set0, ECDSA_SIG_get0};
    } else {
        #[allow(bad_style)]
        unsafe fn ECDSA_SIG_set0(
            sig: *mut ffi::ECDSA_SIG,
            r: *mut ffi::BIGNUM,
            s: *mut ffi::BIGNUM,
        ) -> c_int {
            if r.is_null() || s.is_null() {
                return 0;
            }
            ffi::BN_clear_free((*sig).r);
            ffi::BN_clear_free((*sig).s);
            (*sig).r = r;
            (*sig).s = s;
            1
        }

        #[allow(bad_style)]
        unsafe fn ECDSA_SIG_get0(
            sig: *const ffi::ECDSA_SIG,
            pr: *mut *const ffi::BIGNUM,
            ps: *mut *const ffi::BIGNUM)
        {
            if !pr.is_null() {
                (*pr) = (*sig).r;
            }
            if !ps.is_null() {
                (*ps) = (*sig).s;
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::ec::EcGroup;
    use crate::ec::EcKey;
    use crate::nid::Nid;
    use crate::pkey::{Private, Public};

    fn get_public_key(group: &EcGroup, x: &EcKey<Private>) -> Result<EcKey<Public>, ErrorStack> {
        EcKey::from_public_key(group, x.public_key())
    }

    #[test]
    #[cfg_attr(osslconf = "OPENSSL_NO_EC", ignore)]
    fn sign_and_verify() {
        let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
        let private_key = EcKey::generate(&group).unwrap();
        let public_key = get_public_key(&group, &private_key).unwrap();

        let private_key2 = EcKey::generate(&group).unwrap();
        let public_key2 = get_public_key(&group, &private_key2).unwrap();

        let data = String::from("hello");
        let res = EcdsaSig::sign(data.as_bytes(), &private_key).unwrap();

        // Signature can be verified using the correct data & correct public key
        let verification = res.verify(data.as_bytes(), &public_key).unwrap();
        assert!(verification);

        // Signature will not be verified using the incorrect data but the correct public key
        let verification2 = res
            .verify(String::from("hello2").as_bytes(), &public_key)
            .unwrap();
        assert!(!verification2);

        // Signature will not be verified using the correct data but the incorrect public key
        let verification3 = res.verify(data.as_bytes(), &public_key2).unwrap();
        assert!(!verification3);
    }

    #[test]
    #[cfg_attr(osslconf = "OPENSSL_NO_EC", ignore)]
    fn check_private_components() {
        let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
        let private_key = EcKey::generate(&group).unwrap();
        let public_key = get_public_key(&group, &private_key).unwrap();
        let data = String::from("hello");
        let res = EcdsaSig::sign(data.as_bytes(), &private_key).unwrap();

        let verification = res.verify(data.as_bytes(), &public_key).unwrap();
        assert!(verification);

        let r = res.r().to_owned().unwrap();
        let s = res.s().to_owned().unwrap();

        let res2 = EcdsaSig::from_private_components(r, s).unwrap();
        let verification2 = res2.verify(data.as_bytes(), &public_key).unwrap();
        assert!(verification2);
    }

    #[test]
    #[cfg_attr(osslconf = "OPENSSL_NO_EC", ignore)]
    fn serialize_deserialize() {
        let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
        let private_key = EcKey::generate(&group).unwrap();
        let public_key = get_public_key(&group, &private_key).unwrap();

        let data = String::from("hello");
        let res = EcdsaSig::sign(data.as_bytes(), &private_key).unwrap();

        let der = res.to_der().unwrap();
        let sig = EcdsaSig::from_der(&der).unwrap();

        let verification = sig.verify(data.as_bytes(), &public_key).unwrap();
        assert!(verification);
    }
}
