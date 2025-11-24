// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! [`VarULE`] impls for tuples.
//!
//! This module exports [`Tuple2VarULE`], [`Tuple3VarULE`], ..., the corresponding [`VarULE`] types
//! of tuples containing purely [`VarULE`] types.
//!
//! This can be paired with [`VarTupleULE`] to make arbitrary combinations of [`ULE`] and [`VarULE`] types.
//!
//! [`VarTupleULE`]: crate::ule::vartuple::VarTupleULE

use super::*;
use crate::varzerovec::{Index16, VarZeroVecFormat};
use core::fmt;
use core::marker::PhantomData;
use core::mem;
use zerofrom::ZeroFrom;

macro_rules! tuple_varule {
    // Invocation: Should be called like `tuple_ule!(Tuple2VarULE, 2, [ A a AX 0, B b BX 1 ])`
    //
    // $T is a generic name, $t is a lowercase version of it, $T_alt is an "alternate" name to use when we need two types referring
    // to the same input field, $i is an index.
    //
    // $name is the name of the type, $len MUST be the total number of fields, and then $i must be an integer going from 0 to (n - 1) in sequence
    // (This macro code can rely on $i < $len)
    ($name:ident, $len:literal, [ $($T:ident $t:ident $T_alt: ident $i:tt),+ ]) => {
        #[doc = concat!("VarULE type for tuples with ", $len, " elements. See module docs for more information")]
        #[repr(transparent)]
        #[allow(clippy::exhaustive_structs)] // stable
        pub struct $name<$($T: ?Sized,)+ Format: VarZeroVecFormat = Index16> {
            $($t: PhantomData<$T>,)+
            // Safety invariant: Each "field" $i of the MultiFieldsULE is a valid instance of $t
            //
            // In other words, calling `.get_field::<$T>($i)` is always safe.
            //
            // This invariant is upheld when this type is constructed during VarULE parsing/validation
            multi: MultiFieldsULE<$len, Format>
        }

        impl<$($T: VarULE + ?Sized,)+ Format: VarZeroVecFormat> $name<$($T,)+ Format> {
            $(
                #[doc = concat!("Get field ", $i, "of this tuple")]
                pub fn $t(&self) -> &$T {
                     // Safety: See invariant of `multi`.
                    unsafe {
                        self.multi.get_field::<$T>($i)
                    }
                }


            )+
        }

        // # Safety
        //
        // ## Checklist
        //
        // Safety checklist for `VarULE`:
        //
        // 1. align(1): repr(transparent) around an align(1) VarULE type: MultiFieldsULE
        // 2. No padding: see previous point
        // 3. `validate_bytes` validates that this type is a valid MultiFieldsULE, and that each field is the correct type from the tuple.
        // 4. `validate_bytes` checks length by deferring to the inner ULEs
        // 5. `from_bytes_unchecked` returns a fat pointer to the bytes.
        // 6. All other methods are left at their default impl.
        // 7. The inner ULEs have byte equality, so this composition has byte equality.
        unsafe impl<$($T: VarULE + ?Sized,)+ Format: VarZeroVecFormat> VarULE for $name<$($T,)+ Format>
        {
            fn validate_bytes(bytes: &[u8]) -> Result<(), UleError> {
                // Safety: We validate that this type is the same kind of MultiFieldsULE (with $len, Format)
                // as in the type def
                let multi = <MultiFieldsULE<$len, Format> as VarULE>::parse_bytes(bytes)?;
                $(
                    // Safety invariant: $i < $len, from the macro invocation
                    unsafe {
                        multi.validate_field::<$T>($i)?;
                    }
                )+
                Ok(())
            }

            unsafe fn from_bytes_unchecked(bytes: &[u8]) -> &Self {
                 // Safety: We validate that this type is the same kind of MultiFieldsULE (with $len, Format)
                // as in the type def
                let multi = <MultiFieldsULE<$len, Format> as VarULE>::from_bytes_unchecked(bytes);

                // This type is repr(transparent) over MultiFieldsULE<$len>, so its slices can be transmuted
                // Field invariant upheld here: validate_bytes above validates every field for being the right type
                mem::transmute::<&MultiFieldsULE<$len, Format>, &Self>(multi)
            }
        }

        impl<$($T: fmt::Debug + VarULE + ?Sized,)+ Format: VarZeroVecFormat> fmt::Debug for $name<$($T,)+ Format> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
                ($(self.$t(),)+).fmt(f)
            }
        }

        // We need manual impls since `#[derive()]` is disallowed on packed types
        impl<$($T: PartialEq + VarULE + ?Sized,)+ Format: VarZeroVecFormat> PartialEq for $name<$($T,)+ Format> {
            fn eq(&self, other: &Self) -> bool {

                ($(self.$t(),)+).eq(&($(other.$t(),)+))
            }
        }

        impl<$($T: Eq + VarULE + ?Sized,)+ Format: VarZeroVecFormat> Eq for $name<$($T,)+ Format> {}

        impl<$($T: PartialOrd + VarULE + ?Sized,)+ Format: VarZeroVecFormat> PartialOrd for $name<$($T,)+ Format> {
            fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
                ($(self.$t(),)+).partial_cmp(&($(other.$t(),)+))
            }
        }

        impl<$($T: Ord + VarULE + ?Sized,)+ Format: VarZeroVecFormat> Ord for $name<$($T,)+ Format>  {
            fn cmp(&self, other: &Self) -> core::cmp::Ordering {
                ($(self.$t(),)+).cmp(&($(other.$t(),)+))
            }
        }

        // # Safety
        //
        // encode_var_ule_len: returns the length of the individual VarULEs together.
        //
        // encode_var_ule_write: writes bytes by deferring to the inner VarULE impls.
        unsafe impl<$($T,)+ $($T_alt,)+ Format> EncodeAsVarULE<$name<$($T,)+ Format>> for ( $($T_alt),+ )
        where
            $($T: VarULE + ?Sized,)+
            $($T_alt: EncodeAsVarULE<$T>,)+
            Format: VarZeroVecFormat,
        {
            fn encode_var_ule_as_slices<R>(&self, _: impl FnOnce(&[&[u8]]) -> R) -> R {
                // unnecessary if the other two are implemented
                unreachable!()
            }

            #[inline]
            fn encode_var_ule_len(&self) -> usize {
                // Safety: We validate that this type is the same kind of MultiFieldsULE (with $len, Format)
                // as in the type def
                MultiFieldsULE::<$len, Format>::compute_encoded_len_for([$(self.$i.encode_var_ule_len()),+])
            }

            #[inline]
            fn encode_var_ule_write(&self, dst: &mut [u8]) {
                let lengths = [$(self.$i.encode_var_ule_len()),+];
                // Safety: We validate that this type is the same kind of MultiFieldsULE (with $len, Format)
                // as in the type def
                let multi = MultiFieldsULE::<$len, Format>::new_from_lengths_partially_initialized(lengths, dst);
                $(
                    // Safety: $i < $len, from the macro invocation, and field $i is supposed to be of type $T
                    unsafe {
                        multi.set_field_at::<$T, $T_alt>($i, &self.$i);
                    }
                )+
            }
        }

        #[cfg(feature = "alloc")]
        impl<$($T: VarULE + ?Sized,)+ Format: VarZeroVecFormat> alloc::borrow::ToOwned for $name<$($T,)+ Format> {
            type Owned = alloc::boxed::Box<Self>;
            fn to_owned(&self) -> Self::Owned {
                encode_varule_to_box(self)
            }
        }

        impl<'a, $($T,)+ $($T_alt,)+ Format> ZeroFrom <'a, $name<$($T,)+ Format>> for ($($T_alt),+)
        where
                    $($T: VarULE + ?Sized,)+
                    $($T_alt: ZeroFrom<'a, $T>,)+
                    Format: VarZeroVecFormat {
            fn zero_from(other: &'a $name<$($T,)+ Format>) -> Self {
                (
                    $($T_alt::zero_from(other.$t()),)+
                )
            }
        }

        #[cfg(feature = "serde")]
        impl<$($T: serde::Serialize,)+ Format> serde::Serialize for $name<$($T,)+ Format>
        where
            $($T: VarULE + ?Sized,)+
            // This impl should be present on almost all VarULE types. if it isn't, that is a bug
            $(for<'a> &'a $T: ZeroFrom<'a, $T>,)+
            Format: VarZeroVecFormat
        {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: serde::Serializer {
                if serializer.is_human_readable() {
                    let this = (
                        $(self.$t()),+
                    );
                    <($(&$T),+) as serde::Serialize>::serialize(&this, serializer)
                } else {
                    serializer.serialize_bytes(self.multi.as_bytes())
                }
            }
        }

        #[cfg(all(feature = "serde", feature = "alloc"))]
        impl<'de, $($T: VarULE + ?Sized,)+ Format> serde::Deserialize<'de> for alloc::boxed::Box<$name<$($T,)+ Format>>
            where
                // This impl should be present on almost all deserializable VarULE types
                $( alloc::boxed::Box<$T>: serde::Deserialize<'de>,)+
                Format: VarZeroVecFormat {
            fn deserialize<Des>(deserializer: Des) -> Result<Self, Des::Error> where Des: serde::Deserializer<'de> {
                if deserializer.is_human_readable() {
                    let this = <( $(alloc::boxed::Box<$T>),+) as serde::Deserialize>::deserialize(deserializer)?;
                    let this_ref = (
                        $(&*this.$i),+
                    );
                    Ok(crate::ule::encode_varule_to_box(&this_ref))
                } else {
                    // This branch should usually not be hit, since Cow-like use cases will hit the Deserialize impl for &'a TupleNVarULE instead.

                    let deserialized = <&$name<$($T,)+ Format>>::deserialize(deserializer)?;
                    Ok(deserialized.to_boxed())
                }
            }
        }

        #[cfg(feature = "serde")]
        impl<'a, 'de: 'a, $($T: VarULE + ?Sized,)+ Format: VarZeroVecFormat> serde::Deserialize<'de> for &'a $name<$($T,)+ Format> {
            fn deserialize<Des>(deserializer: Des) -> Result<Self, Des::Error> where Des: serde::Deserializer<'de> {
                if deserializer.is_human_readable() {
                    Err(serde::de::Error::custom(
                        concat!("&", stringify!($name), " can only deserialize in zero-copy ways"),
                    ))
                } else {
                    let bytes = <&[u8]>::deserialize(deserializer)?;
                    $name::<$($T,)+ Format>::parse_bytes(bytes).map_err(serde::de::Error::custom)
                }
            }
        }
    };
}

tuple_varule!(Tuple2VarULE, 2, [ A a AE 0, B b BE 1 ]);
tuple_varule!(Tuple3VarULE, 3, [ A a AE 0, B b BE 1, C c CE 2 ]);
tuple_varule!(Tuple4VarULE, 4, [ A a AE 0, B b BE 1, C c CE 2, D d DE 3 ]);
tuple_varule!(Tuple5VarULE, 5, [ A a AE 0, B b BE 1, C c CE 2, D d DE 3, E e EE 4 ]);
tuple_varule!(Tuple6VarULE, 6, [ A a AE 0, B b BE 1, C c CE 2, D d DE 3, E e EE 4, F f FE 5 ]);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::varzerovec::{Index16, Index32, Index8, VarZeroVecFormat};
    use crate::VarZeroSlice;
    use crate::VarZeroVec;

    #[test]
    fn test_pairvarule_validate() {
        let vec: Vec<(&str, &[u8])> = vec![("a", b"b"), ("foo", b"bar"), ("lorem", b"ipsum\xFF")];
        let zerovec: VarZeroVec<Tuple2VarULE<str, [u8]>> = (&vec).into();
        let bytes = zerovec.as_bytes();
        let zerovec2 = VarZeroVec::parse_bytes(bytes).unwrap();
        assert_eq!(zerovec, zerovec2);

        // Test failed validation with a correctly sized but differently constrained tuple
        // Note: ipsum\xFF is not a valid str
        let zerovec3 = VarZeroVec::<Tuple2VarULE<str, str>>::parse_bytes(bytes);
        assert!(zerovec3.is_err());

        #[cfg(feature = "serde")]
        for val in zerovec.iter() {
            // Can't use inference due to https://github.com/rust-lang/rust/issues/130180
            crate::ule::test_utils::assert_serde_roundtrips::<Tuple2VarULE<str, [u8]>>(val);
        }
    }
    fn test_tripleule_validate_inner<Format: VarZeroVecFormat>() {
        let vec: Vec<(&str, &[u8], VarZeroVec<str>)> = vec![
            ("a", b"b", (&vec!["a", "b", "c"]).into()),
            ("foo", b"bar", (&vec!["baz", "quux"]).into()),
            (
                "lorem",
                b"ipsum\xFF",
                (&vec!["dolor", "sit", "amet"]).into(),
            ),
        ];
        let zerovec: VarZeroVec<Tuple3VarULE<str, [u8], VarZeroSlice<str>, Format>> = (&vec).into();
        let bytes = zerovec.as_bytes();
        let zerovec2 = VarZeroVec::parse_bytes(bytes).unwrap();
        assert_eq!(zerovec, zerovec2);

        // Test failed validation with a correctly sized but differently constrained tuple
        // Note: the str is unlikely to be a valid varzerovec
        let zerovec3 = VarZeroVec::<Tuple3VarULE<VarZeroSlice<str>, [u8], VarZeroSlice<str>, Format>>::parse_bytes(bytes);
        assert!(zerovec3.is_err());

        #[cfg(feature = "serde")]
        for val in zerovec.iter() {
            // Can't use inference due to https://github.com/rust-lang/rust/issues/130180
            crate::ule::test_utils::assert_serde_roundtrips::<
                Tuple3VarULE<str, [u8], VarZeroSlice<str>, Format>,
            >(val);
        }
    }

    #[test]
    fn test_tripleule_validate() {
        test_tripleule_validate_inner::<Index8>();
        test_tripleule_validate_inner::<Index16>();
        test_tripleule_validate_inner::<Index32>();
    }
}
