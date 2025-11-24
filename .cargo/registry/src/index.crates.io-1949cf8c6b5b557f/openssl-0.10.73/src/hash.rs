//! Message digest (hash) computation support.
//!
//! # Examples
//!
//! Calculate a hash in one go:
//!
//! ```
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use openssl::hash::{hash, MessageDigest};
//!
//! let data = b"\x42\xF4\x97\xE0";
//! let spec = b"\x7c\x43\x0f\x17\x8a\xef\xdf\x14\x87\xfe\xe7\x14\x4e\x96\x41\xe2";
//! let res = hash(MessageDigest::md5(), data)?;
//! assert_eq!(&*res, spec);
//! # Ok(()) }
//! ```
//!
//! Supply the input in chunks:
//!
//! ```
//! use openssl::hash::{Hasher, MessageDigest};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut hasher = Hasher::new(MessageDigest::sha256())?;
//! hasher.update(b"test")?;
//! hasher.update(b"this")?;
//! let digest: &[u8] = &hasher.finish()?;
//!
//! let expected = hex::decode("9740e652ab5b4acd997a7cca13d6696702ccb2d441cca59fc6e285127f28cfe6")?;
//! assert_eq!(digest, expected);
//! # Ok(()) }
//! ```
use cfg_if::cfg_if;
use std::ffi::CString;
use std::fmt;
use std::io;
use std::io::prelude::*;
use std::ops::{Deref, DerefMut};
use std::ptr;

use crate::error::ErrorStack;
use crate::nid::Nid;
use crate::{cvt, cvt_p};
use openssl_macros::corresponds;

cfg_if! {
    if #[cfg(any(ossl110, boringssl, libressl382, awslc))] {
        use ffi::{EVP_MD_CTX_free, EVP_MD_CTX_new};
    } else {
        use ffi::{EVP_MD_CTX_create as EVP_MD_CTX_new, EVP_MD_CTX_destroy as EVP_MD_CTX_free};
    }
}

/// A message digest algorithm.
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct MessageDigest(*const ffi::EVP_MD);

impl MessageDigest {
    /// Creates a `MessageDigest` from a raw OpenSSL pointer.
    ///
    /// # Safety
    ///
    /// The caller must ensure the pointer is valid.
    pub unsafe fn from_ptr(x: *const ffi::EVP_MD) -> Self {
        MessageDigest(x)
    }

    /// Returns the `MessageDigest` corresponding to an `Nid`.
    #[corresponds(EVP_get_digestbynid)]
    pub fn from_nid(type_: Nid) -> Option<MessageDigest> {
        ffi::init();
        unsafe {
            let ptr = ffi::EVP_get_digestbynid(type_.as_raw());
            if ptr.is_null() {
                None
            } else {
                Some(MessageDigest(ptr))
            }
        }
    }

    /// Returns the `MessageDigest` corresponding to an algorithm name.
    #[corresponds(EVP_get_digestbyname)]
    pub fn from_name(name: &str) -> Option<MessageDigest> {
        ffi::init();
        let name = CString::new(name).ok()?;
        unsafe {
            let ptr = ffi::EVP_get_digestbyname(name.as_ptr());
            if ptr.is_null() {
                None
            } else {
                Some(MessageDigest(ptr))
            }
        }
    }

    #[cfg(not(boringssl))]
    pub fn null() -> MessageDigest {
        unsafe { MessageDigest(ffi::EVP_md_null()) }
    }

    pub fn md5() -> MessageDigest {
        unsafe { MessageDigest(ffi::EVP_md5()) }
    }

    pub fn sha1() -> MessageDigest {
        unsafe { MessageDigest(ffi::EVP_sha1()) }
    }

    pub fn sha224() -> MessageDigest {
        unsafe { MessageDigest(ffi::EVP_sha224()) }
    }

    pub fn sha256() -> MessageDigest {
        unsafe { MessageDigest(ffi::EVP_sha256()) }
    }

    pub fn sha384() -> MessageDigest {
        unsafe { MessageDigest(ffi::EVP_sha384()) }
    }

    pub fn sha512() -> MessageDigest {
        unsafe { MessageDigest(ffi::EVP_sha512()) }
    }

    #[cfg(any(ossl111, libressl380, awslc))]
    pub fn sha3_224() -> MessageDigest {
        unsafe { MessageDigest(ffi::EVP_sha3_224()) }
    }

    #[cfg(any(ossl111, libressl380, awslc))]
    pub fn sha3_256() -> MessageDigest {
        unsafe { MessageDigest(ffi::EVP_sha3_256()) }
    }

    #[cfg(any(ossl111, libressl380, awslc))]
    pub fn sha3_384() -> MessageDigest {
        unsafe { MessageDigest(ffi::EVP_sha3_384()) }
    }

    #[cfg(any(ossl111, libressl380, awslc))]
    pub fn sha3_512() -> MessageDigest {
        unsafe { MessageDigest(ffi::EVP_sha3_512()) }
    }

    #[cfg(any(ossl111, awslc))]
    pub fn shake_128() -> MessageDigest {
        unsafe { MessageDigest(ffi::EVP_shake128()) }
    }

    #[cfg(any(ossl111, awslc))]
    pub fn shake_256() -> MessageDigest {
        unsafe { MessageDigest(ffi::EVP_shake256()) }
    }

    #[cfg(not(osslconf = "OPENSSL_NO_RMD160"))]
    pub fn ripemd160() -> MessageDigest {
        unsafe { MessageDigest(ffi::EVP_ripemd160()) }
    }

    #[cfg(all(any(ossl111, libressl291), not(osslconf = "OPENSSL_NO_SM3")))]
    pub fn sm3() -> MessageDigest {
        unsafe { MessageDigest(ffi::EVP_sm3()) }
    }

    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn as_ptr(&self) -> *const ffi::EVP_MD {
        self.0
    }

    /// The block size of the digest in bytes.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn block_size(&self) -> usize {
        unsafe { ffi::EVP_MD_block_size(self.0) as usize }
    }

    /// The size of the digest in bytes.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn size(&self) -> usize {
        unsafe { ffi::EVP_MD_size(self.0) as usize }
    }

    /// The name of the digest.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn type_(&self) -> Nid {
        Nid::from_raw(unsafe { ffi::EVP_MD_type(self.0) })
    }
}

unsafe impl Sync for MessageDigest {}
unsafe impl Send for MessageDigest {}

#[derive(PartialEq, Copy, Clone)]
enum State {
    Reset,
    Updated,
    #[cfg(ossl330)]
    Squeeze,
    Finalized,
}

use self::State::*;

/// Provides message digest (hash) computation.
///
/// # Examples
///
/// ```
/// use openssl::hash::{Hasher, MessageDigest};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let data = [b"\x42\xF4", b"\x97\xE0"];
/// let spec = b"\x7c\x43\x0f\x17\x8a\xef\xdf\x14\x87\xfe\xe7\x14\x4e\x96\x41\xe2";
/// let mut h = Hasher::new(MessageDigest::md5())?;
/// h.update(data[0])?;
/// h.update(data[1])?;
/// let res = h.finish()?;
/// assert_eq!(&*res, spec);
/// # Ok(()) }
/// ```
///
/// # Warning
///
/// Don't actually use MD5 and SHA-1 hashes, they're not secure anymore.
///
/// Don't ever hash passwords, use the functions in the `pkcs5` module or bcrypt/scrypt instead.
///
/// For extendable output functions (XOFs, i.e. SHAKE128/SHAKE256),
/// you must use [`Hasher::finish_xof`] instead of [`Hasher::finish`]
/// and provide a `buf` to store the hash. The hash will be as long as
/// the `buf`.
pub struct Hasher {
    ctx: *mut ffi::EVP_MD_CTX,
    md: *const ffi::EVP_MD,
    type_: MessageDigest,
    state: State,
}

unsafe impl Sync for Hasher {}
unsafe impl Send for Hasher {}

impl Hasher {
    /// Creates a new `Hasher` with the specified hash type.
    pub fn new(ty: MessageDigest) -> Result<Hasher, ErrorStack> {
        ffi::init();

        let ctx = unsafe { cvt_p(EVP_MD_CTX_new())? };

        let mut h = Hasher {
            ctx,
            md: ty.as_ptr(),
            type_: ty,
            state: Finalized,
        };
        h.init()?;
        Ok(h)
    }

    fn init(&mut self) -> Result<(), ErrorStack> {
        match self.state {
            Reset => return Ok(()),
            Updated => {
                self.finish()?;
            }
            #[cfg(ossl330)]
            Squeeze => (),
            Finalized => (),
        }
        unsafe {
            cvt(ffi::EVP_DigestInit_ex(self.ctx, self.md, ptr::null_mut()))?;
        }
        self.state = Reset;
        Ok(())
    }

    /// Feeds data into the hasher.
    pub fn update(&mut self, data: &[u8]) -> Result<(), ErrorStack> {
        if self.state == Finalized {
            self.init()?;
        }
        #[cfg(ossl330)]
        if self.state == Squeeze {
            // [`EVP_DigestUpdate`], depending on the implementation, may allow Updates after Squeezes.
            // But, [FIPS 202], as shown in Figure 7, has a distinguished absorbing phase followed by a squeezing phase.
            // Indeed, the [`sha3.c`] implmentation disallows Updates after Squeezes.
            // For consistency, we always return an error when Update is called after Squeeze.
            //
            // [`EVP_DigestUpdate`]: https://github.com/openssl/openssl/blob/b3bb214720f20f3b126ae4b9c330e9a48b835415/crypto/evp/digest.c#L385-L393
            // [FIPS 202]: https://dx.doi.org/10.6028/NIST.FIPS.202
            // [`sha3.c`]: https://github.com/openssl/openssl/blob/b3bb214720f20f3b126ae4b9c330e9a48b835415/crypto/sha/sha3.c#L52-L63
            let errors = ErrorStack::get();
            return Err(errors);
        }
        unsafe {
            cvt(ffi::EVP_DigestUpdate(
                self.ctx,
                data.as_ptr() as *mut _,
                data.len(),
            ))?;
        }
        self.state = Updated;
        Ok(())
    }

    /// Squeezes buf out of the hasher. Can be called multiple times, unlike `finish_xof`.
    /// The output will be as long as the buf.
    #[cfg(ossl330)]
    pub fn squeeze_xof(&mut self, buf: &mut [u8]) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::EVP_DigestSqueeze(
                self.ctx,
                buf.as_mut_ptr(),
                buf.len(),
            ))?;
            self.state = Squeeze;
            Ok(())
        }
    }

    /// Returns the hash of the data written and resets the non-XOF hasher.
    pub fn finish(&mut self) -> Result<DigestBytes, ErrorStack> {
        if self.state == Finalized {
            self.init()?;
        }
        unsafe {
            #[cfg(not(any(boringssl, awslc)))]
            let mut len = ffi::EVP_MAX_MD_SIZE;
            #[cfg(any(boringssl, awslc))]
            let mut len = ffi::EVP_MAX_MD_SIZE as u32;
            let mut buf = [0; ffi::EVP_MAX_MD_SIZE as usize];
            cvt(ffi::EVP_DigestFinal_ex(
                self.ctx,
                buf.as_mut_ptr(),
                &mut len,
            ))?;
            self.state = Finalized;
            Ok(DigestBytes {
                buf,
                len: len as usize,
            })
        }
    }

    /// Writes the hash of the data into the supplied buf and resets the XOF hasher.
    /// The hash will be as long as the buf.
    #[cfg(any(ossl111, awslc))]
    pub fn finish_xof(&mut self, buf: &mut [u8]) -> Result<(), ErrorStack> {
        if self.state == Finalized {
            self.init()?;
        }
        unsafe {
            cvt(ffi::EVP_DigestFinalXOF(
                self.ctx,
                buf.as_mut_ptr(),
                buf.len(),
            ))?;
            self.state = Finalized;
            Ok(())
        }
    }
}

impl Write for Hasher {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.update(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Clone for Hasher {
    fn clone(&self) -> Hasher {
        let ctx = unsafe {
            let ctx = EVP_MD_CTX_new();
            assert!(!ctx.is_null());
            let r = ffi::EVP_MD_CTX_copy_ex(ctx, self.ctx);
            assert_eq!(r, 1);
            ctx
        };
        Hasher {
            ctx,
            md: self.md,
            type_: self.type_,
            state: self.state,
        }
    }
}

impl Drop for Hasher {
    fn drop(&mut self) {
        unsafe {
            if self.state != Finalized {
                drop(self.finish());
            }
            EVP_MD_CTX_free(self.ctx);
        }
    }
}

/// The resulting bytes of a digest.
///
/// This type derefs to a byte slice - it exists to avoid allocating memory to
/// store the digest data.
#[derive(Copy)]
pub struct DigestBytes {
    pub(crate) buf: [u8; ffi::EVP_MAX_MD_SIZE as usize],
    pub(crate) len: usize,
}

impl Clone for DigestBytes {
    #[inline]
    fn clone(&self) -> DigestBytes {
        *self
    }
}

impl Deref for DigestBytes {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        &self.buf[..self.len]
    }
}

impl DerefMut for DigestBytes {
    #[inline]
    fn deref_mut(&mut self) -> &mut [u8] {
        &mut self.buf[..self.len]
    }
}

impl AsRef<[u8]> for DigestBytes {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.deref()
    }
}

impl fmt::Debug for DigestBytes {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, fmt)
    }
}

/// Computes the hash of the `data` with the non-XOF hasher `t`.
///
/// # Examples
///
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use openssl::hash::{hash, MessageDigest};
///
/// let data = b"\x42\xF4\x97\xE0";
/// let spec = b"\x7c\x43\x0f\x17\x8a\xef\xdf\x14\x87\xfe\xe7\x14\x4e\x96\x41\xe2";
/// let res = hash(MessageDigest::md5(), data)?;
/// assert_eq!(&*res, spec);
/// # Ok(()) }
/// ```
pub fn hash(t: MessageDigest, data: &[u8]) -> Result<DigestBytes, ErrorStack> {
    let mut h = Hasher::new(t)?;
    h.update(data)?;
    h.finish()
}

/// Computes the hash of the `data` with the XOF hasher `t` and stores it in `buf`.
///
/// # Examples
///
/// ```
/// use openssl::hash::{hash_xof, MessageDigest};
///
/// let data = b"\x41\x6c\x6c\x20\x79\x6f\x75\x72\x20\x62\x61\x73\x65\x20\x61\x72\x65\x20\x62\x65\x6c\x6f\x6e\x67\x20\x74\x6f\x20\x75\x73";
/// let spec = b"\x49\xd0\x69\x7f\xf5\x08\x11\x1d\x8b\x84\xf1\x5e\x46\xda\xf1\x35";
/// let mut buf = vec![0; 16];
/// hash_xof(MessageDigest::shake_128(), data, buf.as_mut_slice()).unwrap();
/// assert_eq!(buf, spec);
/// ```
///
#[cfg(any(ossl111, awslc))]
pub fn hash_xof(t: MessageDigest, data: &[u8], buf: &mut [u8]) -> Result<(), ErrorStack> {
    let mut h = Hasher::new(t)?;
    h.update(data)?;
    h.finish_xof(buf)
}

#[cfg(test)]
mod tests {
    use hex::{self, FromHex};
    use std::io::prelude::*;

    use super::*;

    fn hash_test(hashtype: MessageDigest, hashtest: &(&str, &str)) {
        let res = hash(hashtype, &Vec::from_hex(hashtest.0).unwrap()).unwrap();
        assert_eq!(hex::encode(res), hashtest.1);
    }

    #[cfg(any(ossl111, awslc))]
    fn hash_xof_test(hashtype: MessageDigest, hashtest: &(&str, &str)) {
        let expected = Vec::from_hex(hashtest.1).unwrap();
        let mut buf = vec![0; expected.len()];
        hash_xof(
            hashtype,
            &Vec::from_hex(hashtest.0).unwrap(),
            buf.as_mut_slice(),
        )
        .unwrap();
        assert_eq!(buf, expected);
    }

    /// Squeezes the expected length by doing two squeezes.
    #[cfg(ossl330)]
    fn hash_xof_squeeze_test(hashtype: MessageDigest, hashtest: &(&str, &str)) {
        let data = Vec::from_hex(hashtest.0).unwrap();
        let mut h = Hasher::new(hashtype).unwrap();
        h.update(&data).unwrap();

        let expected = Vec::from_hex(hashtest.1).unwrap();
        let mut buf = vec![0; expected.len()];
        assert!(expected.len() > 10);
        h.squeeze_xof(&mut buf[..10]).unwrap();
        h.squeeze_xof(&mut buf[10..]).unwrap();
        assert_eq!(buf, expected);
    }

    fn hash_recycle_test(h: &mut Hasher, hashtest: &(&str, &str)) {
        h.write_all(&Vec::from_hex(hashtest.0).unwrap()).unwrap();
        let res = h.finish().unwrap();
        assert_eq!(hex::encode(res), hashtest.1);
    }

    // Test vectors from http://www.nsrl.nist.gov/testdata/
    const MD5_TESTS: [(&str, &str); 13] = [
        ("", "d41d8cd98f00b204e9800998ecf8427e"),
        ("7F", "83acb6e67e50e31db6ed341dd2de1595"),
        ("EC9C", "0b07f0d4ca797d8ac58874f887cb0b68"),
        ("FEE57A", "e0d583171eb06d56198fc0ef22173907"),
        ("42F497E0", "7c430f178aefdf1487fee7144e9641e2"),
        ("C53B777F1C", "75ef141d64cb37ec423da2d9d440c925"),
        ("89D5B576327B", "ebbaf15eb0ed784c6faa9dc32831bf33"),
        ("5D4CCE781EB190", "ce175c4b08172019f05e6b5279889f2c"),
        ("81901FE94932D7B9", "cd4d2f62b8cdb3a0cf968a735a239281"),
        ("C9FFDEE7788EFB4EC9", "e0841a231ab698db30c6c0f3f246c014"),
        ("66AC4B7EBA95E53DC10B", "a3b3cea71910d9af56742aa0bb2fe329"),
        ("A510CD18F7A56852EB0319", "577e216843dd11573574d3fb209b97d8"),
        (
            "AAED18DBE8938C19ED734A8D",
            "6f80fb775f27e0a4ce5c2f42fc72c5f1",
        ),
    ];

    #[test]
    fn test_md5() {
        for test in MD5_TESTS.iter() {
            hash_test(MessageDigest::md5(), test);
        }

        assert_eq!(MessageDigest::md5().block_size(), 64);
        assert_eq!(MessageDigest::md5().size(), 16);
        assert_eq!(MessageDigest::md5().type_().as_raw(), Nid::MD5.as_raw());
    }

    #[test]
    fn test_md5_recycle() {
        let mut h = Hasher::new(MessageDigest::md5()).unwrap();
        for test in MD5_TESTS.iter() {
            hash_recycle_test(&mut h, test);
        }
    }

    #[test]
    fn test_finish_twice() {
        let mut h = Hasher::new(MessageDigest::md5()).unwrap();
        h.write_all(&Vec::from_hex(MD5_TESTS[6].0).unwrap())
            .unwrap();
        h.finish().unwrap();
        let res = h.finish().unwrap();
        let null = hash(MessageDigest::md5(), &[]).unwrap();
        assert_eq!(&*res, &*null);
    }

    #[cfg(ossl330)]
    #[test]
    fn test_finish_then_squeeze() {
        let digest = MessageDigest::shake_128();
        let mut h = Hasher::new(digest).unwrap();
        let mut buf = vec![0; digest.size()];
        h.finish_xof(&mut buf).unwrap();
        h.squeeze_xof(&mut buf)
            .expect_err("squeezing after finalize should fail");
    }

    #[cfg(ossl330)]
    #[test]
    fn test_squeeze_then_update() {
        let digest = MessageDigest::shake_128();
        let data = Vec::from_hex(MD5_TESTS[6].0).unwrap();
        let mut h = Hasher::new(digest).unwrap();
        let mut buf = vec![0; digest.size()];
        h.squeeze_xof(&mut buf).unwrap();
        h.update(&data)
            .expect_err("updating after squeeze should fail");
    }

    #[cfg(ossl330)]
    #[test]
    fn test_squeeze_then_finalize() {
        let digest = MessageDigest::shake_128();
        let mut h = Hasher::new(digest).unwrap();
        let mut buf = vec![0; digest.size()];
        h.squeeze_xof(&mut buf).unwrap();
        h.finish_xof(&mut buf)
            .expect_err("finalize after squeeze should fail");
    }

    #[test]
    #[allow(clippy::redundant_clone)]
    fn test_clone() {
        let i = 7;
        let inp = Vec::from_hex(MD5_TESTS[i].0).unwrap();
        assert!(inp.len() > 2);
        let p = inp.len() / 2;
        let h0 = Hasher::new(MessageDigest::md5()).unwrap();

        println!("Clone a new hasher");
        let mut h1 = h0.clone();
        h1.write_all(&inp[..p]).unwrap();
        {
            println!("Clone an updated hasher");
            let mut h2 = h1.clone();
            h2.write_all(&inp[p..]).unwrap();
            let res = h2.finish().unwrap();
            assert_eq!(hex::encode(res), MD5_TESTS[i].1);
        }
        h1.write_all(&inp[p..]).unwrap();
        let res = h1.finish().unwrap();
        assert_eq!(hex::encode(res), MD5_TESTS[i].1);

        println!("Clone a finished hasher");
        let mut h3 = h1.clone();
        h3.write_all(&Vec::from_hex(MD5_TESTS[i + 1].0).unwrap())
            .unwrap();
        let res = h3.finish().unwrap();
        assert_eq!(hex::encode(res), MD5_TESTS[i + 1].1);
    }

    #[test]
    fn test_sha1() {
        let tests = [("616263", "a9993e364706816aba3e25717850c26c9cd0d89d")];

        for test in tests.iter() {
            hash_test(MessageDigest::sha1(), test);
        }

        assert_eq!(MessageDigest::sha1().block_size(), 64);
        assert_eq!(MessageDigest::sha1().size(), 20);
        assert_eq!(MessageDigest::sha1().type_().as_raw(), Nid::SHA1.as_raw());
    }

    #[test]
    fn test_sha256() {
        let tests = [(
            "616263",
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        )];

        for test in tests.iter() {
            hash_test(MessageDigest::sha256(), test);
        }

        assert_eq!(MessageDigest::sha256().block_size(), 64);
        assert_eq!(MessageDigest::sha256().size(), 32);
        assert_eq!(
            MessageDigest::sha256().type_().as_raw(),
            Nid::SHA256.as_raw()
        );
    }

    #[test]
    fn test_sha512() {
        let tests = [(
            "737465766566696e647365766572797468696e67",
            "ba61d1f1af0f2dd80729f6cc900f19c0966bd38ba5c75e4471ef11b771dfe7551afab7fcbd300fdc4418f2\
            b07a028fcd99e7b6446a566f2d9bcd7c604a1ea801",
        )];

        for test in tests.iter() {
            hash_test(MessageDigest::sha512(), test);
        }

        assert_eq!(MessageDigest::sha512().block_size(), 128);
        assert_eq!(MessageDigest::sha512().size(), 64);
        assert_eq!(
            MessageDigest::sha512().type_().as_raw(),
            Nid::SHA512.as_raw()
        );
    }

    #[cfg(any(ossl111, libressl380, awslc))]
    #[test]
    fn test_sha3_224() {
        let tests = [(
            "416c6c20796f75722062617365206172652062656c6f6e6720746f207573",
            "1de092dd9fbcbbf450f26264f4778abd48af851f2832924554c56913",
        )];

        for test in tests.iter() {
            hash_test(MessageDigest::sha3_224(), test);
        }

        assert_eq!(MessageDigest::sha3_224().block_size(), 144);
        assert_eq!(MessageDigest::sha3_224().size(), 28);
        assert_eq!(
            MessageDigest::sha3_224().type_().as_raw(),
            Nid::SHA3_224.as_raw()
        );
    }

    #[cfg(any(ossl111, libressl380, awslc))]
    #[test]
    fn test_sha3_256() {
        let tests = [(
            "416c6c20796f75722062617365206172652062656c6f6e6720746f207573",
            "b38e38f08bc1c0091ed4b5f060fe13e86aa4179578513ad11a6e3abba0062f61",
        )];

        for test in tests.iter() {
            hash_test(MessageDigest::sha3_256(), test);
        }

        assert_eq!(MessageDigest::sha3_256().block_size(), 136);
        assert_eq!(MessageDigest::sha3_256().size(), 32);
        assert_eq!(
            MessageDigest::sha3_256().type_().as_raw(),
            Nid::SHA3_256.as_raw()
        );
    }

    #[cfg(any(ossl111, libressl380, awslc))]
    #[test]
    fn test_sha3_384() {
        let tests = [("416c6c20796f75722062617365206172652062656c6f6e6720746f207573",
            "966ee786ab3482dd811bf7c8fa8db79aa1f52f6c3c369942ef14240ebd857c6ff626ec35d9e131ff64d328\
            ef2008ff16"
        )];

        for test in tests.iter() {
            hash_test(MessageDigest::sha3_384(), test);
        }

        assert_eq!(MessageDigest::sha3_384().block_size(), 104);
        assert_eq!(MessageDigest::sha3_384().size(), 48);
        assert_eq!(
            MessageDigest::sha3_384().type_().as_raw(),
            Nid::SHA3_384.as_raw()
        );
    }

    #[cfg(any(ossl111, libressl380, awslc))]
    #[test]
    fn test_sha3_512() {
        let tests = [("416c6c20796f75722062617365206172652062656c6f6e6720746f207573",
            "c072288ef728cd53a029c47687960b9225893532f42b923156e37020bdc1eda753aafbf30af859d4f4c3a1\
            807caee3a79f8eb02dcd61589fbbdf5f40c8787a72"
        )];

        for test in tests.iter() {
            hash_test(MessageDigest::sha3_512(), test);
        }

        assert_eq!(MessageDigest::sha3_512().block_size(), 72);
        assert_eq!(MessageDigest::sha3_512().size(), 64);
        assert_eq!(
            MessageDigest::sha3_512().type_().as_raw(),
            Nid::SHA3_512.as_raw()
        );
    }

    #[cfg(any(ossl111, awslc))]
    #[test]
    fn test_shake_128() {
        let tests = [(
            "416c6c20796f75722062617365206172652062656c6f6e6720746f207573",
            "49d0697ff508111d8b84f15e46daf135",
        )];

        for test in tests.iter() {
            hash_xof_test(MessageDigest::shake_128(), test);
            #[cfg(ossl330)]
            hash_xof_squeeze_test(MessageDigest::shake_128(), test);
        }

        assert_eq!(MessageDigest::shake_128().block_size(), 168);
        #[cfg(ossl111)]
        assert_eq!(MessageDigest::shake_128().size(), 16);
        #[cfg(awslc)]
        assert_eq!(MessageDigest::shake_128().size(), 0);
        assert_eq!(
            MessageDigest::shake_128().type_().as_raw(),
            Nid::SHAKE128.as_raw()
        );
    }

    #[cfg(any(ossl111, awslc))]
    #[test]
    fn test_shake_256() {
        let tests = [(
            "416c6c20796f75722062617365206172652062656c6f6e6720746f207573",
            "4e2dfdaa75d1e049d0eaeffe28e76b17cea47b650fb8826fe48b94664326a697",
        )];

        for test in tests.iter() {
            hash_xof_test(MessageDigest::shake_256(), test);
            #[cfg(ossl330)]
            hash_xof_squeeze_test(MessageDigest::shake_256(), test);
        }

        assert_eq!(MessageDigest::shake_256().block_size(), 136);
        #[cfg(ossl111)]
        assert_eq!(MessageDigest::shake_256().size(), 32);
        #[cfg(awslc)]
        assert_eq!(MessageDigest::shake_256().size(), 0);
        assert_eq!(
            MessageDigest::shake_256().type_().as_raw(),
            Nid::SHAKE256.as_raw()
        );
    }

    #[test]
    #[cfg(not(osslconf = "OPENSSL_NO_RMD160"))]
    #[cfg_attr(ossl300, ignore)]
    fn test_ripemd160() {
        #[cfg(ossl300)]
        let _provider = crate::provider::Provider::try_load(None, "legacy", true).unwrap();

        let tests = [("616263", "8eb208f7e05d987a9b044a8e98c6b087f15a0bfc")];

        for test in tests.iter() {
            hash_test(MessageDigest::ripemd160(), test);
        }

        assert_eq!(MessageDigest::ripemd160().block_size(), 64);
        assert_eq!(MessageDigest::ripemd160().size(), 20);
        assert_eq!(
            MessageDigest::ripemd160().type_().as_raw(),
            Nid::RIPEMD160.as_raw()
        );
    }

    #[cfg(all(any(ossl111, libressl291), not(osslconf = "OPENSSL_NO_SM3")))]
    #[test]
    fn test_sm3() {
        let tests = [(
            "616263",
            "66c7f0f462eeedd9d1f2d46bdc10e4e24167c4875cf2f7a2297da02b8f4ba8e0",
        )];

        for test in tests.iter() {
            hash_test(MessageDigest::sm3(), test);
        }

        assert_eq!(MessageDigest::sm3().block_size(), 64);
        assert_eq!(MessageDigest::sm3().size(), 32);
        assert_eq!(MessageDigest::sm3().type_().as_raw(), Nid::SM3.as_raw());
    }

    #[test]
    fn from_nid() {
        assert_eq!(
            MessageDigest::from_nid(Nid::SHA256).unwrap().as_ptr(),
            MessageDigest::sha256().as_ptr()
        );
    }

    #[test]
    fn from_name() {
        assert_eq!(
            MessageDigest::from_name("SHA256").unwrap().as_ptr(),
            MessageDigest::sha256().as_ptr()
        )
    }
}
