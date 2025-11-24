//! Autodetection support for hardware accelerated AES backends with fallback
//! to the fixsliced "soft" implementation.

use crate::soft;
use cipher::{
    consts::{U16, U24, U32},
    AlgorithmName, BlockCipher, BlockClosure, BlockDecrypt, BlockEncrypt, BlockSizeUser, Key,
    KeyInit, KeySizeUser,
};
use core::fmt;
use core::mem::ManuallyDrop;

#[cfg(all(target_arch = "aarch64", aes_armv8))]
use crate::armv8 as intrinsics;

#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
use crate::ni as intrinsics;

cpufeatures::new!(aes_intrinsics, "aes");

macro_rules! define_aes_impl {
    (
        $name:ident,
        $name_enc:ident,
        $name_dec:ident,
        $module:tt,
        $key_size:ty,
        $doc:expr $(,)?
    ) => {
        mod $module {
            use super::{intrinsics, soft};
            use core::mem::ManuallyDrop;

            pub(super) union Inner {
                pub(super) intrinsics: ManuallyDrop<intrinsics::$name>,
                pub(super) soft: ManuallyDrop<soft::$name>,
            }

            pub(super) union InnerEnc {
                pub(super) intrinsics: ManuallyDrop<intrinsics::$name_enc>,
                pub(super) soft: ManuallyDrop<soft::$name_enc>,
            }

            pub(super) union InnerDec {
                pub(super) intrinsics: ManuallyDrop<intrinsics::$name_dec>,
                pub(super) soft: ManuallyDrop<soft::$name_dec>,
            }
        }

        #[doc=$doc]
        #[doc = "block cipher"]
        pub struct $name {
            inner: $module::Inner,
            token: aes_intrinsics::InitToken,
        }

        impl KeySizeUser for $name {
            type KeySize = $key_size;
        }
        impl From<$name_enc> for $name {
            #[inline]
            fn from(enc: $name_enc) -> $name {
                Self::from(&enc)
            }
        }

        impl From<&$name_enc> for $name {
            fn from(enc: &$name_enc) -> $name {
                use core::ops::Deref;
                let inner = if enc.token.get() {
                    $module::Inner {
                        intrinsics: ManuallyDrop::new(unsafe {
                            enc.inner.intrinsics.deref().into()
                        }),
                    }
                } else {
                    $module::Inner {
                        soft: ManuallyDrop::new(unsafe { enc.inner.soft.deref().into() }),
                    }
                };

                Self {
                    inner,
                    token: enc.token,
                }
            }
        }

        impl KeyInit for $name {
            #[inline]
            fn new(key: &Key<Self>) -> Self {
                let (token, aesni_present) = aes_intrinsics::init_get();

                let inner = if aesni_present {
                    $module::Inner {
                        intrinsics: ManuallyDrop::new(intrinsics::$name::new(key)),
                    }
                } else {
                    $module::Inner {
                        soft: ManuallyDrop::new(soft::$name::new(key)),
                    }
                };

                Self { inner, token }
            }
        }

        impl Clone for $name {
            fn clone(&self) -> Self {
                let inner = if self.token.get() {
                    $module::Inner {
                        intrinsics: unsafe { self.inner.intrinsics.clone() },
                    }
                } else {
                    $module::Inner {
                        soft: unsafe { self.inner.soft.clone() },
                    }
                };

                Self {
                    inner,
                    token: self.token,
                }
            }
        }

        impl BlockSizeUser for $name {
            type BlockSize = U16;
        }

        impl BlockCipher for $name {}

        impl BlockEncrypt for $name {
            fn encrypt_with_backend(&self, f: impl BlockClosure<BlockSize = U16>) {
                unsafe {
                    if self.token.get() {
                        #[target_feature(enable = "aes")]
                        unsafe fn inner(
                            state: &intrinsics::$name,
                            f: impl BlockClosure<BlockSize = U16>,
                        ) {
                            f.call(&mut state.get_enc_backend());
                        }
                        inner(&self.inner.intrinsics, f);
                    } else {
                        f.call(&mut self.inner.soft.get_enc_backend());
                    }
                }
            }
        }

        impl BlockDecrypt for $name {
            fn decrypt_with_backend(&self, f: impl BlockClosure<BlockSize = U16>) {
                unsafe {
                    if self.token.get() {
                        #[target_feature(enable = "aes")]
                        unsafe fn inner(
                            state: &intrinsics::$name,
                            f: impl BlockClosure<BlockSize = U16>,
                        ) {
                            f.call(&mut state.get_dec_backend());
                        }
                        inner(&self.inner.intrinsics, f);
                    } else {
                        f.call(&mut self.inner.soft.get_dec_backend());
                    }
                }
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
                f.write_str(concat!(stringify!($name), " { .. }"))
            }
        }

        impl AlgorithmName for $name {
            fn write_alg_name(f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(stringify!($name))
            }
        }

        impl Drop for $name {
            #[inline]
            fn drop(&mut self) {
                if self.token.get() {
                    unsafe { ManuallyDrop::drop(&mut self.inner.intrinsics) };
                } else {
                    unsafe { ManuallyDrop::drop(&mut self.inner.soft) };
                };
            }
        }

        #[cfg(feature = "zeroize")]
        impl zeroize::ZeroizeOnDrop for $name {}

        #[doc=$doc]
        #[doc = "block cipher (encrypt-only)"]
        pub struct $name_enc {
            inner: $module::InnerEnc,
            token: aes_intrinsics::InitToken,
        }

        impl KeySizeUser for $name_enc {
            type KeySize = $key_size;
        }

        impl KeyInit for $name_enc {
            #[inline]
            fn new(key: &Key<Self>) -> Self {
                let (token, aesni_present) = aes_intrinsics::init_get();

                let inner = if aesni_present {
                    $module::InnerEnc {
                        intrinsics: ManuallyDrop::new(intrinsics::$name_enc::new(key)),
                    }
                } else {
                    $module::InnerEnc {
                        soft: ManuallyDrop::new(soft::$name_enc::new(key)),
                    }
                };

                Self { inner, token }
            }
        }

        impl Clone for $name_enc {
            fn clone(&self) -> Self {
                let inner = if self.token.get() {
                    $module::InnerEnc {
                        intrinsics: unsafe { self.inner.intrinsics.clone() },
                    }
                } else {
                    $module::InnerEnc {
                        soft: unsafe { self.inner.soft.clone() },
                    }
                };

                Self {
                    inner,
                    token: self.token,
                }
            }
        }

        impl BlockSizeUser for $name_enc {
            type BlockSize = U16;
        }

        impl BlockCipher for $name_enc {}

        impl BlockEncrypt for $name_enc {
            fn encrypt_with_backend(&self, f: impl BlockClosure<BlockSize = U16>) {
                unsafe {
                    if self.token.get() {
                        #[target_feature(enable = "aes")]
                        unsafe fn inner(
                            state: &intrinsics::$name_enc,
                            f: impl BlockClosure<BlockSize = U16>,
                        ) {
                            f.call(&mut state.get_enc_backend());
                        }
                        inner(&self.inner.intrinsics, f);
                    } else {
                        f.call(&mut self.inner.soft.get_enc_backend());
                    }
                }
            }
        }

        impl fmt::Debug for $name_enc {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
                f.write_str(concat!(stringify!($name_enc), " { .. }"))
            }
        }

        impl AlgorithmName for $name_enc {
            fn write_alg_name(f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(stringify!($name_enc))
            }
        }

        impl Drop for $name_enc {
            #[inline]
            fn drop(&mut self) {
                if self.token.get() {
                    unsafe { ManuallyDrop::drop(&mut self.inner.intrinsics) };
                } else {
                    unsafe { ManuallyDrop::drop(&mut self.inner.soft) };
                };
            }
        }

        #[cfg(feature = "zeroize")]
        impl zeroize::ZeroizeOnDrop for $name_enc {}

        #[doc=$doc]
        #[doc = "block cipher (decrypt-only)"]
        pub struct $name_dec {
            inner: $module::InnerDec,
            token: aes_intrinsics::InitToken,
        }

        impl KeySizeUser for $name_dec {
            type KeySize = $key_size;
        }

        impl From<$name_enc> for $name_dec {
            #[inline]
            fn from(enc: $name_enc) -> $name_dec {
                Self::from(&enc)
            }
        }

        impl From<&$name_enc> for $name_dec {
            fn from(enc: &$name_enc) -> $name_dec {
                use core::ops::Deref;
                let inner = if enc.token.get() {
                    $module::InnerDec {
                        intrinsics: ManuallyDrop::new(unsafe {
                            enc.inner.intrinsics.deref().into()
                        }),
                    }
                } else {
                    $module::InnerDec {
                        soft: ManuallyDrop::new(unsafe { enc.inner.soft.deref().into() }),
                    }
                };

                Self {
                    inner,
                    token: enc.token,
                }
            }
        }

        impl KeyInit for $name_dec {
            #[inline]
            fn new(key: &Key<Self>) -> Self {
                let (token, aesni_present) = aes_intrinsics::init_get();

                let inner = if aesni_present {
                    $module::InnerDec {
                        intrinsics: ManuallyDrop::new(intrinsics::$name_dec::new(key)),
                    }
                } else {
                    $module::InnerDec {
                        soft: ManuallyDrop::new(soft::$name_dec::new(key)),
                    }
                };

                Self { inner, token }
            }
        }

        impl Clone for $name_dec {
            fn clone(&self) -> Self {
                let inner = if self.token.get() {
                    $module::InnerDec {
                        intrinsics: unsafe { self.inner.intrinsics.clone() },
                    }
                } else {
                    $module::InnerDec {
                        soft: unsafe { self.inner.soft.clone() },
                    }
                };

                Self {
                    inner,
                    token: self.token,
                }
            }
        }

        impl BlockSizeUser for $name_dec {
            type BlockSize = U16;
        }

        impl BlockCipher for $name_dec {}

        impl BlockDecrypt for $name_dec {
            fn decrypt_with_backend(&self, f: impl BlockClosure<BlockSize = U16>) {
                unsafe {
                    if self.token.get() {
                        #[target_feature(enable = "aes")]
                        unsafe fn inner(
                            state: &intrinsics::$name_dec,
                            f: impl BlockClosure<BlockSize = U16>,
                        ) {
                            f.call(&mut state.get_dec_backend());
                        }
                        inner(&self.inner.intrinsics, f);
                    } else {
                        f.call(&mut self.inner.soft.get_dec_backend());
                    }
                }
            }
        }

        impl fmt::Debug for $name_dec {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
                f.write_str(concat!(stringify!($name_dec), " { .. }"))
            }
        }

        impl AlgorithmName for $name_dec {
            fn write_alg_name(f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(stringify!($name_dec))
            }
        }

        impl Drop for $name_dec {
            #[inline]
            fn drop(&mut self) {
                if self.token.get() {
                    unsafe { ManuallyDrop::drop(&mut self.inner.intrinsics) };
                } else {
                    unsafe { ManuallyDrop::drop(&mut self.inner.soft) };
                };
            }
        }

        #[cfg(feature = "zeroize")]
        impl zeroize::ZeroizeOnDrop for $name_dec {}
    };
}

define_aes_impl!(Aes128, Aes128Enc, Aes128Dec, aes128, U16, "AES-128");
define_aes_impl!(Aes192, Aes192Enc, Aes192Dec, aes192, U24, "AES-192");
define_aes_impl!(Aes256, Aes256Enc, Aes256Dec, aes256, U32, "AES-256");
