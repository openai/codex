//! A small number of math routines for floats and doubles.
//!
//! These are adapted from libm, a port of musl libc's libm to Rust.
//! libm can be found online [here](https://github.com/rust-lang/libm),
//! and is similarly licensed under an Apache2.0/MIT license

#![cfg(all(not(feature = "std"), feature = "compact"))]
#![doc(hidden)]

/* origin: FreeBSD /usr/src/lib/msun/src/e_powf.c */
/*
 * Conversion to float by Ian Lance Taylor, Cygnus Support, ian@cygnus.com.
 */
/*
 * ====================================================
 * Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
 *
 * Developed at SunPro, a Sun Microsystems, Inc. business.
 * Permission to use, copy, modify, and distribute this
 * software is freely granted, provided that this notice
 * is preserved.
 * ====================================================
 */

/// # Safety
///
/// Safe if `index < array.len()`.
macro_rules! i {
    ($array:ident, $index:expr) => {
        // SAFETY: safe if `index < array.len()`.
        unsafe { *$array.get_unchecked($index) }
    };
}

pub fn powf(x: f32, y: f32) -> f32 {
    const BP: [f32; 2] = [1.0, 1.5];
    const DP_H: [f32; 2] = [0.0, 5.84960938e-01]; /* 0x3f15c000 */
    const DP_L: [f32; 2] = [0.0, 1.56322085e-06]; /* 0x35d1cfdc */
    const TWO24: f32 = 16777216.0; /* 0x4b800000 */
    const HUGE: f32 = 1.0e30;
    const TINY: f32 = 1.0e-30;
    const L1: f32 = 6.0000002384e-01; /* 0x3f19999a */
    const L2: f32 = 4.2857143283e-01; /* 0x3edb6db7 */
    const L3: f32 = 3.3333334327e-01; /* 0x3eaaaaab */
    const L4: f32 = 2.7272811532e-01; /* 0x3e8ba305 */
    const L5: f32 = 2.3066075146e-01; /* 0x3e6c3255 */
    const L6: f32 = 2.0697501302e-01; /* 0x3e53f142 */
    const P1: f32 = 1.6666667163e-01; /* 0x3e2aaaab */
    const P2: f32 = -2.7777778450e-03; /* 0xbb360b61 */
    const P3: f32 = 6.6137559770e-05; /* 0x388ab355 */
    const P4: f32 = -1.6533901999e-06; /* 0xb5ddea0e */
    const P5: f32 = 4.1381369442e-08; /* 0x3331bb4c */
    const LG2: f32 = 6.9314718246e-01; /* 0x3f317218 */
    const LG2_H: f32 = 6.93145752e-01; /* 0x3f317200 */
    const LG2_L: f32 = 1.42860654e-06; /* 0x35bfbe8c */
    const OVT: f32 = 4.2995665694e-08; /* -(128-log2(ovfl+.5ulp)) */
    const CP: f32 = 9.6179670095e-01; /* 0x3f76384f =2/(3ln2) */
    const CP_H: f32 = 9.6191406250e-01; /* 0x3f764000 =12b cp */
    const CP_L: f32 = -1.1736857402e-04; /* 0xb8f623c6 =tail of cp_h */
    const IVLN2: f32 = 1.4426950216e+00;
    const IVLN2_H: f32 = 1.4426879883e+00;
    const IVLN2_L: f32 = 7.0526075433e-06;

    let mut z: f32;
    let mut ax: f32;
    let z_h: f32;
    let z_l: f32;
    let mut p_h: f32;
    let mut p_l: f32;
    let y1: f32;
    let mut t1: f32;
    let t2: f32;
    let mut r: f32;
    let s: f32;
    let mut sn: f32;
    let mut t: f32;
    let mut u: f32;
    let mut v: f32;
    let mut w: f32;
    let i: i32;
    let mut j: i32;
    let mut k: i32;
    let mut yisint: i32;
    let mut n: i32;
    let hx: i32;
    let hy: i32;
    let mut ix: i32;
    let iy: i32;
    let mut is: i32;

    hx = x.to_bits() as i32;
    hy = y.to_bits() as i32;

    ix = hx & 0x7fffffff;
    iy = hy & 0x7fffffff;

    /* x**0 = 1, even if x is NaN */
    if iy == 0 {
        return 1.0;
    }

    /* 1**y = 1, even if y is NaN */
    if hx == 0x3f800000 {
        return 1.0;
    }

    /* NaN if either arg is NaN */
    if ix > 0x7f800000 || iy > 0x7f800000 {
        return x + y;
    }

    /* determine if y is an odd int when x < 0
     * yisint = 0       ... y is not an integer
     * yisint = 1       ... y is an odd int
     * yisint = 2       ... y is an even int
     */
    yisint = 0;
    if hx < 0 {
        if iy >= 0x4b800000 {
            yisint = 2; /* even integer y */
        } else if iy >= 0x3f800000 {
            k = (iy >> 23) - 0x7f; /* exponent */
            j = iy >> (23 - k);
            if (j << (23 - k)) == iy {
                yisint = 2 - (j & 1);
            }
        }
    }

    /* special value of y */
    if iy == 0x7f800000 {
        /* y is +-inf */
        if ix == 0x3f800000 {
            /* (-1)**+-inf is 1 */
            return 1.0;
        } else if ix > 0x3f800000 {
            /* (|x|>1)**+-inf = inf,0 */
            return if hy >= 0 {
                y
            } else {
                0.0
            };
        } else {
            /* (|x|<1)**+-inf = 0,inf */
            return if hy >= 0 {
                0.0
            } else {
                -y
            };
        }
    }
    if iy == 0x3f800000 {
        /* y is +-1 */
        return if hy >= 0 {
            x
        } else {
            1.0 / x
        };
    }

    if hy == 0x40000000 {
        /* y is 2 */
        return x * x;
    }

    if hy == 0x3f000000
       /* y is  0.5 */
       && hx >= 0
    {
        /* x >= +0 */
        return sqrtf(x);
    }

    ax = fabsf(x);
    /* special value of x */
    if ix == 0x7f800000 || ix == 0 || ix == 0x3f800000 {
        /* x is +-0,+-inf,+-1 */
        z = ax;
        if hy < 0 {
            /* z = (1/|x|) */
            z = 1.0 / z;
        }

        if hx < 0 {
            if ((ix - 0x3f800000) | yisint) == 0 {
                z = (z - z) / (z - z); /* (-1)**non-int is NaN */
            } else if yisint == 1 {
                z = -z; /* (x<0)**odd = -(|x|**odd) */
            }
        }
        return z;
    }

    sn = 1.0; /* sign of result */
    if hx < 0 {
        if yisint == 0 {
            /* (x<0)**(non-int) is NaN */
            return (x - x) / (x - x);
        }

        if yisint == 1 {
            /* (x<0)**(odd int) */
            sn = -1.0;
        }
    }

    /* |y| is HUGE */
    if iy > 0x4d000000 {
        /* if |y| > 2**27 */
        /* over/underflow if x is not close to one */
        if ix < 0x3f7ffff8 {
            return if hy < 0 {
                sn * HUGE * HUGE
            } else {
                sn * TINY * TINY
            };
        }

        if ix > 0x3f800007 {
            return if hy > 0 {
                sn * HUGE * HUGE
            } else {
                sn * TINY * TINY
            };
        }

        /* now |1-x| is TINY <= 2**-20, suffice to compute
        log(x) by x-x^2/2+x^3/3-x^4/4 */
        t = ax - 1.; /* t has 20 trailing zeros */
        w = (t * t) * (0.5 - t * (0.333333333333 - t * 0.25));
        u = IVLN2_H * t; /* IVLN2_H has 16 sig. bits */
        v = t * IVLN2_L - w * IVLN2;
        t1 = u + v;
        is = t1.to_bits() as i32;
        t1 = f32::from_bits(is as u32 & 0xfffff000);
        t2 = v - (t1 - u);
    } else {
        let mut s2: f32;
        let mut s_h: f32;
        let s_l: f32;
        let mut t_h: f32;
        let mut t_l: f32;

        n = 0;
        /* take care subnormal number */
        if ix < 0x00800000 {
            ax *= TWO24;
            n -= 24;
            ix = ax.to_bits() as i32;
        }
        n += ((ix) >> 23) - 0x7f;
        j = ix & 0x007fffff;
        /* determine interval */
        ix = j | 0x3f800000; /* normalize ix */
        if j <= 0x1cc471 {
            /* |x|<sqrt(3/2) */
            k = 0;
        } else if j < 0x5db3d7 {
            /* |x|<sqrt(3)   */
            k = 1;
        } else {
            k = 0;
            n += 1;
            ix -= 0x00800000;
        }
        ax = f32::from_bits(ix as u32);

        /* compute s = s_h+s_l = (x-1)/(x+1) or (x-1.5)/(x+1.5) */
        u = ax - i!(BP, k as usize); /* bp[0]=1.0, bp[1]=1.5 */
        v = 1.0 / (ax + i!(BP, k as usize));
        s = u * v;
        s_h = s;
        is = s_h.to_bits() as i32;
        s_h = f32::from_bits(is as u32 & 0xfffff000);
        /* t_h=ax+bp[k] High */
        is = (((ix as u32 >> 1) & 0xfffff000) | 0x20000000) as i32;
        t_h = f32::from_bits(is as u32 + 0x00400000 + ((k as u32) << 21));
        t_l = ax - (t_h - i!(BP, k as usize));
        s_l = v * ((u - s_h * t_h) - s_h * t_l);
        /* compute log(ax) */
        s2 = s * s;
        r = s2 * s2 * (L1 + s2 * (L2 + s2 * (L3 + s2 * (L4 + s2 * (L5 + s2 * L6)))));
        r += s_l * (s_h + s);
        s2 = s_h * s_h;
        t_h = 3.0 + s2 + r;
        is = t_h.to_bits() as i32;
        t_h = f32::from_bits(is as u32 & 0xfffff000);
        t_l = r - ((t_h - 3.0) - s2);
        /* u+v = s*(1+...) */
        u = s_h * t_h;
        v = s_l * t_h + t_l * s;
        /* 2/(3log2)*(s+...) */
        p_h = u + v;
        is = p_h.to_bits() as i32;
        p_h = f32::from_bits(is as u32 & 0xfffff000);
        p_l = v - (p_h - u);
        z_h = CP_H * p_h; /* cp_h+cp_l = 2/(3*log2) */
        z_l = CP_L * p_h + p_l * CP + i!(DP_L, k as usize);
        /* log2(ax) = (s+..)*2/(3*log2) = n + dp_h + z_h + z_l */
        t = n as f32;
        t1 = ((z_h + z_l) + i!(DP_H, k as usize)) + t;
        is = t1.to_bits() as i32;
        t1 = f32::from_bits(is as u32 & 0xfffff000);
        t2 = z_l - (((t1 - t) - i!(DP_H, k as usize)) - z_h);
    };

    /* split up y into y1+y2 and compute (y1+y2)*(t1+t2) */
    is = y.to_bits() as i32;
    y1 = f32::from_bits(is as u32 & 0xfffff000);
    p_l = (y - y1) * t1 + y * t2;
    p_h = y1 * t1;
    z = p_l + p_h;
    j = z.to_bits() as i32;
    if j > 0x43000000 {
        /* if z > 128 */
        return sn * HUGE * HUGE; /* overflow */
    } else if j == 0x43000000 {
        /* if z == 128 */
        if p_l + OVT > z - p_h {
            return sn * HUGE * HUGE; /* overflow */
        }
    } else if (j & 0x7fffffff) > 0x43160000 {
        /* z < -150 */
        // FIXME: check should be  (uint32_t)j > 0xc3160000
        return sn * TINY * TINY; /* underflow */
    } else if j as u32 == 0xc3160000
              /* z == -150 */
              && p_l <= z - p_h
    {
        return sn * TINY * TINY; /* underflow */
    }

    /*
     * compute 2**(p_h+p_l)
     */
    i = j & 0x7fffffff;
    k = (i >> 23) - 0x7f;
    n = 0;
    if i > 0x3f000000 {
        /* if |z| > 0.5, set n = [z+0.5] */
        n = j + (0x00800000 >> (k + 1));
        k = ((n & 0x7fffffff) >> 23) - 0x7f; /* new k for n */
        t = f32::from_bits(n as u32 & !(0x007fffff >> k));
        n = ((n & 0x007fffff) | 0x00800000) >> (23 - k);
        if j < 0 {
            n = -n;
        }
        p_h -= t;
    }
    t = p_l + p_h;
    is = t.to_bits() as i32;
    t = f32::from_bits(is as u32 & 0xffff8000);
    u = t * LG2_H;
    v = (p_l - (t - p_h)) * LG2 + t * LG2_L;
    z = u + v;
    w = v - (z - u);
    t = z * z;
    t1 = z - t * (P1 + t * (P2 + t * (P3 + t * (P4 + t * P5))));
    r = (z * t1) / (t1 - 2.0) - (w + z * w);
    z = 1.0 - (r - z);
    j = z.to_bits() as i32;
    j += n << 23;
    if (j >> 23) <= 0 {
        /* subnormal output */
        z = scalbnf(z, n);
    } else {
        z = f32::from_bits(j as u32);
    }
    sn * z
}

/* origin: FreeBSD /usr/src/lib/msun/src/e_sqrtf.c */
/*
 * Conversion to float by Ian Lance Taylor, Cygnus Support, ian@cygnus.com.
 */
/*
 * ====================================================
 * Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
 *
 * Developed at SunPro, a Sun Microsystems, Inc. business.
 * Permission to use, copy, modify, and distribute this
 * software is freely granted, provided that this notice
 * is preserved.
 * ====================================================
 */

pub fn sqrtf(x: f32) -> f32 {
    #[cfg(target_feature = "sse")]
    {
        // Note: This path is unlikely since LLVM will usually have already
        // optimized sqrt calls into hardware instructions if sse is available,
        // but if someone does end up here they'll apprected the speed increase.
        #[cfg(target_arch = "x86")]
        use core::arch::x86::*;
        #[cfg(target_arch = "x86_64")]
        use core::arch::x86_64::*;
        // SAFETY: safe, since `_mm_set_ss` takes a 32-bit float, and returns
        // a 128-bit type with the lowest 32-bits as `x`, `_mm_sqrt_ss` calculates
        // the sqrt of this 128-bit vector, and `_mm_cvtss_f32` extracts the lower
        // 32-bits as a 32-bit float.
        unsafe {
            let m = _mm_set_ss(x);
            let m_sqrt = _mm_sqrt_ss(m);
            _mm_cvtss_f32(m_sqrt)
        }
    }
    #[cfg(not(target_feature = "sse"))]
    {
        const TINY: f32 = 1.0e-30;

        let mut z: f32;
        let sign: i32 = 0x80000000u32 as i32;
        let mut ix: i32;
        let mut s: i32;
        let mut q: i32;
        let mut m: i32;
        let mut t: i32;
        let mut i: i32;
        let mut r: u32;

        ix = x.to_bits() as i32;

        /* take care of Inf and NaN */
        if (ix as u32 & 0x7f800000) == 0x7f800000 {
            return x * x + x; /* sqrt(NaN)=NaN, sqrt(+inf)=+inf, sqrt(-inf)=sNaN */
        }

        /* take care of zero */
        if ix <= 0 {
            if (ix & !sign) == 0 {
                return x; /* sqrt(+-0) = +-0 */
            }
            if ix < 0 {
                return (x - x) / (x - x); /* sqrt(-ve) = sNaN */
            }
        }

        /* normalize x */
        m = ix >> 23;
        if m == 0 {
            /* subnormal x */
            i = 0;
            while ix & 0x00800000 == 0 {
                ix <<= 1;
                i = i + 1;
            }
            m -= i - 1;
        }
        m -= 127; /* unbias exponent */
        ix = (ix & 0x007fffff) | 0x00800000;
        if m & 1 == 1 {
            /* odd m, double x to make it even */
            ix += ix;
        }
        m >>= 1; /* m = [m/2] */

        /* generate sqrt(x) bit by bit */
        ix += ix;
        q = 0;
        s = 0;
        r = 0x01000000; /* r = moving bit from right to left */

        while r != 0 {
            t = s + r as i32;
            if t <= ix {
                s = t + r as i32;
                ix -= t;
                q += r as i32;
            }
            ix += ix;
            r >>= 1;
        }

        /* use floating add to find out rounding direction */
        if ix != 0 {
            z = 1.0 - TINY; /* raise inexact flag */
            if z >= 1.0 {
                z = 1.0 + TINY;
                if z > 1.0 {
                    q += 2;
                } else {
                    q += q & 1;
                }
            }
        }

        ix = (q >> 1) + 0x3f000000;
        ix += m << 23;
        f32::from_bits(ix as u32)
    }
}

/// Absolute value (magnitude) (f32)
/// Calculates the absolute value (magnitude) of the argument `x`,
/// by direct manipulation of the bit representation of `x`.
pub fn fabsf(x: f32) -> f32 {
    f32::from_bits(x.to_bits() & 0x7fffffff)
}

pub fn scalbnf(mut x: f32, mut n: i32) -> f32 {
    let x1p127 = f32::from_bits(0x7f000000); // 0x1p127f === 2 ^ 127
    let x1p_126 = f32::from_bits(0x800000); // 0x1p-126f === 2 ^ -126
    let x1p24 = f32::from_bits(0x4b800000); // 0x1p24f === 2 ^ 24

    if n > 127 {
        x *= x1p127;
        n -= 127;
        if n > 127 {
            x *= x1p127;
            n -= 127;
            if n > 127 {
                n = 127;
            }
        }
    } else if n < -126 {
        x *= x1p_126 * x1p24;
        n += 126 - 24;
        if n < -126 {
            x *= x1p_126 * x1p24;
            n += 126 - 24;
            if n < -126 {
                n = -126;
            }
        }
    }
    x * f32::from_bits(((0x7f + n) as u32) << 23)
}

/* origin: FreeBSD /usr/src/lib/msun/src/e_pow.c */
/*
 * ====================================================
 * Copyright (C) 2004 by Sun Microsystems, Inc. All rights reserved.
 *
 * Permission to use, copy, modify, and distribute this
 * software is freely granted, provided that this notice
 * is preserved.
 * ====================================================
 */

// pow(x,y) return x**y
//
//                    n
// Method:  Let x =  2   * (1+f)
//      1. Compute and return log2(x) in two pieces:
//              log2(x) = w1 + w2,
//         where w1 has 53-24 = 29 bit trailing zeros.
//      2. Perform y*log2(x) = n+y' by simulating muti-precision
//         arithmetic, where |y'|<=0.5.
//      3. Return x**y = 2**n*exp(y'*log2)
//
// Special cases:
//      1.  (anything) ** 0  is 1
//      2.  1 ** (anything)  is 1
//      3.  (anything except 1) ** NAN is NAN
//      4.  NAN ** (anything except 0) is NAN
//      5.  +-(|x| > 1) **  +INF is +INF
//      6.  +-(|x| > 1) **  -INF is +0
//      7.  +-(|x| < 1) **  +INF is +0
//      8.  +-(|x| < 1) **  -INF is +INF
//      9.  -1          ** +-INF is 1
//      10. +0 ** (+anything except 0, NAN)               is +0
//      11. -0 ** (+anything except 0, NAN, odd integer)  is +0
//      12. +0 ** (-anything except 0, NAN)               is +INF, raise divbyzero
//      13. -0 ** (-anything except 0, NAN, odd integer)  is +INF, raise divbyzero
//      14. -0 ** (+odd integer) is -0
//      15. -0 ** (-odd integer) is -INF, raise divbyzero
//      16. +INF ** (+anything except 0,NAN) is +INF
//      17. +INF ** (-anything except 0,NAN) is +0
//      18. -INF ** (+odd integer) is -INF
//      19. -INF ** (anything) = -0 ** (-anything), (anything except odd integer)
//      20. (anything) ** 1 is (anything)
//      21. (anything) ** -1 is 1/(anything)
//      22. (-anything) ** (integer) is (-1)**(integer)*(+anything**integer)
//      23. (-anything except 0 and inf) ** (non-integer) is NAN
//
// Accuracy:
//      pow(x,y) returns x**y nearly rounded. In particular
//                      pow(integer,integer)
//      always returns the correct integer provided it is
//      representable.
//
// Constants :
// The hexadecimal values are the intended ones for the following
// constants. The decimal values may be used, provided that the
// compiler will convert from decimal to binary accurately enough
// to produce the hexadecimal values shown.

pub fn powd(x: f64, y: f64) -> f64 {
    const BP: [f64; 2] = [1.0, 1.5];
    const DP_H: [f64; 2] = [0.0, 5.84962487220764160156e-01]; /* 0x3fe2b803_40000000 */
    const DP_L: [f64; 2] = [0.0, 1.35003920212974897128e-08]; /* 0x3E4CFDEB, 0x43CFD006 */
    const TWO53: f64 = 9007199254740992.0; /* 0x43400000_00000000 */
    const HUGE: f64 = 1.0e300;
    const TINY: f64 = 1.0e-300;

    // poly coefs for (3/2)*(log(x)-2s-2/3*s**3:
    const L1: f64 = 5.99999999999994648725e-01; /* 0x3fe33333_33333303 */
    const L2: f64 = 4.28571428578550184252e-01; /* 0x3fdb6db6_db6fabff */
    const L3: f64 = 3.33333329818377432918e-01; /* 0x3fd55555_518f264d */
    const L4: f64 = 2.72728123808534006489e-01; /* 0x3fd17460_a91d4101 */
    const L5: f64 = 2.30660745775561754067e-01; /* 0x3fcd864a_93c9db65 */
    const L6: f64 = 2.06975017800338417784e-01; /* 0x3fca7e28_4a454eef */
    const P1: f64 = 1.66666666666666019037e-01; /* 0x3fc55555_5555553e */
    const P2: f64 = -2.77777777770155933842e-03; /* 0xbf66c16c_16bebd93 */
    const P3: f64 = 6.61375632143793436117e-05; /* 0x3f11566a_af25de2c */
    const P4: f64 = -1.65339022054652515390e-06; /* 0xbebbbd41_c5d26bf1 */
    const P5: f64 = 4.13813679705723846039e-08; /* 0x3e663769_72bea4d0 */
    const LG2: f64 = 6.93147180559945286227e-01; /* 0x3fe62e42_fefa39ef */
    const LG2_H: f64 = 6.93147182464599609375e-01; /* 0x3fe62e43_00000000 */
    const LG2_L: f64 = -1.90465429995776804525e-09; /* 0xbe205c61_0ca86c39 */
    const OVT: f64 = 8.0085662595372944372e-017; /* -(1024-log2(ovfl+.5ulp)) */
    const CP: f64 = 9.61796693925975554329e-01; /* 0x3feec709_dc3a03fd =2/(3ln2) */
    const CP_H: f64 = 9.61796700954437255859e-01; /* 0x3feec709_e0000000 =(float)cp */
    const CP_L: f64 = -7.02846165095275826516e-09; /* 0xbe3e2fe0_145b01f5 =tail of cp_h*/
    const IVLN2: f64 = 1.44269504088896338700e+00; /* 0x3ff71547_652b82fe =1/ln2 */
    const IVLN2_H: f64 = 1.44269502162933349609e+00; /* 0x3ff71547_60000000 =24b 1/ln2*/
    const IVLN2_L: f64 = 1.92596299112661746887e-08; /* 0x3e54ae0b_f85ddf44 =1/ln2 tail*/

    let t1: f64;
    let t2: f64;

    let (hx, lx): (i32, u32) = ((x.to_bits() >> 32) as i32, x.to_bits() as u32);
    let (hy, ly): (i32, u32) = ((y.to_bits() >> 32) as i32, y.to_bits() as u32);

    let mut ix: i32 = (hx & 0x7fffffff) as i32;
    let iy: i32 = (hy & 0x7fffffff) as i32;

    /* x**0 = 1, even if x is NaN */
    if ((iy as u32) | ly) == 0 {
        return 1.0;
    }

    /* 1**y = 1, even if y is NaN */
    if hx == 0x3ff00000 && lx == 0 {
        return 1.0;
    }

    /* NaN if either arg is NaN */
    if ix > 0x7ff00000
        || (ix == 0x7ff00000 && lx != 0)
        || iy > 0x7ff00000
        || (iy == 0x7ff00000 && ly != 0)
    {
        return x + y;
    }

    /* determine if y is an odd int when x < 0
     * yisint = 0       ... y is not an integer
     * yisint = 1       ... y is an odd int
     * yisint = 2       ... y is an even int
     */
    let mut yisint: i32 = 0;
    let mut k: i32;
    let mut j: i32;
    if hx < 0 {
        if iy >= 0x43400000 {
            yisint = 2; /* even integer y */
        } else if iy >= 0x3ff00000 {
            k = (iy >> 20) - 0x3ff; /* exponent */

            if k > 20 {
                j = (ly >> (52 - k)) as i32;

                if (j << (52 - k)) == (ly as i32) {
                    yisint = 2 - (j & 1);
                }
            } else if ly == 0 {
                j = iy >> (20 - k);

                if (j << (20 - k)) == iy {
                    yisint = 2 - (j & 1);
                }
            }
        }
    }

    if ly == 0 {
        /* special value of y */
        if iy == 0x7ff00000 {
            /* y is +-inf */

            return if ((ix - 0x3ff00000) | (lx as i32)) == 0 {
                /* (-1)**+-inf is 1 */
                1.0
            } else if ix >= 0x3ff00000 {
                /* (|x|>1)**+-inf = inf,0 */
                if hy >= 0 {
                    y
                } else {
                    0.0
                }
            } else {
                /* (|x|<1)**+-inf = 0,inf */
                if hy >= 0 {
                    0.0
                } else {
                    -y
                }
            };
        }

        if iy == 0x3ff00000 {
            /* y is +-1 */
            return if hy >= 0 {
                x
            } else {
                1.0 / x
            };
        }

        if hy == 0x40000000 {
            /* y is 2 */
            return x * x;
        }

        if hy == 0x3fe00000 {
            /* y is 0.5 */
            if hx >= 0 {
                /* x >= +0 */
                return sqrtd(x);
            }
        }
    }

    let mut ax: f64 = fabsd(x);
    if lx == 0 {
        /* special value of x */
        if ix == 0x7ff00000 || ix == 0 || ix == 0x3ff00000 {
            /* x is +-0,+-inf,+-1 */
            let mut z: f64 = ax;

            if hy < 0 {
                /* z = (1/|x|) */
                z = 1.0 / z;
            }

            if hx < 0 {
                if ((ix - 0x3ff00000) | yisint) == 0 {
                    z = (z - z) / (z - z); /* (-1)**non-int is NaN */
                } else if yisint == 1 {
                    z = -z; /* (x<0)**odd = -(|x|**odd) */
                }
            }

            return z;
        }
    }

    let mut s: f64 = 1.0; /* sign of result */
    if hx < 0 {
        if yisint == 0 {
            /* (x<0)**(non-int) is NaN */
            return (x - x) / (x - x);
        }

        if yisint == 1 {
            /* (x<0)**(odd int) */
            s = -1.0;
        }
    }

    /* |y| is HUGE */
    if iy > 0x41e00000 {
        /* if |y| > 2**31 */
        if iy > 0x43f00000 {
            /* if |y| > 2**64, must o/uflow */
            if ix <= 0x3fefffff {
                return if hy < 0 {
                    HUGE * HUGE
                } else {
                    TINY * TINY
                };
            }

            if ix >= 0x3ff00000 {
                return if hy > 0 {
                    HUGE * HUGE
                } else {
                    TINY * TINY
                };
            }
        }

        /* over/underflow if x is not close to one */
        if ix < 0x3fefffff {
            return if hy < 0 {
                s * HUGE * HUGE
            } else {
                s * TINY * TINY
            };
        }
        if ix > 0x3ff00000 {
            return if hy > 0 {
                s * HUGE * HUGE
            } else {
                s * TINY * TINY
            };
        }

        /* now |1-x| is TINY <= 2**-20, suffice to compute
        log(x) by x-x^2/2+x^3/3-x^4/4 */
        let t: f64 = ax - 1.0; /* t has 20 trailing zeros */
        let w: f64 = (t * t) * (0.5 - t * (0.3333333333333333333333 - t * 0.25));
        let u: f64 = IVLN2_H * t; /* ivln2_h has 21 sig. bits */
        let v: f64 = t * IVLN2_L - w * IVLN2;
        t1 = with_set_low_word(u + v, 0);
        t2 = v - (t1 - u);
    } else {
        // double ss,s2,s_h,s_l,t_h,t_l;
        let mut n: i32 = 0;

        if ix < 0x00100000 {
            /* take care subnormal number */
            ax *= TWO53;
            n -= 53;
            ix = get_high_word(ax) as i32;
        }

        n += (ix >> 20) - 0x3ff;
        j = ix & 0x000fffff;

        /* determine interval */
        let k: i32;
        ix = j | 0x3ff00000; /* normalize ix */
        if j <= 0x3988E {
            /* |x|<sqrt(3/2) */
            k = 0;
        } else if j < 0xBB67A {
            /* |x|<sqrt(3)   */
            k = 1;
        } else {
            k = 0;
            n += 1;
            ix -= 0x00100000;
        }
        ax = with_set_high_word(ax, ix as u32);

        /* compute ss = s_h+s_l = (x-1)/(x+1) or (x-1.5)/(x+1.5) */
        let u: f64 = ax - i!(BP, k as usize); /* bp[0]=1.0, bp[1]=1.5 */
        let v: f64 = 1.0 / (ax + i!(BP, k as usize));
        let ss: f64 = u * v;
        let s_h = with_set_low_word(ss, 0);

        /* t_h=ax+bp[k] High */
        let t_h: f64 = with_set_high_word(
            0.0,
            ((ix as u32 >> 1) | 0x20000000) + 0x00080000 + ((k as u32) << 18),
        );
        let t_l: f64 = ax - (t_h - i!(BP, k as usize));
        let s_l: f64 = v * ((u - s_h * t_h) - s_h * t_l);

        /* compute log(ax) */
        let s2: f64 = ss * ss;
        let mut r: f64 = s2 * s2 * (L1 + s2 * (L2 + s2 * (L3 + s2 * (L4 + s2 * (L5 + s2 * L6)))));
        r += s_l * (s_h + ss);
        let s2: f64 = s_h * s_h;
        let t_h: f64 = with_set_low_word(3.0 + s2 + r, 0);
        let t_l: f64 = r - ((t_h - 3.0) - s2);

        /* u+v = ss*(1+...) */
        let u: f64 = s_h * t_h;
        let v: f64 = s_l * t_h + t_l * ss;

        /* 2/(3log2)*(ss+...) */
        let p_h: f64 = with_set_low_word(u + v, 0);
        let p_l = v - (p_h - u);
        let z_h: f64 = CP_H * p_h; /* cp_h+cp_l = 2/(3*log2) */
        let z_l: f64 = CP_L * p_h + p_l * CP + i!(DP_L, k as usize);

        /* log2(ax) = (ss+..)*2/(3*log2) = n + dp_h + z_h + z_l */
        let t: f64 = n as f64;
        t1 = with_set_low_word(((z_h + z_l) + i!(DP_H, k as usize)) + t, 0);
        t2 = z_l - (((t1 - t) - i!(DP_H, k as usize)) - z_h);
    }

    /* split up y into y1+y2 and compute (y1+y2)*(t1+t2) */
    let y1: f64 = with_set_low_word(y, 0);
    let p_l: f64 = (y - y1) * t1 + y * t2;
    let mut p_h: f64 = y1 * t1;
    let z: f64 = p_l + p_h;
    let mut j: i32 = (z.to_bits() >> 32) as i32;
    let i: i32 = z.to_bits() as i32;
    // let (j, i): (i32, i32) = ((z.to_bits() >> 32) as i32, z.to_bits() as i32);

    if j >= 0x40900000 {
        /* z >= 1024 */
        if (j - 0x40900000) | i != 0 {
            /* if z > 1024 */
            return s * HUGE * HUGE; /* overflow */
        }

        if p_l + OVT > z - p_h {
            return s * HUGE * HUGE; /* overflow */
        }
    } else if (j & 0x7fffffff) >= 0x4090cc00 {
        /* z <= -1075 */
        // FIXME: instead of abs(j) use unsigned j

        if (((j as u32) - 0xc090cc00) | (i as u32)) != 0 {
            /* z < -1075 */
            return s * TINY * TINY; /* underflow */
        }

        if p_l <= z - p_h {
            return s * TINY * TINY; /* underflow */
        }
    }

    /* compute 2**(p_h+p_l) */
    let i: i32 = j & (0x7fffffff as i32);
    k = (i >> 20) - 0x3ff;
    let mut n: i32 = 0;

    if i > 0x3fe00000 {
        /* if |z| > 0.5, set n = [z+0.5] */
        n = j + (0x00100000 >> (k + 1));
        k = ((n & 0x7fffffff) >> 20) - 0x3ff; /* new k for n */
        let t: f64 = with_set_high_word(0.0, (n & !(0x000fffff >> k)) as u32);
        n = ((n & 0x000fffff) | 0x00100000) >> (20 - k);
        if j < 0 {
            n = -n;
        }
        p_h -= t;
    }

    let t: f64 = with_set_low_word(p_l + p_h, 0);
    let u: f64 = t * LG2_H;
    let v: f64 = (p_l - (t - p_h)) * LG2 + t * LG2_L;
    let mut z: f64 = u + v;
    let w: f64 = v - (z - u);
    let t: f64 = z * z;
    let t1: f64 = z - t * (P1 + t * (P2 + t * (P3 + t * (P4 + t * P5))));
    let r: f64 = (z * t1) / (t1 - 2.0) - (w + z * w);
    z = 1.0 - (r - z);
    j = get_high_word(z) as i32;
    j += n << 20;

    if (j >> 20) <= 0 {
        /* subnormal output */
        z = scalbnd(z, n);
    } else {
        z = with_set_high_word(z, j as u32);
    }

    s * z
}

/// Absolute value (magnitude) (f64)
/// Calculates the absolute value (magnitude) of the argument `x`,
/// by direct manipulation of the bit representation of `x`.
pub fn fabsd(x: f64) -> f64 {
    f64::from_bits(x.to_bits() & (u64::MAX / 2))
}

pub fn scalbnd(x: f64, mut n: i32) -> f64 {
    let x1p1023 = f64::from_bits(0x7fe0000000000000); // 0x1p1023 === 2 ^ 1023
    let x1p53 = f64::from_bits(0x4340000000000000); // 0x1p53 === 2 ^ 53
    let x1p_1022 = f64::from_bits(0x0010000000000000); // 0x1p-1022 === 2 ^ (-1022)

    let mut y = x;

    if n > 1023 {
        y *= x1p1023;
        n -= 1023;
        if n > 1023 {
            y *= x1p1023;
            n -= 1023;
            if n > 1023 {
                n = 1023;
            }
        }
    } else if n < -1022 {
        /* make sure final n < -53 to avoid double
        rounding in the subnormal range */
        y *= x1p_1022 * x1p53;
        n += 1022 - 53;
        if n < -1022 {
            y *= x1p_1022 * x1p53;
            n += 1022 - 53;
            if n < -1022 {
                n = -1022;
            }
        }
    }
    y * f64::from_bits(((0x3ff + n) as u64) << 52)
}

/* origin: FreeBSD /usr/src/lib/msun/src/e_sqrt.c */
/*
 * ====================================================
 * Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
 *
 * Developed at SunSoft, a Sun Microsystems, Inc. business.
 * Permission to use, copy, modify, and distribute this
 * software is freely granted, provided that this notice
 * is preserved.
 * ====================================================
 */
/* sqrt(x)
 * Return correctly rounded sqrt.
 *           ------------------------------------------
 *           |  Use the hardware sqrt if you have one |
 *           ------------------------------------------
 * Method:
 *   Bit by bit method using integer arithmetic. (Slow, but portable)
 *   1. Normalization
 *      Scale x to y in [1,4) with even powers of 2:
 *      find an integer k such that  1 <= (y=x*2^(2k)) < 4, then
 *              sqrt(x) = 2^k * sqrt(y)
 *   2. Bit by bit computation
 *      Let q  = sqrt(y) truncated to i bit after binary point (q = 1),
 *           i                                                   0
 *                                     i+1         2
 *          s  = 2*q , and      y  =  2   * ( y - q  ).         (1)
 *           i      i            i                 i
 *
 *      To compute q    from q , one checks whether
 *                  i+1       i
 *
 *                            -(i+1) 2
 *                      (q + 2      ) <= y.                     (2)
 *                        i
 *                                                            -(i+1)
 *      If (2) is false, then q   = q ; otherwise q   = q  + 2      .
 *                             i+1   i             i+1   i
 *
 *      With some algebraic manipulation, it is not difficult to see
 *      that (2) is equivalent to
 *                             -(i+1)
 *                      s  +  2       <= y                      (3)
 *                       i                i
 *
 *      The advantage of (3) is that s  and y  can be computed by
 *                                    i      i
 *      the following recurrence formula:
 *          if (3) is false
 *
 *          s     =  s  ,       y    = y   ;                    (4)
 *           i+1      i          i+1    i
 *
 *          otherwise,
 *                         -i                     -(i+1)
 *          s     =  s  + 2  ,  y    = y  -  s  - 2             (5)
 *           i+1      i          i+1    i     i
 *
 *      One may easily use induction to prove (4) and (5).
 *      Note. Since the left hand side of (3) contain only i+2 bits,
 *            it does not necessary to do a full (53-bit) comparison
 *            in (3).
 *   3. Final rounding
 *      After generating the 53 bits result, we compute one more bit.
 *      Together with the remainder, we can decide whether the
 *      result is exact, bigger than 1/2ulp, or less than 1/2ulp
 *      (it will never equal to 1/2ulp).
 *      The rounding mode can be detected by checking whether
 *      huge + tiny is equal to huge, and whether huge - tiny is
 *      equal to huge for some floating point number "huge" and "tiny".
 *
 * Special cases:
 *      sqrt(+-0) = +-0         ... exact
 *      sqrt(inf) = inf
 *      sqrt(-ve) = NaN         ... with invalid signal
 *      sqrt(NaN) = NaN         ... with invalid signal for signaling NaN
 */

pub fn sqrtd(x: f64) -> f64 {
    #[cfg(target_feature = "sse2")]
    {
        // Note: This path is unlikely since LLVM will usually have already
        // optimized sqrt calls into hardware instructions if sse2 is available,
        // but if someone does end up here they'll apprected the speed increase.
        #[cfg(target_arch = "x86")]
        use core::arch::x86::*;
        #[cfg(target_arch = "x86_64")]
        use core::arch::x86_64::*;
        // SAFETY: safe, since `_mm_set_sd` takes a 64-bit float, and returns
        // a 128-bit type with the lowest 64-bits as `x`, `_mm_sqrt_ss` calculates
        // the sqrt of this 128-bit vector, and `_mm_cvtss_f64` extracts the lower
        // 64-bits as a 64-bit float.
        unsafe {
            let m = _mm_set_sd(x);
            let m_sqrt = _mm_sqrt_pd(m);
            _mm_cvtsd_f64(m_sqrt)
        }
    }
    #[cfg(not(target_feature = "sse2"))]
    {
        use core::num::Wrapping;

        const TINY: f64 = 1.0e-300;

        let mut z: f64;
        let sign: Wrapping<u32> = Wrapping(0x80000000);
        let mut ix0: i32;
        let mut s0: i32;
        let mut q: i32;
        let mut m: i32;
        let mut t: i32;
        let mut i: i32;
        let mut r: Wrapping<u32>;
        let mut t1: Wrapping<u32>;
        let mut s1: Wrapping<u32>;
        let mut ix1: Wrapping<u32>;
        let mut q1: Wrapping<u32>;

        ix0 = (x.to_bits() >> 32) as i32;
        ix1 = Wrapping(x.to_bits() as u32);

        /* take care of Inf and NaN */
        if (ix0 & 0x7ff00000) == 0x7ff00000 {
            return x * x + x; /* sqrt(NaN)=NaN, sqrt(+inf)=+inf, sqrt(-inf)=sNaN */
        }
        /* take care of zero */
        if ix0 <= 0 {
            if ((ix0 & !(sign.0 as i32)) | ix1.0 as i32) == 0 {
                return x; /* sqrt(+-0) = +-0 */
            }
            if ix0 < 0 {
                return (x - x) / (x - x); /* sqrt(-ve) = sNaN */
            }
        }
        /* normalize x */
        m = ix0 >> 20;
        if m == 0 {
            /* subnormal x */
            while ix0 == 0 {
                m -= 21;
                ix0 |= (ix1 >> 11).0 as i32;
                ix1 <<= 21;
            }
            i = 0;
            while (ix0 & 0x00100000) == 0 {
                i += 1;
                ix0 <<= 1;
            }
            m -= i - 1;
            ix0 |= (ix1 >> (32 - i) as usize).0 as i32;
            ix1 = ix1 << i as usize;
        }
        m -= 1023; /* unbias exponent */
        ix0 = (ix0 & 0x000fffff) | 0x00100000;
        if (m & 1) == 1 {
            /* odd m, double x to make it even */
            ix0 += ix0 + ((ix1 & sign) >> 31).0 as i32;
            ix1 += ix1;
        }
        m >>= 1; /* m = [m/2] */

        /* generate sqrt(x) bit by bit */
        ix0 += ix0 + ((ix1 & sign) >> 31).0 as i32;
        ix1 += ix1;
        q = 0; /* [q,q1] = sqrt(x) */
        q1 = Wrapping(0);
        s0 = 0;
        s1 = Wrapping(0);
        r = Wrapping(0x00200000); /* r = moving bit from right to left */

        while r != Wrapping(0) {
            t = s0 + r.0 as i32;
            if t <= ix0 {
                s0 = t + r.0 as i32;
                ix0 -= t;
                q += r.0 as i32;
            }
            ix0 += ix0 + ((ix1 & sign) >> 31).0 as i32;
            ix1 += ix1;
            r >>= 1;
        }

        r = sign;
        while r != Wrapping(0) {
            t1 = s1 + r;
            t = s0;
            if t < ix0 || (t == ix0 && t1 <= ix1) {
                s1 = t1 + r;
                if (t1 & sign) == sign && (s1 & sign) == Wrapping(0) {
                    s0 += 1;
                }
                ix0 -= t;
                if ix1 < t1 {
                    ix0 -= 1;
                }
                ix1 -= t1;
                q1 += r;
            }
            ix0 += ix0 + ((ix1 & sign) >> 31).0 as i32;
            ix1 += ix1;
            r >>= 1;
        }

        /* use floating add to find out rounding direction */
        if (ix0 as u32 | ix1.0) != 0 {
            z = 1.0 - TINY; /* raise inexact flag */
            if z >= 1.0 {
                z = 1.0 + TINY;
                if q1.0 == 0xffffffff {
                    q1 = Wrapping(0);
                    q += 1;
                } else if z > 1.0 {
                    if q1.0 == 0xfffffffe {
                        q += 1;
                    }
                    q1 += Wrapping(2);
                } else {
                    q1 += q1 & Wrapping(1);
                }
            }
        }
        ix0 = (q >> 1) + 0x3fe00000;
        ix1 = q1 >> 1;
        if (q & 1) == 1 {
            ix1 |= sign;
        }
        ix0 += m << 20;
        f64::from_bits((ix0 as u64) << 32 | ix1.0 as u64)
    }
}

#[inline]
fn get_high_word(x: f64) -> u32 {
    (x.to_bits() >> 32) as u32
}

#[inline]
fn with_set_high_word(f: f64, hi: u32) -> f64 {
    let mut tmp = f.to_bits();
    tmp &= 0x00000000_ffffffff;
    tmp |= (hi as u64) << 32;
    f64::from_bits(tmp)
}

#[inline]
fn with_set_low_word(f: f64, lo: u32) -> f64 {
    let mut tmp = f.to_bits();
    tmp &= 0xffffffff_00000000;
    tmp |= lo as u64;
    f64::from_bits(tmp)
}
