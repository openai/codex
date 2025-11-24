/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Routines for IDCT
//!
//! Essentially we provide 2 routines for IDCT, a scalar implementation and a not super optimized
//! AVX2 one, i'll talk about them here.
//!
//! There are 2 reasons why we have the avx one
//! 1. No one compiles with -C target-features=avx2 hence binaries won't probably take advantage(even
//! if it exists).
//! 2. AVX employs zero short circuit in a way the scalar code cannot employ it.
//!     - AVX does this by checking for MCU's whose 63 AC coefficients are zero and if true, it writes
//!        values directly, if false, it goes the long way of calculating.
//!     -   Although this can be trivially implemented in the scalar version, it  generates code
//!         I'm not happy width(scalar version that basically loops and that is too many branches for me)
//!         The avx one does a better job of using bitwise or's with (`_mm256_or_si256`) which is magnitudes of faster
//!         than anything I could come up with
//!
//! The AVX code also has some cool transpose_u16 instructions which look so complicated to be cool
//! (spoiler alert, i barely understand how it works, that's why I credited the owner).
//!
#![allow(
    clippy::excessive_precision,
    clippy::unreadable_literal,
    clippy::module_name_repetitions,
    unused_parens,
    clippy::wildcard_imports
)]

use zune_core::log::debug;
use zune_core::options::DecoderOptions;

use crate::decoder::IDCTPtr;
use crate::idct::scalar::idct_int;

#[cfg(feature = "x86")]
pub mod avx2;
#[cfg(feature = "neon")]
pub mod neon;

pub mod scalar;

/// Choose an appropriate IDCT function
#[allow(unused_variables)]
pub fn choose_idct_func(options: &DecoderOptions) -> IDCTPtr {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[cfg(feature = "x86")]
    {
        if options.use_avx2() {
            debug!("Using vector integer IDCT");
            // use avx one
            return crate::idct::avx2::idct_avx2;
        }
    }
    #[cfg(target_arch = "aarch64")]
    #[cfg(feature = "neon")]
    {
        if options.use_neon() {
            debug!("Using vector integer IDCT");
            return crate::idct::neon::idct_neon;
        }
    }
    debug!("Using scalar integer IDCT");
    // use generic one
    return idct_int;
}

#[cfg(test)]
#[allow(unreachable_code)]
#[allow(dead_code)]
mod tests {
    use super::*;

    #[test]
    fn idct_test0() {
        let stride = 8;
        let mut coeff = [10; 64];
        let mut coeff2 = [10; 64];
        let mut output_scalar = [0; 64];
        let mut output_vector = [0; 64];
        idct_fnc()(&mut coeff, &mut output_vector, stride);
        idct_int(&mut coeff2, &mut output_scalar, stride);
        assert_eq!(output_scalar, output_vector, "IDCT and scalar do not match");
    }

    #[test]
    fn do_idct_test1() {
        let stride = 8;
        let mut coeff = [14; 64];
        let mut coeff2 = [14; 64];
        let mut output_scalar = [0; 64];
        let mut output_vector = [0; 64];
        idct_fnc()(&mut coeff, &mut output_vector, stride);
        idct_int(&mut coeff2, &mut output_scalar, stride);
        assert_eq!(output_scalar, output_vector, "IDCT and scalar do not match");
    }

    #[test]
    fn do_idct_test2() {
        let stride = 8;
        let mut coeff = [0; 64];
        coeff[0] = 255;
        coeff[63] = -256;
        let mut coeff2 = coeff;
        let mut output_scalar = [0; 64];
        let mut output_vector = [0; 64];
        idct_fnc()(&mut coeff, &mut output_vector, stride);
        idct_int(&mut coeff2, &mut output_scalar, stride);
        assert_eq!(output_scalar, output_vector, "IDCT and scalar do not match");
    }

    #[test]
    fn do_idct_zeros() {
        let stride = 8;
        let mut coeff = [0; 64];
        let mut coeff2 = [0; 64];
        let mut output_scalar = [0; 64];
        let mut output_vector = [0; 64];
        idct_fnc()(&mut coeff, &mut output_vector, stride);
        idct_int(&mut coeff2, &mut output_scalar, stride);
        assert_eq!(output_scalar, output_vector, "IDCT and scalar do not match");
    }

    fn idct_fnc() -> IDCTPtr {
        #[cfg(feature = "neon")]
        #[cfg(target_arch = "aarch64")]
        {
            use crate::idct::neon::idct_neon;
            return idct_neon;
        }

        #[cfg(feature = "x86")]
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            use crate::idct::avx2::idct_avx2;
            return idct_avx2;
        }

        idct_int
    }
}
