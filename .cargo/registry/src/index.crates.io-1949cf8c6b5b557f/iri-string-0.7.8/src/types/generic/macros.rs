//! Macros to define resource identifier types.

/// Implements type conversion from slice into smart pointer.
macro_rules! impl_from_slice_into_smartptr {
    (
        // Generic slice type.
        ty: $ty:ident,
        // Smart pointer item path (without type parameter).
        smartptr: $($smartptr:ident)::*,
        // Pointer mutability for `into_raw` and `from_raw`.
        // Use `mut` for `Box`, and `const` for `Arc` and `Rc`.
        mutability: $mut:ident,
    ) => {
        #[cfg(feature = "alloc")]
        impl<S: crate::spec::Spec> From<&$ty<S>> for $($smartptr)::* <$ty<S>> {
            fn from(s: &$ty<S>) -> Self {
                let inner: &str = s.as_str();
                let buf = $($smartptr)::* ::<str>::from(inner);
                // SAFETY: `$ty<S>` has `repr(transparent)` attribute, so the
                // memory layouts of `$smartptr<str>` and `$smartptr<$ty<S>>`
                // are compatible.
                unsafe {
                    let raw: *$mut str = $($smartptr)::* ::into_raw(buf);
                    $($smartptr)::* ::<$ty<S>>::from_raw(raw as *$mut $ty<S>)
                }
            }
        }
    };
}

/// Implements `PartialEq` and `PartialOrd`.
macro_rules! impl_cmp {
    ($ty_common:ty, $ty_lhs:ty, $ty_rhs:ty) => {
        impl<S: crate::spec::Spec> PartialEq<$ty_rhs> for $ty_lhs {
            #[inline]
            fn eq(&self, o: &$ty_rhs) -> bool {
                <$ty_common as PartialEq<$ty_common>>::eq(self.as_ref(), o.as_ref())
            }
        }
        impl<S: crate::spec::Spec> PartialEq<$ty_lhs> for $ty_rhs {
            #[inline]
            fn eq(&self, o: &$ty_lhs) -> bool {
                <$ty_common as PartialEq<$ty_common>>::eq(self.as_ref(), o.as_ref())
            }
        }
        impl<S: crate::spec::Spec> PartialOrd<$ty_rhs> for $ty_lhs {
            #[inline]
            fn partial_cmp(&self, o: &$ty_rhs) -> Option<core::cmp::Ordering> {
                <$ty_common as PartialOrd<$ty_common>>::partial_cmp(self.as_ref(), o.as_ref())
            }
        }
        impl<S: crate::spec::Spec> PartialOrd<$ty_lhs> for $ty_rhs {
            #[inline]
            fn partial_cmp(&self, o: &$ty_lhs) -> Option<core::cmp::Ordering> {
                <$ty_common as PartialOrd<$ty_common>>::partial_cmp(self.as_ref(), o.as_ref())
            }
        }
    };
}

/// Implements `PartialEq` and `PartialOrd` with two independent spec type parameter.
macro_rules! impl_cmp2 {
    ($ty_common:ty, $ty_lhs:ty, $ty_rhs:ty) => {
        impl<S: crate::spec::Spec, T: crate::spec::Spec> PartialEq<$ty_rhs> for $ty_lhs {
            #[inline]
            fn eq(&self, o: &$ty_rhs) -> bool {
                <$ty_common as PartialEq<$ty_common>>::eq(self.as_ref(), o.as_ref())
            }
        }
        impl<S: crate::spec::Spec, T: crate::spec::Spec> PartialEq<$ty_lhs> for $ty_rhs {
            #[inline]
            fn eq(&self, o: &$ty_lhs) -> bool {
                <$ty_common as PartialEq<$ty_common>>::eq(self.as_ref(), o.as_ref())
            }
        }
        impl<S: crate::spec::Spec, T: crate::spec::Spec> PartialOrd<$ty_rhs> for $ty_lhs {
            #[inline]
            fn partial_cmp(&self, o: &$ty_rhs) -> Option<core::cmp::Ordering> {
                <$ty_common as PartialOrd<$ty_common>>::partial_cmp(self.as_ref(), o.as_ref())
            }
        }
        impl<S: crate::spec::Spec, T: crate::spec::Spec> PartialOrd<$ty_lhs> for $ty_rhs {
            #[inline]
            fn partial_cmp(&self, o: &$ty_lhs) -> Option<core::cmp::Ordering> {
                <$ty_common as PartialOrd<$ty_common>>::partial_cmp(self.as_ref(), o.as_ref())
            }
        }
    };
}

/// Implements `PartialEq` and `PartialOrd` with two independent spec type parameter.
macro_rules! impl_cmp2_as_str {
    ($ty_lhs:ty, $ty_rhs:ty) => {
        impl<S: crate::spec::Spec, T: crate::spec::Spec> PartialEq<$ty_rhs> for $ty_lhs {
            #[inline]
            fn eq(&self, o: &$ty_rhs) -> bool {
                PartialEq::eq(self.as_str(), o.as_str())
            }
        }
        impl<S: crate::spec::Spec, T: crate::spec::Spec> PartialEq<$ty_lhs> for $ty_rhs {
            #[inline]
            fn eq(&self, o: &$ty_lhs) -> bool {
                PartialEq::eq(self.as_str(), o.as_str())
            }
        }
        impl<S: crate::spec::Spec, T: crate::spec::Spec> PartialOrd<$ty_rhs> for $ty_lhs {
            #[inline]
            fn partial_cmp(&self, o: &$ty_rhs) -> Option<core::cmp::Ordering> {
                PartialOrd::partial_cmp(self.as_str(), o.as_str())
            }
        }
        impl<S: crate::spec::Spec, T: crate::spec::Spec> PartialOrd<$ty_lhs> for $ty_rhs {
            #[inline]
            fn partial_cmp(&self, o: &$ty_lhs) -> Option<core::cmp::Ordering> {
                PartialOrd::partial_cmp(self.as_str(), o.as_str())
            }
        }
    };
}

/// Define the custom string slice type, and implements methods and traits.
///
/// Methods to be implemented:
///
/// * `pub fn new()`
/// * `pub fn new_unchecked()`
/// * `pub(crate) fn new_maybe_unchecked()`
/// * `fn new_always_unchecked()`
/// * `pub fn as_str()`
/// * `pub fn len()`
/// * `pub fn is_empty()`
///
/// Traits to be implemented:
///
/// * fundamental
///     + `Debug for $ty`
///     + `Eq for $ty`
///     + `Ord for $ty`
///     + `Hash for $ty`
/// * type conversion
///     + `AsRef<&str> for $ty`
///     + `AsRef<&$ty> for $ty`
///     + `From<&$ty>` for Cow<$ty>`
///     + `From<&$ty>` for Arc<$ty>`
///     + `From<&$ty>` for Box<$ty>`
///     + `From<&$ty>` for Rc<$ty>`
///     + `From<&$ty> for &str`
///     + `TryFrom<&str> for &$ty`
///     + `TryFrom<&[u8]> for &$ty`
/// * comparison (only `PartialEq` impls are listed, but `PartialOrd` is also implemented).
///     + `PartialEq<$ty> for $ty`
///     + `str` and `$ty`
///         - `PartialEq<str> for $ty`
///         - `PartialEq<$ty> for str`
///         - `PartialEq<&str> for $ty`
///         - `PartialEq<$ty> for &str`
///         - `PartialEq<str> for &$ty`
///         - `PartialEq<&$ty> for str`
///     + `$ty` and `$ty`
///         - `PartialEq<&$ty> for $ty`
///         - `PartialEq<$ty> for &$ty`
/// * other
///     + `Display for $ty`
/// * serde
///     + `serde::Serialize`
///     + `serde::Deserialize`
macro_rules! define_custom_string_slice {
    (
        $(#[$meta:meta])*
        struct $ty:ident {
            validator = $validate:ident,
            expecting_msg = $expecting:expr,
        }
    ) => {
        $(#[$meta])*
        // `#[derive(..)]` cannot be used here, because it adds `S: DerivedTrait` bounds automatically.
        #[repr(transparent)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize))]
        #[cfg_attr(feature = "serde", serde(bound = "S: crate::spec::Spec"))]
        #[cfg_attr(feature = "serde", serde(transparent))]
        pub struct $ty<S> {
            /// Spec.
            #[cfg_attr(feature = "serde", serde(skip))]
            _spec: core::marker::PhantomData<fn() -> S>,
            /// Inner data.
            inner: str,
        }

        impl<S: crate::spec::Spec> $ty<S> {
            /// Creates a new string.
            #[inline]
            pub fn new(s: &str) -> Result<&Self, crate::validate::Error> {
                core::convert::TryFrom::try_from(s)
            }

            /// Creates a new string without validation.
            ///
            /// This does not validate the given string, so it is caller's
            /// responsibility to ensure the given string is valid.
            ///
            /// # Safety
            ///
            /// The given string must be syntactically valid as `Self` type.
            /// If not, any use of the returned value or the call of this
            /// function itself may result in undefined behavior.
            #[inline]
            #[must_use]
            pub unsafe fn new_unchecked(s: &str) -> &Self {
                // SAFETY: `new_always_unchecked` requires the same precondition
                // as `new_always_unchecked`.
                unsafe { Self::new_always_unchecked(s) }
            }

            /// Creates a new string maybe without validation.
            ///
            /// This does validation on debug build.
            ///
            /// # Safety
            ///
            /// The given string must be syntactically valid as `Self` type.
            #[must_use]
            pub(crate) unsafe fn new_maybe_unchecked(s: &str) -> &Self {
                debug_assert_eq!($validate::<S>(s), Ok(()));
                // SAFETY: `new_always_unchecked` requires the same precondition
                // as `new_always_unchecked`. Additionally in debug build, just
                // checked the content is actually valid by `$validate::<S>(s)`.
                unsafe { Self::new_always_unchecked(s) }
            }

            /// Creates a new string without any validation.
            ///
            /// This does not validate the given string at any time.
            ///
            /// Intended for internal use.
            ///
            /// # Safety
            ///
            /// The given string must be syntactically valid as `Self` type.
            #[inline]
            #[must_use]
            unsafe fn new_always_unchecked(s: &str) -> &Self {
                // SAFETY: the cast is safe since `Self` type has `repr(transparent)`
                // attribute and the content is guaranteed as valid by the
                // precondition of the function.
                unsafe { &*(s as *const str as *const Self) }
            }

            /// Returns `&str`.
            #[inline]
            #[must_use]
            pub fn as_str(&self) -> &str {
                self.as_ref()
            }

            /// Returns the string length.
            #[inline]
            #[must_use]
            pub fn len(&self) -> usize {
                self.as_str().len()
            }

            /// Returns whether the string is empty.
            #[inline]
            #[must_use]
            pub fn is_empty(&self) -> bool {
                self.as_str().is_empty()
            }
        }

        impl<S: crate::spec::Spec> core::fmt::Debug for $ty<S> {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.debug_tuple(stringify!($ty)).field(&&self.inner).finish()
            }
        }

        impl<S: crate::spec::Spec> PartialEq for $ty<S> {
            #[inline]
            fn eq(&self, other: &Self) -> bool {
                self.inner == other.inner
            }
        }

        impl<S: crate::spec::Spec> Eq for $ty<S> {}

        impl<S: crate::spec::Spec> PartialOrd for $ty<S> {
            #[inline]
            fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
                Some(self.inner.cmp(&other.inner))
            }
        }

        impl<S: crate::spec::Spec> Ord for $ty<S> {
            #[inline]
            fn cmp(&self, other: &Self) -> core::cmp::Ordering {
                self.inner.cmp(&other.inner)
            }
        }

        impl<S: crate::spec::Spec> core::hash::Hash for $ty<S> {
            #[inline]
            fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
                self.inner.hash(state);
            }
        }

        impl<S: crate::spec::Spec> AsRef<str> for $ty<S> {
            #[inline]
            fn as_ref(&self) -> &str {
                &self.inner
            }
        }

        impl<S: crate::spec::Spec> AsRef<$ty<S>> for $ty<S> {
            #[inline]
            fn as_ref(&self) -> &$ty<S> {
                self
            }
        }

        #[cfg(feature = "alloc")]
        impl<'a, S: crate::spec::Spec> From<&'a $ty<S>> for alloc::borrow::Cow<'a, $ty<S>> {
            #[inline]
            fn from(s: &'a $ty<S>) -> Self {
                alloc::borrow::Cow::Borrowed(s)
            }
        }

        impl_from_slice_into_smartptr! {
            ty: $ty,
            smartptr: alloc::sync::Arc,
            mutability: const,
        }

        impl_from_slice_into_smartptr! {
            ty: $ty,
            smartptr: alloc::boxed::Box,
            mutability: mut,
        }

        impl_from_slice_into_smartptr! {
            ty: $ty,
            smartptr: alloc::rc::Rc,
            mutability: const,
        }

        impl<'a, S: crate::spec::Spec> From<&'a $ty<S>> for &'a str {
            #[inline]
            fn from(s: &'a $ty<S>) -> &'a str {
                s.as_ref()
            }
        }

        impl<'a, S: crate::spec::Spec> core::convert::TryFrom<&'a str> for &'a $ty<S> {
            type Error = crate::validate::Error;

            #[inline]
            fn try_from(s: &'a str) -> Result<Self, Self::Error> {
                match $validate::<S>(s) {
                    // SAFETY: just checked `s` is valid as `$ty`.
                    Ok(()) => Ok(unsafe { $ty::new_always_unchecked(s) }),
                    Err(e) => Err(e),
                }
            }
        }

        impl<'a, S: crate::spec::Spec> core::convert::TryFrom<&'a [u8]> for &'a $ty<S> {
            type Error = crate::validate::Error;

            #[inline]
            fn try_from(bytes: &'a [u8]) -> Result<Self, Self::Error> {
                let s = core::str::from_utf8(bytes).map_err(|_| crate::validate::Error::new())?;
                match $validate::<S>(s) {
                    // SAFETY: just checked `s` is valid as `$ty`.
                    Ok(()) => Ok(unsafe { $ty::new_always_unchecked(s) }),
                    Err(e) => Err(e),
                }
            }
        }

        impl_cmp!(str, str, $ty<S>);
        impl_cmp!(str, &str, $ty<S>);
        impl_cmp!(str, str, &$ty<S>);
        impl_cmp2!(str, &$ty<S>, $ty<T>);

        impl<S: crate::spec::Spec> core::fmt::Display for $ty<S> {
            #[inline]
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        /// Serde deserializer implementation.
        #[cfg(feature = "serde")]
        mod __serde_slice {
            use super::$ty;

            use core::{convert::TryFrom, fmt, marker::PhantomData};

            use serde::{
                de::{self, Visitor},
                Deserialize, Deserializer,
            };

            /// Custom borrowed string visitor.
            #[derive(Debug, Clone, Copy)]
            struct CustomStrVisitor<S>(PhantomData<fn() -> S>);

            impl<'de, S: 'de + crate::spec::Spec> Visitor<'de> for CustomStrVisitor<S> {
                type Value = &'de $ty<S>;

                #[inline]
                fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    f.write_str($expecting)
                }

                #[inline]
                fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
                where
                    E: de::Error,
                {
                    <&'de $ty<S> as TryFrom<&'de str>>::try_from(v).map_err(E::custom)
                }
            }

            // About `'de` and `'a`, see
            // <https://serde.rs/lifetimes.html#the-deserializede-lifetime>.
            impl<'de: 'a, 'a, S: 'de + crate::spec::Spec> Deserialize<'de> for &'a $ty<S> {
                #[inline]
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where
                    D: Deserializer<'de>,
                {
                    deserializer.deserialize_string(CustomStrVisitor::<S>(PhantomData))
                }
            }
        }
    };
}

/// Define the custom owned string type, and implements methods and traits.
///
/// Methods to be implemented:
///
/// * `pub fn new_unchecked()`
/// * `pub(crate) fn new_maybe_unchecked()`
/// * `pub(crate) fn new_always_unchecked()`
/// * `pub fn shrink_to_fit()`
///
/// Traits to be implemented:
///
/// * fundamental
///     + `Debug for $ty`
///     + `Clone for $ty`
///     + `Eq for $ty`
///     + `Ord for $ty`
///     + `Hash for $ty`
/// * type conversion
///     + `AsRef<str> for $ty`
///     + `AsRef<$slice> for $ty`
///     + `Borrow<str> for $ty`
///     + `Borrow<$slice> for $ty`
///     + `ToOwned<Owned = $ty> for $slice`
///     + `From<&$slice> for $ty`
///     + `From<$ty> for String`
///     + `From<$ty> for Cow<'_, $slice>`
///     + `From<$ty> for Box<$slice>`
///     + `TryFrom<&str> for $ty`
///     + `TryFrom<&[u8]> for $ty`
///     + `TryFrom<String> for $ty`
///     + `FromStr for $ty`
///     + `Deref<Target = $slice> for $ty`
/// * comparison (only `PartialEq` impls are listed, but `PartialOrd` is also implemented.
///     + `PartialEq<$ty> for $ty`
///     + `$slice` and `str`
///         - `PartialEq<$slice> for Cow<'_, str>`
///         - `PartialEq<Cow<'_, str>> for $slice`
///         - `PartialEq<&$slice> for Cow<'_, str>`
///         - `PartialEq<Cow<'_, str>> for &$slice`
///     + `$slice` and `Cow<$slice>`
///         - `PartialEq<$slice> for Cow<'_, $slice>`
///         - `PartialEq<Cow<'_, $slice>> for $slice`
///         - `PartialEq<&$slice> for Cow<'_, $slice>`
///         - `PartialEq<Cow<'_, $slice>> for &$slice`
///     + `str` and `$ty`
///         - `PartialEq<str> for $ty`
///         - `PartialEq<$ty> for str`
///         - `PartialEq<&str> for $ty`
///         - `PartialEq<$ty> for &str`
///         - `PartialEq<Cow<'_, str>> for $ty`
///         - `PartialEq<$ty> for Cow<'_, str>`
///     + `String` and `$ty`
///         - `PartialEq<String> for $ty`
///         - `PartialEq<$ty> for String`
///     + `$slice` and `$ty`
///         - `PartialEq<$slice> for $ty`
///         - `PartialEq<$ty> for $slice`
///         - `PartialEq<&$slice> for $ty`
///         - `PartialEq<$ty> for &$slice`
///         - `PartialEq<Cow<'_, $slice>> for $ty`
///         - `PartialEq<$ty> for Cow<'_, $slice>`
/// * other
///     + `Display for $ty`
/// * serde
///     + `serde::Serialize`
///     + `serde::Deserialize`
// Note that `From<$ty> for {Arc,Rc}<$slice>` is currently not implemented since
// this won't reuse allocated memory and hides internal memory reallocation. See
// <https://github.com/lo48576/iri-string/issues/20#issuecomment-1105207849>.
// However, this is not decided with firm belief or opinion, so there would be
// a chance that they are implemented in future.
#[cfg(feature = "alloc")]
macro_rules! define_custom_string_owned {
    (
        $(#[$meta:meta])*
        struct $ty:ident {
            validator = $validate:ident,
            slice = $slice:ident,
            expecting_msg = $expecting:expr,
        }
    ) => {
        $(#[$meta])*
        // `#[derive(..)]` cannot be used here, because it adds `S: DerivedTrait` bounds automatically.
        #[cfg(feature = "alloc")]
        #[cfg_attr(all(feature = "serde", feature = "alloc"), derive(serde::Serialize))]
        #[cfg_attr(all(feature = "serde", feature = "alloc"), serde(bound = "S: crate::spec::Spec"))]
        #[cfg_attr(all(feature = "serde", feature = "alloc"), serde(transparent))]
        pub struct $ty<S> {
            /// Spec.
            #[cfg_attr(all(feature = "serde", feature = "alloc"), serde(skip))]
            _spec: core::marker::PhantomData<fn() -> S>,
            /// Inner data.
            inner: alloc::string::String,
        }

        impl<S: crate::spec::Spec> $ty<S> {
            /// Creates a new string without validation.
            ///
            /// This does not validate the given string, so it is caller's
            /// responsibility to ensure the given string is valid.
            ///
            /// # Safety
            ///
            /// The given string must be syntactically valid as `Self` type.
            /// If not, any use of the returned value or the call of this
            /// function itself may result in undefined behavior.
            #[inline]
            #[must_use]
            pub unsafe fn new_unchecked(s: alloc::string::String) -> Self {
                // SAFETY: `new_always_unchecked` requires the same precondition
                // as `new_always_unchecked`.
                unsafe { Self::new_always_unchecked(s) }
            }

            /// Creates a new string maybe without validation.
            ///
            /// This does not validate the given string at any time.
            ///
            /// Intended for internal use.
            ///
            /// # Safety
            ///
            /// The given string must be syntactically valid as `Self` type.
            #[inline]
            #[must_use]
            pub(crate) unsafe fn new_always_unchecked(s: alloc::string::String) -> Self {
                // The construction itself can be written in safe Rust, but
                // every other place including unsafe functions expects
                // `self.inner` to be syntactically valid as `Self`. In order to
                // make them safe, the construction should validate the value
                // or at least should require users to validate the value by
                // making the function `unsafe`.
                Self {
                    _spec: core::marker::PhantomData,
                    inner: s,
                }
            }

            /// Creates a new string maybe without validation.
            ///
            /// This does validation on debug build.
            ///
            /// # Safety
            ///
            /// The given string must be syntactically valid as `Self` type.
            #[must_use]
            pub(crate) unsafe fn new_maybe_unchecked(s: alloc::string::String) -> Self {
                debug_assert_eq!(
                    $validate::<S>(&s),
                    Ok(()),
                    "[precondition] the given string must be valid"
                );
                // SAFETY: `new_always_unchecked` requires the same precondition
                // as `new_always_unchecked`. Additionally in debug build, just
                // checked the content is actually valid by `$validate::<S>(s)`.
                unsafe { Self::new_always_unchecked(s) }
            }

            /// Returns a mutable reference to the inner string buffer.
            ///
            /// This may be useful to implement inline modification algorithm,
            /// but be careful as this method itself cannot validate the new
            /// content.
            ///
            /// # Safety
            ///
            /// The content after modification must be syntactically valid as
            /// `Self` type.
            /// If not, any use of the returned value or the call of this
            /// function itself may result in undefined behavior.
            #[inline]
            #[must_use]
            // TODO: Use wrapper type to enforce validation on finish?
            pub(crate) unsafe fn as_inner_mut(&mut self) -> &mut alloc::string::String {
                &mut self.inner
            }

            /// Shrinks the capacity of the inner buffer to match its length.
            #[inline]
            pub fn shrink_to_fit(&mut self) {
                self.inner.shrink_to_fit()
            }

            /// Returns the internal buffer capacity in bytes.
            #[inline]
            #[must_use]
            pub fn capacity(&self) -> usize {
                self.inner.capacity()
            }

            /// Returns the borrowed IRI string slice.
            ///
            /// This is equivalent to `&*self`.
            #[inline]
            #[must_use]
            pub fn as_slice(&self) -> &$slice<S> {
                self.as_ref()
            }
        }

        impl<S: crate::spec::Spec> core::fmt::Debug for $ty<S> {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.debug_tuple(stringify!($ty)).field(&&self.inner).finish()
            }
        }

        impl<S: crate::spec::Spec> Clone for $ty<S> {
            #[inline]
            fn clone(&self) -> Self {
                // This is safe because `self` must be valid.
                Self {
                    _spec: core::marker::PhantomData,
                    inner: self.inner.clone(),
                }
            }
        }

        impl<S: crate::spec::Spec, T: crate::spec::Spec> PartialEq<$ty<T>> for $ty<S> {
            #[inline]
            fn eq(&self, other: &$ty<T>) -> bool {
                self.inner == other.inner
            }
        }

        impl<S: crate::spec::Spec> Eq for $ty<S> {}

        impl<S: crate::spec::Spec, T: crate::spec::Spec> PartialOrd<$ty<T>> for $ty<S> {
            #[inline]
            fn partial_cmp(&self, other: &$ty<T>) -> Option<core::cmp::Ordering> {
                self.inner.partial_cmp(&other.inner)
            }
        }

        impl<S: crate::spec::Spec> Ord for $ty<S> {
            #[inline]
            fn cmp(&self, other: &Self) -> core::cmp::Ordering {
                self.inner.cmp(&other.inner)
            }
        }

        impl<S: crate::spec::Spec> core::hash::Hash for $ty<S> {
            #[inline]
            fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
                self.inner.hash(state);
            }
        }

        impl<S: crate::spec::Spec> AsRef<str> for $ty<S> {
            #[inline]
            fn as_ref(&self) -> &str {
                &self.inner
            }
        }

        impl<S: crate::spec::Spec> AsRef<$slice<S>> for $ty<S> {
            #[inline]
            fn as_ref(&self) -> &$slice<S> {
                // SAFETY: `$ty<S>` and `$slice<S>` requires same validation, so
                // the content of `self: &$ty<S>` must be valid as `$slice<S>`.
                unsafe { $slice::new_always_unchecked(AsRef::<str>::as_ref(self)) }
            }
        }

        impl<S: crate::spec::Spec> core::borrow::Borrow<str> for $ty<S> {
            #[inline]
            fn borrow(&self) -> &str {
                self.as_ref()
            }
        }

        impl<S: crate::spec::Spec> core::borrow::Borrow<$slice<S>> for $ty<S> {
            #[inline]
            fn borrow(&self) -> &$slice<S> {
                self.as_ref()
            }
        }

        impl<S: crate::spec::Spec> alloc::borrow::ToOwned for $slice<S> {
            type Owned = $ty<S>;

            #[inline]
            fn to_owned(&self) -> Self::Owned {
                self.into()
            }
        }

        impl<S: crate::spec::Spec> From<&'_ $slice<S>> for $ty<S> {
            #[inline]
            fn from(s: &$slice<S>) -> Self {
                // This is safe because `s` must be valid.
                $ty {
                    _spec: core::marker::PhantomData,
                    inner: alloc::string::String::from(s.as_str()),
                }
            }
        }

        impl<S: crate::spec::Spec> From<$ty<S>> for alloc::string::String {
            #[inline]
            fn from(s: $ty<S>) -> Self {
                s.inner
            }
        }

        impl<'a, S: crate::spec::Spec> From<$ty<S>> for alloc::borrow::Cow<'a, $slice<S>> {
            #[inline]
            fn from(s: $ty<S>) -> alloc::borrow::Cow<'a, $slice<S>> {
                alloc::borrow::Cow::Owned(s)
            }
        }

        impl<S: crate::spec::Spec> From<$ty<S>> for alloc::boxed::Box<$slice<S>> {
            #[inline]
            fn from(s: $ty<S>) -> alloc::boxed::Box<$slice<S>> {
                let inner: alloc::string::String = s.into();
                let buf = alloc::boxed::Box::<str>::from(inner);
                // SAFETY: `$slice<S>` has `repr(transparent)` attribute, so
                // the memory layouts of `Box<str>` and `Box<$slice<S>>` are
                // compatible. Additionally, `$ty<S>` and `$slice<S>` require
                // the same syntax (it is the macro user's responsibility to
                // guarantee).
                unsafe {
                    let raw: *mut str = alloc::boxed::Box::into_raw(buf);
                    alloc::boxed::Box::<$slice<S>>::from_raw(raw as *mut $slice<S>)
                }
            }
        }

        impl<S: crate::spec::Spec> core::convert::TryFrom<&'_ str> for $ty<S> {
            type Error = crate::validate::Error;

            #[inline]
            fn try_from(s: &str) -> Result<Self, Self::Error> {
                <&$slice<S>>::try_from(s).map(Into::into)
            }
        }

        impl<S: crate::spec::Spec> core::convert::TryFrom<&'_ [u8]> for $ty<S> {
            type Error = crate::validate::Error;

            #[inline]
            fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
                let s = core::str::from_utf8(bytes).map_err(|_| crate::validate::Error::new())?;
                <&$slice<S>>::try_from(s).map(Into::into)
            }
        }

        impl<S: crate::spec::Spec> core::convert::TryFrom<alloc::string::String> for $ty<S> {
            type Error = crate::types::CreationError<alloc::string::String>;

            #[inline]
            fn try_from(s: alloc::string::String) -> Result<Self, Self::Error> {
                match <&$slice<S>>::try_from(s.as_str()) {
                    Ok(_) => {
                        // This is safe because `<&$slice<S>>::try_from(s)?` ensures
                        // that the string `s` is valid.
                        Ok(Self {
                            _spec: core::marker::PhantomData,
                            inner: s,
                        })
                    }
                    Err(e) => Err(crate::types::CreationError::new(e, s)),
                }
            }
        }

        impl<S: crate::spec::Spec> alloc::str::FromStr for $ty<S> {
            type Err = crate::validate::Error;

            #[inline]
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                core::convert::TryFrom::try_from(s)
            }
        }

        impl<S: crate::spec::Spec> core::ops::Deref for $ty<S> {
            type Target = $slice<S>;

            #[inline]
            fn deref(&self) -> &$slice<S> {
                self.as_ref()
            }
        }

        impl_cmp!(str, $slice<S>, alloc::borrow::Cow<'_, str>);
        impl_cmp!(str, &$slice<S>, alloc::borrow::Cow<'_, str>);
        impl_cmp2_as_str!(&$slice<S>, alloc::borrow::Cow<'_, $slice<T>>);

        impl_cmp!(str, str, $ty<S>);
        impl_cmp!(str, &str, $ty<S>);
        impl_cmp!(str, alloc::borrow::Cow<'_, str>, $ty<S>);
        impl_cmp!(str, alloc::string::String, $ty<S>);
        impl_cmp2!(str, $slice<S>, $ty<T>);
        impl_cmp2!(str, &$slice<S>, $ty<T>);
        impl_cmp2_as_str!(alloc::borrow::Cow<'_, $slice<S>>, $ty<T>);

        impl<S: crate::spec::Spec> core::fmt::Display for $ty<S> {
            #[inline]
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        /// Serde deserializer implementation.
        #[cfg(all(feature = "alloc", feature = "serde"))]
        mod __serde_owned {
            use super::$ty;

            use core::{convert::TryFrom, fmt, marker::PhantomData};

            #[cfg(all(feature = "serde", feature = "alloc"))]
            use alloc::string::String;

            use serde::{
                de::{self, Visitor},
                Deserialize, Deserializer,
            };

            /// Custom owned string visitor.
            #[derive(Debug, Clone, Copy)]
            struct CustomStringVisitor<S>(PhantomData<fn() -> S>);

            impl<'de, S: crate::spec::Spec> Visitor<'de> for CustomStringVisitor<S> {
                type Value = $ty<S>;

                #[inline]
                fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    f.write_str($expecting)
                }

                #[inline]
                fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
                where
                    E: de::Error,
                {
                    <$ty<S> as TryFrom<&str>>::try_from(v).map_err(E::custom)
                }

                #[cfg(all(feature = "serde", feature = "alloc"))]
                #[inline]
                fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
                where
                    E: de::Error,
                {
                    <$ty<S> as TryFrom<String>>::try_from(v).map_err(E::custom)
                }
            }

            impl<'de, S: crate::spec::Spec> Deserialize<'de> for $ty<S> {
                #[inline]
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where
                    D: Deserializer<'de>,
                {
                    deserializer.deserialize_str(CustomStringVisitor::<S>(PhantomData))
                }
            }
        }
    };
}

/// Implements trivial conversions and other useful traits between two IRI types.
///
/// Implemented traits:
///
/// * type conversion
///     + `AsRef<$to_slice> for $from_slice`
///     + `AsRef<$to_slice> for $from_owned`
///     + `From<$from_slice> for $to_slice`
///     + `From<$from_owned> for $to_owned`
///     + `TryFrom<&$to_slice> for &$from_slice`
///     + `TryFrom<$to_owned> for $from_owned`
/// * comparison (only `PartialEq` impls are listed, but `PartialOrd` is also implemented).
///     + `$from_slice` and `$to_slice`
///         - `PartialEq<$from_slice> for $to_slice`
///         - `PartialEq<$to_slice> for $from_slice`
///         - `PartialEq<&$from_slice> for $to_slice`
///         - `PartialEq<$to_slice> for &$from_slice`
///         - `PartialEq<$from_slice> for &$to_slice`
///         - `PartialEq<&$to_slice> for $from_slice`
///         - `PartialEq<$from_slice> for Cow<'_, $to_slice>`
///         - `PartialEq<Cow<'_, $to_slice>> for $from_slice`
///         - `PartialEq<&$from_slice> for Cow<'_, $to_slice>`
///         - `PartialEq<Cow<'_, $to_slice>> for &$from_slice`
///         - `PartialEq<Cow<'_, $from_slice>> for $to_slice`
///         - `PartialEq<$to_slice> for Cow<'_, $from_slice>`
///         - `PartialEq<Cow<'_, $from_slice>> for &$to_slice`
///         - `PartialEq<&$to_slice> for Cow<'_, $from_slice>`
///     + `$from_slice` and `$to_owned`
///         - `PartialEq<$from_slice> for $to_owned`
///         - `PartialEq<$to_owned> for $from_slice`
///         - `PartialEq<&$from_slice> for $to_owned`
///         - `PartialEq<$to_owned> for &$from_slice`
///         - `PartialEq<Cow<'_, $from_slice>> for $to_owned`
///         - `PartialEq<$to_owned> for Cow<'_, $from_slice>`
///     + `$from_owned` and `$to_slice`
///         - `PartialEq<$from_owned> for $to_slice`
///         - `PartialEq<$to_slice> for $from_owned`
///         - `PartialEq<$from_owned> for &$to_slice`
///         - `PartialEq<&$to_slice> for $from_owned`
///         - `PartialEq<$from_owned> for Cow<'_, $to_slice>`
///         - `PartialEq<Cow<'_, $to_slice>> for $from_owned`
///     + `$from_owned` and `$to_owned`
///         - `PartialEq<$from_owned> for $to_owned`
///         - `PartialEq<$to_owned> for $from_owned`
macro_rules! impl_trivial_conv_between_iri {
    (
        from_slice: $from_slice:ident,
        from_owned: $from_owned:ident,
        to_slice: $to_slice:ident,
        to_owned: $to_owned:ident,
    ) => {
        impl<S: crate::spec::Spec> AsRef<$to_slice<S>> for $from_slice<S> {
            #[inline]
            fn as_ref(&self) -> &$to_slice<S> {
                // SAFETY: `$from_slice<S>` should be subset of `$to_slice<S>`.
                // The caller of `impl_trivial_conv_between_iri!` macro is
                // responsible for guaranteeing that.
                unsafe { <$to_slice<S>>::new_maybe_unchecked(self.as_str()) }
            }
        }

        #[cfg(feature = "alloc")]
        impl<S: crate::spec::Spec> AsRef<$to_slice<S>> for $from_owned<S> {
            #[inline]
            fn as_ref(&self) -> &$to_slice<S> {
                AsRef::<$from_slice<S>>::as_ref(self).as_ref()
            }
        }

        impl<'a, S: crate::spec::Spec> From<&'a $from_slice<S>> for &'a $to_slice<S> {
            #[inline]
            fn from(s: &'a $from_slice<S>) -> &'a $to_slice<S> {
                s.as_ref()
            }
        }

        #[cfg(feature = "alloc")]
        impl<S: crate::spec::Spec> From<$from_owned<S>> for $to_owned<S> {
            #[inline]
            fn from(s: $from_owned<S>) -> $to_owned<S> {
                // SAFETY: `$from_slice<S>` should be subset of `$to_slice<S>`.
                // The caller of `impl_trivial_conv_between_iri!` macro is
                // responsible for guaranteeing that.
                unsafe { <$to_owned<S>>::new_maybe_unchecked(s.into()) }
            }
        }

        impl<'a, S: crate::spec::Spec> core::convert::TryFrom<&'a $to_slice<S>>
            for &'a $from_slice<S>
        {
            type Error = crate::validate::Error;

            #[inline]
            fn try_from(s: &'a $to_slice<S>) -> Result<Self, Self::Error> {
                Self::try_from(s.as_str())
            }
        }

        #[cfg(feature = "alloc")]
        impl<S: crate::spec::Spec> core::convert::TryFrom<$to_owned<S>> for $from_owned<S> {
            type Error = crate::types::CreationError<$to_owned<S>>;

            fn try_from(s: $to_owned<S>) -> Result<Self, Self::Error> {
                match <&$from_slice<S>>::try_from(s.as_str()) {
                    // SAFETY: just checked `s.as_str()` is valid as `$from_slice<S>`, and it
                    // requires the same syntax as `$from_owned<S>`.
                    Ok(_) => Ok(unsafe { <$from_owned<S>>::new_always_unchecked(s.into()) }),
                    Err(e) => Err(crate::types::CreationError::new(e, s)),
                }
            }
        }

        impl_cmp2_as_str!($from_slice<S>, $to_slice<T>);
        impl_cmp2_as_str!(&$from_slice<S>, $to_slice<T>);
        impl_cmp2_as_str!($from_slice<S>, &$to_slice<T>);
        #[cfg(feature = "alloc")]
        impl_cmp2_as_str!($from_slice<S>, alloc::borrow::Cow<'_, $to_slice<T>>);
        #[cfg(feature = "alloc")]
        impl_cmp2_as_str!(&$from_slice<S>, alloc::borrow::Cow<'_, $to_slice<T>>);
        #[cfg(feature = "alloc")]
        impl_cmp2_as_str!(alloc::borrow::Cow<'_, $from_slice<S>>, $to_slice<T>);
        #[cfg(feature = "alloc")]
        impl_cmp2_as_str!(alloc::borrow::Cow<'_, $from_slice<S>>, &$to_slice<T>);

        #[cfg(feature = "alloc")]
        impl_cmp2_as_str!($from_slice<S>, $to_owned<T>);
        #[cfg(feature = "alloc")]
        impl_cmp2_as_str!(&$from_slice<S>, $to_owned<T>);
        #[cfg(feature = "alloc")]
        impl_cmp2_as_str!(alloc::borrow::Cow<'_, $from_slice<S>>, $to_owned<T>);

        #[cfg(feature = "alloc")]
        impl_cmp2_as_str!($from_owned<S>, $to_slice<T>);
        #[cfg(feature = "alloc")]
        impl_cmp2_as_str!($from_owned<S>, &$to_slice<T>);
        #[cfg(feature = "alloc")]
        impl_cmp2_as_str!($from_owned<S>, alloc::borrow::Cow<'_, $to_slice<T>>);
        #[cfg(feature = "alloc")]
        impl_cmp2_as_str!($from_owned<S>, $to_owned<T>);
    };
}
