/*
fdct is a Rust translation of jfdctint.c from the
Independent JPEG Group's libjpeg version 9a
obtained from http://www.ijg.org/files/jpegsr9a.zip
It comes with the following conditions of distribution and use:

    In plain English:

    1. We don't promise that this software works.  (But if you find any bugs,
        please let us know!)
    2. You can use this software for whatever you want.  You don't have to pay us.
    3. You may not pretend that you wrote this software.  If you use it in a
       program, you must acknowledge somewhere in your documentation that
       you've used the IJG code.

    In legalese:

    The authors make NO WARRANTY or representation, either express or implied,
    with respect to this software, its quality, accuracy, merchantability, or
    fitness for a particular purpose.  This software is provided "AS IS", and you,
    its user, assume the entire risk as to its quality and accuracy.

    This software is copyright (C) 1991-2014, Thomas G. Lane, Guido Vollbeding.
    All Rights Reserved except as specified below.

    Permission is hereby granted to use, copy, modify, and distribute this
    software (or portions thereof) for any purpose, without fee, subject to these
    conditions:
    (1) If any part of the source code for this software is distributed, then this
    README file must be included, with this copyright and no-warranty notice
    unaltered; and any additions, deletions, or changes to the original files
    must be clearly indicated in accompanying documentation.
    (2) If only executable code is distributed, then the accompanying
    documentation must state that "this software is based in part on the work of
    the Independent JPEG Group".
    (3) Permission for use of this software is granted only if the user accepts
    full responsibility for any undesirable consequences; the authors accept
    NO LIABILITY for damages of any kind.

    These conditions apply to any software derived from or based on the IJG code,
    not just to the unmodified library.  If you use our work, you ought to
    acknowledge us.

    Permission is NOT granted for the use of any IJG author's name or company name
    in advertising or publicity relating to this software or products derived from
    it.  This software may be referred to only as "the Independent JPEG Group's
    software".

    We specifically permit and encourage the use of this software as the basis of
    commercial products, provided that all warranty or liability claims are
    assumed by the product vendor.
*/

static CONST_BITS: i32 = 13;
static PASS1_BITS: i32 = 2;

static FIX_0_298631336: i32 = 2446;
static FIX_0_390180644: i32 = 3196;
static FIX_0_541196100: i32 = 4433;
static FIX_0_765366865: i32 = 6270;
static FIX_0_899976223: i32 = 7373;
static FIX_1_175875602: i32 = 9633;
static FIX_1_501321110: i32 = 12_299;
static FIX_1_847759065: i32 = 15_137;
static FIX_1_961570560: i32 = 16_069;
static FIX_2_053119869: i32 = 16_819;
static FIX_2_562915447: i32 = 20_995;
static FIX_3_072711026: i32 = 25_172;

pub(crate) fn fdct(samples: &[u8; 64], coeffs: &mut [i32; 64]) {
    // Pass 1: process rows.
    // Results are scaled by sqrt(8) compared to a true DCT
    // furthermore we scale the results by 2**PASS1_BITS
    for y in 0usize..8 {
        let y0 = y * 8;

        // Even part
        let t0 = i32::from(samples[y0]) + i32::from(samples[y0 + 7]);
        let t1 = i32::from(samples[y0 + 1]) + i32::from(samples[y0 + 6]);
        let t2 = i32::from(samples[y0 + 2]) + i32::from(samples[y0 + 5]);
        let t3 = i32::from(samples[y0 + 3]) + i32::from(samples[y0 + 4]);

        let t10 = t0 + t3;
        let t12 = t0 - t3;
        let t11 = t1 + t2;
        let t13 = t1 - t2;

        let t0 = i32::from(samples[y0]) - i32::from(samples[y0 + 7]);
        let t1 = i32::from(samples[y0 + 1]) - i32::from(samples[y0 + 6]);
        let t2 = i32::from(samples[y0 + 2]) - i32::from(samples[y0 + 5]);
        let t3 = i32::from(samples[y0 + 3]) - i32::from(samples[y0 + 4]);

        // Apply unsigned -> signed conversion
        coeffs[y0] = (t10 + t11 - 8 * 128) << PASS1_BITS as usize;
        coeffs[y0 + 4] = (t10 - t11) << PASS1_BITS as usize;

        let mut z1 = (t12 + t13) * FIX_0_541196100;
        // Add fudge factor here for final descale
        z1 += 1 << (CONST_BITS - PASS1_BITS - 1) as usize;

        coeffs[y0 + 2] = (z1 + t12 * FIX_0_765366865) >> (CONST_BITS - PASS1_BITS) as usize;
        coeffs[y0 + 6] = (z1 - t13 * FIX_1_847759065) >> (CONST_BITS - PASS1_BITS) as usize;

        // Odd part
        let t12 = t0 + t2;
        let t13 = t1 + t3;

        let mut z1 = (t12 + t13) * FIX_1_175875602;
        // Add fudge factor here for final descale
        z1 += 1 << (CONST_BITS - PASS1_BITS - 1) as usize;

        let mut t12 = t12 * (-FIX_0_390180644);
        let mut t13 = t13 * (-FIX_1_961570560);
        t12 += z1;
        t13 += z1;

        let z1 = (t0 + t3) * (-FIX_0_899976223);
        let mut t0 = t0 * FIX_1_501321110;
        let mut t3 = t3 * FIX_0_298631336;
        t0 += z1 + t12;
        t3 += z1 + t13;

        let z1 = (t1 + t2) * (-FIX_2_562915447);
        let mut t1 = t1 * FIX_3_072711026;
        let mut t2 = t2 * FIX_2_053119869;
        t1 += z1 + t13;
        t2 += z1 + t12;

        coeffs[y0 + 1] = t0 >> (CONST_BITS - PASS1_BITS) as usize;
        coeffs[y0 + 3] = t1 >> (CONST_BITS - PASS1_BITS) as usize;
        coeffs[y0 + 5] = t2 >> (CONST_BITS - PASS1_BITS) as usize;
        coeffs[y0 + 7] = t3 >> (CONST_BITS - PASS1_BITS) as usize;
    }

    // Pass 2: process columns
    // We remove the PASS1_BITS scaling but leave the results scaled up an
    // overall factor of 8
    for x in (0usize..8).rev() {
        // Even part
        let t0 = coeffs[x] + coeffs[x + 8 * 7];
        let t1 = coeffs[x + 8] + coeffs[x + 8 * 6];
        let t2 = coeffs[x + 8 * 2] + coeffs[x + 8 * 5];
        let t3 = coeffs[x + 8 * 3] + coeffs[x + 8 * 4];

        // Add fudge factor here for final descale
        let t10 = t0 + t3 + (1 << (PASS1_BITS - 1) as usize);
        let t12 = t0 - t3;
        let t11 = t1 + t2;
        let t13 = t1 - t2;

        let t0 = coeffs[x] - coeffs[x + 8 * 7];
        let t1 = coeffs[x + 8] - coeffs[x + 8 * 6];
        let t2 = coeffs[x + 8 * 2] - coeffs[x + 8 * 5];
        let t3 = coeffs[x + 8 * 3] - coeffs[x + 8 * 4];

        coeffs[x] = (t10 + t11) >> PASS1_BITS as usize;
        coeffs[x + 8 * 4] = (t10 - t11) >> PASS1_BITS as usize;

        let mut z1 = (t12 + t13) * FIX_0_541196100;
        // Add fudge factor here for final descale
        z1 += 1 << (CONST_BITS + PASS1_BITS - 1) as usize;

        coeffs[x + 8 * 2] = (z1 + t12 * FIX_0_765366865) >> (CONST_BITS + PASS1_BITS) as usize;
        coeffs[x + 8 * 6] = (z1 - t13 * FIX_1_847759065) >> (CONST_BITS + PASS1_BITS) as usize;

        // Odd part
        let t12 = t0 + t2;
        let t13 = t1 + t3;

        let mut z1 = (t12 + t13) * FIX_1_175875602;
        // Add fudge factor here for final descale
        z1 += 1 << (CONST_BITS - PASS1_BITS - 1) as usize;

        let mut t12 = t12 * (-FIX_0_390180644);
        let mut t13 = t13 * (-FIX_1_961570560);
        t12 += z1;
        t13 += z1;

        let z1 = (t0 + t3) * (-FIX_0_899976223);
        let mut t0 = t0 * FIX_1_501321110;
        let mut t3 = t3 * FIX_0_298631336;
        t0 += z1 + t12;
        t3 += z1 + t13;

        let z1 = (t1 + t2) * (-FIX_2_562915447);
        let mut t1 = t1 * FIX_3_072711026;
        let mut t2 = t2 * FIX_2_053119869;
        t1 += z1 + t13;
        t2 += z1 + t12;

        coeffs[x + 8] = t0 >> (CONST_BITS + PASS1_BITS) as usize;
        coeffs[x + 8 * 3] = t1 >> (CONST_BITS + PASS1_BITS) as usize;
        coeffs[x + 8 * 5] = t2 >> (CONST_BITS + PASS1_BITS) as usize;
        coeffs[x + 8 * 7] = t3 >> (CONST_BITS + PASS1_BITS) as usize;
    }
}
