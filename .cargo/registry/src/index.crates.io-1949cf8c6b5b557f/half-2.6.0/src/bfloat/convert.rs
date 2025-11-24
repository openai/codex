use crate::leading_zeros::leading_zeros_u16;
use core::mem;

#[inline]
pub(crate) const fn f32_to_bf16(value: f32) -> u16 {
    // TODO: Replace mem::transmute with to_bits() once to_bits is const-stabilized
    // Convert to raw bytes
    let x: u32 = unsafe { mem::transmute::<f32, u32>(value) };

    // check for NaN
    if x & 0x7FFF_FFFFu32 > 0x7F80_0000u32 {
        // Keep high part of current mantissa but also set most significiant mantissa bit
        return ((x >> 16) | 0x0040u32) as u16;
    }

    // round and shift
    let round_bit = 0x0000_8000u32;
    if (x & round_bit) != 0 && (x & (3 * round_bit - 1)) != 0 {
        (x >> 16) as u16 + 1
    } else {
        (x >> 16) as u16
    }
}

#[inline]
pub(crate) const fn f64_to_bf16(value: f64) -> u16 {
    // TODO: Replace mem::transmute with to_bits() once to_bits is const-stabilized
    // Convert to raw bytes, truncating the last 32-bits of mantissa; that precision will always
    // be lost on half-precision.
    let val: u64 = unsafe { mem::transmute::<f64, u64>(value) };
    let x = (val >> 32) as u32;

    // Extract IEEE754 components
    let sign = x & 0x8000_0000u32;
    let exp = x & 0x7FF0_0000u32;
    let man = x & 0x000F_FFFFu32;

    // Check for all exponent bits being set, which is Infinity or NaN
    if exp == 0x7FF0_0000u32 {
        // Set mantissa MSB for NaN (and also keep shifted mantissa bits).
        // We also have to check the last 32 bits.
        let nan_bit = if man == 0 && (val as u32 == 0) {
            0
        } else {
            0x0040u32
        };
        return ((sign >> 16) | 0x7F80u32 | nan_bit | (man >> 13)) as u16;
    }

    // The number is normalized, start assembling half precision version
    let half_sign = sign >> 16;
    // Unbias the exponent, then bias for bfloat16 precision
    let unbiased_exp = ((exp >> 20) as i64) - 1023;
    let half_exp = unbiased_exp + 127;

    // Check for exponent overflow, return +infinity
    if half_exp >= 0xFF {
        return (half_sign | 0x7F80u32) as u16;
    }

    // Check for underflow
    if half_exp <= 0 {
        // Check mantissa for what we can do
        if 7 - half_exp > 21 {
            // No rounding possibility, so this is a full underflow, return signed zero
            return half_sign as u16;
        }
        // Don't forget about hidden leading mantissa bit when assembling mantissa
        let man = man | 0x0010_0000u32;
        let mut half_man = man >> (14 - half_exp);
        // Check for rounding
        let round_bit = 1 << (13 - half_exp);
        if (man & round_bit) != 0 && (man & (3 * round_bit - 1)) != 0 {
            half_man += 1;
        }
        // No exponent for subnormals
        return (half_sign | half_man) as u16;
    }

    // Rebias the exponent
    let half_exp = (half_exp as u32) << 7;
    let half_man = man >> 13;
    // Check for rounding
    let round_bit = 0x0000_1000u32;
    if (man & round_bit) != 0 && (man & (3 * round_bit - 1)) != 0 {
        // Round it
        ((half_sign | half_exp | half_man) + 1) as u16
    } else {
        (half_sign | half_exp | half_man) as u16
    }
}

#[inline]
pub(crate) const fn bf16_to_f32(i: u16) -> f32 {
    // TODO: Replace mem::transmute with from_bits() once from_bits is const-stabilized
    // If NaN, keep current mantissa but also set most significiant mantissa bit
    if i & 0x7FFFu16 > 0x7F80u16 {
        unsafe { mem::transmute::<u32, f32>((i as u32 | 0x0040u32) << 16) }
    } else {
        unsafe { mem::transmute::<u32, f32>((i as u32) << 16) }
    }
}

#[inline]
pub(crate) const fn bf16_to_f64(i: u16) -> f64 {
    // TODO: Replace mem::transmute with from_bits() once from_bits is const-stabilized
    // Check for signed zero
    if i & 0x7FFFu16 == 0 {
        return unsafe { mem::transmute::<u64, f64>((i as u64) << 48) };
    }

    let half_sign = (i & 0x8000u16) as u64;
    let half_exp = (i & 0x7F80u16) as u64;
    let half_man = (i & 0x007Fu16) as u64;

    // Check for an infinity or NaN when all exponent bits set
    if half_exp == 0x7F80u64 {
        // Check for signed infinity if mantissa is zero
        if half_man == 0 {
            return unsafe {
                mem::transmute::<u64, f64>((half_sign << 48) | 0x7FF0_0000_0000_0000u64)
            };
        } else {
            // NaN, keep current mantissa but also set most significiant mantissa bit
            return unsafe {
                mem::transmute::<u64, f64>(
                    (half_sign << 48) | 0x7FF8_0000_0000_0000u64 | (half_man << 45),
                )
            };
        }
    }

    // Calculate double-precision components with adjusted exponent
    let sign = half_sign << 48;
    // Unbias exponent
    let unbiased_exp = ((half_exp as i64) >> 7) - 127;

    // Check for subnormals, which will be normalized by adjusting exponent
    if half_exp == 0 {
        // Calculate how much to adjust the exponent by
        let e = leading_zeros_u16(half_man as u16) - 9;

        // Rebias and adjust exponent
        let exp = ((1023 - 127 - e) as u64) << 52;
        let man = (half_man << (46 + e)) & 0xF_FFFF_FFFF_FFFFu64;
        return unsafe { mem::transmute::<u64, f64>(sign | exp | man) };
    }
    // Rebias exponent for a normalized normal
    let exp = ((unbiased_exp + 1023) as u64) << 52;
    let man = (half_man & 0x007Fu64) << 45;
    unsafe { mem::transmute::<u64, f64>(sign | exp | man) }
}
