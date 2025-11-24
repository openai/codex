//! The SHA family of hashes.
//!
//! SHA, or Secure Hash Algorithms, are a family of cryptographic hashing algorithms published by
//! the National Institute of Standards and Technology (NIST).  Hash algorithms such as those in
//! the SHA family are used to map data of an arbitrary size to a fixed-size string of bytes.
//! As cryptographic hashing algorithms, these mappings have the property of being irreversible.
//! This property makes hash algorithms like these excellent for uses such as verifying the
//! contents of a file- if you know the hash you expect beforehand, then you can verify that the
//! data you have is correct if it hashes to the same value.
//!
//! # Examples
//!
//! When dealing with data that becomes available in chunks, such as while buffering data from IO,
//! you can create a hasher that you can repeatedly update to add bytes to.
//!
//! ```rust
//! use openssl::sha;
//!
//! let mut hasher = sha::Sha256::new();
//!
//! hasher.update(b"Hello, ");
//! hasher.update(b"world");
//!
//! let hash = hasher.finish();
//! println!("Hashed \"Hello, world\" to {}", hex::encode(hash));
//! ```
//!
//! On the other hand, if you already have access to all of the data you would like to hash, you
//! may prefer to use the slightly simpler method of simply calling the hash function corresponding
//! to the algorithm you want to use.
//!
//! ```rust
//! use openssl::sha::sha256;
//!
//! let hash = sha256(b"your data or message");
//! println!("Hash = {}", hex::encode(hash));
//! ```
use cfg_if::cfg_if;
use libc::c_void;
use openssl_macros::corresponds;
use std::mem::MaybeUninit;

/// Computes the SHA1 hash of some data.
///
/// # Warning
///
/// SHA1 is known to be insecure - it should not be used unless required for
/// compatibility with existing systems.
#[corresponds(SHA1)]
#[inline]
pub fn sha1(data: &[u8]) -> [u8; 20] {
    unsafe {
        let mut hash = MaybeUninit::<[u8; 20]>::uninit();
        ffi::SHA1(data.as_ptr(), data.len(), hash.as_mut_ptr() as *mut _);
        hash.assume_init()
    }
}

/// Computes the SHA224 hash of some data.
#[corresponds(SHA224)]
#[inline]
pub fn sha224(data: &[u8]) -> [u8; 28] {
    unsafe {
        let mut hash = MaybeUninit::<[u8; 28]>::uninit();
        ffi::SHA224(data.as_ptr(), data.len(), hash.as_mut_ptr() as *mut _);
        hash.assume_init()
    }
}

/// Computes the SHA256 hash of some data.
#[corresponds(SHA256)]
#[inline]
pub fn sha256(data: &[u8]) -> [u8; 32] {
    unsafe {
        let mut hash = MaybeUninit::<[u8; 32]>::uninit();
        ffi::SHA256(data.as_ptr(), data.len(), hash.as_mut_ptr() as *mut _);
        hash.assume_init()
    }
}

/// Computes the SHA384 hash of some data.
#[corresponds(SHA384)]
#[inline]
pub fn sha384(data: &[u8]) -> [u8; 48] {
    unsafe {
        let mut hash = MaybeUninit::<[u8; 48]>::uninit();
        ffi::SHA384(data.as_ptr(), data.len(), hash.as_mut_ptr() as *mut _);
        hash.assume_init()
    }
}

/// Computes the SHA512 hash of some data.
#[corresponds(SHA512)]
#[inline]
pub fn sha512(data: &[u8]) -> [u8; 64] {
    unsafe {
        let mut hash = MaybeUninit::<[u8; 64]>::uninit();
        ffi::SHA512(data.as_ptr(), data.len(), hash.as_mut_ptr() as *mut _);
        hash.assume_init()
    }
}

cfg_if! {
    if #[cfg(not(osslconf = "OPENSSL_NO_DEPRECATED_3_0"))] {
        /// An object which calculates a SHA1 hash of some data.
        ///
        /// # Warning
        ///
        /// SHA1 is known to be insecure - it should not be used unless required for
        /// compatibility with existing systems.
        #[derive(Clone)]
        pub struct Sha1(ffi::SHA_CTX);

        impl Default for Sha1 {
            #[inline]
            fn default() -> Sha1 {
                Sha1::new()
            }
        }

        impl Sha1 {
            /// Creates a new hasher.
            #[corresponds(SHA1_Init)]
            #[inline]
            pub fn new() -> Sha1 {
                unsafe {
                    let mut ctx = MaybeUninit::uninit();
                    ffi::SHA1_Init( ctx.as_mut_ptr());
                    Sha1(ctx.assume_init())
                }
            }

            /// Feeds some data into the hasher.
            ///
            /// This can be called multiple times.
            #[corresponds(SHA1_Update)]
            #[inline]
            pub fn update(&mut self, buf: &[u8]) {
                unsafe {
                    ffi::SHA1_Update(&mut self.0, buf.as_ptr() as *const c_void, buf.len());
                }
            }

            /// Returns the hash of the data.
            #[corresponds(SHA1_Final)]
            #[inline]
            pub fn finish(mut self) -> [u8; 20] {
                unsafe {
                    let mut hash = MaybeUninit::<[u8; 20]>::uninit();
                    ffi::SHA1_Final(hash.as_mut_ptr() as *mut _, &mut self.0);
                    hash.assume_init()
                }
            }
        }

        /// An object which calculates a SHA224 hash of some data.
        #[derive(Clone)]
        pub struct Sha224(ffi::SHA256_CTX);

        impl Default for Sha224 {
            #[inline]
            fn default() -> Sha224 {
                Sha224::new()
            }
        }

        impl Sha224 {
            /// Creates a new hasher.
            #[corresponds(SHA224_Init)]
            #[inline]
            pub fn new() -> Sha224 {
                unsafe {
                    let mut ctx = MaybeUninit::uninit();
                    ffi::SHA224_Init(ctx.as_mut_ptr());
                    Sha224(ctx.assume_init())
                }
            }

            /// Feeds some data into the hasher.
            ///
            /// This can be called multiple times.
            #[corresponds(SHA224_Update)]
            #[inline]
            pub fn update(&mut self, buf: &[u8]) {
                unsafe {
                    ffi::SHA224_Update(&mut self.0, buf.as_ptr() as *const c_void, buf.len());
                }
            }

            /// Returns the hash of the data.
            #[corresponds(SHA224_Final)]
            #[inline]
            pub fn finish(mut self) -> [u8; 28] {
                unsafe {
                    let mut hash = MaybeUninit::<[u8; 28]>::uninit();
                    ffi::SHA224_Final(hash.as_mut_ptr() as *mut _, &mut self.0);
                    hash.assume_init()
                }
            }
        }

        /// An object which calculates a SHA256 hash of some data.
        #[derive(Clone)]
        pub struct Sha256(ffi::SHA256_CTX);

        impl Default for Sha256 {
            #[inline]
            fn default() -> Sha256 {
                Sha256::new()
            }
        }

        impl Sha256 {
            /// Creates a new hasher.
            #[corresponds(SHA256_Init)]
            #[inline]
            pub fn new() -> Sha256 {
                unsafe {
                    let mut ctx = MaybeUninit::uninit();
                    ffi::SHA256_Init(ctx.as_mut_ptr());
                    Sha256(ctx.assume_init())
                }
            }

            /// Feeds some data into the hasher.
            ///
            /// This can be called multiple times.
            #[corresponds(SHA256_Update)]
            #[inline]
            pub fn update(&mut self, buf: &[u8]) {
                unsafe {
                    ffi::SHA256_Update(&mut self.0, buf.as_ptr() as *const c_void, buf.len());
                }
            }

            /// Returns the hash of the data.
            #[corresponds(SHA256_Final)]
            #[inline]
            pub fn finish(mut self) -> [u8; 32] {
                unsafe {
                    let mut hash = MaybeUninit::<[u8; 32]>::uninit();
                    ffi::SHA256_Final(hash.as_mut_ptr() as *mut _, &mut self.0);
                    hash.assume_init()
                }
            }
        }

        /// An object which calculates a SHA384 hash of some data.
        #[derive(Clone)]
        pub struct Sha384(ffi::SHA512_CTX);

        impl Default for Sha384 {
            #[inline]
            fn default() -> Sha384 {
                Sha384::new()
            }
        }

        impl Sha384 {
            /// Creates a new hasher.
            #[corresponds(SHA384_Init)]
            #[inline]
            pub fn new() -> Sha384 {
                unsafe {
                    let mut ctx = MaybeUninit::uninit();
                    ffi::SHA384_Init(ctx.as_mut_ptr());
                    Sha384(ctx.assume_init())
                }
            }

            /// Feeds some data into the hasher.
            ///
            /// This can be called multiple times.
            #[corresponds(SHA384_Update)]
            #[inline]
            pub fn update(&mut self, buf: &[u8]) {
                unsafe {
                    ffi::SHA384_Update(&mut self.0, buf.as_ptr() as *const c_void, buf.len());
                }
            }

            /// Returns the hash of the data.
            #[corresponds(SHA384_Final)]
            #[inline]
            pub fn finish(mut self) -> [u8; 48] {
                unsafe {
                    let mut hash = MaybeUninit::<[u8; 48]>::uninit();
                    ffi::SHA384_Final(hash.as_mut_ptr() as *mut _, &mut self.0);
                    hash.assume_init()
                }
            }
        }

        /// An object which calculates a SHA512 hash of some data.
        #[derive(Clone)]
        pub struct Sha512(ffi::SHA512_CTX);

        impl Default for Sha512 {
            #[inline]
            fn default() -> Sha512 {
                Sha512::new()
            }
        }

        impl Sha512 {
            /// Creates a new hasher.
            #[corresponds(SHA512_Init)]
            #[inline]
            pub fn new() -> Sha512 {
                unsafe {
                    let mut ctx = MaybeUninit::uninit();
                    ffi::SHA512_Init(ctx.as_mut_ptr());
                    Sha512(ctx.assume_init())
                }
            }

            /// Feeds some data into the hasher.
            ///
            /// This can be called multiple times.
            #[corresponds(SHA512_Update)]
            #[inline]
            pub fn update(&mut self, buf: &[u8]) {
                unsafe {
                    ffi::SHA512_Update(&mut self.0, buf.as_ptr() as *const c_void, buf.len());
                }
            }

            /// Returns the hash of the data.
            #[corresponds(SHA512_Final)]
            #[inline]
            pub fn finish(mut self) -> [u8; 64] {
                unsafe {
                    let mut hash= MaybeUninit::<[u8; 64]>::uninit();
                ffi::SHA512_Final(hash.as_mut_ptr() as *mut _, &mut self.0);
                hash.assume_init()
            }
        }
    }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn standalone_1() {
        let data = b"abc";
        let expected = "a9993e364706816aba3e25717850c26c9cd0d89d";

        assert_eq!(hex::encode(sha1(data)), expected);
    }

    #[test]
    #[cfg(not(osslconf = "OPENSSL_NO_DEPRECATED_3_0"))]
    fn struct_1() {
        let expected = "a9993e364706816aba3e25717850c26c9cd0d89d";

        let mut hasher = Sha1::new();
        hasher.update(b"a");
        hasher.update(b"bc");
        assert_eq!(hex::encode(hasher.finish()), expected);
    }

    #[test]
    #[cfg(not(osslconf = "OPENSSL_NO_DEPRECATED_3_0"))]
    fn cloning_allows_incremental_hashing() {
        let expected = "a9993e364706816aba3e25717850c26c9cd0d89d";

        let mut hasher = Sha1::new();
        hasher.update(b"a");

        let mut incr_hasher = hasher.clone();
        incr_hasher.update(b"bc");

        assert_eq!(hex::encode(incr_hasher.finish()), expected);
        assert_ne!(hex::encode(hasher.finish()), expected);
    }

    #[test]
    fn standalone_224() {
        let data = b"abc";
        let expected = "23097d223405d8228642a477bda255b32aadbce4bda0b3f7e36c9da7";

        assert_eq!(hex::encode(sha224(data)), expected);
    }

    #[test]
    #[cfg(not(osslconf = "OPENSSL_NO_DEPRECATED_3_0"))]
    fn struct_224() {
        let expected = "23097d223405d8228642a477bda255b32aadbce4bda0b3f7e36c9da7";

        let mut hasher = Sha224::new();
        hasher.update(b"a");
        hasher.update(b"bc");
        assert_eq!(hex::encode(hasher.finish()), expected);
    }

    #[test]
    fn standalone_256() {
        let data = b"abc";
        let expected = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

        assert_eq!(hex::encode(sha256(data)), expected);
    }

    #[test]
    #[cfg(not(osslconf = "OPENSSL_NO_DEPRECATED_3_0"))]
    fn struct_256() {
        let expected = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

        let mut hasher = Sha256::new();
        hasher.update(b"a");
        hasher.update(b"bc");
        assert_eq!(hex::encode(hasher.finish()), expected);
    }

    #[test]
    fn standalone_384() {
        let data = b"abc";
        let expected =
            "cb00753f45a35e8bb5a03d699ac65007272c32ab0eded1631a8b605a43ff5bed8086072ba1e\
             7cc2358baeca134c825a7";

        assert_eq!(hex::encode(&sha384(data)[..]), expected);
    }

    #[test]
    #[cfg(not(osslconf = "OPENSSL_NO_DEPRECATED_3_0"))]
    fn struct_384() {
        let expected =
            "cb00753f45a35e8bb5a03d699ac65007272c32ab0eded1631a8b605a43ff5bed8086072ba1e\
             7cc2358baeca134c825a7";

        let mut hasher = Sha384::new();
        hasher.update(b"a");
        hasher.update(b"bc");
        assert_eq!(hex::encode(&hasher.finish()[..]), expected);
    }

    #[test]
    fn standalone_512() {
        let data = b"abc";
        let expected =
            "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274\
             fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f";

        assert_eq!(hex::encode(&sha512(data)[..]), expected);
    }

    #[test]
    #[cfg(not(osslconf = "OPENSSL_NO_DEPRECATED_3_0"))]
    fn struct_512() {
        let expected =
            "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274\
             fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f";

        let mut hasher = Sha512::new();
        hasher.update(b"a");
        hasher.update(b"bc");
        assert_eq!(hex::encode(&hasher.finish()[..]), expected);
    }
}
