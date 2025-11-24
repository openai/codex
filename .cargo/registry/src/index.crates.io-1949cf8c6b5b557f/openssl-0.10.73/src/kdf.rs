#[cfg(ossl320)]
struct EvpKdf(*mut ffi::EVP_KDF);

#[cfg(ossl320)]
impl Drop for EvpKdf {
    fn drop(&mut self) {
        unsafe {
            ffi::EVP_KDF_free(self.0);
        }
    }
}

#[cfg(ossl320)]
struct EvpKdfCtx(*mut ffi::EVP_KDF_CTX);

#[cfg(ossl320)]
impl Drop for EvpKdfCtx {
    fn drop(&mut self) {
        unsafe {
            ffi::EVP_KDF_CTX_free(self.0);
        }
    }
}

cfg_if::cfg_if! {
    if #[cfg(all(ossl320, not(osslconf = "OPENSSL_NO_ARGON2")))] {
        use std::cmp;
        use std::ffi::c_void;
        use std::mem::MaybeUninit;
        use std::ptr;
        use foreign_types::ForeignTypeRef;
        use libc::c_char;
        use crate::{cvt, cvt_p};
        use crate::lib_ctx::LibCtxRef;
        use crate::error::ErrorStack;

        /// Derives a key using the argon2id algorithm.
        ///
        /// To use multiple cores to process the lanes in parallel you must
        /// set a global max thread count using `OSSL_set_max_threads`. On
        /// builds with no threads all lanes will be processed sequentially.
        ///
        /// Requires OpenSSL 3.2.0 or newer.
        #[allow(clippy::too_many_arguments)]
        pub fn argon2id(
            ctx: Option<&LibCtxRef>,
            pass: &[u8],
            salt: &[u8],
            ad: Option<&[u8]>,
            secret: Option<&[u8]>,
            mut iter: u32,
            mut lanes: u32,
            mut memcost: u32,
            out: &mut [u8],
        ) -> Result<(), ErrorStack> {
            unsafe {
                ffi::init();
                let libctx = ctx.map_or(ptr::null_mut(), ForeignTypeRef::as_ptr);

                let max_threads = ffi::OSSL_get_max_threads(libctx);
                let mut threads = 1;
                // If max_threads is 0, then this isn't a threaded build.
                // If max_threads is > u32::MAX we need to clamp since
                // argon2id's threads parameter is a u32.
                if max_threads > 0 {
                    threads = cmp::min(lanes, cmp::min(max_threads, u32::MAX as u64) as u32);
                }
                let mut params: [ffi::OSSL_PARAM; 10] =
                    core::array::from_fn(|_| MaybeUninit::<ffi::OSSL_PARAM>::zeroed().assume_init());
                let mut idx = 0;
                params[idx] = ffi::OSSL_PARAM_construct_octet_string(
                    b"pass\0".as_ptr() as *const c_char,
                    pass.as_ptr() as *mut c_void,
                    pass.len(),
                );
                idx += 1;
                params[idx] = ffi::OSSL_PARAM_construct_octet_string(
                    b"salt\0".as_ptr() as *const c_char,
                    salt.as_ptr() as *mut c_void,
                    salt.len(),
                );
                idx += 1;
                params[idx] =
                    ffi::OSSL_PARAM_construct_uint(b"threads\0".as_ptr() as *const c_char, &mut threads);
                idx += 1;
                params[idx] =
                    ffi::OSSL_PARAM_construct_uint(b"lanes\0".as_ptr() as *const c_char, &mut lanes);
                idx += 1;
                params[idx] =
                    ffi::OSSL_PARAM_construct_uint(b"memcost\0".as_ptr() as *const c_char, &mut memcost);
                idx += 1;
                params[idx] =
                    ffi::OSSL_PARAM_construct_uint(b"iter\0".as_ptr() as *const c_char, &mut iter);
                idx += 1;
                let mut size = out.len() as u32;
                params[idx] =
                    ffi::OSSL_PARAM_construct_uint(b"size\0".as_ptr() as *const c_char, &mut size);
                idx += 1;
                if let Some(ad) = ad {
                    params[idx] = ffi::OSSL_PARAM_construct_octet_string(
                        b"ad\0".as_ptr() as *const c_char,
                        ad.as_ptr() as *mut c_void,
                        ad.len(),
                    );
                    idx += 1;
                }
                if let Some(secret) = secret {
                    params[idx] = ffi::OSSL_PARAM_construct_octet_string(
                        b"secret\0".as_ptr() as *const c_char,
                        secret.as_ptr() as *mut c_void,
                        secret.len(),
                    );
                    idx += 1;
                }
                params[idx] = ffi::OSSL_PARAM_construct_end();

                let argon2 = EvpKdf(cvt_p(ffi::EVP_KDF_fetch(
                    libctx,
                    b"ARGON2ID\0".as_ptr() as *const c_char,
                    ptr::null(),
                ))?);
                let ctx = EvpKdfCtx(cvt_p(ffi::EVP_KDF_CTX_new(argon2.0))?);
                cvt(ffi::EVP_KDF_derive(
                    ctx.0,
                    out.as_mut_ptr(),
                    out.len(),
                    params.as_ptr(),
                ))
                .map(|_| ())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(all(ossl320, not(osslconf = "OPENSSL_NO_ARGON2")))]
    fn argon2id() {
        // RFC 9106 test vector for argon2id
        let pass = hex::decode("0101010101010101010101010101010101010101010101010101010101010101")
            .unwrap();
        let salt = hex::decode("02020202020202020202020202020202").unwrap();
        let secret = hex::decode("0303030303030303").unwrap();
        let ad = hex::decode("040404040404040404040404").unwrap();
        let expected = "0d640df58d78766c08c037a34a8b53c9d01ef0452d75b65eb52520e96b01e659";

        let mut actual = [0u8; 32];
        super::argon2id(
            None,
            &pass,
            &salt,
            Some(&ad),
            Some(&secret),
            3,
            4,
            32,
            &mut actual,
        )
        .unwrap();
        assert_eq!(hex::encode(&actual[..]), expected);
    }

    #[test]
    #[cfg(all(ossl320, not(osslconf = "OPENSSL_NO_ARGON2")))]
    fn argon2id_no_ad_secret() {
        // Test vector from OpenSSL
        let pass = b"";
        let salt = hex::decode("02020202020202020202020202020202").unwrap();
        let expected = "0a34f1abde67086c82e785eaf17c68382259a264f4e61b91cd2763cb75ac189a";

        let mut actual = [0u8; 32];
        super::argon2id(None, pass, &salt, None, None, 3, 4, 32, &mut actual).unwrap();
        assert_eq!(hex::encode(&actual[..]), expected);
    }
}
