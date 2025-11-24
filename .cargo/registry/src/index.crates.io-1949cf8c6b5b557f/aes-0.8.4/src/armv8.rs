//! AES block cipher implementation using the ARMv8 Cryptography Extensions.
//!
//! Based on this C intrinsics implementation:
//! <https://github.com/noloader/AES-Intrinsics/blob/master/aes-arm.c>
//!
//! Original C written and placed in public domain by Jeffrey Walton.
//! Based on code from ARM, and by Johannes Schneiders, Skip Hovsmith and
//! Barry O'Rourke for the mbedTLS project.

#![allow(clippy::needless_range_loop)]

#[cfg(feature = "hazmat")]
pub(crate) mod hazmat;

mod encdec;
mod expand;
mod intrinsics;
#[cfg(test)]
mod test_expand;

use self::{
    encdec::{decrypt1, decrypt8, encrypt1, encrypt8},
    expand::{expand_key, inv_expanded_keys},
};
use crate::{Block, Block8};
use cipher::{
    consts::{U16, U24, U32, U8},
    inout::InOut,
    AlgorithmName, BlockBackend, BlockCipher, BlockClosure, BlockDecrypt, BlockEncrypt,
    BlockSizeUser, Key, KeyInit, KeySizeUser, ParBlocksSizeUser,
};
use core::arch::aarch64::*;
use core::fmt;

macro_rules! define_aes_impl {
    (
        $name:ident,
        $name_enc:ident,
        $name_dec:ident,
        $name_back_enc:ident,
        $name_back_dec:ident,
        $key_size:ty,
        $rounds:tt,
        $doc:expr $(,)?
    ) => {
        #[doc=$doc]
        #[doc = "block cipher"]
        #[derive(Clone)]
        pub struct $name {
            encrypt: $name_enc,
            decrypt: $name_dec,
        }

        impl $name {
            #[inline(always)]
            pub(crate) fn get_enc_backend(&self) -> $name_back_enc<'_> {
                self.encrypt.get_enc_backend()
            }

            #[inline(always)]
            pub(crate) fn get_dec_backend(&self) -> $name_back_dec<'_> {
                self.decrypt.get_dec_backend()
            }
        }

        impl BlockCipher for $name {}

        impl KeySizeUser for $name {
            type KeySize = $key_size;
        }

        impl KeyInit for $name {
            #[inline]
            fn new(key: &Key<Self>) -> Self {
                let encrypt = $name_enc::new(key);
                let decrypt = $name_dec::from(&encrypt);
                Self { encrypt, decrypt }
            }
        }

        impl From<$name_enc> for $name {
            #[inline]
            fn from(encrypt: $name_enc) -> $name {
                let decrypt = (&encrypt).into();
                Self { encrypt, decrypt }
            }
        }

        impl From<&$name_enc> for $name {
            #[inline]
            fn from(encrypt: &$name_enc) -> $name {
                let decrypt = encrypt.into();
                let encrypt = encrypt.clone();
                Self { encrypt, decrypt }
            }
        }

        impl BlockSizeUser for $name {
            type BlockSize = U16;
        }

        impl BlockEncrypt for $name {
            fn encrypt_with_backend(&self, f: impl BlockClosure<BlockSize = U16>) {
                self.encrypt.encrypt_with_backend(f)
            }
        }

        impl BlockDecrypt for $name {
            fn decrypt_with_backend(&self, f: impl BlockClosure<BlockSize = U16>) {
                self.decrypt.decrypt_with_backend(f)
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

        #[cfg(feature = "zeroize")]
        impl zeroize::ZeroizeOnDrop for $name {}

        #[doc=$doc]
        #[doc = "block cipher (encrypt-only)"]
        #[derive(Clone)]
        pub struct $name_enc {
            round_keys: [uint8x16_t; $rounds],
        }

        impl $name_enc {
            #[inline(always)]
            pub(crate) fn get_enc_backend(&self) -> $name_back_enc<'_> {
                $name_back_enc(self)
            }
        }

        impl BlockCipher for $name_enc {}

        impl KeySizeUser for $name_enc {
            type KeySize = $key_size;
        }

        impl KeyInit for $name_enc {
            fn new(key: &Key<Self>) -> Self {
                Self {
                    round_keys: unsafe { expand_key(key.as_ref()) },
                }
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

        impl Drop for $name_enc {
            #[inline]
            fn drop(&mut self) {
                #[cfg(feature = "zeroize")]
                zeroize::Zeroize::zeroize(&mut self.round_keys);
            }
        }

        #[cfg(feature = "zeroize")]
        impl zeroize::ZeroizeOnDrop for $name_enc {}

        #[doc=$doc]
        #[doc = "block cipher (decrypt-only)"]
        #[derive(Clone)]
        pub struct $name_dec {
            round_keys: [uint8x16_t; $rounds],
        }

        impl $name_dec {
            #[inline(always)]
            pub(crate) fn get_dec_backend(&self) -> $name_back_dec<'_> {
                $name_back_dec(self)
            }
        }

        impl BlockCipher for $name_dec {}

        impl KeySizeUser for $name_dec {
            type KeySize = $key_size;
        }

        impl KeyInit for $name_dec {
            fn new(key: &Key<Self>) -> Self {
                $name_enc::new(key).into()
            }
        }

        impl From<$name_enc> for $name_dec {
            #[inline]
            fn from(enc: $name_enc) -> $name_dec {
                Self::from(&enc)
            }
        }

        impl From<&$name_enc> for $name_dec {
            fn from(enc: &$name_enc) -> $name_dec {
                let mut round_keys = enc.round_keys;
                unsafe { inv_expanded_keys(&mut round_keys) };
                Self { round_keys }
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

        impl Drop for $name_dec {
            #[inline]
            fn drop(&mut self) {
                #[cfg(feature = "zeroize")]
                zeroize::Zeroize::zeroize(&mut self.round_keys);
            }
        }

        #[cfg(feature = "zeroize")]
        impl zeroize::ZeroizeOnDrop for $name_dec {}

        pub(crate) struct $name_back_enc<'a>(&'a $name_enc);

        impl<'a> BlockSizeUser for $name_back_enc<'a> {
            type BlockSize = U16;
        }

        impl<'a> ParBlocksSizeUser for $name_back_enc<'a> {
            type ParBlocksSize = U8;
        }

        impl<'a> BlockBackend for $name_back_enc<'a> {
            #[inline(always)]
            fn proc_block(&mut self, block: InOut<'_, '_, Block>) {
                unsafe {
                    encrypt1(&self.0.round_keys, block);
                }
            }

            #[inline(always)]
            fn proc_par_blocks(&mut self, blocks: InOut<'_, '_, Block8>) {
                unsafe { encrypt8(&self.0.round_keys, blocks) }
            }
        }

        pub(crate) struct $name_back_dec<'a>(&'a $name_dec);

        impl<'a> BlockSizeUser for $name_back_dec<'a> {
            type BlockSize = U16;
        }

        impl<'a> ParBlocksSizeUser for $name_back_dec<'a> {
            type ParBlocksSize = U8;
        }

        impl<'a> BlockBackend for $name_back_dec<'a> {
            #[inline(always)]
            fn proc_block(&mut self, block: InOut<'_, '_, Block>) {
                unsafe {
                    decrypt1(&self.0.round_keys, block);
                }
            }

            #[inline(always)]
            fn proc_par_blocks(&mut self, blocks: InOut<'_, '_, Block8>) {
                unsafe { decrypt8(&self.0.round_keys, blocks) }
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
    11,
    "AES-128",
);
define_aes_impl!(
    Aes192,
    Aes192Enc,
    Aes192Dec,
    Aes192BackEnc,
    Aes192BackDec,
    U24,
    13,
    "AES-192",
);
define_aes_impl!(
    Aes256,
    Aes256Enc,
    Aes256Dec,
    Aes256BackEnc,
    Aes256BackDec,
    U32,
    15,
    "AES-256",
);
