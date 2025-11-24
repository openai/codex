// Translated from C to Rust. The original C code can be found at
// https://github.com/ulfjack/ryu and carries the following license:
//
// Copyright 2018 Ulf Adams
//
// The contents of this file may be used under the terms of the Apache License,
// Version 2.0.
//
//    (See accompanying file LICENSE-Apache or copy at
//     http://www.apache.org/licenses/LICENSE-2.0)
//
// Alternatively, the contents of this file may be used under the terms of
// the Boost Software License, Version 1.0.
//    (See accompanying file LICENSE-Boost or copy at
//     https://www.boost.org/LICENSE_1_0.txt)
//
// Unless required by applicable law or agreed to in writing, this software
// is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.

use crate::common::{log10_pow2, log10_pow5, pow5bits};
#[cfg(not(feature = "small"))]
pub use crate::d2s_full_table::{DOUBLE_POW5_INV_SPLIT, DOUBLE_POW5_SPLIT};
use crate::d2s_intrinsics::{
    div10, div100, div5, mul_shift_all_64, multiple_of_power_of_2, multiple_of_power_of_5,
};
#[cfg(feature = "small")]
pub use crate::d2s_small_table::{compute_inv_pow5, compute_pow5};
use core::mem::MaybeUninit;

pub const DOUBLE_MANTISSA_BITS: u32 = 52;
pub const DOUBLE_EXPONENT_BITS: u32 = 11;
pub const DOUBLE_BIAS: i32 = 1023;
pub const DOUBLE_POW5_INV_BITCOUNT: i32 = 125;
pub const DOUBLE_POW5_BITCOUNT: i32 = 125;

#[cfg_attr(feature = "no-panic", inline)]
pub fn decimal_length17(v: u64) -> u32 {
    // This is slightly faster than a loop.
    // The average output length is 16.38 digits, so we check high-to-low.
    // Function precondition: v is not an 18, 19, or 20-digit number.
    // (17 digits are sufficient for round-tripping.)
    debug_assert!(v < 100000000000000000);

    if v >= 10000000000000000 {
        17
    } else if v >= 1000000000000000 {
        16
    } else if v >= 100000000000000 {
        15
    } else if v >= 10000000000000 {
        14
    } else if v >= 1000000000000 {
        13
    } else if v >= 100000000000 {
        12
    } else if v >= 10000000000 {
        11
    } else if v >= 1000000000 {
        10
    } else if v >= 100000000 {
        9
    } else if v >= 10000000 {
        8
    } else if v >= 1000000 {
        7
    } else if v >= 100000 {
        6
    } else if v >= 10000 {
        5
    } else if v >= 1000 {
        4
    } else if v >= 100 {
        3
    } else if v >= 10 {
        2
    } else {
        1
    }
}

// A floating decimal representing m * 10^e.
pub struct FloatingDecimal64 {
    pub mantissa: u64,
    // Decimal exponent's range is -324 to 308
    // inclusive, and can fit in i16 if needed.
    pub exponent: i32,
}

#[cfg_attr(feature = "no-panic", inline)]
pub fn d2d(ieee_mantissa: u64, ieee_exponent: u32) -> FloatingDecimal64 {
    let (e2, m2) = if ieee_exponent == 0 {
        (
            // We subtract 2 so that the bounds computation has 2 additional bits.
            1 - DOUBLE_BIAS - DOUBLE_MANTISSA_BITS as i32 - 2,
            ieee_mantissa,
        )
    } else {
        (
            ieee_exponent as i32 - DOUBLE_BIAS - DOUBLE_MANTISSA_BITS as i32 - 2,
            (1u64 << DOUBLE_MANTISSA_BITS) | ieee_mantissa,
        )
    };
    let even = (m2 & 1) == 0;
    let accept_bounds = even;

    // Step 2: Determine the interval of valid decimal representations.
    let mv = 4 * m2;
    // Implicit bool -> int conversion. True is 1, false is 0.
    let mm_shift = (ieee_mantissa != 0 || ieee_exponent <= 1) as u32;
    // We would compute mp and mm like this:
    // uint64_t mp = 4 * m2 + 2;
    // uint64_t mm = mv - 1 - mm_shift;

    // Step 3: Convert to a decimal power base using 128-bit arithmetic.
    let mut vr: u64;
    let mut vp: u64;
    let mut vm: u64;
    let mut vp_uninit: MaybeUninit<u64> = MaybeUninit::uninit();
    let mut vm_uninit: MaybeUninit<u64> = MaybeUninit::uninit();
    let e10: i32;
    let mut vm_is_trailing_zeros = false;
    let mut vr_is_trailing_zeros = false;
    if e2 >= 0 {
        // I tried special-casing q == 0, but there was no effect on performance.
        // This expression is slightly faster than max(0, log10_pow2(e2) - 1).
        let q = log10_pow2(e2) - (e2 > 3) as u32;
        e10 = q as i32;
        let k = DOUBLE_POW5_INV_BITCOUNT + pow5bits(q as i32) - 1;
        let i = -e2 + q as i32 + k;
        vr = unsafe {
            mul_shift_all_64(
                m2,
                #[cfg(feature = "small")]
                &compute_inv_pow5(q),
                #[cfg(not(feature = "small"))]
                {
                    debug_assert!(q < DOUBLE_POW5_INV_SPLIT.len() as u32);
                    DOUBLE_POW5_INV_SPLIT.get_unchecked(q as usize)
                },
                i as u32,
                vp_uninit.as_mut_ptr(),
                vm_uninit.as_mut_ptr(),
                mm_shift,
            )
        };
        vp = unsafe { vp_uninit.assume_init() };
        vm = unsafe { vm_uninit.assume_init() };
        if q <= 21 {
            // This should use q <= 22, but I think 21 is also safe. Smaller values
            // may still be safe, but it's more difficult to reason about them.
            // Only one of mp, mv, and mm can be a multiple of 5, if any.
            let mv_mod5 = (mv as u32).wrapping_sub(5u32.wrapping_mul(div5(mv) as u32));
            if mv_mod5 == 0 {
                vr_is_trailing_zeros = multiple_of_power_of_5(mv, q);
            } else if accept_bounds {
                // Same as min(e2 + (~mm & 1), pow5_factor(mm)) >= q
                // <=> e2 + (~mm & 1) >= q && pow5_factor(mm) >= q
                // <=> true && pow5_factor(mm) >= q, since e2 >= q.
                vm_is_trailing_zeros = multiple_of_power_of_5(mv - 1 - mm_shift as u64, q);
            } else {
                // Same as min(e2 + 1, pow5_factor(mp)) >= q.
                vp -= multiple_of_power_of_5(mv + 2, q) as u64;
            }
        }
    } else {
        // This expression is slightly faster than max(0, log10_pow5(-e2) - 1).
        let q = log10_pow5(-e2) - (-e2 > 1) as u32;
        e10 = q as i32 + e2;
        let i = -e2 - q as i32;
        let k = pow5bits(i) - DOUBLE_POW5_BITCOUNT;
        let j = q as i32 - k;
        vr = unsafe {
            mul_shift_all_64(
                m2,
                #[cfg(feature = "small")]
                &compute_pow5(i as u32),
                #[cfg(not(feature = "small"))]
                {
                    debug_assert!(i < DOUBLE_POW5_SPLIT.len() as i32);
                    DOUBLE_POW5_SPLIT.get_unchecked(i as usize)
                },
                j as u32,
                vp_uninit.as_mut_ptr(),
                vm_uninit.as_mut_ptr(),
                mm_shift,
            )
        };
        vp = unsafe { vp_uninit.assume_init() };
        vm = unsafe { vm_uninit.assume_init() };
        if q <= 1 {
            // {vr,vp,vm} is trailing zeros if {mv,mp,mm} has at least q trailing 0 bits.
            // mv = 4 * m2, so it always has at least two trailing 0 bits.
            vr_is_trailing_zeros = true;
            if accept_bounds {
                // mm = mv - 1 - mm_shift, so it has 1 trailing 0 bit iff mm_shift == 1.
                vm_is_trailing_zeros = mm_shift == 1;
            } else {
                // mp = mv + 2, so it always has at least one trailing 0 bit.
                vp -= 1;
            }
        } else if q < 63 {
            // TODO(ulfjack): Use a tighter bound here.
            // We want to know if the full product has at least q trailing zeros.
            // We need to compute min(p2(mv), p5(mv) - e2) >= q
            // <=> p2(mv) >= q && p5(mv) - e2 >= q
            // <=> p2(mv) >= q (because -e2 >= q)
            vr_is_trailing_zeros = multiple_of_power_of_2(mv, q);
        }
    }

    // Step 4: Find the shortest decimal representation in the interval of valid representations.
    let mut removed = 0i32;
    let mut last_removed_digit = 0u8;
    // On average, we remove ~2 digits.
    let output = if vm_is_trailing_zeros || vr_is_trailing_zeros {
        // General case, which happens rarely (~0.7%).
        loop {
            let vp_div10 = div10(vp);
            let vm_div10 = div10(vm);
            if vp_div10 <= vm_div10 {
                break;
            }
            let vm_mod10 = (vm as u32).wrapping_sub(10u32.wrapping_mul(vm_div10 as u32));
            let vr_div10 = div10(vr);
            let vr_mod10 = (vr as u32).wrapping_sub(10u32.wrapping_mul(vr_div10 as u32));
            vm_is_trailing_zeros &= vm_mod10 == 0;
            vr_is_trailing_zeros &= last_removed_digit == 0;
            last_removed_digit = vr_mod10 as u8;
            vr = vr_div10;
            vp = vp_div10;
            vm = vm_div10;
            removed += 1;
        }
        if vm_is_trailing_zeros {
            loop {
                let vm_div10 = div10(vm);
                let vm_mod10 = (vm as u32).wrapping_sub(10u32.wrapping_mul(vm_div10 as u32));
                if vm_mod10 != 0 {
                    break;
                }
                let vp_div10 = div10(vp);
                let vr_div10 = div10(vr);
                let vr_mod10 = (vr as u32).wrapping_sub(10u32.wrapping_mul(vr_div10 as u32));
                vr_is_trailing_zeros &= last_removed_digit == 0;
                last_removed_digit = vr_mod10 as u8;
                vr = vr_div10;
                vp = vp_div10;
                vm = vm_div10;
                removed += 1;
            }
        }
        if vr_is_trailing_zeros && last_removed_digit == 5 && vr % 2 == 0 {
            // Round even if the exact number is .....50..0.
            last_removed_digit = 4;
        }
        // We need to take vr + 1 if vr is outside bounds or we need to round up.
        vr + ((vr == vm && (!accept_bounds || !vm_is_trailing_zeros)) || last_removed_digit >= 5)
            as u64
    } else {
        // Specialized for the common case (~99.3%). Percentages below are relative to this.
        let mut round_up = false;
        let vp_div100 = div100(vp);
        let vm_div100 = div100(vm);
        // Optimization: remove two digits at a time (~86.2%).
        if vp_div100 > vm_div100 {
            let vr_div100 = div100(vr);
            let vr_mod100 = (vr as u32).wrapping_sub(100u32.wrapping_mul(vr_div100 as u32));
            round_up = vr_mod100 >= 50;
            vr = vr_div100;
            vp = vp_div100;
            vm = vm_div100;
            removed += 2;
        }
        // Loop iterations below (approximately), without optimization above:
        // 0: 0.03%, 1: 13.8%, 2: 70.6%, 3: 14.0%, 4: 1.40%, 5: 0.14%, 6+: 0.02%
        // Loop iterations below (approximately), with optimization above:
        // 0: 70.6%, 1: 27.8%, 2: 1.40%, 3: 0.14%, 4+: 0.02%
        loop {
            let vp_div10 = div10(vp);
            let vm_div10 = div10(vm);
            if vp_div10 <= vm_div10 {
                break;
            }
            let vr_div10 = div10(vr);
            let vr_mod10 = (vr as u32).wrapping_sub(10u32.wrapping_mul(vr_div10 as u32));
            round_up = vr_mod10 >= 5;
            vr = vr_div10;
            vp = vp_div10;
            vm = vm_div10;
            removed += 1;
        }
        // We need to take vr + 1 if vr is outside bounds or we need to round up.
        vr + (vr == vm || round_up) as u64
    };
    let exp = e10 + removed;

    FloatingDecimal64 {
        exponent: exp,
        mantissa: output,
    }
}
