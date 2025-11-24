// Copyright (c) 2020 Gilad Naaman, Ralf Jung
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

/// `addr_of!`, or just ref-then-cast when that is not available.
#[cfg(raw_ref_macros)]
#[macro_export]
#[doc(hidden)]
macro_rules! _memoffset__addr_of {
    ($path:expr) => {{
        $crate::__priv::ptr::addr_of!($path)
    }};
}
#[cfg(not(raw_ref_macros))]
#[macro_export]
#[doc(hidden)]
macro_rules! _memoffset__addr_of {
    ($path:expr) => {{
        // This is UB because we create an intermediate reference to uninitialized memory.
        // Nothing we can do about that without `addr_of!` though.
        &$path as *const _
    }};
}

/// Deref-coercion protection macro.
///
/// Prevents compilation if the specified field name is not a part of the
/// struct definition.
///
/// ```compile_fail
/// use memoffset::_memoffset__field_check;
///
/// struct Foo {
///     foo: i32,
/// }
///
/// type BoxedFoo = Box<Foo>;
///
/// _memoffset__field_check!(BoxedFoo, foo);
/// ```
#[cfg(allow_clippy)]
#[macro_export]
#[doc(hidden)]
macro_rules! _memoffset__field_check {
    ($type:path, $field:tt) => {
        // Make sure the field actually exists. This line ensures that a
        // compile-time error is generated if $field is accessed through a
        // Deref impl.
        #[allow(clippy::unneeded_field_pattern)]
        let $type { $field: _, .. };
    };
}
#[cfg(not(allow_clippy))]
#[macro_export]
#[doc(hidden)]
macro_rules! _memoffset__field_check {
    ($type:path, $field:tt) => {
        // Make sure the field actually exists. This line ensures that a
        // compile-time error is generated if $field is accessed through a
        // Deref impl.
        let $type { $field: _, .. };
    };
}

/// Deref-coercion protection macro.
///
/// Prevents compilation if the specified type is not a tuple.
///
/// ```compile_fail
/// use memoffset::_memoffset__field_check_tuple;
///
/// _memoffset__field_check_tuple!(i32, 0);
/// ```
#[cfg(allow_clippy)]
#[macro_export]
#[doc(hidden)]
macro_rules! _memoffset__field_check_tuple {
    ($type:ty, $field:tt) => {
        // Make sure the type argument is a tuple
        #[allow(clippy::unneeded_wildcard_pattern)]
        let (_, ..): $type;
    };
}
#[cfg(not(allow_clippy))]
#[macro_export]
#[doc(hidden)]
macro_rules! _memoffset__field_check_tuple {
    ($type:ty, $field:tt) => {
        // Make sure the type argument is a tuple
        let (_, ..): $type;
    };
}

/// Deref-coercion protection macro for unions.
/// Unfortunately accepts single-field structs as well, which is not ideal,
/// but ultimately pretty harmless.
///
/// ```compile_fail
/// use memoffset::_memoffset__field_check_union;
///
/// union Foo {
///     variant_a: i32,
/// }
///
/// type BoxedFoo = Box<Foo>;
///
/// _memoffset__field_check_union!(BoxedFoo, variant_a);
/// ```
#[cfg(allow_clippy)]
#[macro_export]
#[doc(hidden)]
macro_rules! _memoffset__field_check_union {
    ($type:path, $field:tt) => {
        // Make sure the field actually exists. This line ensures that a
        // compile-time error is generated if $field is accessed through a
        // Deref impl.
        #[allow(clippy::unneeded_wildcard_pattern)]
        // rustc1.19 requires unsafe here for the pattern; not needed in newer versions
        #[allow(unused_unsafe)]
        unsafe {
            let $type { $field: _ };
        }
    };
}
#[cfg(not(allow_clippy))]
#[macro_export]
#[doc(hidden)]
macro_rules! _memoffset__field_check_union {
    ($type:path, $field:tt) => {
        // Make sure the field actually exists. This line ensures that a
        // compile-time error is generated if $field is accessed through a
        // Deref impl.
        // rustc1.19 requires unsafe here for the pattern; not needed in newer versions
        #[allow(unused_unsafe)]
        unsafe {
            let $type { $field: _ };
        }
    };
}

/// Computes a const raw pointer to the given field of the given base pointer
/// to the given parent type.
///
/// The `base` pointer *must not* be dangling, but it *may* point to
/// uninitialized memory.
#[macro_export(local_inner_macros)]
macro_rules! raw_field {
    ($base:expr, $parent:path, $field:tt) => {{
        _memoffset__field_check!($parent, $field);
        let base = $base; // evaluate $base outside the `unsafe` block

        // Get the field address.
        // Crucially, we know that this will not trigger a deref coercion because
        // of the field check we did above.
        #[allow(unused_unsafe)] // for when the macro is used in an unsafe block
        unsafe {
            _memoffset__addr_of!((*(base as *const $parent)).$field)
        }
    }};
}

/// Computes a const raw pointer to the given field of the given base pointer
/// to the given parent tuple type.
///
/// The `base` pointer *must not* be dangling, but it *may* point to
/// uninitialized memory.
#[cfg(tuple_ty)]
#[macro_export(local_inner_macros)]
macro_rules! raw_field_tuple {
    ($base:expr, $parent:ty, $field:tt) => {{
        _memoffset__field_check_tuple!($parent, $field);
        let base = $base; // evaluate $base outside the `unsafe` block

        // Get the field address.
        // Crucially, we know that this will not trigger a deref coercion because
        // of the field check we did above.
        #[allow(unused_unsafe)] // for when the macro is used in an unsafe block
        unsafe {
            _memoffset__addr_of!((*(base as *const $parent)).$field)
        }
    }};
}

/// Computes a const raw pointer to the given field of the given base pointer
/// to the given parent tuple type.
///
/// The `base` pointer *must not* be dangling, but it *may* point to
/// uninitialized memory.
///
/// ## Note
/// This macro is the same as `raw_field`, except for a different Deref-coercion check that
/// supports unions.
/// Due to `macro_rules!` limitations, this check will accept structs with a single field as well as unions.
/// This is not a stable guarantee, and future versions of this crate might fail
/// on any use of this macro with a struct, without a semver bump.
#[macro_export(local_inner_macros)]
macro_rules! raw_field_union {
    ($base:expr, $parent:path, $field:tt) => {{
        _memoffset__field_check_union!($parent, $field);
        let base = $base; // evaluate $base outside the `unsafe` block

        // Get the field address.
        // Crucially, we know that this will not trigger a deref coercion because
        // of the field check we did above.
        #[allow(unused_unsafe)] // for when the macro is used in an unsafe block
        unsafe {
            _memoffset__addr_of!((*(base as *const $parent)).$field)
        }
    }};
}
