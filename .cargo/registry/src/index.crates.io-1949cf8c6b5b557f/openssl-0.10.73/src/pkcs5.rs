#[cfg(not(any(boringssl, awslc)))]
use libc::c_int;
use std::convert::TryInto;
#[cfg(not(any(boringssl, awslc)))]
use std::ptr;

use crate::cvt;
use crate::error::ErrorStack;
use crate::hash::MessageDigest;
#[cfg(not(any(boringssl, awslc)))]
use crate::symm::Cipher;
use openssl_macros::corresponds;

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct KeyIvPair {
    pub key: Vec<u8>,
    pub iv: Option<Vec<u8>>,
}

/// Derives a key and an IV from various parameters.
///
/// If specified, `salt` must be 8 bytes in length.
///
/// If the total key and IV length is less than 16 bytes and MD5 is used then
/// the algorithm is compatible with the key derivation algorithm from PKCS#5
/// v1.5 or PBKDF1 from PKCS#5 v2.0.
///
/// New applications should not use this and instead use
/// `pbkdf2_hmac` or another more modern key derivation algorithm.
#[corresponds(EVP_BytesToKey)]
#[allow(clippy::useless_conversion)]
#[cfg(not(any(boringssl, awslc)))]
pub fn bytes_to_key(
    cipher: Cipher,
    digest: MessageDigest,
    data: &[u8],
    salt: Option<&[u8]>,
    count: i32,
) -> Result<KeyIvPair, ErrorStack> {
    unsafe {
        assert!(data.len() <= c_int::MAX as usize);
        let salt_ptr = match salt {
            Some(salt) => {
                assert_eq!(salt.len(), ffi::PKCS5_SALT_LEN as usize);
                salt.as_ptr()
            }
            None => ptr::null(),
        };

        ffi::init();

        let mut iv = cipher.iv_len().map(|l| vec![0; l]);

        let cipher = cipher.as_ptr();
        let digest = digest.as_ptr();

        let len = cvt(ffi::EVP_BytesToKey(
            cipher,
            digest,
            salt_ptr,
            ptr::null(),
            data.len() as c_int,
            count.into(),
            ptr::null_mut(),
            ptr::null_mut(),
        ))?;

        let mut key = vec![0; len as usize];
        let iv_ptr = iv
            .as_mut()
            .map(|v| v.as_mut_ptr())
            .unwrap_or(ptr::null_mut());

        cvt(ffi::EVP_BytesToKey(
            cipher,
            digest,
            salt_ptr,
            data.as_ptr(),
            data.len() as c_int,
            count as c_int,
            key.as_mut_ptr(),
            iv_ptr,
        ))?;

        Ok(KeyIvPair { key, iv })
    }
}

/// Derives a key from a password and salt using the PBKDF2-HMAC algorithm with a digest function.
#[corresponds(PKCS5_PBKDF2_HMAC)]
pub fn pbkdf2_hmac(
    pass: &[u8],
    salt: &[u8],
    iter: usize,
    hash: MessageDigest,
    key: &mut [u8],
) -> Result<(), ErrorStack> {
    unsafe {
        ffi::init();
        cvt(ffi::PKCS5_PBKDF2_HMAC(
            pass.as_ptr() as *const _,
            pass.len().try_into().unwrap(),
            salt.as_ptr(),
            salt.len().try_into().unwrap(),
            iter.try_into().unwrap(),
            hash.as_ptr(),
            key.len().try_into().unwrap(),
            key.as_mut_ptr(),
        ))
        .map(|_| ())
    }
}

/// Derives a key from a password and salt using the scrypt algorithm.
///
/// Requires OpenSSL 1.1.0 or newer.
#[corresponds(EVP_PBE_scrypt)]
#[cfg(all(any(ossl110, boringssl, awslc), not(osslconf = "OPENSSL_NO_SCRYPT")))]
#[allow(clippy::useless_conversion)]
pub fn scrypt(
    pass: &[u8],
    salt: &[u8],
    n: u64,
    r: u64,
    p: u64,
    maxmem: u64,
    key: &mut [u8],
) -> Result<(), ErrorStack> {
    unsafe {
        ffi::init();
        cvt(ffi::EVP_PBE_scrypt(
            pass.as_ptr() as *const _,
            pass.len(),
            salt.as_ptr() as *const _,
            salt.len(),
            n,
            r,
            p,
            maxmem.try_into().unwrap(),
            key.as_mut_ptr() as *mut _,
            key.len(),
        ))
        .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use crate::hash::MessageDigest;
    #[cfg(not(any(boringssl, awslc)))]
    use crate::symm::Cipher;

    // Test vectors from
    // https://git.lysator.liu.se/nettle/nettle/blob/nettle_3.1.1_release_20150424/testsuite/pbkdf2-test.c
    #[test]
    fn pbkdf2_hmac_sha256() {
        let mut buf = [0; 16];

        super::pbkdf2_hmac(b"passwd", b"salt", 1, MessageDigest::sha256(), &mut buf).unwrap();
        assert_eq!(
            buf,
            &[
                0x55_u8, 0xac_u8, 0x04_u8, 0x6e_u8, 0x56_u8, 0xe3_u8, 0x08_u8, 0x9f_u8, 0xec_u8,
                0x16_u8, 0x91_u8, 0xc2_u8, 0x25_u8, 0x44_u8, 0xb6_u8, 0x05_u8,
            ][..]
        );

        super::pbkdf2_hmac(
            b"Password",
            b"NaCl",
            80000,
            MessageDigest::sha256(),
            &mut buf,
        )
        .unwrap();
        assert_eq!(
            buf,
            &[
                0x4d_u8, 0xdc_u8, 0xd8_u8, 0xf6_u8, 0x0b_u8, 0x98_u8, 0xbe_u8, 0x21_u8, 0x83_u8,
                0x0c_u8, 0xee_u8, 0x5e_u8, 0xf2_u8, 0x27_u8, 0x01_u8, 0xf9_u8,
            ][..]
        );
    }

    // Test vectors from
    // https://git.lysator.liu.se/nettle/nettle/blob/nettle_3.1.1_release_20150424/testsuite/pbkdf2-test.c
    #[test]
    fn pbkdf2_hmac_sha512() {
        let mut buf = [0; 64];

        super::pbkdf2_hmac(b"password", b"NaCL", 1, MessageDigest::sha512(), &mut buf).unwrap();
        assert_eq!(
            &buf[..],
            &[
                0x73_u8, 0xde_u8, 0xcf_u8, 0xa5_u8, 0x8a_u8, 0xa2_u8, 0xe8_u8, 0x4f_u8, 0x94_u8,
                0x77_u8, 0x1a_u8, 0x75_u8, 0x73_u8, 0x6b_u8, 0xb8_u8, 0x8b_u8, 0xd3_u8, 0xc7_u8,
                0xb3_u8, 0x82_u8, 0x70_u8, 0xcf_u8, 0xb5_u8, 0x0c_u8, 0xb3_u8, 0x90_u8, 0xed_u8,
                0x78_u8, 0xb3_u8, 0x05_u8, 0x65_u8, 0x6a_u8, 0xf8_u8, 0x14_u8, 0x8e_u8, 0x52_u8,
                0x45_u8, 0x2b_u8, 0x22_u8, 0x16_u8, 0xb2_u8, 0xb8_u8, 0x09_u8, 0x8b_u8, 0x76_u8,
                0x1f_u8, 0xc6_u8, 0x33_u8, 0x60_u8, 0x60_u8, 0xa0_u8, 0x9f_u8, 0x76_u8, 0x41_u8,
                0x5e_u8, 0x9f_u8, 0x71_u8, 0xea_u8, 0x47_u8, 0xf9_u8, 0xe9_u8, 0x06_u8, 0x43_u8,
                0x06_u8,
            ][..]
        );

        super::pbkdf2_hmac(
            b"pass\0word",
            b"sa\0lt",
            1,
            MessageDigest::sha512(),
            &mut buf,
        )
        .unwrap();
        assert_eq!(
            &buf[..],
            &[
                0x71_u8, 0xa0_u8, 0xec_u8, 0x84_u8, 0x2a_u8, 0xbd_u8, 0x5c_u8, 0x67_u8, 0x8b_u8,
                0xcf_u8, 0xd1_u8, 0x45_u8, 0xf0_u8, 0x9d_u8, 0x83_u8, 0x52_u8, 0x2f_u8, 0x93_u8,
                0x36_u8, 0x15_u8, 0x60_u8, 0x56_u8, 0x3c_u8, 0x4d_u8, 0x0d_u8, 0x63_u8, 0xb8_u8,
                0x83_u8, 0x29_u8, 0x87_u8, 0x10_u8, 0x90_u8, 0xe7_u8, 0x66_u8, 0x04_u8, 0xa4_u8,
                0x9a_u8, 0xf0_u8, 0x8f_u8, 0xe7_u8, 0xc9_u8, 0xf5_u8, 0x71_u8, 0x56_u8, 0xc8_u8,
                0x79_u8, 0x09_u8, 0x96_u8, 0xb2_u8, 0x0f_u8, 0x06_u8, 0xbc_u8, 0x53_u8, 0x5e_u8,
                0x5a_u8, 0xb5_u8, 0x44_u8, 0x0d_u8, 0xf7_u8, 0xe8_u8, 0x78_u8, 0x29_u8, 0x6f_u8,
                0xa7_u8,
            ][..]
        );

        super::pbkdf2_hmac(
            b"passwordPASSWORDpassword",
            b"salt\0\0\0",
            50,
            MessageDigest::sha512(),
            &mut buf,
        )
        .unwrap();
        assert_eq!(
            &buf[..],
            &[
                0x01_u8, 0x68_u8, 0x71_u8, 0xa4_u8, 0xc4_u8, 0xb7_u8, 0x5f_u8, 0x96_u8, 0x85_u8,
                0x7f_u8, 0xd2_u8, 0xb9_u8, 0xf8_u8, 0xca_u8, 0x28_u8, 0x02_u8, 0x3b_u8, 0x30_u8,
                0xee_u8, 0x2a_u8, 0x39_u8, 0xf5_u8, 0xad_u8, 0xca_u8, 0xc8_u8, 0xc9_u8, 0x37_u8,
                0x5f_u8, 0x9b_u8, 0xda_u8, 0x1c_u8, 0xcd_u8, 0x1b_u8, 0x6f_u8, 0x0b_u8, 0x2f_u8,
                0xc3_u8, 0xad_u8, 0xda_u8, 0x50_u8, 0x54_u8, 0x12_u8, 0xe7_u8, 0x9d_u8, 0x89_u8,
                0x00_u8, 0x56_u8, 0xc6_u8, 0x2e_u8, 0x52_u8, 0x4c_u8, 0x7d_u8, 0x51_u8, 0x15_u8,
                0x4b_u8, 0x1a_u8, 0x85_u8, 0x34_u8, 0x57_u8, 0x5b_u8, 0xd0_u8, 0x2d_u8, 0xee_u8,
                0x39_u8,
            ][..]
        );
    }

    #[test]
    #[cfg(not(any(boringssl, awslc)))]
    fn bytes_to_key() {
        let salt = [16_u8, 34_u8, 19_u8, 23_u8, 141_u8, 4_u8, 207_u8, 221_u8];

        let data = [
            143_u8, 210_u8, 75_u8, 63_u8, 214_u8, 179_u8, 155_u8, 241_u8, 242_u8, 31_u8, 154_u8,
            56_u8, 198_u8, 145_u8, 192_u8, 64_u8, 2_u8, 245_u8, 167_u8, 220_u8, 55_u8, 119_u8,
            233_u8, 136_u8, 139_u8, 27_u8, 71_u8, 242_u8, 119_u8, 175_u8, 65_u8, 207_u8,
        ];

        let expected_key = vec![
            249_u8, 115_u8, 114_u8, 97_u8, 32_u8, 213_u8, 165_u8, 146_u8, 58_u8, 87_u8, 234_u8,
            3_u8, 43_u8, 250_u8, 97_u8, 114_u8, 26_u8, 98_u8, 245_u8, 246_u8, 238_u8, 177_u8,
            229_u8, 161_u8, 183_u8, 224_u8, 174_u8, 3_u8, 6_u8, 244_u8, 236_u8, 255_u8,
        ];
        let expected_iv = vec![
            4_u8, 223_u8, 153_u8, 219_u8, 28_u8, 142_u8, 234_u8, 68_u8, 227_u8, 69_u8, 98_u8,
            107_u8, 208_u8, 14_u8, 236_u8, 60_u8,
        ];

        assert_eq!(
            super::bytes_to_key(
                Cipher::aes_256_cbc(),
                MessageDigest::sha1(),
                &data,
                Some(&salt),
                1,
            )
            .unwrap(),
            super::KeyIvPair {
                key: expected_key,
                iv: Some(expected_iv),
            }
        );
    }

    #[test]
    #[cfg(any(ossl110, boringssl, awslc))]
    fn scrypt() {
        let pass = "pleaseletmein";
        let salt = "SodiumChloride";
        let expected =
            "7023bdcb3afd7348461c06cd81fd38ebfda8fbba904f8e3ea9b543f6545da1f2d5432955613\
             f0fcf62d49705242a9af9e61e85dc0d651e40dfcf017b45575887";

        let mut actual = [0; 64];
        super::scrypt(
            pass.as_bytes(),
            salt.as_bytes(),
            16384,
            8,
            1,
            0,
            &mut actual,
        )
        .unwrap();
        assert_eq!(hex::encode(&actual[..]), expected);
    }
}
