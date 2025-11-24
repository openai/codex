//! Diffie-Hellman key agreement.

use cfg_if::cfg_if;
use foreign_types::{ForeignType, ForeignTypeRef};
use std::mem;
use std::ptr;

use crate::bn::{BigNum, BigNumRef};
use crate::error::ErrorStack;
use crate::pkey::{HasParams, HasPrivate, HasPublic, Params, Private, Public};
use crate::{cvt, cvt_p};
use openssl_macros::corresponds;

generic_foreign_type_and_impl_send_sync! {
    type CType = ffi::DH;
    fn drop = ffi::DH_free;

    pub struct Dh<T>;

    pub struct DhRef<T>;
}

impl<T> DhRef<T>
where
    T: HasParams,
{
    to_pem! {
        /// Serializes the parameters into a PEM-encoded PKCS#3 DHparameter structure.
        ///
        /// The output will have a header of `-----BEGIN DH PARAMETERS-----`.
        #[corresponds(PEM_write_bio_DHparams)]
        params_to_pem,
        ffi::PEM_write_bio_DHparams
    }

    to_der! {
        /// Serializes the parameters into a DER-encoded PKCS#3 DHparameter structure.
        #[corresponds(i2d_DHparams)]
        params_to_der,
        ffi::i2d_DHparams
    }

    /// Validates DH parameters for correctness
    #[corresponds(DH_check_key)]
    pub fn check_key(&self) -> Result<bool, ErrorStack> {
        unsafe {
            let mut codes = 0;
            cvt(ffi::DH_check(self.as_ptr(), &mut codes))?;
            Ok(codes == 0)
        }
    }
}

impl Dh<Params> {
    pub fn from_params(p: BigNum, g: BigNum, q: BigNum) -> Result<Dh<Params>, ErrorStack> {
        Self::from_pqg(p, Some(q), g)
    }

    /// Creates a DH instance based upon the given primes and generator params.
    #[corresponds(DH_set0_pqg)]
    pub fn from_pqg(
        prime_p: BigNum,
        prime_q: Option<BigNum>,
        generator: BigNum,
    ) -> Result<Dh<Params>, ErrorStack> {
        unsafe {
            let dh = Dh::from_ptr(cvt_p(ffi::DH_new())?);
            cvt(DH_set0_pqg(
                dh.0,
                prime_p.as_ptr(),
                prime_q.as_ref().map_or(ptr::null_mut(), |q| q.as_ptr()),
                generator.as_ptr(),
            ))?;
            mem::forget((prime_p, prime_q, generator));
            Ok(dh)
        }
    }

    /// Sets the public key on the DH object.
    pub fn set_public_key(self, pub_key: BigNum) -> Result<Dh<Public>, ErrorStack> {
        unsafe {
            let dh_ptr = self.0;
            cvt(DH_set0_key(dh_ptr, pub_key.as_ptr(), ptr::null_mut()))?;
            mem::forget((self, pub_key));
            Ok(Dh::from_ptr(dh_ptr))
        }
    }

    /// Sets the private key on the DH object and recomputes the public key.
    pub fn set_private_key(self, priv_key: BigNum) -> Result<Dh<Private>, ErrorStack> {
        unsafe {
            let dh_ptr = self.0;
            cvt(DH_set0_key(dh_ptr, ptr::null_mut(), priv_key.as_ptr()))?;
            mem::forget(priv_key);

            cvt(ffi::DH_generate_key(dh_ptr))?;
            mem::forget(self);
            Ok(Dh::from_ptr(dh_ptr))
        }
    }

    /// Sets the public and private keys on the DH object.
    pub fn set_key(self, pub_key: BigNum, priv_key: BigNum) -> Result<Dh<Private>, ErrorStack> {
        unsafe {
            let dh_ptr = self.0;
            cvt(DH_set0_key(dh_ptr, pub_key.as_ptr(), priv_key.as_ptr()))?;
            mem::forget((self, pub_key, priv_key));
            Ok(Dh::from_ptr(dh_ptr))
        }
    }

    /// Generates DH params based on the given `prime_len` and a fixed `generator` value.
    #[corresponds(DH_generate_parameters_ex)]
    pub fn generate_params(prime_len: u32, generator: u32) -> Result<Dh<Params>, ErrorStack> {
        unsafe {
            let dh = Dh::from_ptr(cvt_p(ffi::DH_new())?);
            cvt(ffi::DH_generate_parameters_ex(
                dh.0,
                prime_len as i32,
                generator as i32,
                ptr::null_mut(),
            ))?;
            Ok(dh)
        }
    }

    /// Generates a public and a private key based on the DH params.
    #[corresponds(DH_generate_key)]
    pub fn generate_key(self) -> Result<Dh<Private>, ErrorStack> {
        unsafe {
            let dh_ptr = self.0;
            cvt(ffi::DH_generate_key(dh_ptr))?;
            mem::forget(self);
            Ok(Dh::from_ptr(dh_ptr))
        }
    }

    from_pem! {
        /// Deserializes a PEM-encoded PKCS#3 DHpararameters structure.
        ///
        /// The input should have a header of `-----BEGIN DH PARAMETERS-----`.
        #[corresponds(PEM_read_bio_DHparams)]
        params_from_pem,
        Dh<Params>,
        ffi::PEM_read_bio_DHparams
    }

    from_der! {
        /// Deserializes a DER-encoded PKCS#3 DHparameters structure.
        #[corresponds(d2i_DHparams)]
        params_from_der,
        Dh<Params>,
        ffi::d2i_DHparams
    }

    /// Requires OpenSSL 1.0.2 or newer.
    #[corresponds(DH_get_1024_160)]
    #[cfg(any(ossl102, ossl110))]
    pub fn get_1024_160() -> Result<Dh<Params>, ErrorStack> {
        unsafe {
            ffi::init();
            cvt_p(ffi::DH_get_1024_160()).map(|p| Dh::from_ptr(p))
        }
    }

    /// Requires OpenSSL 1.0.2 or newer.
    #[corresponds(DH_get_2048_224)]
    #[cfg(any(ossl102, ossl110))]
    pub fn get_2048_224() -> Result<Dh<Params>, ErrorStack> {
        unsafe {
            ffi::init();
            cvt_p(ffi::DH_get_2048_224()).map(|p| Dh::from_ptr(p))
        }
    }

    /// Requires OpenSSL 1.0.2 or newer.
    #[corresponds(DH_get_2048_256)]
    #[cfg(any(ossl102, ossl110))]
    pub fn get_2048_256() -> Result<Dh<Params>, ErrorStack> {
        unsafe {
            ffi::init();
            cvt_p(ffi::DH_get_2048_256()).map(|p| Dh::from_ptr(p))
        }
    }
}

impl<T> Dh<T>
where
    T: HasParams,
{
    /// Returns the prime `p` from the DH instance.
    #[corresponds(DH_get0_pqg)]
    pub fn prime_p(&self) -> &BigNumRef {
        let mut p = ptr::null();
        unsafe {
            DH_get0_pqg(self.as_ptr(), &mut p, ptr::null_mut(), ptr::null_mut());
            BigNumRef::from_ptr(p as *mut _)
        }
    }

    /// Returns the prime `q` from the DH instance.
    #[corresponds(DH_get0_pqg)]
    pub fn prime_q(&self) -> Option<&BigNumRef> {
        let mut q = ptr::null();
        unsafe {
            DH_get0_pqg(self.as_ptr(), ptr::null_mut(), &mut q, ptr::null_mut());
            if q.is_null() {
                None
            } else {
                Some(BigNumRef::from_ptr(q as *mut _))
            }
        }
    }

    /// Returns the generator from the DH instance.
    #[corresponds(DH_get0_pqg)]
    pub fn generator(&self) -> &BigNumRef {
        let mut g = ptr::null();
        unsafe {
            DH_get0_pqg(self.as_ptr(), ptr::null_mut(), ptr::null_mut(), &mut g);
            BigNumRef::from_ptr(g as *mut _)
        }
    }
}

impl<T> DhRef<T>
where
    T: HasPublic,
{
    /// Returns the public key from the DH instance.
    #[corresponds(DH_get0_key)]
    pub fn public_key(&self) -> &BigNumRef {
        let mut pub_key = ptr::null();
        unsafe {
            DH_get0_key(self.as_ptr(), &mut pub_key, ptr::null_mut());
            BigNumRef::from_ptr(pub_key as *mut _)
        }
    }
}

impl<T> DhRef<T>
where
    T: HasPrivate,
{
    /// Computes a shared secret from the own private key and the given `public_key`.
    #[corresponds(DH_compute_key)]
    pub fn compute_key(&self, public_key: &BigNumRef) -> Result<Vec<u8>, ErrorStack> {
        unsafe {
            let key_len = ffi::DH_size(self.as_ptr());
            let mut key = vec![0u8; key_len as usize];
            cvt(ffi::DH_compute_key(
                key.as_mut_ptr(),
                public_key.as_ptr(),
                self.as_ptr(),
            ))?;
            Ok(key)
        }
    }

    /// Returns the private key from the DH instance.
    #[corresponds(DH_get0_key)]
    pub fn private_key(&self) -> &BigNumRef {
        let mut priv_key = ptr::null();
        unsafe {
            DH_get0_key(self.as_ptr(), ptr::null_mut(), &mut priv_key);
            BigNumRef::from_ptr(priv_key as *mut _)
        }
    }
}

cfg_if! {
    if #[cfg(any(ossl110, libressl270, boringssl, awslc))] {
        use ffi::{DH_set0_pqg, DH_get0_pqg, DH_get0_key, DH_set0_key};
    } else {
        #[allow(bad_style)]
        unsafe fn DH_set0_pqg(
            dh: *mut ffi::DH,
            p: *mut ffi::BIGNUM,
            q: *mut ffi::BIGNUM,
            g: *mut ffi::BIGNUM,
        ) -> ::libc::c_int {
            (*dh).p = p;
            (*dh).q = q;
            (*dh).g = g;
            1
        }

        #[allow(bad_style)]
        unsafe fn DH_get0_pqg(
            dh: *mut ffi::DH,
            p: *mut *const ffi::BIGNUM,
            q: *mut *const ffi::BIGNUM,
            g: *mut *const ffi::BIGNUM,
        ) {
            if !p.is_null() {
                *p = (*dh).p;
            }
            if !q.is_null() {
                *q = (*dh).q;
            }
            if !g.is_null() {
                *g = (*dh).g;
            }
        }

        #[allow(bad_style)]
        unsafe fn DH_set0_key(
            dh: *mut ffi::DH,
            pub_key: *mut ffi::BIGNUM,
            priv_key: *mut ffi::BIGNUM,
        ) -> ::libc::c_int {
            (*dh).pub_key = pub_key;
            (*dh).priv_key = priv_key;
            1
        }

        #[allow(bad_style)]
        unsafe fn DH_get0_key(
            dh: *mut ffi::DH,
            pub_key: *mut *const ffi::BIGNUM,
            priv_key: *mut *const ffi::BIGNUM,
        ) {
            if !pub_key.is_null() {
                *pub_key = (*dh).pub_key;
            }
            if !priv_key.is_null() {
                *priv_key = (*dh).priv_key;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::bn::BigNum;
    use crate::dh::Dh;
    #[cfg(all(not(any(boringssl, awslc)), ossl110))]
    use crate::pkey::PKey;
    use crate::ssl::{SslContext, SslMethod};

    #[test]
    #[cfg(ossl102)]
    fn test_dh_rfc5114() {
        let mut ctx = SslContext::builder(SslMethod::tls()).unwrap();
        let dh2 = Dh::get_2048_224().unwrap();
        ctx.set_tmp_dh(&dh2).unwrap();
        let dh3 = Dh::get_2048_256().unwrap();
        ctx.set_tmp_dh(&dh3).unwrap();
    }

    #[test]
    fn test_dh_params() {
        let mut ctx = SslContext::builder(SslMethod::tls()).unwrap();
        let prime_p = BigNum::from_hex_str(
            "87A8E61DB4B6663CFFBBD19C651959998CEEF608660DD0F25D2CEED4435E3B00E00DF8F1D61957D4FAF7DF\
             4561B2AA3016C3D91134096FAA3BF4296D830E9A7C209E0C6497517ABD5A8A9D306BCF67ED91F9E6725B47\
             58C022E0B1EF4275BF7B6C5BFC11D45F9088B941F54EB1E59BB8BC39A0BF12307F5C4FDB70C581B23F76B6\
             3ACAE1CAA6B7902D52526735488A0EF13C6D9A51BFA4AB3AD8347796524D8EF6A167B5A41825D967E144E5\
             140564251CCACB83E6B486F6B3CA3F7971506026C0B857F689962856DED4010ABD0BE621C3A3960A54E710\
             C375F26375D7014103A4B54330C198AF126116D2276E11715F693877FAD7EF09CADB094AE91E1A1597",
        ).unwrap();
        let prime_q = BigNum::from_hex_str(
            "3FB32C9B73134D0B2E77506660EDBD484CA7B18F21EF205407F4793A1A0BA12510DBC15077BE463FFF4FED\
             4AAC0BB555BE3A6C1B0C6B47B1BC3773BF7E8C6F62901228F8C28CBB18A55AE31341000A650196F931C77A\
             57F2DDF463E5E9EC144B777DE62AAAB8A8628AC376D282D6ED3864E67982428EBC831D14348F6F2F9193B5\
             045AF2767164E1DFC967C1FB3F2E55A4BD1BFFE83B9C80D052B985D182EA0ADB2A3B7313D3FE14C8484B1E\
             052588B9B7D2BBD2DF016199ECD06E1557CD0915B3353BBB64E0EC377FD028370DF92B52C7891428CDC67E\
             B6184B523D1DB246C32F63078490F00EF8D647D148D47954515E2327CFEF98C582664B4C0F6CC41659",
        ).unwrap();
        let generator = BigNum::from_hex_str(
            "8CF83642A709A097B447997640129DA299B1A47D1EB3750BA308B0FE64F5FBD3",
        )
        .unwrap();
        let dh = Dh::from_params(
            prime_p.to_owned().unwrap(),
            generator.to_owned().unwrap(),
            prime_q.to_owned().unwrap(),
        )
        .unwrap();
        ctx.set_tmp_dh(&dh).unwrap();

        assert_eq!(dh.prime_p(), &prime_p);
        assert_eq!(dh.prime_q().unwrap(), &prime_q);
        assert_eq!(dh.generator(), &generator);
    }

    #[test]
    #[cfg(all(not(any(boringssl, awslc)), ossl110))]
    fn test_from_dhx_serializes_q() {
        let p = BigNum::from_hex_str("00ad107e1e9123a9d0d660faa79559c51fa20d64e5683b9fd1b54b1597b61d0a75e6fa141df95a56dbaf9a3c407ba1df15eb3d688a309c180e1de6b85a1274a0a66d3f8152ad6ac2129037c9edefda4df8d91e8fef55b7394b7ad5b7d0b6c12207c9f98d11ed34dbf6c6ba0b2c8bbc27be6a00e0a0b9c49708b3bf8a317091883681286130bc8985db1602e714415d9330278273c7de31efdc7310f7121fd5a07415987d9adc0a486dcdf93acc44328387315d75e198c641a480cd86a1b9e587e8be60e69cc928b2b9c52172e413042e9b23f10b0e16e79763c9b53dcf4ba80a29e3fb73c16b8e75b97ef363e2ffa31f71cf9de5384e71b81c0ac4dffe0c10e64f").unwrap();
        let g = BigNum::from_hex_str("00ac4032ef4f2d9ae39df30b5c8ffdac506cdebe7b89998caf74866a08cfe4ffe3a6824a4e10b9a6f0dd921f01a70c4afaab739d7700c29f52c57db17c620a8652be5e9001a8d66ad7c17669101999024af4d027275ac1348bb8a762d0521bc98ae247150422ea1ed409939d54da7460cdb5f6c6b250717cbef180eb34118e98d119529a45d6f834566e3025e316a330efbb77a86f0c1ab15b051ae3d428c8f8acb70a8137150b8eeb10e183edd19963ddd9e263e4770589ef6aa21e7f5f2ff381b539cce3409d13cd566afbb48d6c019181e1bcfe94b30269edfe72fe9b6aa4bd7b5a0f1c71cfff4c19c418e1f6ec017981bc087f2a7065b384b890d3191f2bfa").unwrap();
        let q = BigNum::from_hex_str("00801c0d34c58d93fe997177101f80535a4738cebcbf389a99b36371eb")
            .unwrap();
        let y = BigNum::from_hex_str("0082c165bb576243ecf46d58c3d1501616955fca0320fa95ea11d2e6c1b9cf217676720dc1c08c85bf20c4d232b60a29a1e51c7b773bc645014587c525c86151b30d75486ec7b6c98efb5f74955b83116d01d0af1232af89213c2de574369d701aba9357300b920d3d8b98252d46c46952c16a5f33554b38317809c7b9add4701f5c158c1b7035e9fe39366ececb90d2896b78c523c4a577287ef5ba7a2663ed58aa20b5ec66e30f316610dfaa38583e495ab6af771c284387e660edbef4edb872e2e80e1d244ee95622e76d028e61c1e887c2aa792717362139f4dd26eafd49b2366eeb2350b01fe1b56022a2809e379559c37b375ba01c4eaacc14fd1b247837").unwrap();

        let dh = Dh::from_params(p, g, q).unwrap();
        let dh = dh.set_public_key(y).unwrap();

        // Verify that 'q' is serialized in the public key.
        let pkey = PKey::from_dhx(dh).unwrap();
        assert_eq!(pkey.public_key_to_der().unwrap(), b"\x30\x82\x03\x44\x30\x82\x02\x36\x06\x07\x2a\x86\x48\xce\x3e\x02\x01\x30\x82\x02\x29\x02\x82\x01\x01\x00\xad\x10\x7e\x1e\x91\x23\xa9\xd0\xd6\x60\xfa\xa7\x95\x59\xc5\x1f\xa2\x0d\x64\xe5\x68\x3b\x9f\xd1\xb5\x4b\x15\x97\xb6\x1d\x0a\x75\xe6\xfa\x14\x1d\xf9\x5a\x56\xdb\xaf\x9a\x3c\x40\x7b\xa1\xdf\x15\xeb\x3d\x68\x8a\x30\x9c\x18\x0e\x1d\xe6\xb8\x5a\x12\x74\xa0\xa6\x6d\x3f\x81\x52\xad\x6a\xc2\x12\x90\x37\xc9\xed\xef\xda\x4d\xf8\xd9\x1e\x8f\xef\x55\xb7\x39\x4b\x7a\xd5\xb7\xd0\xb6\xc1\x22\x07\xc9\xf9\x8d\x11\xed\x34\xdb\xf6\xc6\xba\x0b\x2c\x8b\xbc\x27\xbe\x6a\x00\xe0\xa0\xb9\xc4\x97\x08\xb3\xbf\x8a\x31\x70\x91\x88\x36\x81\x28\x61\x30\xbc\x89\x85\xdb\x16\x02\xe7\x14\x41\x5d\x93\x30\x27\x82\x73\xc7\xde\x31\xef\xdc\x73\x10\xf7\x12\x1f\xd5\xa0\x74\x15\x98\x7d\x9a\xdc\x0a\x48\x6d\xcd\xf9\x3a\xcc\x44\x32\x83\x87\x31\x5d\x75\xe1\x98\xc6\x41\xa4\x80\xcd\x86\xa1\xb9\xe5\x87\xe8\xbe\x60\xe6\x9c\xc9\x28\xb2\xb9\xc5\x21\x72\xe4\x13\x04\x2e\x9b\x23\xf1\x0b\x0e\x16\xe7\x97\x63\xc9\xb5\x3d\xcf\x4b\xa8\x0a\x29\xe3\xfb\x73\xc1\x6b\x8e\x75\xb9\x7e\xf3\x63\xe2\xff\xa3\x1f\x71\xcf\x9d\xe5\x38\x4e\x71\xb8\x1c\x0a\xc4\xdf\xfe\x0c\x10\xe6\x4f\x02\x82\x01\x01\x00\xac\x40\x32\xef\x4f\x2d\x9a\xe3\x9d\xf3\x0b\x5c\x8f\xfd\xac\x50\x6c\xde\xbe\x7b\x89\x99\x8c\xaf\x74\x86\x6a\x08\xcf\xe4\xff\xe3\xa6\x82\x4a\x4e\x10\xb9\xa6\xf0\xdd\x92\x1f\x01\xa7\x0c\x4a\xfa\xab\x73\x9d\x77\x00\xc2\x9f\x52\xc5\x7d\xb1\x7c\x62\x0a\x86\x52\xbe\x5e\x90\x01\xa8\xd6\x6a\xd7\xc1\x76\x69\x10\x19\x99\x02\x4a\xf4\xd0\x27\x27\x5a\xc1\x34\x8b\xb8\xa7\x62\xd0\x52\x1b\xc9\x8a\xe2\x47\x15\x04\x22\xea\x1e\xd4\x09\x93\x9d\x54\xda\x74\x60\xcd\xb5\xf6\xc6\xb2\x50\x71\x7c\xbe\xf1\x80\xeb\x34\x11\x8e\x98\xd1\x19\x52\x9a\x45\xd6\xf8\x34\x56\x6e\x30\x25\xe3\x16\xa3\x30\xef\xbb\x77\xa8\x6f\x0c\x1a\xb1\x5b\x05\x1a\xe3\xd4\x28\xc8\xf8\xac\xb7\x0a\x81\x37\x15\x0b\x8e\xeb\x10\xe1\x83\xed\xd1\x99\x63\xdd\xd9\xe2\x63\xe4\x77\x05\x89\xef\x6a\xa2\x1e\x7f\x5f\x2f\xf3\x81\xb5\x39\xcc\xe3\x40\x9d\x13\xcd\x56\x6a\xfb\xb4\x8d\x6c\x01\x91\x81\xe1\xbc\xfe\x94\xb3\x02\x69\xed\xfe\x72\xfe\x9b\x6a\xa4\xbd\x7b\x5a\x0f\x1c\x71\xcf\xff\x4c\x19\xc4\x18\xe1\xf6\xec\x01\x79\x81\xbc\x08\x7f\x2a\x70\x65\xb3\x84\xb8\x90\xd3\x19\x1f\x2b\xfa\x02\x1d\x00\x80\x1c\x0d\x34\xc5\x8d\x93\xfe\x99\x71\x77\x10\x1f\x80\x53\x5a\x47\x38\xce\xbc\xbf\x38\x9a\x99\xb3\x63\x71\xeb\x03\x82\x01\x06\x00\x02\x82\x01\x01\x00\x82\xc1\x65\xbb\x57\x62\x43\xec\xf4\x6d\x58\xc3\xd1\x50\x16\x16\x95\x5f\xca\x03\x20\xfa\x95\xea\x11\xd2\xe6\xc1\xb9\xcf\x21\x76\x76\x72\x0d\xc1\xc0\x8c\x85\xbf\x20\xc4\xd2\x32\xb6\x0a\x29\xa1\xe5\x1c\x7b\x77\x3b\xc6\x45\x01\x45\x87\xc5\x25\xc8\x61\x51\xb3\x0d\x75\x48\x6e\xc7\xb6\xc9\x8e\xfb\x5f\x74\x95\x5b\x83\x11\x6d\x01\xd0\xaf\x12\x32\xaf\x89\x21\x3c\x2d\xe5\x74\x36\x9d\x70\x1a\xba\x93\x57\x30\x0b\x92\x0d\x3d\x8b\x98\x25\x2d\x46\xc4\x69\x52\xc1\x6a\x5f\x33\x55\x4b\x38\x31\x78\x09\xc7\xb9\xad\xd4\x70\x1f\x5c\x15\x8c\x1b\x70\x35\xe9\xfe\x39\x36\x6e\xce\xcb\x90\xd2\x89\x6b\x78\xc5\x23\xc4\xa5\x77\x28\x7e\xf5\xba\x7a\x26\x63\xed\x58\xaa\x20\xb5\xec\x66\xe3\x0f\x31\x66\x10\xdf\xaa\x38\x58\x3e\x49\x5a\xb6\xaf\x77\x1c\x28\x43\x87\xe6\x60\xed\xbe\xf4\xed\xb8\x72\xe2\xe8\x0e\x1d\x24\x4e\xe9\x56\x22\xe7\x6d\x02\x8e\x61\xc1\xe8\x87\xc2\xaa\x79\x27\x17\x36\x21\x39\xf4\xdd\x26\xea\xfd\x49\xb2\x36\x6e\xeb\x23\x50\xb0\x1f\xe1\xb5\x60\x22\xa2\x80\x9e\x37\x95\x59\xc3\x7b\x37\x5b\xa0\x1c\x4e\xaa\xcc\x14\xfd\x1b\x24\x78\x37");
    }

    #[test]
    #[cfg(ossl102)]
    fn test_dh_stored_restored() {
        let dh1 = Dh::get_2048_256().unwrap();
        let key1 = dh1.generate_key().unwrap();

        let dh2 = Dh::get_2048_256().unwrap();
        let key2 = dh2
            .set_private_key(key1.private_key().to_owned().unwrap())
            .unwrap();

        assert_eq!(key1.public_key(), key2.public_key());
        assert_eq!(key1.private_key(), key2.private_key());
    }

    #[test]
    #[cfg(ossl102)]
    fn test_set_keys() {
        let dh1 = Dh::get_2048_256().unwrap();
        let key1 = dh1.generate_key().unwrap();

        let dh2 = Dh::get_2048_256().unwrap();
        let key2 = dh2
            .set_public_key(key1.public_key().to_owned().unwrap())
            .unwrap();

        assert_eq!(key1.public_key(), key2.public_key());

        let dh3 = Dh::get_2048_256().unwrap();
        let key3 = dh3
            .set_key(
                key1.public_key().to_owned().unwrap(),
                key1.private_key().to_owned().unwrap(),
            )
            .unwrap();
        assert_eq!(key1.public_key(), key3.public_key());
        assert_eq!(key1.private_key(), key3.private_key());
    }

    #[test]
    fn test_dh_from_pem() {
        let mut ctx = SslContext::builder(SslMethod::tls()).unwrap();
        let params = include_bytes!("../test/dhparams.pem");
        let dh = Dh::params_from_pem(params).unwrap();
        ctx.set_tmp_dh(&dh).unwrap();
    }

    #[test]
    fn test_dh_from_der() {
        let params = include_bytes!("../test/dhparams.pem");
        let dh = Dh::params_from_pem(params).unwrap();
        let der = dh.params_to_der().unwrap();
        Dh::params_from_der(&der).unwrap();
    }

    #[test]
    #[cfg(ossl102)]
    fn test_dh_generate_key_compute_key() {
        let dh1 = Dh::get_2048_224().unwrap().generate_key().unwrap();
        let dh2 = Dh::get_2048_224().unwrap().generate_key().unwrap();

        let shared_a = dh1.compute_key(dh2.public_key()).unwrap();
        let shared_b = dh2.compute_key(dh1.public_key()).unwrap();

        assert_eq!(shared_a, shared_b);
    }

    #[test]
    fn test_dh_generate_params_generate_key_compute_key() {
        let dh_params1 = Dh::generate_params(512, 2).unwrap();
        let dh_params2 = Dh::from_pqg(
            dh_params1.prime_p().to_owned().unwrap(),
            None,
            dh_params1.generator().to_owned().unwrap(),
        )
        .unwrap();

        let dh1 = dh_params1.generate_key().unwrap();
        let dh2 = dh_params2.generate_key().unwrap();

        let shared_a = dh1.compute_key(dh2.public_key()).unwrap();
        let shared_b = dh2.compute_key(dh1.public_key()).unwrap();

        assert_eq!(shared_a, shared_b);
    }

    #[test]
    fn test_dh_check_key() {
        let dh1 = Dh::generate_params(512, 2).unwrap();
        let p = BigNum::from_hex_str("04").unwrap();
        let g = BigNum::from_hex_str("02").unwrap();
        let dh2 = Dh::from_pqg(p, None, g).unwrap();
        assert!(dh1.check_key().unwrap());
        assert!(matches!(dh2.check_key(), Ok(false) | Err(_)));
    }
}
