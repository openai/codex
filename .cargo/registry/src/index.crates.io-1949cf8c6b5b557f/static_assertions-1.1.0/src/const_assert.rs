/// Asserts that constant expressions evaluate to `true`.
///
/// Constant expressions can be ensured to have certain properties via this
/// macro If the expression evaluates to `false`, the file will fail to compile.
/// This is synonymous to [`static_assert` in C++][static_assert].
///
/// # Alternatives
///
/// There also exists [`const_assert_eq`](macro.const_assert_eq.html) for
/// validating whether a sequence of expressions are equal to one another.
///
/// # Examples
///
/// A common use case is to guarantee properties about a constant value that's
/// generated via meta-programming.
///
/// ```
/// # #[macro_use] extern crate static_assertions; fn main() {}
/// const VALUE: i32 = // ...
/// # 3;
///
/// const_assert!(VALUE >= 2);
/// ```
///
/// Inputs are type-checked as booleans:
///
/// ```compile_fail
/// # #[macro_use] extern crate static_assertions; fn main() {}
/// const_assert!(!0);
/// ```
///
/// Despite this being a macro, we see this produces a type error:
///
/// ```txt
///   | const_assert!(!0);
///   |               ^^ expected bool, found integral variable
///   |
///   = note: expected type `bool`
///              found type `{integer}`
/// ```
///
/// The following fails to compile because multiplying by 5 does not have an
/// identity property:
///
/// ```compile_fail
/// # #[macro_use] extern crate static_assertions; fn main() {}
/// const_assert!(5 * 5 == 5);
/// ```
///
/// [static_assert]: http://en.cppreference.com/w/cpp/language/static_assert
#[macro_export]
macro_rules! const_assert {
    ($x:expr $(,)?) => {
        #[allow(unknown_lints, eq_op)]
        const _: [(); 0 - !{ const ASSERT: bool = $x; ASSERT } as usize] = [];
    };
}

/// Asserts that constants are equal in value.
///
/// # Examples
///
/// This works as a shorthand for `const_assert!(a == b)`:
///
/// ```
/// # #[macro_use] extern crate static_assertions; fn main() {}
/// const TWO: usize = 2;
///
/// const_assert_eq!(TWO * TWO, TWO + TWO);
/// ```
///
/// Just because 2 × 2 = 2 + 2 doesn't mean it holds true for other numbers:
///
/// ```compile_fail
/// # #[macro_use] extern crate static_assertions; fn main() {}
/// const_assert_eq!(4 + 4, 4 * 4);
/// ```
#[macro_export(local_inner_macros)]
macro_rules! const_assert_eq {
    ($x:expr, $y:expr $(,)?) => {
        const_assert!($x == $y);
    };
}

/// Asserts that constants are **not** equal in value.
///
/// # Examples
///
/// This works as a shorthand for `const_assert!(a != b)`:
///
/// ```
/// # #[macro_use] extern crate static_assertions; fn main() {}
/// const NUM: usize = 32;
///
/// const_assert_ne!(NUM * NUM, 64);
/// ```
///
/// The following example fails to compile because 2 is magic and 2 × 2 = 2 + 2:
///
/// ```compile_fail
/// # #[macro_use] extern crate static_assertions; fn main() {}
/// const_assert_ne!(2 + 2, 2 * 2);
/// ```
#[macro_export(local_inner_macros)]
macro_rules! const_assert_ne {
    ($x:expr, $y:expr $(,)?) => {
        const_assert!($x != $y);
    };
}
