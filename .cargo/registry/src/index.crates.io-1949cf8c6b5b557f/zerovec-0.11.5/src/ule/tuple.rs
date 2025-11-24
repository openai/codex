// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! ULE impls for tuples.
//!
//! Rust does not guarantee the layout of tuples, so ZeroVec defines its own tuple ULE types.
//!
//! Impls are defined for tuples of up to 6 elements. For longer tuples, use a custom struct
//! with [`#[make_ule]`](crate::make_ule).
//!
//! # Examples
//!
//! ```
//! use zerovec::ZeroVec;
//!
//! // ZeroVec of tuples!
//! let zerovec: ZeroVec<(u32, char)> = [(1, 'a'), (1234901, '啊'), (100, 'अ')]
//!     .iter()
//!     .copied()
//!     .collect();
//!
//! assert_eq!(zerovec.get(1), Some((1234901, '啊')));
//! ```

use super::*;
use core::fmt;
use core::mem;

macro_rules! tuple_ule {
    ($name:ident, $len:literal, [ $($t:ident $i:tt),+ ]) => {
        #[doc = concat!("ULE type for tuples with ", $len, " elements.")]
        #[repr(C, packed)]
        #[allow(clippy::exhaustive_structs)] // stable
        pub struct $name<$($t),+>($(pub $t),+);

        // Safety (based on the safety checklist on the ULE trait):
        //  1. TupleULE does not include any uninitialized or padding bytes.
        //     (achieved by `#[repr(C, packed)]` on a struct containing only ULE fields)
        //  2. TupleULE is aligned to 1 byte.
        //     (achieved by `#[repr(C, packed)]` on a struct containing only ULE fields)
        //  3. The impl of validate_bytes() returns an error if any byte is not valid.
        //  4. The impl of validate_bytes() returns an error if there are extra bytes.
        //  5. The other ULE methods use the default impl.
        //  6. TupleULE byte equality is semantic equality by relying on the ULE equality
        //     invariant on the subfields
        unsafe impl<$($t: ULE),+> ULE for $name<$($t),+> {
            fn validate_bytes(bytes: &[u8]) -> Result<(), UleError> {
                // expands to: 0size + mem::size_of::<A>() + mem::size_of::<B>();
                let ule_bytes = 0usize $(+ mem::size_of::<$t>())+;
                if bytes.len() % ule_bytes != 0 {
                    return Err(UleError::length::<Self>(bytes.len()));
                }
                for chunk in bytes.chunks(ule_bytes) {
                    let mut i = 0;
                    $(
                        let j = i;
                        i += mem::size_of::<$t>();
                        #[expect(clippy::indexing_slicing)] // length checked
                        <$t>::validate_bytes(&chunk[j..i])?;
                    )+
                }
                Ok(())
            }
        }

        impl<$($t: AsULE),+> AsULE for ($($t),+) {
            type ULE = $name<$(<$t>::ULE),+>;

            #[inline]
            fn to_unaligned(self) -> Self::ULE {
                $name($(
                    self.$i.to_unaligned()
                ),+)
            }

            #[inline]
            fn from_unaligned(unaligned: Self::ULE) -> Self {
                ($(
                    <$t>::from_unaligned(unaligned.$i)
                ),+)
            }
        }

        impl<$($t: fmt::Debug + ULE),+> fmt::Debug for $name<$($t),+> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
                ($(self.$i),+).fmt(f)
            }
        }

        // We need manual impls since `#[derive()]` is disallowed on packed types
        impl<$($t: PartialEq + ULE),+> PartialEq for $name<$($t),+> {
            fn eq(&self, other: &Self) -> bool {
                ($(self.$i),+).eq(&($(other.$i),+))
            }
        }

        impl<$($t: Eq + ULE),+> Eq for $name<$($t),+> {}

        impl<$($t: PartialOrd + ULE),+> PartialOrd for $name<$($t),+> {
            fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
                ($(self.$i),+).partial_cmp(&($(other.$i),+))
            }
        }

        impl<$($t: Ord + ULE),+> Ord for $name<$($t),+> {
            fn cmp(&self, other: &Self) -> core::cmp::Ordering {
                ($(self.$i),+).cmp(&($(other.$i),+))
            }
        }

        impl<$($t: ULE),+> Clone for $name<$($t),+> {
            fn clone(&self) -> Self {
                *self
            }
        }

        impl<$($t: ULE),+> Copy for $name<$($t),+> {}

        #[cfg(feature = "alloc")]
        impl<'a, $($t: Ord + AsULE + 'static),+> crate::map::ZeroMapKV<'a> for ($($t),+) {
            type Container = crate::ZeroVec<'a, ($($t),+)>;
            type Slice = crate::ZeroSlice<($($t),+)>;
            type GetType = $name<$(<$t>::ULE),+>;
            type OwnedType = ($($t),+);
        }
    };
}

tuple_ule!(Tuple2ULE, "2", [ A 0, B 1 ]);
tuple_ule!(Tuple3ULE, "3", [ A 0, B 1, C 2 ]);
tuple_ule!(Tuple4ULE, "4", [ A 0, B 1, C 2, D 3 ]);
tuple_ule!(Tuple5ULE, "5", [ A 0, B 1, C 2, D 3, E 4 ]);
tuple_ule!(Tuple6ULE, "6", [ A 0, B 1, C 2, D 3, E 4, F 5 ]);

#[test]
fn test_pairule_validate() {
    use crate::ZeroVec;
    let vec: Vec<(u32, char)> = vec![(1, 'a'), (1234901, '啊'), (100, 'अ')];
    let zerovec: ZeroVec<(u32, char)> = vec.iter().copied().collect();
    let bytes = zerovec.as_bytes();
    let zerovec2 = ZeroVec::parse_bytes(bytes).unwrap();
    assert_eq!(zerovec, zerovec2);

    // Test failed validation with a correctly sized but differently constrained tuple
    // Note: 1234901 is not a valid char
    let zerovec3 = ZeroVec::<(char, u32)>::parse_bytes(bytes);
    assert!(zerovec3.is_err());
}

#[test]
fn test_tripleule_validate() {
    use crate::ZeroVec;
    let vec: Vec<(u32, char, i8)> = vec![(1, 'a', -5), (1234901, '啊', 3), (100, 'अ', -127)];
    let zerovec: ZeroVec<(u32, char, i8)> = vec.iter().copied().collect();
    let bytes = zerovec.as_bytes();
    let zerovec2 = ZeroVec::parse_bytes(bytes).unwrap();
    assert_eq!(zerovec, zerovec2);

    // Test failed validation with a correctly sized but differently constrained tuple
    // Note: 1234901 is not a valid char
    let zerovec3 = ZeroVec::<(char, i8, u32)>::parse_bytes(bytes);
    assert!(zerovec3.is_err());
}

#[test]
fn test_quadule_validate() {
    use crate::ZeroVec;
    let vec: Vec<(u32, char, i8, u16)> =
        vec![(1, 'a', -5, 3), (1234901, '啊', 3, 11), (100, 'अ', -127, 0)];
    let zerovec: ZeroVec<(u32, char, i8, u16)> = vec.iter().copied().collect();
    let bytes = zerovec.as_bytes();
    let zerovec2 = ZeroVec::parse_bytes(bytes).unwrap();
    assert_eq!(zerovec, zerovec2);

    // Test failed validation with a correctly sized but differently constrained tuple
    // Note: 1234901 is not a valid char
    let zerovec3 = ZeroVec::<(char, i8, u16, u32)>::parse_bytes(bytes);
    assert!(zerovec3.is_err());
}
