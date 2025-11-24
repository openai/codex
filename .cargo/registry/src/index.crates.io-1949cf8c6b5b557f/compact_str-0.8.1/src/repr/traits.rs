use core::hint::unreachable_unchecked;

use super::Repr;
use crate::ToCompactStringError;

const FALSE: Repr = Repr::const_new("false");
const TRUE: Repr = Repr::const_new("true");

/// Defines how to _efficiently_ create a [`Repr`] from `self`
pub(crate) trait IntoRepr {
    fn into_repr(self) -> Result<Repr, ToCompactStringError>;
}

impl IntoRepr for f32 {
    #[inline]
    fn into_repr(self) -> Result<Repr, ToCompactStringError> {
        let mut buf = ryu::Buffer::new();
        let s = buf.format(self);
        Ok(Repr::new(s)?)
    }
}

impl IntoRepr for f64 {
    #[inline]
    fn into_repr(self) -> Result<Repr, ToCompactStringError> {
        let mut buf = ryu::Buffer::new();
        let s = buf.format(self);
        Ok(Repr::new(s)?)
    }
}

impl IntoRepr for bool {
    #[inline]
    fn into_repr(self) -> Result<Repr, ToCompactStringError> {
        if self {
            Ok(TRUE)
        } else {
            Ok(FALSE)
        }
    }
}

impl IntoRepr for char {
    #[inline]
    fn into_repr(self) -> Result<Repr, ToCompactStringError> {
        let mut buf = [0_u8; 4];
        let s = self.encode_utf8(&mut buf);

        // This match is just a hint for the compiler.
        match s.len() {
            1..=4 => (),
            // SAFETY: a UTF-8 character is 1 to 4 bytes.
            _ => unsafe { unreachable_unchecked() },
        }

        Ok(Repr::new(s)?)
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString;

    use quickcheck_macros::quickcheck;

    use super::IntoRepr;

    #[test]
    fn test_into_repr_bool() {
        let t = true;
        let repr = t.into_repr().unwrap();
        assert_eq!(repr.as_str(), t.to_string());

        let f = false;
        let repr = f.into_repr().unwrap();
        assert_eq!(repr.as_str(), f.to_string());
    }

    #[quickcheck]
    #[cfg_attr(miri, ignore)]
    fn quickcheck_into_repr_char(val: char) {
        let repr = char::into_repr(val).unwrap();
        assert_eq!(repr.as_str(), val.to_string());
    }

    #[test]
    fn test_into_repr_f64_sanity() {
        let vals = [
            f64::MIN,
            f64::MIN_POSITIVE,
            f64::MAX,
            f64::NEG_INFINITY,
            f64::INFINITY,
        ];

        for x in &vals {
            let repr = f64::into_repr(*x).unwrap();
            let roundtrip = repr.as_str().parse::<f64>().unwrap();

            assert_eq!(*x, roundtrip);
        }
    }

    #[test]
    fn test_into_repr_f64_nan() {
        let repr = f64::into_repr(f64::NAN).unwrap();
        let roundtrip = repr.as_str().parse::<f64>().unwrap();
        assert!(roundtrip.is_nan());
    }

    #[quickcheck]
    #[cfg_attr(miri, ignore)]
    fn quickcheck_into_repr_f64(val: f64) {
        let repr = f64::into_repr(val).unwrap();
        let roundtrip = repr.as_str().parse::<f64>().unwrap();

        // Note: The formatting of floats by `ryu` sometimes differs from that of `std`, so instead
        // of asserting equality with `std` we just make sure the value roundtrips

        if val.is_nan() != roundtrip.is_nan() {
            assert_eq!(val, roundtrip);
        }
    }

    // `f32` formatting is broken on powerpc64le, not only in `ryu` but also `std`
    //
    // See: https://github.com/rust-lang/rust/issues/96306
    #[test]
    #[cfg_attr(all(target_arch = "powerpc64", target_pointer_width = "64"), ignore)]
    fn test_into_repr_f32_sanity() {
        let vals = [
            f32::MIN,
            f32::MIN_POSITIVE,
            f32::MAX,
            f32::NEG_INFINITY,
            f32::INFINITY,
        ];

        for x in &vals {
            let repr = f32::into_repr(*x).unwrap();
            let roundtrip = repr.as_str().parse::<f32>().unwrap();

            assert_eq!(*x, roundtrip);
        }
    }

    #[test]
    #[cfg_attr(all(target_arch = "powerpc64", target_pointer_width = "64"), ignore)]
    fn test_into_repr_f32_nan() {
        let repr = f32::into_repr(f32::NAN).unwrap();
        let roundtrip = repr.as_str().parse::<f32>().unwrap();
        assert!(roundtrip.is_nan());
    }

    #[quickcheck]
    #[cfg_attr(all(target_arch = "powerpc64", target_pointer_width = "64"), ignore)]
    fn proptest_into_repr_f32(val: f32) {
        let repr = f32::into_repr(val).unwrap();
        let roundtrip = repr.as_str().parse::<f32>().unwrap();

        // Note: The formatting of floats by `ryu` sometimes differs from that of `std`, so instead
        // of asserting equality with `std` we just make sure the value roundtrips

        if val.is_nan() != roundtrip.is_nan() {
            assert_eq!(val, roundtrip);
        }
    }
}
