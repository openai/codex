//! [Cipher Block Chaining][1] (CBC) mode.
//!
//! <img src="https://raw.githubusercontent.com/RustCrypto/media/26acc39f/img/block-modes/cbc_enc.svg" width="49%" />
//! <img src="https://raw.githubusercontent.com/RustCrypto/media/26acc39f/img/block-modes/cbc_dec.svg" width="49%"/>
//!
//! Mode functionality is accessed using traits from re-exported [`cipher`] crate.
//!
//! # ⚠️ Security Warning: Hazmat!
//!
//! This crate does not ensure ciphertexts are authentic! Thus ciphertext integrity
//! is not verified, which can lead to serious vulnerabilities!
//!
//! # Example
//! ```
//! # #[cfg(feature = "block-padding")] {
//! use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
//! use hex_literal::hex;
//!
//! type Aes128CbcEnc = cbc::Encryptor<aes::Aes128>;
//! type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;
//!
//! let key = [0x42; 16];
//! let iv = [0x24; 16];
//! let plaintext = *b"hello world! this is my plaintext.";
//! let ciphertext = hex!(
//!     "c7fe247ef97b21f07cbdd26cb5d346bf"
//!     "d27867cb00d9486723e159978fb9a5f9"
//!     "14cfb228a710de4171e396e7b6cf859e"
//! );
//!
//! // encrypt/decrypt in-place
//! // buffer must be big enough for padded plaintext
//! let mut buf = [0u8; 48];
//! let pt_len = plaintext.len();
//! buf[..pt_len].copy_from_slice(&plaintext);
//! let ct = Aes128CbcEnc::new(&key.into(), &iv.into())
//!     .encrypt_padded_mut::<Pkcs7>(&mut buf, pt_len)
//!     .unwrap();
//! assert_eq!(ct, &ciphertext[..]);
//!
//! let pt = Aes128CbcDec::new(&key.into(), &iv.into())
//!     .decrypt_padded_mut::<Pkcs7>(&mut buf)
//!     .unwrap();
//! assert_eq!(pt, &plaintext);
//!
//! // encrypt/decrypt from buffer to buffer
//! let mut buf = [0u8; 48];
//! let ct = Aes128CbcEnc::new(&key.into(), &iv.into())
//!     .encrypt_padded_b2b_mut::<Pkcs7>(&plaintext, &mut buf)
//!     .unwrap();
//! assert_eq!(ct, &ciphertext[..]);
//!
//! let mut buf = [0u8; 48];
//! let pt = Aes128CbcDec::new(&key.into(), &iv.into())
//!     .decrypt_padded_b2b_mut::<Pkcs7>(&ct, &mut buf)
//!     .unwrap();
//! assert_eq!(pt, &plaintext);
//! # }
//! ```
//!
//! With enabled `alloc` (or `std`) feature you also can use allocating
//! convinience methods:
//! ```
//! # #[cfg(all(feature = "alloc", feature = "block-padding"))] {
//! # use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
//! # use hex_literal::hex;
//! # type Aes128CbcEnc = cbc::Encryptor<aes::Aes128>;
//! # type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;
//! # let key = [0x42; 16];
//! # let iv = [0x24; 16];
//! # let plaintext = *b"hello world! this is my plaintext.";
//! # let ciphertext = hex!(
//! #     "c7fe247ef97b21f07cbdd26cb5d346bf"
//! #     "d27867cb00d9486723e159978fb9a5f9"
//! #     "14cfb228a710de4171e396e7b6cf859e"
//! # );
//! let res = Aes128CbcEnc::new(&key.into(), &iv.into())
//!     .encrypt_padded_vec_mut::<Pkcs7>(&plaintext);
//! assert_eq!(res[..], ciphertext[..]);
//! let res = Aes128CbcDec::new(&key.into(), &iv.into())
//!     .decrypt_padded_vec_mut::<Pkcs7>(&res)
//!     .unwrap();
//! assert_eq!(res[..], plaintext[..]);
//! # }
//! ```
//!
//! [1]: https://en.wikipedia.org/wiki/Block_cipher_mode_of_operation#CBC

#![no_std]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/RustCrypto/media/26acc39f/logo.svg",
    html_favicon_url = "https://raw.githubusercontent.com/RustCrypto/media/26acc39f/logo.svg",
    html_root_url = "https://docs.rs/cbc/0.1.2"
)]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs, rust_2018_idioms)]

mod decrypt;
mod encrypt;

pub use cipher;
pub use decrypt::Decryptor;
pub use encrypt::Encryptor;

use cipher::generic_array::{ArrayLength, GenericArray};

#[inline(always)]
fn xor<N: ArrayLength<u8>>(out: &mut GenericArray<u8, N>, buf: &GenericArray<u8, N>) {
    for (a, b) in out.iter_mut().zip(buf) {
        *a ^= *b;
    }
}
