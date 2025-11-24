use pin_project_lite::pin_project;
pub struct Struct<T, U> {
    pub pinned: T,
    pub unpinned: U,
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
    pub(crate) struct Projection<'__pin, T, U>
    where
        Struct<T, U>: '__pin,
    {
        pub pinned: ::pin_project_lite::__private::Pin<&'__pin mut (T)>,
        pub unpinned: &'__pin mut (U),
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
    pub(crate) struct ProjectionRef<'__pin, T, U>
    where
        Struct<T, U>: '__pin,
    {
        pub pinned: ::pin_project_lite::__private::Pin<&'__pin (T)>,
        pub unpinned: &'__pin (U),
    }
    impl<T, U> Struct<T, U> {
        #[doc(hidden)]
        #[inline]
        pub(crate) fn project<'__pin>(
            self: ::pin_project_lite::__private::Pin<&'__pin mut Self>,
        ) -> Projection<'__pin, T, U> {
            unsafe {
                let Self { pinned, unpinned } = self.get_unchecked_mut();
                Projection {
                    pinned: ::pin_project_lite::__private::Pin::new_unchecked(pinned),
                    unpinned: unpinned,
                }
            }
        }
        #[doc(hidden)]
        #[inline]
        pub(crate) fn project_ref<'__pin>(
            self: ::pin_project_lite::__private::Pin<&'__pin Self>,
        ) -> ProjectionRef<'__pin, T, U> {
            unsafe {
                let Self { pinned, unpinned } = self.get_ref();
                ProjectionRef {
                    pinned: ::pin_project_lite::__private::Pin::new_unchecked(pinned),
                    unpinned: unpinned,
                }
            }
        }
    }
    #[allow(non_snake_case)]
    pub struct __Origin<'__pin, T, U> {
        __dummy_lifetime: ::pin_project_lite::__private::PhantomData<&'__pin ()>,
        pinned: T,
        unpinned: ::pin_project_lite::__private::AlwaysUnpin<U>,
    }
    impl<'__pin, T, U> ::pin_project_lite::__private::Unpin for Struct<T, U>
    where
        ::pin_project_lite::__private::PinnedFieldsOf<
            __Origin<'__pin, T, U>,
        >: ::pin_project_lite::__private::Unpin,
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
