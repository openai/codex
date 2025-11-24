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
#[macro_export]
#[doc(hidden)]
macro_rules! _memoffset__field_check_tuple {
    ($type:ty, $field:tt) => {
        // Make sure the type argument is a tuple
        let (_, ..): $type;
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
/// to the given parent tuple typle.
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
