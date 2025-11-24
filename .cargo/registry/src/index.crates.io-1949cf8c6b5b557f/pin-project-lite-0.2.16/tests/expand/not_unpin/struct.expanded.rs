use pin_project_lite::pin_project;
struct Struct<T, U> {
    pinned: T,
    unpinned: U,
}
#[doc(hidden)]
#[allow(
    dead_code,
    single_use_lifetimes,
    clippy::unknown_clippy_lints,
    clippy::absolute_paths,
    clippy::min_ident_chars,
    clippy::mut_mut,
    clippy::redundant_pub_crate,
    clippy::ref_option_ref,
    clippy::single_char_lifetime_names,
    clippy::type_repetition_in_bounds
)]
struct StructProj<'__pin, T, U>
where
    Struct<T, U>: '__pin,
{
    pinned: ::pin_project_lite::__private::Pin<&'__pin mut (T)>,
    unpinned: &'__pin mut (U),
}
#[doc(hidden)]
#[allow(
    dead_code,
    single_use_lifetimes,
    clippy::unknown_clippy_lints,
    clippy::absolute_paths,
    clippy::min_ident_chars,
    clippy::mut_mut,
    clippy::redundant_pub_crate,
    clippy::ref_option_ref,
    clippy::single_char_lifetime_names,
    clippy::type_repetition_in_bounds
)]
struct StructProjRef<'__pin, T, U>
where
    Struct<T, U>: '__pin,
{
    pinned: ::pin_project_lite::__private::Pin<&'__pin (T)>,
    unpinned: &'__pin (U),
}
#[allow(
    explicit_outlives_requirements,
    single_use_lifetimes,
    clippy::unknown_clippy_lints,
    clippy::absolute_paths,
    clippy::min_ident_chars,
    clippy::redundant_pub_crate,
    clippy::single_char_lifetime_names,
    clippy::used_underscore_binding
)]
const _: () = {
    impl<T, U> Struct<T, U> {
        #[doc(hidden)]
        #[inline]
        fn project<'__pin>(
            self: ::pin_project_lite::__private::Pin<&'__pin mut Self>,
        ) -> StructProj<'__pin, T, U> {
            unsafe {
                let Self { pinned, unpinned } = self.get_unchecked_mut();
                StructProj {
                    pinned: ::pin_project_lite::__private::Pin::new_unchecked(pinned),
                    unpinned: unpinned,
                }
            }
        }
        #[doc(hidden)]
        #[inline]
        fn project_ref<'__pin>(
            self: ::pin_project_lite::__private::Pin<&'__pin Self>,
        ) -> StructProjRef<'__pin, T, U> {
            unsafe {
                let Self { pinned, unpinned } = self.get_ref();
                StructProjRef {
                    pinned: ::pin_project_lite::__private::Pin::new_unchecked(pinned),
                    unpinned: unpinned,
                }
            }
        }
    }
    #[doc(hidden)]
    impl<'__pin, T, U> ::pin_project_lite::__private::Unpin for Struct<T, U>
    where
        (
            ::pin_project_lite::__private::PhantomData<&'__pin ()>,
            ::pin_project_lite::__private::PhantomPinned,
        ): ::pin_project_lite::__private::Unpin,
    {}
    trait MustNotImplDrop {}
    #[allow(clippy::drop_bounds, drop_bounds)]
    impl<T: ::pin_project_lite::__private::Drop> MustNotImplDrop for T {}
    impl<T, U> MustNotImplDrop for Struct<T, U> {}
    #[forbid(unaligned_references, safe_packed_borrows)]
    fn __assert_not_repr_packed<T, U>(this: &Struct<T, U>) {
        let _ = &this.pinned;
        let _ = &this.unpinned;
    }
};
fn main() {}
