// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! Documentation on implementing custom VarULE types.
//!
//! This module contains documentation for defining custom VarULE types,
//! especially those using complex custom dynamically sized types.
//!
//! In *most cases* you should be able to create custom VarULE types using
//! [`#[make_varule]`](crate::make_ule).
//!
//! # Example
//!
//! For example, if your regular stack type is:
//!
//! ```rust
//! use zerofrom::ZeroFrom;
//! use zerovec::ule::*;
//! use zerovec::ZeroVec;
//!
//! #[derive(serde::Serialize, serde::Deserialize)]
//! struct Foo<'a> {
//!     field1: char,
//!     field2: u32,
//!     #[serde(borrow)]
//!     field3: ZeroVec<'a, u32>,
//! }
//! ```
//!
//! then the ULE type will be implemented as follows. Ideally, you should have
//! `EncodeAsVarULE` and `ZeroFrom` implementations on `Foo` pertaining to `FooULE`,
//! as well as a `Serialize` impl on `FooULE` and a `Deserialize` impl on `Box<FooULE>`
//! to enable human-readable serialization and deserialization.
//!
//! ```rust
//! use zerovec::{ZeroVec, VarZeroVec, ZeroSlice};
//! use zerovec::ule::*;
//! use zerofrom::ZeroFrom;
//! use core::mem;
//!
//! # #[derive(serde::Serialize, serde::Deserialize)]
//! # struct Foo<'a> {
//! #    field1: char,
//! #    field2: u32,
//! #    #[serde(borrow)]
//! #    field3: ZeroVec<'a, u32>   
//! # }
//!
//! // Must be repr(C, packed) for safety of VarULE!
//! // Must also only contain ULE types
//! #[repr(C, packed)]
//! struct FooULE {
//!     field1: <char as AsULE>::ULE,   
//!     field2: <u32 as AsULE>::ULE,
//!     field3: ZeroSlice<u32>,
//! }
//!
//! // Safety (based on the safety checklist on the VarULE trait):
//! //  1. FooULE does not include any uninitialized or padding bytes. (achieved by `#[repr(C, packed)]` on
//! //     a struct with only ULE fields)
//! //  2. FooULE is aligned to 1 byte. (achieved by `#[repr(C, packed)]` on
//! //     a struct with only ULE fields)
//! //  3. The impl of `validate_bytes()` returns an error if any byte is not valid.
//! //  4. The impl of `validate_bytes()` returns an error if the slice cannot be used in its entirety
//! //  5. The impl of `from_bytes_unchecked()` returns a reference to the same data.
//! //  6. The other VarULE methods use the default impl.
//! //  7. FooULE byte equality is semantic equality
//! unsafe impl VarULE for FooULE {
//!     fn validate_bytes(bytes: &[u8]) -> Result<(), UleError> {
//!         // validate each field
//!         <char as AsULE>::ULE::validate_bytes(&bytes[0..3]).map_err(|_| UleError::parse::<Self>())?;
//!         <u32 as AsULE>::ULE::validate_bytes(&bytes[3..7]).map_err(|_| UleError::parse::<Self>())?;
//!         let _ = ZeroVec::<u32>::parse_bytes(&bytes[7..]).map_err(|_| UleError::parse::<Self>())?;
//!         Ok(())
//!     }
//!     unsafe fn from_bytes_unchecked(bytes: &[u8]) -> &Self {
//!         let ptr = bytes.as_ptr();
//!         let len = bytes.len();
//!         // subtract the length of the char and u32 to get the length of the array
//!         let len_new = (len - 7) / 4;
//!         // it's hard constructing custom DSTs, we fake a pointer/length construction
//!         // eventually we can use the Pointer::Metadata APIs when they stabilize
//!         let fake_slice = core::ptr::slice_from_raw_parts(ptr as *const <u32 as AsULE>::ULE, len_new);
//!         &*(fake_slice as *const Self)
//!     }
//! }
//!
//! unsafe impl EncodeAsVarULE<FooULE> for Foo<'_> {
//!    fn encode_var_ule_as_slices<R>(&self, cb: impl FnOnce(&[&[u8]]) -> R) -> R {
//!        // take each field, convert to ULE byte slices, and pass them through
//!        cb(&[<char as AsULE>::ULE::slice_as_bytes(&[self.field1.to_unaligned()]),
//!             <u32 as AsULE>::ULE::slice_as_bytes(&[self.field2.to_unaligned()]),
//!             // the ZeroVec is already in the correct slice format
//!             self.field3.as_bytes()])
//!    }
//! }
//!
//! impl<'a> ZeroFrom<'a, FooULE> for Foo<'a> {
//!     fn zero_from(other: &'a FooULE) -> Self {
//!         Self {
//!             field1: AsULE::from_unaligned(other.field1),
//!             field2: AsULE::from_unaligned(other.field2),
//!             field3: ZeroFrom::zero_from(&other.field3),
//!         }
//!     }
//! }
//!
//!
//! impl serde::Serialize for FooULE
//! {
//!     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//!     where
//!         S: serde::Serializer,
//!     {
//!         Foo::zero_from(self).serialize(serializer)
//!     }
//! }
//!
//! impl<'de> serde::Deserialize<'de> for Box<FooULE>
//! {
//!     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//!     where
//!         D: serde::Deserializer<'de>,
//!     {
//!         let mut foo = Foo::deserialize(deserializer)?;
//!         Ok(encode_varule_to_box(&foo))
//!     }
//! }
//!
//! fn main() {
//!     let mut foos = [Foo {field1: 'u', field2: 983, field3: ZeroVec::alloc_from_slice(&[1212,2309,500,7000])},
//!                     Foo {field1: 'l', field2: 1010, field3: ZeroVec::alloc_from_slice(&[1932, 0, 8888, 91237])}];
//!
//!     let vzv = VarZeroVec::<_>::from(&foos);
//!
//!     assert_eq!(char::from_unaligned(vzv.get(0).unwrap().field1), 'u');
//!     assert_eq!(u32::from_unaligned(vzv.get(0).unwrap().field2), 983);
//!     assert_eq!(&vzv.get(0).unwrap().field3, &[1212,2309,500,7000][..]);
//!
//!     assert_eq!(char::from_unaligned(vzv.get(1).unwrap().field1), 'l');
//!     assert_eq!(u32::from_unaligned(vzv.get(1).unwrap().field2), 1010);
//!     assert_eq!(&vzv.get(1).unwrap().field3, &[1932, 0, 8888, 91237][..]);
//! }
//! ```
