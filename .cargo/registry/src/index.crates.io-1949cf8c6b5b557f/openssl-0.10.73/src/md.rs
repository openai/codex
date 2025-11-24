//! Message digest algorithms.

#[cfg(ossl300)]
use crate::cvt_p;
#[cfg(ossl300)]
use crate::error::ErrorStack;
#[cfg(ossl300)]
use crate::lib_ctx::LibCtxRef;
use crate::nid::Nid;
use cfg_if::cfg_if;
use foreign_types::{ForeignTypeRef, Opaque};
use openssl_macros::corresponds;
#[cfg(ossl300)]
use std::ffi::CString;
#[cfg(ossl300)]
use std::ptr;

cfg_if! {
    if #[cfg(ossl300)] {
        use foreign_types::ForeignType;
        use std::ops::{Deref, DerefMut};

        type Inner = *mut ffi::EVP_MD;

        impl Drop for Md {
            #[inline]
            fn drop(&mut self) {
                unsafe {
                    ffi::EVP_MD_free(self.as_ptr());
                }
            }
        }

        impl ForeignType for Md {
            type CType = ffi::EVP_MD;
            type Ref = MdRef;

            #[inline]
            unsafe fn from_ptr(ptr: *mut Self::CType) -> Self {
                Md(ptr)
            }

            #[inline]
            fn as_ptr(&self) -> *mut Self::CType {
                self.0
            }
        }

        impl Deref for Md {
            type Target = MdRef;

            #[inline]
            fn deref(&self) -> &Self::Target {
                unsafe {
                    MdRef::from_ptr(self.as_ptr())
                }
            }
        }

        impl DerefMut for Md {
            #[inline]
            fn deref_mut(&mut self) -> &mut Self::Target {
                unsafe {
                    MdRef::from_ptr_mut(self.as_ptr())
                }
            }
        }
    } else {
        enum Inner {}
    }
}

/// A message digest algorithm.
pub struct Md(Inner);

unsafe impl Sync for Md {}
unsafe impl Send for Md {}

impl Md {
    /// Returns the `Md` corresponding to an [`Nid`].
    #[corresponds(EVP_get_digestbynid)]
    pub fn from_nid(type_: Nid) -> Option<&'static MdRef> {
        ffi::init();
        unsafe {
            let ptr = ffi::EVP_get_digestbynid(type_.as_raw());
            if ptr.is_null() {
                None
            } else {
                Some(MdRef::from_ptr(ptr as *mut _))
            }
        }
    }

    /// Fetches an `Md` object corresponding to the specified algorithm name and properties.
    ///
    /// Requires OpenSSL 3.0.0 or newer.
    #[corresponds(EVP_MD_fetch)]
    #[cfg(ossl300)]
    pub fn fetch(
        ctx: Option<&LibCtxRef>,
        algorithm: &str,
        properties: Option<&str>,
    ) -> Result<Self, ErrorStack> {
        ffi::init();
        let algorithm = CString::new(algorithm).unwrap();
        let properties = properties.map(|s| CString::new(s).unwrap());

        unsafe {
            let ptr = cvt_p(ffi::EVP_MD_fetch(
                ctx.map_or(ptr::null_mut(), ForeignTypeRef::as_ptr),
                algorithm.as_ptr(),
                properties.as_ref().map_or(ptr::null_mut(), |s| s.as_ptr()),
            ))?;

            Ok(Md::from_ptr(ptr))
        }
    }

    #[inline]
    #[cfg(not(boringssl))]
    pub fn null() -> &'static MdRef {
        unsafe { MdRef::from_ptr(ffi::EVP_md_null() as *mut _) }
    }

    #[inline]
    pub fn md5() -> &'static MdRef {
        unsafe { MdRef::from_ptr(ffi::EVP_md5() as *mut _) }
    }

    #[inline]
    pub fn sha1() -> &'static MdRef {
        unsafe { MdRef::from_ptr(ffi::EVP_sha1() as *mut _) }
    }

    #[inline]
    pub fn sha224() -> &'static MdRef {
        unsafe { MdRef::from_ptr(ffi::EVP_sha224() as *mut _) }
    }

    #[inline]
    pub fn sha256() -> &'static MdRef {
        unsafe { MdRef::from_ptr(ffi::EVP_sha256() as *mut _) }
    }

    #[inline]
    pub fn sha384() -> &'static MdRef {
        unsafe { MdRef::from_ptr(ffi::EVP_sha384() as *mut _) }
    }

    #[inline]
    pub fn sha512() -> &'static MdRef {
        unsafe { MdRef::from_ptr(ffi::EVP_sha512() as *mut _) }
    }

    #[cfg(any(ossl111, libressl380))]
    #[inline]
    pub fn sha3_224() -> &'static MdRef {
        unsafe { MdRef::from_ptr(ffi::EVP_sha3_224() as *mut _) }
    }

    #[cfg(any(ossl111, libressl380))]
    #[inline]
    pub fn sha3_256() -> &'static MdRef {
        unsafe { MdRef::from_ptr(ffi::EVP_sha3_256() as *mut _) }
    }

    #[cfg(any(ossl111, libressl380))]
    #[inline]
    pub fn sha3_384() -> &'static MdRef {
        unsafe { MdRef::from_ptr(ffi::EVP_sha3_384() as *mut _) }
    }

    #[cfg(any(ossl111, libressl380))]
    #[inline]
    pub fn sha3_512() -> &'static MdRef {
        unsafe { MdRef::from_ptr(ffi::EVP_sha3_512() as *mut _) }
    }

    #[cfg(ossl111)]
    #[inline]
    pub fn shake128() -> &'static MdRef {
        unsafe { MdRef::from_ptr(ffi::EVP_shake128() as *mut _) }
    }

    #[cfg(ossl111)]
    #[inline]
    pub fn shake256() -> &'static MdRef {
        unsafe { MdRef::from_ptr(ffi::EVP_shake256() as *mut _) }
    }

    #[cfg(not(osslconf = "OPENSSL_NO_RMD160"))]
    #[inline]
    pub fn ripemd160() -> &'static MdRef {
        unsafe { MdRef::from_ptr(ffi::EVP_ripemd160() as *mut _) }
    }

    #[cfg(all(any(ossl111, libressl291), not(osslconf = "OPENSSL_NO_SM3")))]
    #[inline]
    pub fn sm3() -> &'static MdRef {
        unsafe { MdRef::from_ptr(ffi::EVP_sm3() as *mut _) }
    }
}

/// A reference to an [`Md`].
pub struct MdRef(Opaque);

impl ForeignTypeRef for MdRef {
    type CType = ffi::EVP_MD;
}

unsafe impl Sync for MdRef {}
unsafe impl Send for MdRef {}

impl MdRef {
    /// Returns the block size of the digest in bytes.
    #[corresponds(EVP_MD_block_size)]
    #[inline]
    pub fn block_size(&self) -> usize {
        unsafe { ffi::EVP_MD_block_size(self.as_ptr()) as usize }
    }

    /// Returns the size of the digest in bytes.
    #[corresponds(EVP_MD_size)]
    #[inline]
    pub fn size(&self) -> usize {
        unsafe { ffi::EVP_MD_size(self.as_ptr()) as usize }
    }

    /// Returns the [`Nid`] of the digest.
    #[corresponds(EVP_MD_type)]
    #[inline]
    pub fn type_(&self) -> Nid {
        unsafe { Nid::from_raw(ffi::EVP_MD_type(self.as_ptr())) }
    }
}

#[cfg(test)]
mod test {
    #[cfg(ossl300)]
    use super::Md;

    #[test]
    #[cfg(ossl300)]
    fn test_md_fetch_properties() {
        assert!(Md::fetch(None, "SHA-256", Some("provider=gibberish")).is_err());
    }
}
