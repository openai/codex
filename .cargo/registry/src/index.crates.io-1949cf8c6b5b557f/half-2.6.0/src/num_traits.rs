use crate::{bf16, f16};
use core::cmp::Ordering;
use core::{num::FpCategory, ops::Div};
use num_traits::{
    AsPrimitive, Bounded, FloatConst, FromBytes, FromPrimitive, Num, NumCast, One, ToBytes,
    ToPrimitive, Zero,
};

impl ToPrimitive for f16 {
    #[inline]
    fn to_i64(&self) -> Option<i64> {
        Self::to_f32(*self).to_i64()
    }
    #[inline]
    fn to_u64(&self) -> Option<u64> {
        Self::to_f32(*self).to_u64()
    }
    #[inline]
    fn to_i8(&self) -> Option<i8> {
        Self::to_f32(*self).to_i8()
    }
    #[inline]
    fn to_u8(&self) -> Option<u8> {
        Self::to_f32(*self).to_u8()
    }
    #[inline]
    fn to_i16(&self) -> Option<i16> {
        Self::to_f32(*self).to_i16()
    }
    #[inline]
    fn to_u16(&self) -> Option<u16> {
        Self::to_f32(*self).to_u16()
    }
    #[inline]
    fn to_i32(&self) -> Option<i32> {
        Self::to_f32(*self).to_i32()
    }
    #[inline]
    fn to_u32(&self) -> Option<u32> {
        Self::to_f32(*self).to_u32()
    }
    #[inline]
    fn to_f32(&self) -> Option<f32> {
        Some(Self::to_f32(*self))
    }
    #[inline]
    fn to_f64(&self) -> Option<f64> {
        Some(Self::to_f64(*self))
    }
}

impl FromPrimitive for f16 {
    #[inline]
    fn from_i64(n: i64) -> Option<Self> {
        n.to_f32().map(Self::from_f32)
    }
    #[inline]
    fn from_u64(n: u64) -> Option<Self> {
        n.to_f32().map(Self::from_f32)
    }
    #[inline]
    fn from_i8(n: i8) -> Option<Self> {
        n.to_f32().map(Self::from_f32)
    }
    #[inline]
    fn from_u8(n: u8) -> Option<Self> {
        n.to_f32().map(Self::from_f32)
    }
    #[inline]
    fn from_i16(n: i16) -> Option<Self> {
        n.to_f32().map(Self::from_f32)
    }
    #[inline]
    fn from_u16(n: u16) -> Option<Self> {
        n.to_f32().map(Self::from_f32)
    }
    #[inline]
    fn from_i32(n: i32) -> Option<Self> {
        n.to_f32().map(Self::from_f32)
    }
    #[inline]
    fn from_u32(n: u32) -> Option<Self> {
        n.to_f32().map(Self::from_f32)
    }
    #[inline]
    fn from_f32(n: f32) -> Option<Self> {
        n.to_f32().map(Self::from_f32)
    }
    #[inline]
    fn from_f64(n: f64) -> Option<Self> {
        n.to_f64().map(Self::from_f64)
    }
}

impl Num for f16 {
    type FromStrRadixErr = <f32 as Num>::FromStrRadixErr;

    #[inline]
    fn from_str_radix(str: &str, radix: u32) -> Result<Self, Self::FromStrRadixErr> {
        Ok(Self::from_f32(f32::from_str_radix(str, radix)?))
    }
}

impl One for f16 {
    #[inline]
    fn one() -> Self {
        Self::ONE
    }
}

impl Zero for f16 {
    #[inline]
    fn zero() -> Self {
        Self::ZERO
    }

    #[inline]
    fn is_zero(&self) -> bool {
        *self == Self::ZERO
    }
}

impl NumCast for f16 {
    #[inline]
    fn from<T: ToPrimitive>(n: T) -> Option<Self> {
        n.to_f32().map(Self::from_f32)
    }
}

impl num_traits::float::FloatCore for f16 {
    #[inline]
    fn infinity() -> Self {
        Self::INFINITY
    }

    #[inline]
    fn neg_infinity() -> Self {
        Self::NEG_INFINITY
    }

    #[inline]
    fn nan() -> Self {
        Self::NAN
    }

    #[inline]
    fn neg_zero() -> Self {
        Self::NEG_ZERO
    }

    #[inline]
    fn min_value() -> Self {
        Self::MIN
    }

    #[inline]
    fn min_positive_value() -> Self {
        Self::MIN_POSITIVE
    }

    #[inline]
    fn epsilon() -> Self {
        Self::EPSILON
    }

    #[inline]
    fn max_value() -> Self {
        Self::MAX
    }

    #[inline]
    fn is_nan(self) -> bool {
        self.is_nan()
    }

    #[inline]
    fn is_infinite(self) -> bool {
        self.is_infinite()
    }

    #[inline]
    fn is_finite(self) -> bool {
        self.is_finite()
    }

    #[inline]
    fn is_normal(self) -> bool {
        self.is_normal()
    }

    #[inline]
    fn classify(self) -> FpCategory {
        self.classify()
    }

    #[inline]
    fn floor(self) -> Self {
        Self::from_f32(self.to_f32().floor())
    }

    #[inline]
    fn ceil(self) -> Self {
        Self::from_f32(self.to_f32().ceil())
    }

    #[inline]
    fn round(self) -> Self {
        Self::from_f32(self.to_f32().round())
    }

    #[inline]
    fn trunc(self) -> Self {
        Self::from_f32(self.to_f32().trunc())
    }

    #[inline]
    fn fract(self) -> Self {
        Self::from_f32(self.to_f32().fract())
    }

    #[inline]
    fn abs(self) -> Self {
        Self::from_bits(self.to_bits() & 0x7FFF)
    }

    #[inline]
    fn signum(self) -> Self {
        self.signum()
    }

    #[inline]
    fn is_sign_positive(self) -> bool {
        self.is_sign_positive()
    }

    #[inline]
    fn is_sign_negative(self) -> bool {
        self.is_sign_negative()
    }

    fn min(self, other: Self) -> Self {
        match self.partial_cmp(&other) {
            None => {
                if self.is_nan() {
                    other
                } else {
                    self
                }
            }
            Some(Ordering::Greater) | Some(Ordering::Equal) => other,
            Some(Ordering::Less) => self,
        }
    }

    fn max(self, other: Self) -> Self {
        match self.partial_cmp(&other) {
            None => {
                if self.is_nan() {
                    other
                } else {
                    self
                }
            }
            Some(Ordering::Greater) | Some(Ordering::Equal) => self,
            Some(Ordering::Less) => other,
        }
    }

    #[inline]
    fn recip(self) -> Self {
        Self::from_f32(self.to_f32().recip())
    }

    #[inline]
    fn powi(self, exp: i32) -> Self {
        Self::from_f32(self.to_f32().powi(exp))
    }

    #[inline]
    fn to_degrees(self) -> Self {
        Self::from_f32(self.to_f32().to_degrees())
    }

    #[inline]
    fn to_radians(self) -> Self {
        Self::from_f32(self.to_f32().to_radians())
    }

    #[inline]
    fn integer_decode(self) -> (u64, i16, i8) {
        num_traits::float::FloatCore::integer_decode(self.to_f32())
    }
}

impl num_traits::float::Float for f16 {
    #[inline]
    fn nan() -> Self {
        Self::NAN
    }

    #[inline]
    fn infinity() -> Self {
        Self::INFINITY
    }

    #[inline]
    fn neg_infinity() -> Self {
        Self::NEG_INFINITY
    }

    #[inline]
    fn neg_zero() -> Self {
        Self::NEG_ZERO
    }

    #[inline]
    fn min_value() -> Self {
        Self::MIN
    }

    #[inline]
    fn min_positive_value() -> Self {
        Self::MIN_POSITIVE
    }

    #[inline]
    fn epsilon() -> Self {
        Self::EPSILON
    }

    #[inline]
    fn max_value() -> Self {
        Self::MAX
    }

    #[inline]
    fn is_nan(self) -> bool {
        self.is_nan()
    }

    #[inline]
    fn is_infinite(self) -> bool {
        self.is_infinite()
    }

    #[inline]
    fn is_finite(self) -> bool {
        self.is_finite()
    }

    #[inline]
    fn is_normal(self) -> bool {
        self.is_normal()
    }

    #[inline]
    fn classify(self) -> FpCategory {
        self.classify()
    }

    #[inline]
    fn floor(self) -> Self {
        Self::from_f32(self.to_f32().floor())
    }

    #[inline]
    fn ceil(self) -> Self {
        Self::from_f32(self.to_f32().ceil())
    }

    #[inline]
    fn round(self) -> Self {
        Self::from_f32(self.to_f32().round())
    }

    #[inline]
    fn trunc(self) -> Self {
        Self::from_f32(self.to_f32().trunc())
    }

    #[inline]
    fn fract(self) -> Self {
        Self::from_f32(self.to_f32().fract())
    }

    #[inline]
    fn abs(self) -> Self {
        Self::from_f32(self.to_f32().abs())
    }

    #[inline]
    fn signum(self) -> Self {
        Self::from_f32(self.to_f32().signum())
    }

    #[inline]
    fn is_sign_positive(self) -> bool {
        self.is_sign_positive()
    }

    #[inline]
    fn is_sign_negative(self) -> bool {
        self.is_sign_negative()
    }

    #[inline]
    fn mul_add(self, a: Self, b: Self) -> Self {
        Self::from_f32(self.to_f32().mul_add(a.to_f32(), b.to_f32()))
    }

    #[inline]
    fn recip(self) -> Self {
        Self::from_f32(self.to_f32().recip())
    }

    #[inline]
    fn powi(self, n: i32) -> Self {
        Self::from_f32(self.to_f32().powi(n))
    }

    #[inline]
    fn powf(self, n: Self) -> Self {
        Self::from_f32(self.to_f32().powf(n.to_f32()))
    }

    #[inline]
    fn sqrt(self) -> Self {
        Self::from_f32(self.to_f32().sqrt())
    }

    #[inline]
    fn exp(self) -> Self {
        Self::from_f32(self.to_f32().exp())
    }

    #[inline]
    fn exp2(self) -> Self {
        Self::from_f32(self.to_f32().exp2())
    }

    #[inline]
    fn ln(self) -> Self {
        Self::from_f32(self.to_f32().ln())
    }

    #[inline]
    fn log(self, base: Self) -> Self {
        Self::from_f32(self.to_f32().log(base.to_f32()))
    }

    #[inline]
    fn log2(self) -> Self {
        Self::from_f32(self.to_f32().log2())
    }

    #[inline]
    fn log10(self) -> Self {
        Self::from_f32(self.to_f32().log10())
    }

    #[inline]
    fn to_degrees(self) -> Self {
        Self::from_f32(self.to_f32().to_degrees())
    }

    #[inline]
    fn to_radians(self) -> Self {
        Self::from_f32(self.to_f32().to_radians())
    }

    #[inline]
    fn max(self, other: Self) -> Self {
        self.max(other)
    }

    #[inline]
    fn min(self, other: Self) -> Self {
        self.min(other)
    }

    #[inline]
    fn abs_sub(self, other: Self) -> Self {
        Self::from_f32((self.to_f32() - other.to_f32()).max(0.0))
    }

    #[inline]
    fn cbrt(self) -> Self {
        Self::from_f32(self.to_f32().cbrt())
    }

    #[inline]
    fn hypot(self, other: Self) -> Self {
        Self::from_f32(self.to_f32().hypot(other.to_f32()))
    }

    #[inline]
    fn sin(self) -> Self {
        Self::from_f32(self.to_f32().sin())
    }

    #[inline]
    fn cos(self) -> Self {
        Self::from_f32(self.to_f32().cos())
    }

    #[inline]
    fn tan(self) -> Self {
        Self::from_f32(self.to_f32().tan())
    }

    #[inline]
    fn asin(self) -> Self {
        Self::from_f32(self.to_f32().asin())
    }

    #[inline]
    fn acos(self) -> Self {
        Self::from_f32(self.to_f32().acos())
    }

    #[inline]
    fn atan(self) -> Self {
        Self::from_f32(self.to_f32().atan())
    }

    #[inline]
    fn atan2(self, other: Self) -> Self {
        Self::from_f32(self.to_f32().atan2(other.to_f32()))
    }

    #[inline]
    fn sin_cos(self) -> (Self, Self) {
        let (sin, cos) = self.to_f32().sin_cos();
        (Self::from_f32(sin), Self::from_f32(cos))
    }

    #[inline]
    fn exp_m1(self) -> Self {
        Self::from_f32(self.to_f32().exp_m1())
    }

    #[inline]
    fn ln_1p(self) -> Self {
        Self::from_f32(self.to_f32().ln_1p())
    }

    #[inline]
    fn sinh(self) -> Self {
        Self::from_f32(self.to_f32().sinh())
    }

    #[inline]
    fn cosh(self) -> Self {
        Self::from_f32(self.to_f32().cosh())
    }

    #[inline]
    fn tanh(self) -> Self {
        Self::from_f32(self.to_f32().tanh())
    }

    #[inline]
    fn asinh(self) -> Self {
        Self::from_f32(self.to_f32().asinh())
    }

    #[inline]
    fn acosh(self) -> Self {
        Self::from_f32(self.to_f32().acosh())
    }

    #[inline]
    fn atanh(self) -> Self {
        Self::from_f32(self.to_f32().atanh())
    }

    #[inline]
    fn integer_decode(self) -> (u64, i16, i8) {
        num_traits::float::Float::integer_decode(self.to_f32())
    }
}

impl FloatConst for f16 {
    #[inline]
    fn E() -> Self {
        Self::E
    }

    #[inline]
    fn FRAC_1_PI() -> Self {
        Self::FRAC_1_PI
    }

    #[inline]
    fn FRAC_1_SQRT_2() -> Self {
        Self::FRAC_1_SQRT_2
    }

    #[inline]
    fn FRAC_2_PI() -> Self {
        Self::FRAC_2_PI
    }

    #[inline]
    fn FRAC_2_SQRT_PI() -> Self {
        Self::FRAC_2_SQRT_PI
    }

    #[inline]
    fn FRAC_PI_2() -> Self {
        Self::FRAC_PI_2
    }

    #[inline]
    fn FRAC_PI_3() -> Self {
        Self::FRAC_PI_3
    }

    #[inline]
    fn FRAC_PI_4() -> Self {
        Self::FRAC_PI_4
    }

    #[inline]
    fn FRAC_PI_6() -> Self {
        Self::FRAC_PI_6
    }

    #[inline]
    fn FRAC_PI_8() -> Self {
        Self::FRAC_PI_8
    }

    #[inline]
    fn LN_10() -> Self {
        Self::LN_10
    }

    #[inline]
    fn LN_2() -> Self {
        Self::LN_2
    }

    #[inline]
    fn LOG10_E() -> Self {
        Self::LOG10_E
    }

    #[inline]
    fn LOG2_E() -> Self {
        Self::LOG2_E
    }

    #[inline]
    fn PI() -> Self {
        Self::PI
    }

    fn SQRT_2() -> Self {
        Self::SQRT_2
    }

    #[inline]
    fn LOG10_2() -> Self
    where
        Self: Sized + Div<Self, Output = Self>,
    {
        Self::LOG10_2
    }

    #[inline]
    fn LOG2_10() -> Self
    where
        Self: Sized + Div<Self, Output = Self>,
    {
        Self::LOG2_10
    }
}

impl Bounded for f16 {
    #[inline]
    fn min_value() -> Self {
        f16::MIN
    }

    #[inline]
    fn max_value() -> Self {
        f16::MAX
    }
}

macro_rules! impl_as_primitive_to_f16 {
    ($ty:ty, $meth:ident) => {
        impl AsPrimitive<$ty> for f16 {
            #[inline]
            fn as_(self) -> $ty {
                self.$meth().as_()
            }
        }
    };
}

impl AsPrimitive<f16> for f16 {
    #[inline]
    fn as_(self) -> f16 {
        self
    }
}

impl_as_primitive_to_f16!(i64, to_f32);
impl_as_primitive_to_f16!(u64, to_f32);
impl_as_primitive_to_f16!(i8, to_f32);
impl_as_primitive_to_f16!(u8, to_f32);
impl_as_primitive_to_f16!(i16, to_f32);
impl_as_primitive_to_f16!(u16, to_f32);
impl_as_primitive_to_f16!(i32, to_f32);
impl_as_primitive_to_f16!(u32, to_f32);
impl_as_primitive_to_f16!(isize, to_f32);
impl_as_primitive_to_f16!(usize, to_f32);
impl_as_primitive_to_f16!(f32, to_f32);
impl_as_primitive_to_f16!(f64, to_f64);
impl_as_primitive_to_f16!(bf16, to_f32);

macro_rules! impl_as_primitive_f16_from {
    ($ty:ty, $meth:ident) => {
        impl AsPrimitive<f16> for $ty {
            #[inline]
            fn as_(self) -> f16 {
                f16::$meth(self.as_())
            }
        }
    };
}

impl_as_primitive_f16_from!(i64, from_f32);
impl_as_primitive_f16_from!(u64, from_f32);
impl_as_primitive_f16_from!(i8, from_f32);
impl_as_primitive_f16_from!(u8, from_f32);
impl_as_primitive_f16_from!(i16, from_f32);
impl_as_primitive_f16_from!(u16, from_f32);
impl_as_primitive_f16_from!(i32, from_f32);
impl_as_primitive_f16_from!(u32, from_f32);
impl_as_primitive_f16_from!(isize, from_f32);
impl_as_primitive_f16_from!(usize, from_f32);
impl_as_primitive_f16_from!(f32, from_f32);
impl_as_primitive_f16_from!(f64, from_f64);

impl ToBytes for f16 {
    type Bytes = [u8; 2];

    fn to_be_bytes(&self) -> Self::Bytes {
        Self::to_be_bytes(*self)
    }

    fn to_le_bytes(&self) -> Self::Bytes {
        Self::to_le_bytes(*self)
    }

    fn to_ne_bytes(&self) -> Self::Bytes {
        Self::to_ne_bytes(*self)
    }
}

impl FromBytes for f16 {
    type Bytes = [u8; 2];

    fn from_be_bytes(bytes: &Self::Bytes) -> Self {
        Self::from_be_bytes(*bytes)
    }

    fn from_le_bytes(bytes: &Self::Bytes) -> Self {
        Self::from_le_bytes(*bytes)
    }

    fn from_ne_bytes(bytes: &Self::Bytes) -> Self {
        Self::from_ne_bytes(*bytes)
    }
}

impl ToPrimitive for bf16 {
    #[inline]
    fn to_i64(&self) -> Option<i64> {
        Self::to_f32(*self).to_i64()
    }
    #[inline]
    fn to_u64(&self) -> Option<u64> {
        Self::to_f32(*self).to_u64()
    }
    #[inline]
    fn to_i8(&self) -> Option<i8> {
        Self::to_f32(*self).to_i8()
    }
    #[inline]
    fn to_u8(&self) -> Option<u8> {
        Self::to_f32(*self).to_u8()
    }
    #[inline]
    fn to_i16(&self) -> Option<i16> {
        Self::to_f32(*self).to_i16()
    }
    #[inline]
    fn to_u16(&self) -> Option<u16> {
        Self::to_f32(*self).to_u16()
    }
    #[inline]
    fn to_i32(&self) -> Option<i32> {
        Self::to_f32(*self).to_i32()
    }
    #[inline]
    fn to_u32(&self) -> Option<u32> {
        Self::to_f32(*self).to_u32()
    }
    #[inline]
    fn to_f32(&self) -> Option<f32> {
        Some(Self::to_f32(*self))
    }
    #[inline]
    fn to_f64(&self) -> Option<f64> {
        Some(Self::to_f64(*self))
    }
}

impl FromPrimitive for bf16 {
    #[inline]
    fn from_i64(n: i64) -> Option<Self> {
        n.to_f32().map(Self::from_f32)
    }
    #[inline]
    fn from_u64(n: u64) -> Option<Self> {
        n.to_f32().map(Self::from_f32)
    }
    #[inline]
    fn from_i8(n: i8) -> Option<Self> {
        n.to_f32().map(Self::from_f32)
    }
    #[inline]
    fn from_u8(n: u8) -> Option<Self> {
        n.to_f32().map(Self::from_f32)
    }
    #[inline]
    fn from_i16(n: i16) -> Option<Self> {
        n.to_f32().map(Self::from_f32)
    }
    #[inline]
    fn from_u16(n: u16) -> Option<Self> {
        n.to_f32().map(Self::from_f32)
    }
    #[inline]
    fn from_i32(n: i32) -> Option<Self> {
        n.to_f32().map(Self::from_f32)
    }
    #[inline]
    fn from_u32(n: u32) -> Option<Self> {
        n.to_f32().map(Self::from_f32)
    }
    #[inline]
    fn from_f32(n: f32) -> Option<Self> {
        n.to_f32().map(Self::from_f32)
    }
    #[inline]
    fn from_f64(n: f64) -> Option<Self> {
        n.to_f64().map(Self::from_f64)
    }
}

impl Num for bf16 {
    type FromStrRadixErr = <f32 as Num>::FromStrRadixErr;

    #[inline]
    fn from_str_radix(str: &str, radix: u32) -> Result<Self, Self::FromStrRadixErr> {
        Ok(Self::from_f32(f32::from_str_radix(str, radix)?))
    }
}

impl One for bf16 {
    #[inline]
    fn one() -> Self {
        Self::ONE
    }
}

impl Zero for bf16 {
    #[inline]
    fn zero() -> Self {
        Self::ZERO
    }

    #[inline]
    fn is_zero(&self) -> bool {
        *self == Self::ZERO
    }
}

impl NumCast for bf16 {
    #[inline]
    fn from<T: ToPrimitive>(n: T) -> Option<Self> {
        n.to_f32().map(Self::from_f32)
    }
}

impl num_traits::float::FloatCore for bf16 {
    #[inline]
    fn infinity() -> Self {
        Self::INFINITY
    }

    #[inline]
    fn neg_infinity() -> Self {
        Self::NEG_INFINITY
    }

    #[inline]
    fn nan() -> Self {
        Self::NAN
    }

    #[inline]
    fn neg_zero() -> Self {
        Self::NEG_ZERO
    }

    #[inline]
    fn min_value() -> Self {
        Self::MIN
    }

    #[inline]
    fn min_positive_value() -> Self {
        Self::MIN_POSITIVE
    }

    #[inline]
    fn epsilon() -> Self {
        Self::EPSILON
    }

    #[inline]
    fn max_value() -> Self {
        Self::MAX
    }

    #[inline]
    fn is_nan(self) -> bool {
        self.is_nan()
    }

    #[inline]
    fn is_infinite(self) -> bool {
        self.is_infinite()
    }

    #[inline]
    fn is_finite(self) -> bool {
        self.is_finite()
    }

    #[inline]
    fn is_normal(self) -> bool {
        self.is_normal()
    }

    #[inline]
    fn classify(self) -> FpCategory {
        self.classify()
    }

    #[inline]
    fn floor(self) -> Self {
        Self::from_f32(self.to_f32().floor())
    }

    #[inline]
    fn ceil(self) -> Self {
        Self::from_f32(self.to_f32().ceil())
    }

    #[inline]
    fn round(self) -> Self {
        Self::from_f32(self.to_f32().round())
    }

    #[inline]
    fn trunc(self) -> Self {
        Self::from_f32(self.to_f32().trunc())
    }

    #[inline]
    fn fract(self) -> Self {
        Self::from_f32(self.to_f32().fract())
    }

    #[inline]
    fn abs(self) -> Self {
        Self::from_bits(self.to_bits() & 0x7FFF)
    }

    #[inline]
    fn signum(self) -> Self {
        self.signum()
    }

    #[inline]
    fn is_sign_positive(self) -> bool {
        self.is_sign_positive()
    }

    #[inline]
    fn is_sign_negative(self) -> bool {
        self.is_sign_negative()
    }

    fn min(self, other: Self) -> Self {
        match self.partial_cmp(&other) {
            None => {
                if self.is_nan() {
                    other
                } else {
                    self
                }
            }
            Some(Ordering::Greater) | Some(Ordering::Equal) => other,
            Some(Ordering::Less) => self,
        }
    }

    fn max(self, other: Self) -> Self {
        match self.partial_cmp(&other) {
            None => {
                if self.is_nan() {
                    other
                } else {
                    self
                }
            }
            Some(Ordering::Greater) | Some(Ordering::Equal) => self,
            Some(Ordering::Less) => other,
        }
    }

    #[inline]
    fn recip(self) -> Self {
        Self::from_f32(self.to_f32().recip())
    }

    #[inline]
    fn powi(self, exp: i32) -> Self {
        Self::from_f32(self.to_f32().powi(exp))
    }

    #[inline]
    fn to_degrees(self) -> Self {
        Self::from_f32(self.to_f32().to_degrees())
    }

    #[inline]
    fn to_radians(self) -> Self {
        Self::from_f32(self.to_f32().to_radians())
    }

    #[inline]
    fn integer_decode(self) -> (u64, i16, i8) {
        num_traits::float::FloatCore::integer_decode(self.to_f32())
    }
}

impl num_traits::float::Float for bf16 {
    #[inline]
    fn nan() -> Self {
        Self::NAN
    }

    #[inline]
    fn infinity() -> Self {
        Self::INFINITY
    }

    #[inline]
    fn neg_infinity() -> Self {
        Self::NEG_INFINITY
    }

    #[inline]
    fn neg_zero() -> Self {
        Self::NEG_ZERO
    }

    #[inline]
    fn min_value() -> Self {
        Self::MIN
    }

    #[inline]
    fn min_positive_value() -> Self {
        Self::MIN_POSITIVE
    }

    #[inline]
    fn epsilon() -> Self {
        Self::EPSILON
    }

    #[inline]
    fn max_value() -> Self {
        Self::MAX
    }

    #[inline]
    fn is_nan(self) -> bool {
        self.is_nan()
    }

    #[inline]
    fn is_infinite(self) -> bool {
        self.is_infinite()
    }

    #[inline]
    fn is_finite(self) -> bool {
        self.is_finite()
    }

    #[inline]
    fn is_normal(self) -> bool {
        self.is_normal()
    }

    #[inline]
    fn classify(self) -> FpCategory {
        self.classify()
    }

    #[inline]
    fn floor(self) -> Self {
        Self::from_f32(self.to_f32().floor())
    }

    #[inline]
    fn ceil(self) -> Self {
        Self::from_f32(self.to_f32().ceil())
    }

    #[inline]
    fn round(self) -> Self {
        Self::from_f32(self.to_f32().round())
    }

    #[inline]
    fn trunc(self) -> Self {
        Self::from_f32(self.to_f32().trunc())
    }

    #[inline]
    fn fract(self) -> Self {
        Self::from_f32(self.to_f32().fract())
    }

    #[inline]
    fn abs(self) -> Self {
        Self::from_f32(self.to_f32().abs())
    }

    #[inline]
    fn signum(self) -> Self {
        Self::from_f32(self.to_f32().signum())
    }

    #[inline]
    fn is_sign_positive(self) -> bool {
        self.is_sign_positive()
    }

    #[inline]
    fn is_sign_negative(self) -> bool {
        self.is_sign_negative()
    }

    #[inline]
    fn mul_add(self, a: Self, b: Self) -> Self {
        Self::from_f32(self.to_f32().mul_add(a.to_f32(), b.to_f32()))
    }

    #[inline]
    fn recip(self) -> Self {
        Self::from_f32(self.to_f32().recip())
    }

    #[inline]
    fn powi(self, n: i32) -> Self {
        Self::from_f32(self.to_f32().powi(n))
    }

    #[inline]
    fn powf(self, n: Self) -> Self {
        Self::from_f32(self.to_f32().powf(n.to_f32()))
    }

    #[inline]
    fn sqrt(self) -> Self {
        Self::from_f32(self.to_f32().sqrt())
    }

    #[inline]
    fn exp(self) -> Self {
        Self::from_f32(self.to_f32().exp())
    }

    #[inline]
    fn exp2(self) -> Self {
        Self::from_f32(self.to_f32().exp2())
    }

    #[inline]
    fn ln(self) -> Self {
        Self::from_f32(self.to_f32().ln())
    }

    #[inline]
    fn log(self, base: Self) -> Self {
        Self::from_f32(self.to_f32().log(base.to_f32()))
    }

    #[inline]
    fn log2(self) -> Self {
        Self::from_f32(self.to_f32().log2())
    }

    #[inline]
    fn log10(self) -> Self {
        Self::from_f32(self.to_f32().log10())
    }

    #[inline]
    fn to_degrees(self) -> Self {
        Self::from_f32(self.to_f32().to_degrees())
    }

    #[inline]
    fn to_radians(self) -> Self {
        Self::from_f32(self.to_f32().to_radians())
    }

    #[inline]
    fn max(self, other: Self) -> Self {
        self.max(other)
    }

    #[inline]
    fn min(self, other: Self) -> Self {
        self.min(other)
    }

    #[inline]
    fn abs_sub(self, other: Self) -> Self {
        Self::from_f32((self.to_f32() - other.to_f32()).max(0.0))
    }

    #[inline]
    fn cbrt(self) -> Self {
        Self::from_f32(self.to_f32().cbrt())
    }

    #[inline]
    fn hypot(self, other: Self) -> Self {
        Self::from_f32(self.to_f32().hypot(other.to_f32()))
    }

    #[inline]
    fn sin(self) -> Self {
        Self::from_f32(self.to_f32().sin())
    }

    #[inline]
    fn cos(self) -> Self {
        Self::from_f32(self.to_f32().cos())
    }

    #[inline]
    fn tan(self) -> Self {
        Self::from_f32(self.to_f32().tan())
    }

    #[inline]
    fn asin(self) -> Self {
        Self::from_f32(self.to_f32().asin())
    }

    #[inline]
    fn acos(self) -> Self {
        Self::from_f32(self.to_f32().acos())
    }

    #[inline]
    fn atan(self) -> Self {
        Self::from_f32(self.to_f32().atan())
    }

    #[inline]
    fn atan2(self, other: Self) -> Self {
        Self::from_f32(self.to_f32().atan2(other.to_f32()))
    }

    #[inline]
    fn sin_cos(self) -> (Self, Self) {
        let (sin, cos) = self.to_f32().sin_cos();
        (Self::from_f32(sin), Self::from_f32(cos))
    }

    #[inline]
    fn exp_m1(self) -> Self {
        Self::from_f32(self.to_f32().exp_m1())
    }

    #[inline]
    fn ln_1p(self) -> Self {
        Self::from_f32(self.to_f32().ln_1p())
    }

    #[inline]
    fn sinh(self) -> Self {
        Self::from_f32(self.to_f32().sinh())
    }

    #[inline]
    fn cosh(self) -> Self {
        Self::from_f32(self.to_f32().cosh())
    }

    #[inline]
    fn tanh(self) -> Self {
        Self::from_f32(self.to_f32().tanh())
    }

    #[inline]
    fn asinh(self) -> Self {
        Self::from_f32(self.to_f32().asinh())
    }

    #[inline]
    fn acosh(self) -> Self {
        Self::from_f32(self.to_f32().acosh())
    }

    #[inline]
    fn atanh(self) -> Self {
        Self::from_f32(self.to_f32().atanh())
    }

    #[inline]
    fn integer_decode(self) -> (u64, i16, i8) {
        num_traits::float::Float::integer_decode(self.to_f32())
    }
}

impl FloatConst for bf16 {
    #[inline]
    fn E() -> Self {
        Self::E
    }

    #[inline]
    fn FRAC_1_PI() -> Self {
        Self::FRAC_1_PI
    }

    #[inline]
    fn FRAC_1_SQRT_2() -> Self {
        Self::FRAC_1_SQRT_2
    }

    #[inline]
    fn FRAC_2_PI() -> Self {
        Self::FRAC_2_PI
    }

    #[inline]
    fn FRAC_2_SQRT_PI() -> Self {
        Self::FRAC_2_SQRT_PI
    }

    #[inline]
    fn FRAC_PI_2() -> Self {
        Self::FRAC_PI_2
    }

    #[inline]
    fn FRAC_PI_3() -> Self {
        Self::FRAC_PI_3
    }

    #[inline]
    fn FRAC_PI_4() -> Self {
        Self::FRAC_PI_4
    }

    #[inline]
    fn FRAC_PI_6() -> Self {
        Self::FRAC_PI_6
    }

    #[inline]
    fn FRAC_PI_8() -> Self {
        Self::FRAC_PI_8
    }

    #[inline]
    fn LN_10() -> Self {
        Self::LN_10
    }

    #[inline]
    fn LN_2() -> Self {
        Self::LN_2
    }

    #[inline]
    fn LOG10_E() -> Self {
        Self::LOG10_E
    }

    #[inline]
    fn LOG2_E() -> Self {
        Self::LOG2_E
    }

    #[inline]
    fn PI() -> Self {
        Self::PI
    }

    #[inline]
    fn SQRT_2() -> Self {
        Self::SQRT_2
    }

    #[inline]
    fn LOG10_2() -> Self
    where
        Self: Sized + Div<Self, Output = Self>,
    {
        Self::LOG10_2
    }

    #[inline]
    fn LOG2_10() -> Self
    where
        Self: Sized + Div<Self, Output = Self>,
    {
        Self::LOG2_10
    }
}

impl Bounded for bf16 {
    #[inline]
    fn min_value() -> Self {
        bf16::MIN
    }

    #[inline]
    fn max_value() -> Self {
        bf16::MAX
    }
}

impl AsPrimitive<bf16> for bf16 {
    #[inline]
    fn as_(self) -> bf16 {
        self
    }
}

macro_rules! impl_as_primitive_to_bf16 {
    ($ty:ty, $meth:ident) => {
        impl AsPrimitive<$ty> for bf16 {
            #[inline]
            fn as_(self) -> $ty {
                self.$meth().as_()
            }
        }
    };
}

impl_as_primitive_to_bf16!(i64, to_f32);
impl_as_primitive_to_bf16!(u64, to_f32);
impl_as_primitive_to_bf16!(i8, to_f32);
impl_as_primitive_to_bf16!(u8, to_f32);
impl_as_primitive_to_bf16!(i16, to_f32);
impl_as_primitive_to_bf16!(u16, to_f32);
impl_as_primitive_to_bf16!(i32, to_f32);
impl_as_primitive_to_bf16!(u32, to_f32);
impl_as_primitive_to_bf16!(isize, to_f32);
impl_as_primitive_to_bf16!(usize, to_f32);
impl_as_primitive_to_bf16!(f32, to_f32);
impl_as_primitive_to_bf16!(f64, to_f64);
impl_as_primitive_to_bf16!(f16, to_f32);

macro_rules! impl_as_primitive_bf16_from {
    ($ty:ty, $meth:ident) => {
        impl AsPrimitive<bf16> for $ty {
            #[inline]
            fn as_(self) -> bf16 {
                bf16::$meth(self.as_())
            }
        }
    };
}

impl_as_primitive_bf16_from!(i64, from_f32);
impl_as_primitive_bf16_from!(u64, from_f32);
impl_as_primitive_bf16_from!(i8, from_f32);
impl_as_primitive_bf16_from!(u8, from_f32);
impl_as_primitive_bf16_from!(i16, from_f32);
impl_as_primitive_bf16_from!(u16, from_f32);
impl_as_primitive_bf16_from!(i32, from_f32);
impl_as_primitive_bf16_from!(u32, from_f32);
impl_as_primitive_bf16_from!(isize, from_f32);
impl_as_primitive_bf16_from!(usize, from_f32);
impl_as_primitive_bf16_from!(f32, from_f32);
impl_as_primitive_bf16_from!(f64, from_f64);

impl ToBytes for bf16 {
    type Bytes = [u8; 2];

    fn to_be_bytes(&self) -> Self::Bytes {
        Self::to_be_bytes(*self)
    }

    fn to_le_bytes(&self) -> Self::Bytes {
        Self::to_le_bytes(*self)
    }

    fn to_ne_bytes(&self) -> Self::Bytes {
        Self::to_ne_bytes(*self)
    }
}

impl FromBytes for bf16 {
    type Bytes = [u8; 2];

    fn from_be_bytes(bytes: &Self::Bytes) -> Self {
        Self::from_be_bytes(*bytes)
    }

    fn from_le_bytes(bytes: &Self::Bytes) -> Self {
        Self::from_le_bytes(*bytes)
    }

    fn from_ne_bytes(bytes: &Self::Bytes) -> Self {
        Self::from_ne_bytes(*bytes)
    }
}
