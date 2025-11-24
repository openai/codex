//! AES block ciphers implementation using AES-NI instruction set.
//!
//! Ciphers functionality is accessed using `BlockCipher` trait from the
//! [`cipher`](https://docs.rs/cipher) crate.
//!
//! # Vulnerability
//! Lazy FP state restory vulnerability can allow local process to leak content
//! of the FPU register, in which round keys are stored. This vulnerability
//! can be mitigated at the operating system level by installing relevant
//! patches. (i.e. keep your OS updated!) More info:
//! - [Intel advisory](https://www.intel.com/content/www/us/en/security-center/advisory/intel-sa-00145.html)
//! - [Wikipedia](https://en.wikipedia.org/wiki/Lazy_FP_state_restore)
//!
//! # Related documents
//! - [Intel AES-NI whitepaper](https://software.intel.com/sites/default/files/article/165683/aes-wp-2012-09-22-v01.pdf)
//! - [Use of the AES Instruction Set](https://www.cosic.esat.kuleuven.be/ecrypt/AESday/slides/Use_of_the_AES_Instruction_Set.pdf)

#[macro_use]
mod utils;

mod aes128;
mod aes192;
mod aes256;

#[cfg(test)]
mod test_expand;

#[cfg(feature = "hazmat")]
pub(crate) mod hazmat;

#[cfg(target_arch = "x86")]
use core::arch::x86 as arch;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64 as arch;

use crate::{Block, Block8};
use cipher::{
    consts::{U16, U24, U32, U8},
    inout::InOut,
    AlgorithmName, BlockBackend, BlockCipher, BlockClosure, BlockDecrypt, BlockEncrypt,
    BlockSizeUser, Key, KeyInit, KeySizeUser, ParBlocksSizeUser,
};
use core::fmt;

macro_rules! define_aes_impl {
    (
        $name:tt,
        $name_enc:ident,
        $name_dec:ident,
        $name_back_enc:ident,
        $name_back_dec:ident,
        $module:tt,
        $key_size:ty,
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
            round_keys: $module::RoundKeys,
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
                // SAFETY: we enforce that this code is called only when
                // target features required by `expand` were properly checked.
                Self {
                    round_keys: unsafe { $module::expand_key(key.as_ref()) },
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
            round_keys: $module::RoundKeys,
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
            #[inline]
            fn from(enc: &$name_enc) -> $name_dec {
                let round_keys = unsafe { $module::inv_expanded_keys(&enc.round_keys) };
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
                    $module::encrypt1(&self.0.round_keys, block);
                }
            }

            #[inline(always)]
            fn proc_par_blocks(&mut self, blocks: InOut<'_, '_, Block8>) {
                unsafe {
                    $module::encrypt8(&self.0.round_keys, blocks);
                }
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
                    $module::decrypt1(&self.0.round_keys, block);
                }
            }

            #[inline(always)]
            fn proc_par_blocks(&mut self, blocks: InOut<'_, '_, Block8>) {
                unsafe {
                    $module::decrypt8(&self.0.round_keys, blocks);
                }
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
    aes128,
    U16,
    "AES-128",
);

define_aes_impl!(
    Aes192,
    Aes192Enc,
    Aes192Dec,
    Aes192BackEnc,
    Aes192BackDec,
    aes192,
    U24,
    "AES-192",
);

define_aes_impl!(
    Aes256,
    Aes256Enc,
    Aes256Dec,
    Aes256BackEnc,
    Aes256BackDec,
    aes256,
    U32,
    "AES-256",
);
