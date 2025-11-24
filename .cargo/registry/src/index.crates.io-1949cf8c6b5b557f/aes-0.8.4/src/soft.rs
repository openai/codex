//! AES block cipher constant-time implementation.
//!
//! The implementation uses a technique called [fixslicing][1], an improved
//! form of bitslicing which represents ciphers in a way which enables
//! very efficient constant-time implementations in software.
//!
//! [1]: https://eprint.iacr.org/2020/1123.pdf

#![deny(unsafe_code)]

#[cfg_attr(not(target_pointer_width = "64"), path = "soft/fixslice32.rs")]
#[cfg_attr(target_pointer_width = "64", path = "soft/fixslice64.rs")]
pub(crate) mod fixslice;

use crate::Block;
use cipher::{
    consts::{U16, U24, U32},
    inout::InOut,
    AlgorithmName, BlockBackend, BlockCipher, BlockClosure, BlockDecrypt, BlockEncrypt,
    BlockSizeUser, Key, KeyInit, KeySizeUser, ParBlocksSizeUser,
};
use core::fmt;
use fixslice::{BatchBlocks, FixsliceBlocks, FixsliceKeys128, FixsliceKeys192, FixsliceKeys256};

macro_rules! define_aes_impl {
    (
        $name:tt,
        $name_enc:ident,
        $name_dec:ident,
        $name_back_enc:ident,
        $name_back_dec:ident,
        $key_size:ty,
        $fixslice_keys:ty,
        $fixslice_key_schedule:path,
        $fixslice_decrypt:path,
        $fixslice_encrypt:path,
        $doc:expr $(,)?
    ) => {
        #[doc=$doc]
        #[doc = "block cipher"]
        #[derive(Clone)]
        pub struct $name {
            keys: $fixslice_keys,
        }

        impl $name {
            #[inline(always)]
            pub(crate) fn get_enc_backend(&self) -> $name_back_enc<'_> {
                $name_back_enc(self)
            }

            #[inline(always)]
            pub(crate) fn get_dec_backend(&self) -> $name_back_dec<'_> {
                $name_back_dec(self)
            }
        }

        impl KeySizeUser for $name {
            type KeySize = $key_size;
        }

        impl KeyInit for $name {
            #[inline]
            fn new(key: &Key<Self>) -> Self {
                Self {
                    keys: $fixslice_key_schedule(key.as_ref()),
                }
            }
        }

        impl BlockSizeUser for $name {
            type BlockSize = U16;
        }

        impl BlockCipher for $name {}

        impl BlockEncrypt for $name {
            fn encrypt_with_backend(&self, f: impl BlockClosure<BlockSize = U16>) {
                f.call(&mut self.get_enc_backend())
            }
        }

        impl BlockDecrypt for $name {
            fn decrypt_with_backend(&self, f: impl BlockClosure<BlockSize = U16>) {
                f.call(&mut self.get_dec_backend())
            }
        }

        impl From<$name_enc> for $name {
            #[inline]
            fn from(enc: $name_enc) -> $name {
                enc.inner
            }
        }

        impl From<&$name_enc> for $name {
            #[inline]
            fn from(enc: &$name_enc) -> $name {
                enc.inner.clone()
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
                #[cfg(feature = "zeroize")]
                zeroize::Zeroize::zeroize(&mut self.keys);
            }
        }

        #[cfg(feature = "zeroize")]
        impl zeroize::ZeroizeOnDrop for $name {}

        #[doc=$doc]
        #[doc = "block cipher (encrypt-only)"]
        #[derive(Clone)]
        pub struct $name_enc {
            inner: $name,
        }

        impl $name_enc {
            #[inline(always)]
            pub(crate) fn get_enc_backend(&self) -> $name_back_enc<'_> {
                self.inner.get_enc_backend()
            }
        }

        impl BlockCipher for $name_enc {}

        impl KeySizeUser for $name_enc {
            type KeySize = $key_size;
        }

        impl KeyInit for $name_enc {
            #[inline(always)]
            fn new(key: &Key<Self>) -> Self {
                let inner = $name::new(key);
                Self { inner }
            }
        }

        impl BlockSizeUser for $name_enc {
            type BlockSize = U16;
        }

        impl BlockEncrypt for $name_enc {
            fn encrypt_with_backend(&self, f: impl BlockClosure<BlockSize = U16>) {
                f.call(&mut self.get_enc_backend())
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

        #[cfg(feature = "zeroize")]
        impl zeroize::ZeroizeOnDrop for $name_enc {}

        #[doc=$doc]
        #[doc = "block cipher (decrypt-only)"]
        #[derive(Clone)]
        pub struct $name_dec {
            inner: $name,
        }

        impl $name_dec {
            #[inline(always)]
            pub(crate) fn get_dec_backend(&self) -> $name_back_dec<'_> {
                self.inner.get_dec_backend()
            }
        }

        impl BlockCipher for $name_dec {}

        impl KeySizeUser for $name_dec {
            type KeySize = $key_size;
        }

        impl KeyInit for $name_dec {
            #[inline(always)]
            fn new(key: &Key<Self>) -> Self {
                let inner = $name::new(key);
                Self { inner }
            }
        }

        impl From<$name_enc> for $name_dec {
            #[inline]
            fn from(enc: $name_enc) -> $name_dec {
                Self { inner: enc.inner }
            }
        }

        impl From<&$name_enc> for $name_dec {
            #[inline]
            fn from(enc: &$name_enc) -> $name_dec {
                Self {
                    inner: enc.inner.clone(),
                }
            }
        }

        impl BlockSizeUser for $name_dec {
            type BlockSize = U16;
        }

        impl BlockDecrypt for $name_dec {
            fn decrypt_with_backend(&self, f: impl BlockClosure<BlockSize = U16>) {
                f.call(&mut self.get_dec_backend());
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

        #[cfg(feature = "zeroize")]
        impl zeroize::ZeroizeOnDrop for $name_dec {}

        pub(crate) struct $name_back_enc<'a>(&'a $name);

        impl<'a> BlockSizeUser for $name_back_enc<'a> {
            type BlockSize = U16;
        }

        impl<'a> ParBlocksSizeUser for $name_back_enc<'a> {
            type ParBlocksSize = FixsliceBlocks;
        }

        impl<'a> BlockBackend for $name_back_enc<'a> {
            #[inline(always)]
            fn proc_block(&mut self, mut block: InOut<'_, '_, Block>) {
                let mut blocks = BatchBlocks::default();
                blocks[0] = block.clone_in().into();
                let res = $fixslice_encrypt(&self.0.keys, &blocks);
                *block.get_out() = res[0].into();
            }

            #[inline(always)]
            fn proc_par_blocks(&mut self, mut blocks: InOut<'_, '_, BatchBlocks>) {
                let res = $fixslice_encrypt(&self.0.keys, blocks.get_in());
                *blocks.get_out() = res;
            }
        }

        pub(crate) struct $name_back_dec<'a>(&'a $name);

        impl<'a> BlockSizeUser for $name_back_dec<'a> {
            type BlockSize = U16;
        }

        impl<'a> ParBlocksSizeUser for $name_back_dec<'a> {
            type ParBlocksSize = FixsliceBlocks;
        }

        impl<'a> BlockBackend for $name_back_dec<'a> {
            #[inline(always)]
            fn proc_block(&mut self, mut block: InOut<'_, '_, Block>) {
                let mut blocks = BatchBlocks::default();
                blocks[0] = block.clone_in();
                let res = $fixslice_decrypt(&self.0.keys, &blocks);
                *block.get_out() = res[0];
            }

            #[inline(always)]
            fn proc_par_blocks(&mut self, mut blocks: InOut<'_, '_, BatchBlocks>) {
                let res = $fixslice_decrypt(&self.0.keys, blocks.get_in());
                *blocks.get_out() = res;
            }
        }
    };
}

define_aes_impl!(
    Aes128,
    Aes128Enc,
    Aes128Dec,
    Aes128BackEnc,
    Aes128BackDec,
    U16,
    FixsliceKeys128,
    fixslice::aes128_key_schedule,
    fixslice::aes128_decrypt,
    fixslice::aes128_encrypt,
    "AES-128",
);

define_aes_impl!(
    Aes192,
    Aes192Enc,
    Aes192Dec,
    Aes192BackEnc,
    Aes192BackDec,
    U24,
    FixsliceKeys192,
    fixslice::aes192_key_schedule,
    fixslice::aes192_decrypt,
    fixslice::aes192_encrypt,
    "AES-192",
);

define_aes_impl!(
    Aes256,
    Aes256Enc,
    Aes256Dec,
    Aes256BackEnc,
    Aes256BackDec,
    U32,
    FixsliceKeys256,
    fixslice::aes256_key_schedule,
    fixslice::aes256_decrypt,
    fixslice::aes256_encrypt,
    "AES-256",
);
