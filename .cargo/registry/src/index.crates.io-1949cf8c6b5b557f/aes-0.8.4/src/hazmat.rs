//! ⚠️ Low-level "hazmat" AES functions.
//!
//! # ☢️️ WARNING: HAZARDOUS API ☢️
//!
//! This module contains an extremely low-level cryptographic primitive
//! which is likewise extremely difficult to use correctly.
//!
//! There are very few valid uses cases for this API. It's intended to be used
//! for implementing well-reviewed higher-level constructions.
//!
//! We do NOT recommend using it to implement any algorithm which has not
//! received extensive peer review by cryptographers.

use crate::{soft::fixslice::hazmat as soft, Block, Block8};

#[cfg(all(target_arch = "aarch64", aes_armv8, not(aes_force_soft)))]
use crate::armv8::hazmat as intrinsics;

#[cfg(all(any(target_arch = "x86_64", target_arch = "x86"), not(aes_force_soft)))]
use crate::ni::hazmat as intrinsics;

#[cfg(all(
    any(
        target_arch = "x86",
        target_arch = "x86_64",
        all(target_arch = "aarch64", aes_armv8)
    ),
    not(aes_force_soft)
))]
cpufeatures::new!(aes_intrinsics, "aes");

/// Execute the provided body if CPU intrinsics are available.
// TODO(tarcieri): more `cfg-if`-like macro with an else branch?
macro_rules! if_intrinsics_available {
    ($body:expr) => {{
        #[cfg(all(
            any(
                target_arch = "x86",
                target_arch = "x86_64",
                all(target_arch = "aarch64", aes_armv8)
            ),
            not(aes_force_soft)
        ))]
        if aes_intrinsics::get() {
            unsafe { $body }
            return;
        }
    }};
}

/// ⚠️ AES cipher (encrypt) round function.
///
/// This API performs the following steps as described in FIPS 197 Appendix C:
///
/// - `s_box`: state after `SubBytes()`
/// - `s_row`: state after `ShiftRows()`
/// - `m_col`: state after `MixColumns()`
/// - `k_sch`: key schedule value for `round[r]`
///
/// This series of operations is equivalent to the Intel AES-NI `AESENC` instruction.
///
/// # ☢️️ WARNING: HAZARDOUS API ☢️
///
/// Use this function with great care! See the [module-level documentation][crate::hazmat]
/// for more information.
pub fn cipher_round(block: &mut Block, round_key: &Block) {
    if_intrinsics_available! {
        intrinsics::cipher_round(block, round_key)
    }

    soft::cipher_round(block, round_key);
}

/// ⚠️ AES cipher (encrypt) round function: parallel version.
///
/// Equivalent to [`cipher_round`], but acts on 8 blocks-at-a-time, applying
/// the same number of round keys.
///
/// # ☢️️ WARNING: HAZARDOUS API ☢️
///
/// Use this function with great care! See the [module-level documentation][crate::hazmat]
/// for more information.
pub fn cipher_round_par(blocks: &mut Block8, round_keys: &Block8) {
    if_intrinsics_available! {
        intrinsics::cipher_round_par(blocks, round_keys)
    }

    soft::cipher_round_par(blocks, round_keys);
}

/// ⚠️ AES equivalent inverse cipher (decrypt) round function.
///
/// This API performs the following steps as described in FIPS 197 Appendix C:
///
/// - `is_box`: state after `InvSubBytes()`
/// - `is_row`: state after `InvShiftRows()`
/// - `im_col`: state after `InvMixColumns()`
/// - `ik_sch`: key schedule value for `round[r]`
///
/// This series of operations is equivalent to the Intel AES-NI `AESDEC` instruction.
///
/// # ☢️️ WARNING: HAZARDOUS API ☢️
///
/// Use this function with great care! See the [module-level documentation][crate::hazmat]
/// for more information.
pub fn equiv_inv_cipher_round(block: &mut Block, round_key: &Block) {
    if_intrinsics_available! {
        intrinsics::equiv_inv_cipher_round(block, round_key)
    }

    soft::equiv_inv_cipher_round(block, round_key);
}

/// ⚠️ AES equivalent inverse cipher (decrypt) round function: parallel version.
///
/// Equivalent to [`equiv_inv_cipher_round`], but acts on 8 blocks-at-a-time,
/// applying the same number of round keys.
///
/// # ☢️️ WARNING: HAZARDOUS API ☢️
///
/// Use this function with great care! See the [module-level documentation][crate::hazmat]
/// for more information.
pub fn equiv_inv_cipher_round_par(blocks: &mut Block8, round_keys: &Block8) {
    if_intrinsics_available! {
        intrinsics::equiv_inv_cipher_round_par(blocks, round_keys)
    }

    soft::equiv_inv_cipher_round_par(blocks, round_keys);
}

/// ⚠️ AES mix columns function.
///
/// # ☢️️ WARNING: HAZARDOUS API ☢️
///
/// Use this function with great care! See the [module-level documentation][crate::hazmat]
/// for more information.
pub fn mix_columns(block: &mut Block) {
    if_intrinsics_available! {
        intrinsics::mix_columns(block)
    }

    soft::mix_columns(block);
}

/// ⚠️ AES inverse mix columns function.
///
/// This function is equivalent to the Intel AES-NI `AESIMC` instruction.
///
/// # ☢️️ WARNING: HAZARDOUS API ☢️
///
/// Use this function with great care! See the [module-level documentation][crate::hazmat]
/// for more information.
pub fn inv_mix_columns(block: &mut Block) {
    if_intrinsics_available! {
        intrinsics::inv_mix_columns(block)
    }

    soft::inv_mix_columns(block);
}
