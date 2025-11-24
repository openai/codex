// Copyright (c) 2017 Gilad Naaman
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

/// Reexport for `local_inner_macros`; see
/// <https://doc.rust-lang.org/edition-guide/rust-2018/macros/macro-changes.html#macros-using-local_inner_macros>.
#[doc(hidden)]
#[macro_export]
macro_rules! _memoffset__compile_error {
    ($($inner:tt)*) => {
        compile_error! { $($inner)* }
    }
}

/// Produces a range instance representing the sub-slice containing the specified member.
///
/// This macro provides 2 forms of differing functionalities.
///
/// The first form is identical to the appearance of the `offset_of!` macro.
///
/// ```ignore
/// span_of!(Struct, member)
/// ```
///
/// The second form of `span_of!` returns a sub-slice which starts at one field, and ends at another.
/// The general pattern of this form is:
///
/// ```ignore
/// // Exclusive
/// span_of!(Struct, member_a .. member_b)
/// // Inclusive
/// span_of!(Struct, member_a ..= member_b)
///
/// // Open-ended ranges
/// span_of!(Struct, .. end)
/// span_of!(Struct, start ..)
/// ```
///
/// *Note*:
/// This macro uses recursion in order to resolve the range expressions, so there is a limit to
/// the complexity of the expression.
/// In order to raise the limit, the compiler's recursion limit should be lifted.
///
/// ## Examples
/// ```
/// use memoffset::span_of;
///
/// #[repr(C)]
/// struct Florp {
///     a: u32
/// }
///
/// #[repr(C)]
/// struct Blarg {
///     x: [u32; 2],
///     y: [u8; 56],
///     z: Florp,
///     egg: [[u8; 4]; 4]
/// }
///
/// fn main() {
///     assert_eq!(0..84,  span_of!(Blarg, ..));
///     assert_eq!(0..8,   span_of!(Blarg, .. y));
///     assert_eq!(0..64,  span_of!(Blarg, ..= y));
///     assert_eq!(0..8,   span_of!(Blarg, x));
///     assert_eq!(8..84,  span_of!(Blarg, y ..));
///     assert_eq!(0..8,   span_of!(Blarg, x .. y));
///     assert_eq!(0..64,  span_of!(Blarg, x ..= y));
/// }
/// ```
#[macro_export(local_inner_macros)]
macro_rules! span_of {
    (@helper  $root:ident, [] ..=) => {
        _memoffset__compile_error!("Expected a range, found '..='")
    };
    (@helper $root:ident, [] ..) => {
        _memoffset__compile_error!("Expected a range, found '..'")
    };
    // No explicit begin for range.
    (@helper $root:ident, $parent:path, [] ..) => {{
        ($root as usize,
         $root as usize + $crate::__priv::size_of_pointee($root))
    }};
    (@helper $root:ident, $parent:path, [] ..= $end:tt) => {{
        let end = raw_field!($root, $parent, $end);
        ($root as usize, end as usize + $crate::__priv::size_of_pointee(end))
    }};
    (@helper $root:ident, $parent:path, [] .. $end:tt) => {{
        ($root as usize, raw_field!($root, $parent, $end) as usize)
    }};
    // Explicit begin and end for range.
    (@helper $root:ident, $parent:path, # $begin:tt [] ..= $end:tt) => {{
        let begin = raw_field!($root, $parent, $begin);
        let end = raw_field!($root, $parent, $end);
        (begin as usize, end as usize + $crate::__priv::size_of_pointee(end))
    }};
    (@helper $root:ident, $parent:path, # $begin:tt [] .. $end:tt) => {{
        (raw_field!($root, $parent, $begin) as usize,
         raw_field!($root, $parent, $end) as usize)
    }};
    // No explicit end for range.
    (@helper $root:ident, $parent:path, # $begin:tt [] ..) => {{
        (raw_field!($root, $parent, $begin) as usize,
         $root as usize + $crate::__priv::size_of_pointee($root))
    }};
    (@helper $root:ident, $parent:path, # $begin:tt [] ..=) => {{
        _memoffset__compile_error!(
            "Found inclusive range to the end of a struct. Did you mean '..' instead of '..='?")
    }};
    // Just one field.
    (@helper $root:ident, $parent:path, # $field:tt []) => {{
        let field = raw_field!($root, $parent, $field);
        (field as usize, field as usize + $crate::__priv::size_of_pointee(field))
    }};
    // Parsing.
    (@helper $root:ident, $parent:path, $(# $begin:tt)+ [] $tt:tt $($rest:tt)*) => {{
        span_of!(@helper $root, $parent, $(#$begin)* #$tt [] $($rest)*)
    }};
    (@helper $root:ident, $parent:path, [] $tt:tt $($rest:tt)*) => {{
        span_of!(@helper $root, $parent, #$tt [] $($rest)*)
    }};

    // Entry point.
    ($sty:path, $($exp:tt)+) => ({
        // Get a base pointer.
        _memoffset__let_base_ptr!(root, $sty);
        let base = root as usize;
        let (begin, end) = span_of!(@helper root, $sty, [] $($exp)*);
        begin-base..end-base
    });
}

#[cfg(test)]
mod tests {
    use core::mem;

    #[test]
    fn span_simple() {
        #[repr(C)]
        struct Foo {
            a: u32,
            b: [u8; 2],
            c: i64,
        }

        assert_eq!(span_of!(Foo, a), 0..4);
        assert_eq!(span_of!(Foo, b), 4..6);
        assert_eq!(span_of!(Foo, c), 8..8 + 8);
    }

    #[test]
    #[cfg_attr(miri, ignore)] // this creates unaligned references
    fn span_simple_packed() {
        #[repr(C, packed)]
        struct Foo {
            a: u32,
            b: [u8; 2],
            c: i64,
        }

        assert_eq!(span_of!(Foo, a), 0..4);
        assert_eq!(span_of!(Foo, b), 4..6);
        assert_eq!(span_of!(Foo, c), 6..6 + 8);
    }

    #[test]
    fn span_forms() {
        #[repr(C)]
        struct Florp {
            a: u32,
        }

        #[repr(C)]
        struct Blarg {
            x: u64,
            y: [u8; 56],
            z: Florp,
            egg: [[u8; 4]; 5],
        }

        // Love me some brute force
        assert_eq!(0..8, span_of!(Blarg, x));
        assert_eq!(64..68, span_of!(Blarg, z));
        assert_eq!(68..mem::size_of::<Blarg>(), span_of!(Blarg, egg));

        assert_eq!(8..64, span_of!(Blarg, y..z));
        assert_eq!(0..64, span_of!(Blarg, x..=y));
    }

    #[test]
    fn ig_test() {
        #[repr(C)]
        struct Member {
            foo: u32,
        }

        #[repr(C)]
        struct Test {
            x: u64,
            y: [u8; 56],
            z: Member,
            egg: [[u8; 4]; 4],
        }

        assert_eq!(span_of!(Test, ..x), 0..0);
        assert_eq!(span_of!(Test, ..=x), 0..8);
        assert_eq!(span_of!(Test, ..y), 0..8);
        assert_eq!(span_of!(Test, ..=y), 0..64);
        assert_eq!(span_of!(Test, ..z), 0..64);
        assert_eq!(span_of!(Test, ..=z), 0..68);
        assert_eq!(span_of!(Test, ..egg), 0..68);
        assert_eq!(span_of!(Test, ..=egg), 0..84);
        assert_eq!(span_of!(Test, ..), 0..mem::size_of::<Test>());
        assert_eq!(
            span_of!(Test, x..),
            offset_of!(Test, x)..mem::size_of::<Test>()
        );
        assert_eq!(
            span_of!(Test, y..),
            offset_of!(Test, y)..mem::size_of::<Test>()
        );

        assert_eq!(
            span_of!(Test, z..),
            offset_of!(Test, z)..mem::size_of::<Test>()
        );
        assert_eq!(
            span_of!(Test, egg..),
            offset_of!(Test, egg)..mem::size_of::<Test>()
        );
        assert_eq!(
            span_of!(Test, x..y),
            offset_of!(Test, x)..offset_of!(Test, y)
        );
        assert_eq!(
            span_of!(Test, x..=y),
            offset_of!(Test, x)..offset_of!(Test, y) + mem::size_of::<[u8; 56]>()
        );
    }
}
