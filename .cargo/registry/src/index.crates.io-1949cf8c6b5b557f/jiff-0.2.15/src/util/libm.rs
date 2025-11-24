/*!
A no-std module for some floating point operations.

These were vendored from the [`libm`] crate. For no-std specifically, I
wouldn't necessarily mind depending on `libm`, but I don't think there's a
way to express "only depend on a crate if some on-by-default feature is not
enabled."

We also very intentionally have very minimal floating point requirements. It's
used for `Span::total` where it's part of the API and thus necessary. It's also
used when rounding in some cases, although I would like to remove that use of
floating point. And now it's also used in parts of `SignedDuration`.

[`libm`]: https://github.com/rust-lang/libm
*/

pub(crate) trait Float {
    fn abs(self) -> Self;
    fn ceil(self) -> Self;
    fn floor(self) -> Self;
    fn round(self) -> Self;
    fn signum(self) -> Self;
    fn trunc(self) -> Self;
    fn fract(self) -> Self;
}

const TOINT64: f64 = 1. / f64::EPSILON;

impl Float for f64 {
    fn abs(self) -> f64 {
        if self.is_sign_negative() {
            -self
        } else {
            self
        }
    }

    fn ceil(self) -> f64 {
        let x = self;
        let u: u64 = x.to_bits();
        let e: i64 = (u >> 52 & 0x7ff) as i64;
        let y: f64;

        if e >= 0x3ff + 52 || x == 0. {
            return x;
        }
        // y = int(x) - x, where int(x) is an integer neighbor of x
        y = if (u >> 63) != 0 {
            x - TOINT64 + TOINT64 - x
        } else {
            x + TOINT64 - TOINT64 - x
        };
        // special case because of non-nearest rounding modes
        if e < 0x3ff {
            core::hint::black_box(y);
            return if (u >> 63) != 0 { -0. } else { 1. };
        }
        if y < 0. {
            x + y + 1.
        } else {
            x + y
        }
    }

    fn floor(self) -> f64 {
        let x = self;
        let ui = x.to_bits();
        let e = ((ui >> 52) & 0x7ff) as i32;

        if (e >= 0x3ff + 52) || (x == 0.) {
            return x;
        }
        /* y = int(x) - x, where int(x) is an integer neighbor of x */
        let y = if (ui >> 63) != 0 {
            x - TOINT64 + TOINT64 - x
        } else {
            x + TOINT64 - TOINT64 - x
        };
        /* special case because of non-nearest rounding modes */
        if e < 0x3ff {
            core::hint::black_box(y);
            return if (ui >> 63) != 0 { -1. } else { 0. };
        }
        if y > 0. {
            x + y - 1.
        } else {
            x + y
        }
    }

    fn round(self) -> f64 {
        (self + copysign64(0.5 - 0.25 * f64::EPSILON, self)).trunc()
    }

    fn signum(self) -> f64 {
        if self.is_nan() {
            Self::NAN
        } else {
            copysign64(1.0, self)
        }
    }

    fn trunc(self) -> f64 {
        let x = self;
        let x1p120 = f64::from_bits(0x4770000000000000); // 0x1p120f === 2 ^ 120

        let mut i: u64 = x.to_bits();
        let mut e: i64 = (i >> 52 & 0x7ff) as i64 - 0x3ff + 12;
        let m: u64;

        if e >= 52 + 12 {
            return x;
        }
        if e < 12 {
            e = 1;
        }
        m = -1i64 as u64 >> e;
        if (i & m) == 0 {
            return x;
        }
        core::hint::black_box(x + x1p120);
        i &= !m;
        f64::from_bits(i)
    }

    fn fract(self) -> f64 {
        self - self.trunc()
    }
}

impl Float for f32 {
    fn abs(self) -> f32 {
        if self.is_sign_negative() {
            -self
        } else {
            self
        }
    }

    fn ceil(self) -> f32 {
        let x = self;
        let mut ui = x.to_bits();
        let e = (((ui >> 23) & 0xff).wrapping_sub(0x7f)) as i32;

        if e >= 23 {
            return x;
        }
        if e >= 0 {
            let m = 0x007fffff >> e;
            if (ui & m) == 0 {
                return x;
            }
            core::hint::black_box(x + f32::from_bits(0x7b800000));
            if ui >> 31 == 0 {
                ui += m;
            }
            ui &= !m;
        } else {
            core::hint::black_box(x + f32::from_bits(0x7b800000));
            if ui >> 31 != 0 {
                return -0.0;
            } else if ui << 1 != 0 {
                return 1.0;
            }
        }
        f32::from_bits(ui)
    }

    fn floor(self) -> f32 {
        let x = self;
        let mut ui = x.to_bits();
        let e = (((ui >> 23) as i32) & 0xff) - 0x7f;

        if e >= 23 {
            return x;
        }
        if e >= 0 {
            let m: u32 = 0x007fffff >> e;
            if (ui & m) == 0 {
                return x;
            }
            core::hint::black_box(x + f32::from_bits(0x7b800000));
            if ui >> 31 != 0 {
                ui += m;
            }
            ui &= !m;
        } else {
            core::hint::black_box(x + f32::from_bits(0x7b800000));
            if ui >> 31 == 0 {
                ui = 0;
            } else if ui << 1 != 0 {
                return -1.0;
            }
        }
        f32::from_bits(ui)
    }

    fn round(self) -> f32 {
        (self + copysign32(0.5 - 0.25 * f32::EPSILON, self)).trunc()
    }

    fn signum(self) -> f32 {
        if self.is_nan() {
            Self::NAN
        } else {
            copysign32(1.0, self)
        }
    }

    fn trunc(self) -> f32 {
        let x = self;
        let x1p120 = f32::from_bits(0x7b800000); // 0x1p120f === 2 ^ 120

        let mut i: u32 = x.to_bits();
        let mut e: i32 = (i >> 23 & 0xff) as i32 - 0x7f + 9;
        let m: u32;

        if e >= 23 + 9 {
            return x;
        }
        if e < 9 {
            e = 1;
        }
        m = -1i32 as u32 >> e;
        if (i & m) == 0 {
            return x;
        }
        core::hint::black_box(x + x1p120);
        i &= !m;
        f32::from_bits(i)
    }

    fn fract(self) -> f32 {
        self - self.trunc()
    }
}

fn copysign64(x: f64, y: f64) -> f64 {
    let mut ux = x.to_bits();
    let uy = y.to_bits();
    ux &= (!0) >> 1;
    ux |= uy & (1 << 63);
    f64::from_bits(ux)
}

fn copysign32(x: f32, y: f32) -> f32 {
    let mut ux = x.to_bits();
    let uy = y.to_bits();
    ux &= 0x7fffffff;
    ux |= uy & 0x80000000;
    f32::from_bits(ux)
}
