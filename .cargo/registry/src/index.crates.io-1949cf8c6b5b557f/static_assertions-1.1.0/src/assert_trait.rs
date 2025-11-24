/// Asserts that the trait is a child of all of the other traits.
///
/// Related:
/// - [`assert_trait_super_all!`]
///
/// # Examples
///
/// All types that implement [`Copy`] must implement [`Clone`]:
///
/// ```
/// # #[macro_use] extern crate static_assertions; fn main() {}
/// assert_trait_sub_all!(Copy: Clone);
/// ```
///
/// All types that implement [`Ord`] must implement [`PartialEq`], [`Eq`], and
/// [`PartialOrd`]:
///
/// ```
/// # #[macro_use] extern crate static_assertions; fn main() {}
/// assert_trait_sub_all!(Ord: PartialEq, Eq, PartialOrd);
/// ```
///
/// The following example fails to compile because [`Eq`] is not required for
/// [`PartialOrd`]:
///
/// ```compile_fail
/// # #[macro_use] extern crate static_assertions; fn main() {}
/// assert_trait_sub_all!(PartialOrd: Eq);
/// ```
///
/// [`assert_trait_super_all!`]: macro.assert_trait_super_all.html
///
/// [`Copy`]:       https://doc.rust-lang.org/std/marker/trait.Copy.html
/// [`Clone`]:      https://doc.rust-lang.org/std/clone/trait.Clone.html
/// [`Ord`]:        https://doc.rust-lang.org/std/cmp/trait.Ord.html
/// [`PartialOrd`]: https://doc.rust-lang.org/std/cmp/trait.PartialOrd.html
/// [`Eq`]:         https://doc.rust-lang.org/std/cmp/trait.Eq.html
/// [`PartialEq`]:  https://doc.rust-lang.org/std/cmp/trait.PartialEq.html
#[macro_export]
macro_rules! assert_trait_sub_all {
    ($sub:path: $($super:path),+ $(,)?) => {
        const _: () = {
            // One scope per super-trait.
            $({
                #[allow(non_camel_case_types)]
                trait __Impl_Implication: $super {}

                // Can only be implemented for `$sub` types if `$super` is
                // also implemented.
                impl<T: $sub> __Impl_Implication for T {}
            })+
        };
    };
}

/// Asserts that the trait is a parent of all of the other traits.
///
/// Related:
/// - [`assert_trait_sub_all!`]
///
/// # Examples
///
/// With this, traits `A` and `B` can both be tested to require [`Copy`] on a
/// single line:
///
/// ```
/// # use static_assertions::assert_trait_super_all;
/// trait A: Copy {}
/// trait B: Copy {}
///
/// assert_trait_super_all!(Copy: A, B);
/// ```
///
/// Otherwise, each sub-trait would require its own call to
/// [`assert_trait_sub_all!`]:
///
/// ```
/// # #[macro_use] extern crate static_assertions; fn main() {}
/// # trait A: Copy {}
/// # trait B: Copy {}
/// assert_trait_sub_all!(A: Copy);
/// assert_trait_sub_all!(B: Copy);
/// ```
///
/// The following example fails to compile because trait `C` does not require
/// [`Copy`]:
///
/// ```compile_fail
/// # use static_assertions::assert_trait_super_all;
/// # trait A: Copy {}
/// # trait B: Copy {}
/// trait C {}
///
/// assert_trait_super_all!(Copy: A, B, C);
/// ```
///
/// [`assert_trait_sub_all!`]: macro.assert_trait_sub_all.html
///
/// [`Copy`]: https://doc.rust-lang.org/std/marker/trait.Copy.html
#[macro_export(local_inner_macros)]
macro_rules! assert_trait_super_all {
    ($super:path: $($sub:path),+ $(,)?) => {
        $(assert_trait_sub_all!($sub: $super);)+
    };
}
