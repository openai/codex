// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

// In this case consistency between impls is more important
// than using pointer casts
#![allow(clippy::transmute_ptr_to_ptr)]

use crate::ZeroFrom;

macro_rules! impl_copy_type {
    ($ty:ident) => {
        impl<'a> ZeroFrom<'a, $ty> for $ty {
            #[inline]
            fn zero_from(this: &'a Self) -> Self {
                // Essentially only works when the struct is fully Copy
                *this
            }
        }
    };
}

impl_copy_type!(u8);
impl_copy_type!(u16);
impl_copy_type!(u32);
impl_copy_type!(u64);
impl_copy_type!(u128);
impl_copy_type!(usize);
impl_copy_type!(i8);
impl_copy_type!(i16);
impl_copy_type!(i32);
impl_copy_type!(i64);
impl_copy_type!(i128);
impl_copy_type!(isize);
impl_copy_type!(char);
impl_copy_type!(bool);

// This can be cleaned up once `[T; N]`::each_ref() is stabilized
// https://github.com/rust-lang/rust/issues/76118
macro_rules! array_zf_impl {
    ($n:expr; $($i:expr),+) => {
        impl<'a, C, T: ZeroFrom<'a, C>> ZeroFrom<'a, [C; $n]> for [T; $n] {
            fn zero_from(this: &'a [C; $n]) -> Self {
                [
                    $(
                        <T as ZeroFrom<C>>::zero_from(&this[$i])
                    ),+

                ]
            }
        }
    }
}

array_zf_impl!(1; 0);
array_zf_impl!(2; 0, 1);
array_zf_impl!(3; 0, 1, 2);
array_zf_impl!(4; 0, 1, 2, 3);
array_zf_impl!(5; 0, 1, 2, 3, 4);
array_zf_impl!(6; 0, 1, 2, 3, 4, 5);
array_zf_impl!(7; 0, 1, 2, 3, 4, 5, 6);
array_zf_impl!(8; 0, 1, 2, 3, 4, 5, 6, 7);
array_zf_impl!(9; 0, 1, 2, 3, 4, 5, 6, 7, 8);
array_zf_impl!(10; 0, 1, 2, 3, 4, 5, 6, 7, 8, 9);
array_zf_impl!(11; 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10);
array_zf_impl!(12; 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11);
array_zf_impl!(13; 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12);
array_zf_impl!(14; 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13);
array_zf_impl!(15; 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14);
array_zf_impl!(16; 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15);

macro_rules! tuple_zf_impl {
    ($(($c:ident, $t:ident, $i:tt)),+) => {
        impl<'zf, $($c, $t: ZeroFrom<'zf, $c>),+> ZeroFrom<'zf, ($($c),+)>
            for ($($t),+)
            {
            fn zero_from(other: &'zf ($($c),+)) -> Self {
                (
                    $(<$t as ZeroFrom<$c>>::zero_from(&other.$i)),+
                )
            }
        }
    };
}

tuple_zf_impl!((C1, T1, 0), (C2, T2, 1));
tuple_zf_impl!((C1, T1, 0), (C2, T2, 1), (C3, T3, 2));
tuple_zf_impl!((C1, T1, 0), (C2, T2, 1), (C3, T3, 2), (C4, T4, 3));
tuple_zf_impl!(
    (C1, T1, 0),
    (C2, T2, 1),
    (C3, T3, 2),
    (C4, T4, 3),
    (C5, T5, 4)
);
tuple_zf_impl!(
    (C1, T1, 0),
    (C2, T2, 1),
    (C3, T3, 2),
    (C4, T4, 3),
    (C5, T5, 4),
    (C6, T6, 5)
);
tuple_zf_impl!(
    (C1, T1, 0),
    (C2, T2, 1),
    (C3, T3, 2),
    (C4, T4, 3),
    (C5, T5, 4),
    (C6, T6, 5),
    (C7, T7, 6)
);
tuple_zf_impl!(
    (C1, T1, 0),
    (C2, T2, 1),
    (C3, T3, 2),
    (C4, T4, 3),
    (C5, T5, 4),
    (C6, T6, 5),
    (C7, T7, 6),
    (C8, T8, 7)
);
tuple_zf_impl!(
    (C1, T1, 0),
    (C2, T2, 1),
    (C3, T3, 2),
    (C4, T4, 3),
    (C5, T5, 4),
    (C6, T6, 5),
    (C7, T7, 6),
    (C8, T8, 7),
    (C9, T9, 8)
);
tuple_zf_impl!(
    (C1, T1, 0),
    (C2, T2, 1),
    (C3, T3, 2),
    (C4, T4, 3),
    (C5, T5, 4),
    (C6, T6, 5),
    (C7, T7, 6),
    (C8, T8, 7),
    (C9, T9, 8),
    (C10, T10, 9)
);
