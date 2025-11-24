/// Asserts that the type implements exactly one in a set of traits.
///
/// Related:
/// - [`assert_impl_any!`]
/// - [`assert_impl_all!`]
/// - [`assert_not_impl_all!`]
/// - [`assert_not_impl_any!`]
///
/// # Examples
///
/// Given some type `Foo`, it is expected to implement either `Snap`, `Crackle`,
/// or `Pop`:
///
/// ```compile_fail
/// # use static_assertions::assert_impl_one; fn main() {}
/// struct Foo;
///
/// trait Snap {}
/// trait Crackle {}
/// trait Pop {}
///
/// assert_impl_one!(Foo: Snap, Crackle, Pop);
/// ```
///
/// If _only_ `Crackle` is implemented, the assertion passes:
///
/// ```
/// # use static_assertions::assert_impl_one; fn main() {}
/// # struct Foo;
/// # trait Snap {}
/// # trait Crackle {}
/// # trait Pop {}
/// impl Crackle for Foo {}
///
/// assert_impl_one!(Foo: Snap, Crackle, Pop);
/// ```
///
/// If `Snap` or `Pop` is _also_ implemented, the assertion fails:
///
/// ```compile_fail
/// # use static_assertions::assert_impl_one; fn main() {}
/// # struct Foo;
/// # trait Snap {}
/// # trait Crackle {}
/// # trait Pop {}
/// # impl Crackle for Foo {}
/// impl Pop for Foo {}
///
/// assert_impl_one!(Foo: Snap, Crackle, Pop);
/// ```
///
/// [`assert_impl_any!`]:     macro.assert_impl_any.html
/// [`assert_impl_all!`]:     macro.assert_impl_all.html
/// [`assert_not_impl_all!`]: macro.assert_not_impl_all.html
/// [`assert_not_impl_any!`]: macro.assert_not_impl_any.html
#[macro_export]
macro_rules! assert_impl_one {
    ($x:ty: $($t:path),+ $(,)?) => {
        const _: fn() = || {
            // Generic trait that must be implemented for `$x` exactly once.
            trait AmbiguousIfMoreThanOne<A> {
                // Required for actually being able to reference the trait.
                fn some_item() {}
            }

            // Creates multiple scoped `Token` types for each trait `$t`, over
            // which a specialized `AmbiguousIfMoreThanOne<Token>` is
            // implemented for every type that implements `$t`.
            $({
                #[allow(dead_code)]
                struct Token;

                impl<T: ?Sized + $t> AmbiguousIfMoreThanOne<Token> for T {}
            })+

            // If there is only one specialized trait impl, type inference with
            // `_` can be resolved and this can compile. Fails to compile if
            // `$x` implements more than one `AmbiguousIfMoreThanOne<Token>` or
            // does not implement any at all.
            let _ = <$x as AmbiguousIfMoreThanOne<_>>::some_item;
        };
    };
}

/// Asserts that the type implements _all_ of the given traits.
///
/// See [`assert_not_impl_all!`] for achieving the opposite effect.
///
/// # Examples
///
/// This can be used to ensure types implement auto traits such as [`Send`] and
/// [`Sync`], as well as traits with [blanket `impl`s][blanket].
///
/// ```
/// # #[macro_use] extern crate static_assertions; fn main() {}
/// assert_impl_all!(u32: Copy, Send);
/// assert_impl_all!(&str: Into<String>);
/// ```
///
/// The following example fails to compile because raw pointers do not implement
/// [`Send`] since they cannot be moved between threads safely:
///
/// ```compile_fail
/// # #[macro_use] extern crate static_assertions; fn main() {}
/// assert_impl_all!(*const u8: Send);
/// ```
///
/// [`assert_not_impl_all!`]: macro.assert_not_impl_all.html
/// [`Send`]: https://doc.rust-lang.org/std/marker/trait.Send.html
/// [`Sync`]: https://doc.rust-lang.org/std/marker/trait.Sync.html
/// [blanket]: https://doc.rust-lang.org/book/ch10-02-traits.html#using-trait-bounds-to-conditionally-implement-methods
#[macro_export]
macro_rules! assert_impl_all {
    ($type:ty: $($trait:path),+ $(,)?) => {
        const _: fn() = || {
            // Only callable when `$type` implements all traits in `$($trait)+`.
            fn assert_impl_all<T: ?Sized $(+ $trait)+>() {}
            assert_impl_all::<$type>();
        };
    };
}

/// Asserts that the type implements _any_ of the given traits.
///
/// See [`assert_not_impl_any!`] for achieving the opposite effect.
///
/// # Examples
///
/// `u8` cannot be converted from `u16`, but it can be converted into `u16`:
///
/// ```
/// # #[macro_use] extern crate static_assertions; fn main() {}
/// assert_impl_any!(u8: From<u16>, Into<u16>);
/// ```
///
/// The unit type cannot be converted from `u8` or `u16`, but it does implement
/// [`Send`]:
///
/// ```
/// # #[macro_use] extern crate static_assertions; fn main() {}
/// assert_impl_any!((): From<u8>, From<u16>, Send);
/// ```
///
/// The following example fails to compile because raw pointers do not implement
/// [`Send`] or [`Sync`] since they cannot be moved or shared between threads
/// safely:
///
/// ```compile_fail
/// # #[macro_use] extern crate static_assertions; fn main() {}
/// assert_impl_any!(*const u8: Send, Sync);
/// ```
///
/// [`assert_not_impl_any!`]: macro.assert_not_impl_any.html
/// [`Send`]: https://doc.rust-lang.org/std/marker/trait.Send.html
/// [`Sync`]: https://doc.rust-lang.org/std/marker/trait.Sync.html
#[macro_export]
macro_rules! assert_impl_any {
    ($x:ty: $($t:path),+ $(,)?) => {
        const _: fn() = || {
            use $crate::_core::marker::PhantomData;
            use $crate::_core::ops::Deref;

            // Fallback to use as the first iterative assignment to `previous`.
            let previous = AssertImplAnyFallback;
            struct AssertImplAnyFallback;

            // Ensures that blanket traits can't impersonate the method. This
            // prevents a false positive attack where---if a blanket trait is in
            // scope that has `_static_assertions_impl_any`---the macro will
            // compile when it shouldn't.
            //
            // See https://github.com/nvzqz/static-assertions-rs/issues/19 for
            // more info.
            struct ActualAssertImplAnyToken;
            trait AssertImplAnyToken {}
            impl AssertImplAnyToken for ActualAssertImplAnyToken {}
            fn assert_impl_any_token<T: AssertImplAnyToken>(_: T) {}

            $(let previous = {
                struct Wrapper<T, N>(PhantomData<T>, N);

                // If the method for this wrapper can't be called then the
                // compiler will insert a deref and try again. This forwards the
                // compiler's next attempt to the previous wrapper.
                impl<T, N> Deref for Wrapper<T, N> {
                    type Target = N;

                    fn deref(&self) -> &Self::Target {
                        &self.1
                    }
                }

                // This impl is bounded on the `$t` trait, so the method can
                // only be called if `$x` implements `$t`. This is why a new
                // `Wrapper` is defined for each `previous`.
                impl<T: $t, N> Wrapper<T, N> {
                    fn _static_assertions_impl_any(&self) -> ActualAssertImplAnyToken {
                        ActualAssertImplAnyToken
                    }
                }

                Wrapper::<$x, _>(PhantomData, previous)
            };)+

            // Attempt to find the method that can actually be called. The found
            // method must return a type that implements the sealed `Token`
            // trait, this ensures that blanket trait methods can't cause this
            // macro to compile.
            assert_impl_any_token(previous._static_assertions_impl_any());
        };
    };
}

/// Asserts that the type does **not** implement _all_ of the given traits.
///
/// This can be used to ensure types do not implement auto traits such as
/// [`Send`] and [`Sync`], as well as traits with [blanket `impl`s][blanket].
///
/// Note that the combination of all provided traits is required to not be
/// implemented. If you want to check that none of multiple traits are
/// implemented you should invoke [`assert_not_impl_any!`] instead.
///
/// # Examples
///
/// Although `u32` implements `From<u16>`, it does not implement `Into<usize>`:
///
/// ```
/// # #[macro_use] extern crate static_assertions; fn main() {}
/// assert_not_impl_all!(u32: From<u16>, Into<usize>);
/// ```
///
/// The following example fails to compile since `u32` can be converted into
/// `u64`.
///
/// ```compile_fail
/// # #[macro_use] extern crate static_assertions; fn main() {}
/// assert_not_impl_all!(u32: Into<u64>);
/// ```
///
/// The following compiles because [`Cell`] is not both [`Sync`] _and_ [`Send`]:
///
/// ```
/// # #[macro_use] extern crate static_assertions; fn main() {}
/// use std::cell::Cell;
///
/// assert_not_impl_all!(Cell<u32>: Sync, Send);
/// ```
///
/// But it is [`Send`], so this fails to compile:
///
/// ```compile_fail
/// # #[macro_use] extern crate static_assertions; fn main() {}
/// # std::cell::Cell;
/// assert_not_impl_all!(Cell<u32>: Send);
/// ```
///
/// [`Send`]: https://doc.rust-lang.org/std/marker/trait.Send.html
/// [`Sync`]: https://doc.rust-lang.org/std/marker/trait.Sync.html
/// [`assert_not_impl_any!`]: macro.assert_not_impl_any.html
/// [`Cell`]: https://doc.rust-lang.org/std/cell/struct.Cell.html
/// [blanket]: https://doc.rust-lang.org/book/ch10-02-traits.html#using-trait-bounds-to-conditionally-implement-methods
#[macro_export]
macro_rules! assert_not_impl_all {
    ($x:ty: $($t:path),+ $(,)?) => {
        const _: fn() = || {
            // Generic trait with a blanket impl over `()` for all types.
            trait AmbiguousIfImpl<A> {
                // Required for actually being able to reference the trait.
                fn some_item() {}
            }

            impl<T: ?Sized> AmbiguousIfImpl<()> for T {}

            // Used for the specialized impl when *all* traits in
            // `$($t)+` are implemented.
            #[allow(dead_code)]
            struct Invalid;

            impl<T: ?Sized $(+ $t)+> AmbiguousIfImpl<Invalid> for T {}

            // If there is only one specialized trait impl, type inference with
            // `_` can be resolved and this can compile. Fails to compile if
            // `$x` implements `AmbiguousIfImpl<Invalid>`.
            let _ = <$x as AmbiguousIfImpl<_>>::some_item;
        };
    };
}

/// Asserts that the type does **not** implement _any_ of the given traits.
///
/// This can be used to ensure types do not implement auto traits such as
/// [`Send`] and [`Sync`], as well as traits with [blanket `impl`s][blanket].
///
/// This macro causes a compilation failure if any of the provided individual
/// traits are implemented for the type. If you want to check that a combination
/// of traits is not implemented you should invoke [`assert_not_impl_all!`]
/// instead. For single traits both macros behave the same.
///
/// # Examples
///
/// If `u32` were to implement `Into` conversions for `usize` _and_ for `u8`,
/// the following would fail to compile:
///
/// ```
/// # #[macro_use] extern crate static_assertions; fn main() {}
/// assert_not_impl_any!(u32: Into<usize>, Into<u8>);
/// ```
///
/// This is also good for simple one-off cases:
///
/// ```
/// # #[macro_use] extern crate static_assertions; fn main() {}
/// assert_not_impl_any!(&'static mut u8: Copy);
/// ```
///
/// The following example fails to compile since `u32` can be converted into
/// `u64` even though it can not be converted into a `u16`:
///
/// ```compile_fail
/// # #[macro_use] extern crate static_assertions; fn main() {}
/// assert_not_impl_any!(u32: Into<u64>, Into<u16>);
/// ```
///
/// [`Send`]: https://doc.rust-lang.org/std/marker/trait.Send.html
/// [`Sync`]: https://doc.rust-lang.org/std/marker/trait.Sync.html
/// [`assert_not_impl_all!`]: macro.assert_not_impl_all.html
/// [blanket]: https://doc.rust-lang.org/book/ch10-02-traits.html#using-trait-bounds-to-conditionally-implement-methods
#[macro_export]
macro_rules! assert_not_impl_any {
    ($x:ty: $($t:path),+ $(,)?) => {
        const _: fn() = || {
            // Generic trait with a blanket impl over `()` for all types.
            trait AmbiguousIfImpl<A> {
                // Required for actually being able to reference the trait.
                fn some_item() {}
            }

            impl<T: ?Sized> AmbiguousIfImpl<()> for T {}

            // Creates multiple scoped `Invalid` types for each trait `$t`, over
            // which a specialized `AmbiguousIfImpl<Invalid>` is implemented for
            // every type that implements `$t`.
            $({
                #[allow(dead_code)]
                struct Invalid;

                impl<T: ?Sized + $t> AmbiguousIfImpl<Invalid> for T {}
            })+

            // If there is only one specialized trait impl, type inference with
            // `_` can be resolved and this can compile. Fails to compile if
            // `$x` implements any `AmbiguousIfImpl<Invalid>`.
            let _ = <$x as AmbiguousIfImpl<_>>::some_item;
        };
    };
}
