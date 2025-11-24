// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

/// An iterator over the decimal digits of an integer, from least to most significant
pub struct IntIterator<T> {
    /// Digits remaining to be returned from the iterator
    unum: T,
    /// Whether the number is negative
    pub is_negative: bool,
}

macro_rules! impl_iterator_unsigned_integer_type {
    ($utype: ident) => {
        impl Iterator for IntIterator<$utype> {
            type Item = u8;
            fn next(&mut self) -> Option<Self::Item> {
                if self.unum == 0 {
                    None
                } else {
                    let div = self.unum / 10;
                    let rem = self.unum % 10;
                    self.unum = div;
                    Some(rem as u8)
                }
            }
        }
    };
}

macro_rules! impl_from_signed_integer_type {
    ($itype:ident, $utype:ident) => {
        impl From<$itype> for IntIterator<$utype> {
            fn from(value: $itype) -> Self {
                Self {
                    unum: {
                        if value == $itype::MIN {
                            $itype::MAX as $utype + 1
                        } else {
                            value.unsigned_abs()
                        }
                    },
                    is_negative: value.is_negative(),
                }
            }
        }
    };
}

macro_rules! impl_from_unsigned_integer_type {
    ($utype:ident) => {
        impl From<$utype> for IntIterator<$utype> {
            fn from(value: $utype) -> Self {
                Self {
                    unum: value,
                    is_negative: false,
                }
            }
        }
        impl_iterator_unsigned_integer_type!($utype);
    };
}

impl_from_signed_integer_type!(isize, usize);
impl_from_signed_integer_type!(i128, u128);
impl_from_signed_integer_type!(i64, u64);
impl_from_signed_integer_type!(i32, u32);
impl_from_signed_integer_type!(i16, u16);
impl_from_signed_integer_type!(i8, u8);

impl_from_unsigned_integer_type!(usize);
impl_from_unsigned_integer_type!(u128);
impl_from_unsigned_integer_type!(u64);
impl_from_unsigned_integer_type!(u32);
impl_from_unsigned_integer_type!(u16);
impl_from_unsigned_integer_type!(u8);

#[test]
fn test_basic() {
    let mut it = IntIterator {
        unum: 123usize,
        is_negative: false,
    };
    assert_eq!(Some(3), it.next());
    assert_eq!(Some(2), it.next());
    assert_eq!(Some(1), it.next());
    assert_eq!(None, it.next());
}

#[test]
fn test_zeros() {
    let mut it = IntIterator {
        unum: 9080usize,
        is_negative: false,
    };
    assert_eq!(Some(0), it.next());
    assert_eq!(Some(8), it.next());
    assert_eq!(Some(0), it.next());
    assert_eq!(Some(9), it.next());
    assert_eq!(None, it.next());
}
