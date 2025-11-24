// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::ule::{EncodeAsVarULE, UleError, VarULE};
#[cfg(feature = "alloc")]
use alloc::boxed::Box;
use core::fmt;
use core::marker::PhantomData;
#[cfg(feature = "alloc")]
use core::mem::ManuallyDrop;
use core::ops::Deref;
use core::ptr::NonNull;
use zerofrom::ZeroFrom;

/// Copy-on-write type that efficiently represents [`VarULE`] types as their bitstream representation.
///
/// The primary use case for [`VarULE`] types is the ability to store complex variable-length datastructures
/// inside variable-length collections like [`crate::VarZeroVec`].
///
/// Underlying this ability is the fact that [`VarULE`] types can be efficiently represented as a flat
/// bytestream.
///
/// In zero-copy cases, sometimes one wishes to unconditionally use this bytestream representation, for example
/// to save stack size. A struct with five `Cow<'a, str>`s is not as stack-efficient as a single `Cow` containing
/// the bytestream representation of, say, `Tuple5VarULE<str, str, str, str, str>`.
///
/// This type helps in this case: It is logically a `Cow<'a, V>`, with some optimizations, that is guaranteed
/// to serialize as a byte stream in machine-readable scenarios.
///
/// During human-readable serialization, it will fall back to the serde impls on `V`, which ought to have
/// a human-readable variant.
pub struct VarZeroCow<'a, V: ?Sized> {
    /// Safety invariant: Contained slice must be a valid V
    /// It may or may not have a lifetime valid for 'a, it must be valid for as long as this type is around.
    raw: RawVarZeroCow,
    marker1: PhantomData<&'a V>,
    #[cfg(feature = "alloc")]
    marker2: PhantomData<Box<V>>,
}

/// VarZeroCow without the `V` to simulate a dropck eyepatch
/// (i.e., prove to rustc that the dtor is not able to observe V or 'a)
///
/// This is effectively `Cow<'a, [u8]>`, with the lifetime managed externally
struct RawVarZeroCow {
    /// Pointer to data
    ///
    /// # Safety Invariants
    ///
    /// 1. This slice must always be valid as a byte slice
    /// 2. If `owned` is true, this slice can be freed.
    /// 3. VarZeroCow, the only user of this type, will impose an additional invariant that the buffer is a valid V
    buf: NonNull<[u8]>,
    /// The buffer is `Box<[u8]>` if true
    #[cfg(feature = "alloc")]
    owned: bool,
    // Safety: We do not need any PhantomDatas here, since the Drop impl does not observe borrowed data
    // if there is any.
}

#[cfg(feature = "alloc")]
impl Drop for RawVarZeroCow {
    fn drop(&mut self) {
        // Note: this drop impl NEVER observes borrowed data (which may have already been cleaned up by the time the impl is called)
        if self.owned {
            unsafe {
                // Safety: (Invariant 2 on buf)
                // since owned is true, this is a valid Box<[u8]> and can be cleaned up
                let _ = Box::<[u8]>::from_raw(self.buf.as_ptr());
            }
        }
    }
}

// This is mostly just a `Cow<[u8]>`, safe to implement Send and Sync on
unsafe impl Send for RawVarZeroCow {}
unsafe impl Sync for RawVarZeroCow {}

impl Clone for RawVarZeroCow {
    fn clone(&self) -> Self {
        #[cfg(feature = "alloc")]
        if self.is_owned() {
            // This clones the box
            let b: Box<[u8]> = self.as_bytes().into();
            let b = ManuallyDrop::new(b);
            let buf: NonNull<[u8]> = (&**b).into();
            return Self {
                // Invariants upheld:
                // 1 & 3: The bytes came from `self` so they're a valid value and byte slice
                // 2: This is owned (we cloned it), so we set owned to true.
                buf,
                owned: true,
            };
        }
        // Unfortunately we can't just use `new_borrowed(self.deref())` since the lifetime is shorter
        Self {
            // Invariants upheld:
            // 1 & 3: The bytes came from `self` so they're a valid value and byte slice
            // 2: This is borrowed (we're sharing a borrow), so we set owned to false.
            buf: self.buf,
            #[cfg(feature = "alloc")]
            owned: false,
        }
    }
}

impl<'a, V: ?Sized> Clone for VarZeroCow<'a, V> {
    fn clone(&self) -> Self {
        let raw = self.raw.clone();
        // Invariant upheld: raw came from a valid VarZeroCow, so it
        // is a valid V
        unsafe { Self::from_raw(raw) }
    }
}

impl<'a, V: VarULE + ?Sized> VarZeroCow<'a, V> {
    /// Construct from a slice. Errors if the slice doesn't represent a valid `V`
    pub fn parse_bytes(bytes: &'a [u8]) -> Result<Self, UleError> {
        let val = V::parse_bytes(bytes)?;
        Ok(Self::new_borrowed(val))
    }

    /// Construct from an owned slice. Errors if the slice doesn't represent a valid `V`
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    #[cfg(feature = "alloc")]
    pub fn parse_owned_bytes(bytes: Box<[u8]>) -> Result<Self, UleError> {
        V::validate_bytes(&bytes)?;
        let bytes = ManuallyDrop::new(bytes);
        let buf: NonNull<[u8]> = (&**bytes).into();
        let raw = RawVarZeroCow {
            // Invariants upheld:
            // 1 & 3: The bytes came from `val` so they're a valid value and byte slice
            // 2: This is owned, so we set owned to true.
            buf,
            owned: true,
        };
        Ok(Self {
            raw,
            marker1: PhantomData,
            #[cfg(feature = "alloc")]
            marker2: PhantomData,
        })
    }

    /// Construct from a slice that is known to represent a valid `V`
    ///
    /// # Safety
    ///
    /// `bytes` must be a valid `V`, i.e. it must successfully pass through
    /// `V::parse_bytes()` or `V::validate_bytes()`.
    pub const unsafe fn from_bytes_unchecked(bytes: &'a [u8]) -> Self {
        unsafe {
            // Safety: bytes is an &T which is always non-null
            let buf: NonNull<[u8]> = NonNull::new_unchecked(bytes as *const [u8] as *mut [u8]);
            let raw = RawVarZeroCow {
                // Invariants upheld:
                // 1 & 3: Passed upstream to caller
                // 2: This is borrowed, so we set owned to false.
                buf,
                #[cfg(feature = "alloc")]
                owned: false,
            };
            // Invariant passed upstream to caller
            Self::from_raw(raw)
        }
    }

    /// Construct this from an [`EncodeAsVarULE`] version of the contained type
    ///
    /// Will always construct an owned version
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    #[cfg(feature = "alloc")]
    pub fn from_encodeable<E: EncodeAsVarULE<V>>(encodeable: &E) -> Self {
        let b = crate::ule::encode_varule_to_box(encodeable);
        Self::new_owned(b)
    }

    /// Construct a new borrowed version of this
    pub fn new_borrowed(val: &'a V) -> Self {
        unsafe {
            // Safety: val is a valid V, by type
            Self::from_bytes_unchecked(val.as_bytes())
        }
    }

    /// Construct a new borrowed version of this
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    #[cfg(feature = "alloc")]
    pub fn new_owned(val: Box<V>) -> Self {
        let val = ManuallyDrop::new(val);
        let buf: NonNull<[u8]> = val.as_bytes().into();
        let raw = RawVarZeroCow {
            // Invariants upheld:
            // 1 & 3: The bytes came from `val` so they're a valid value and byte slice
            // 2: This is owned, so we set owned to true.
            buf,
            #[cfg(feature = "alloc")]
            owned: true,
        };
        // The bytes came from `val`, so it's a valid value
        unsafe { Self::from_raw(raw) }
    }
}

impl<'a, V: ?Sized> VarZeroCow<'a, V> {
    /// Whether or not this is owned
    pub fn is_owned(&self) -> bool {
        self.raw.is_owned()
    }

    /// Get the byte representation of this type
    ///
    /// Is also always a valid `V` and can be passed to
    /// `V::from_bytes_unchecked()`
    pub fn as_bytes(&self) -> &[u8] {
        // The valid V invariant comes from Invariant 2
        self.raw.as_bytes()
    }

    /// Invariant: `raw` must wrap a valid V, either owned or borrowed for 'a
    const unsafe fn from_raw(raw: RawVarZeroCow) -> Self {
        Self {
            // Invariant passed up to caller
            raw,
            marker1: PhantomData,
            #[cfg(feature = "alloc")]
            marker2: PhantomData,
        }
    }
}

impl RawVarZeroCow {
    /// Whether or not this is owned
    #[inline]
    pub fn is_owned(&self) -> bool {
        #[cfg(feature = "alloc")]
        return self.owned;
        #[cfg(not(feature = "alloc"))]
        return false;
    }

    /// Get the byte representation of this type
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        // Safety: Invariant 1 on self.buf
        unsafe { self.buf.as_ref() }
    }
}

impl<'a, V: VarULE + ?Sized> Deref for VarZeroCow<'a, V> {
    type Target = V;
    fn deref(&self) -> &V {
        // Safety: From invariant 2 on self.buf
        unsafe { V::from_bytes_unchecked(self.as_bytes()) }
    }
}

impl<'a, V: VarULE + ?Sized> From<&'a V> for VarZeroCow<'a, V> {
    fn from(other: &'a V) -> Self {
        Self::new_borrowed(other)
    }
}

#[cfg(feature = "alloc")]
impl<'a, V: VarULE + ?Sized> From<Box<V>> for VarZeroCow<'a, V> {
    fn from(other: Box<V>) -> Self {
        Self::new_owned(other)
    }
}

impl<'a, V: VarULE + ?Sized + fmt::Debug> fmt::Debug for VarZeroCow<'a, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        self.deref().fmt(f)
    }
}

// We need manual impls since `#[derive()]` is disallowed on packed types
impl<'a, V: VarULE + ?Sized + PartialEq> PartialEq for VarZeroCow<'a, V> {
    fn eq(&self, other: &Self) -> bool {
        self.deref().eq(other.deref())
    }
}

impl<'a, V: VarULE + ?Sized + Eq> Eq for VarZeroCow<'a, V> {}

impl<'a, V: VarULE + ?Sized + PartialOrd> PartialOrd for VarZeroCow<'a, V> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.deref().partial_cmp(other.deref())
    }
}

impl<'a, V: VarULE + ?Sized + Ord> Ord for VarZeroCow<'a, V> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.deref().cmp(other.deref())
    }
}

// # Safety
//
// encode_var_ule_len: Produces the length of the contained bytes, which are known to be a valid V by invariant
//
// encode_var_ule_write: Writes the contained bytes, which are known to be a valid V by invariant
unsafe impl<'a, V: VarULE + ?Sized> EncodeAsVarULE<V> for VarZeroCow<'a, V> {
    fn encode_var_ule_as_slices<R>(&self, _: impl FnOnce(&[&[u8]]) -> R) -> R {
        // unnecessary if the other two are implemented
        unreachable!()
    }

    #[inline]
    fn encode_var_ule_len(&self) -> usize {
        self.as_bytes().len()
    }

    #[inline]
    fn encode_var_ule_write(&self, dst: &mut [u8]) {
        dst.copy_from_slice(self.as_bytes())
    }
}

#[cfg(feature = "serde")]
impl<'a, V: VarULE + ?Sized + serde::Serialize> serde::Serialize for VarZeroCow<'a, V> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if serializer.is_human_readable() {
            <V as serde::Serialize>::serialize(self.deref(), serializer)
        } else {
            serializer.serialize_bytes(self.as_bytes())
        }
    }
}

#[cfg(all(feature = "serde", feature = "alloc"))]
impl<'a, 'de: 'a, V: VarULE + ?Sized> serde::Deserialize<'de> for VarZeroCow<'a, V>
where
    Box<V>: serde::Deserialize<'de>,
{
    fn deserialize<Des>(deserializer: Des) -> Result<Self, Des::Error>
    where
        Des: serde::Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let b = Box::<V>::deserialize(deserializer)?;
            Ok(Self::new_owned(b))
        } else {
            let bytes = <&[u8]>::deserialize(deserializer)?;
            Self::parse_bytes(bytes).map_err(serde::de::Error::custom)
        }
    }
}

#[cfg(feature = "databake")]
impl<'a, V: VarULE + ?Sized> databake::Bake for VarZeroCow<'a, V> {
    fn bake(&self, env: &databake::CrateEnv) -> databake::TokenStream {
        env.insert("zerovec");
        let bytes = self.as_bytes().bake(env);
        databake::quote! {
            // Safety: Known to come from a valid V since self.as_bytes() is always a valid V
            unsafe {
                zerovec::VarZeroCow::from_bytes_unchecked(#bytes)
            }
        }
    }
}

#[cfg(feature = "databake")]
impl<'a, V: VarULE + ?Sized> databake::BakeSize for VarZeroCow<'a, V> {
    fn borrows_size(&self) -> usize {
        self.as_bytes().len()
    }
}

impl<'a, V: VarULE + ?Sized> ZeroFrom<'a, V> for VarZeroCow<'a, V> {
    #[inline]
    fn zero_from(other: &'a V) -> Self {
        Self::new_borrowed(other)
    }
}

impl<'a, 'b, V: VarULE + ?Sized> ZeroFrom<'a, VarZeroCow<'b, V>> for VarZeroCow<'a, V> {
    #[inline]
    fn zero_from(other: &'a VarZeroCow<'b, V>) -> Self {
        Self::new_borrowed(other)
    }
}

#[cfg(test)]
mod tests {
    use super::VarZeroCow;
    use crate::ule::tuplevar::Tuple3VarULE;
    use crate::vecs::VarZeroSlice;
    #[test]
    fn test_cow_roundtrip() {
        type Messy = Tuple3VarULE<str, [u8], VarZeroSlice<str>>;
        let vec = vec!["one", "two", "three"];
        let messy: VarZeroCow<Messy> =
            VarZeroCow::from_encodeable(&("hello", &b"g\xFF\xFFdbye"[..], vec));

        assert_eq!(messy.a(), "hello");
        assert_eq!(messy.b(), b"g\xFF\xFFdbye");
        assert_eq!(&messy.c()[1], "two");

        #[cfg(feature = "serde")]
        {
            let bincode = bincode::serialize(&messy).unwrap();
            let deserialized: VarZeroCow<Messy> = bincode::deserialize(&bincode).unwrap();
            assert_eq!(
                messy, deserialized,
                "Single element roundtrips with bincode"
            );
            assert!(!deserialized.is_owned());

            let json = serde_json::to_string(&messy).unwrap();
            let deserialized: VarZeroCow<Messy> = serde_json::from_str(&json).unwrap();
            assert_eq!(messy, deserialized, "Single element roundtrips with serde");
        }
    }

    struct TwoCows<'a> {
        cow1: VarZeroCow<'a, str>,
        cow2: VarZeroCow<'a, str>,
    }

    #[test]
    fn test_eyepatch_works() {
        // This code should compile
        let mut two = TwoCows {
            cow1: VarZeroCow::new_borrowed("hello"),
            cow2: VarZeroCow::new_owned("world".into()),
        };
        let three = VarZeroCow::new_borrowed(&*two.cow2);
        two.cow1 = three;

        // Without the eyepatch, dropck will be worried that the dtor of two.cow1 can observe the
        // data it borrowed from two.cow2, which may have already been deleted

        // This test will fail if you add an empty `impl<'a, V: ?Sized> Drop for VarZeroCow<'a, V>`
    }
}
