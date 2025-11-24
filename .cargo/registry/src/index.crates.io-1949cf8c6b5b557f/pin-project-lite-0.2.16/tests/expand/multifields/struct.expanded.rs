use pin_project_lite::pin_project;
struct Struct<T, U> {
    pinned1: T,
    pinned2: T,
    unpinned1: U,
    unpinned2: U,
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
    clippy::single_char_lifetime_names,
    clippy::type_repetition_in_bounds
)]
struct StructProjReplace<T, U> {
    pinned1: ::pin_project_lite::__private::PhantomData<T>,
    pinned2: ::pin_project_lite::__private::PhantomData<T>,
    unpinned1: U,
    unpinned2: U,
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
    struct Projection<'__pin, T, U>
    where
        Struct<T, U>: '__pin,
    {
        pinned1: ::pin_project_lite::__private::Pin<&'__pin mut (T)>,
        pinned2: ::pin_project_lite::__private::Pin<&'__pin mut (T)>,
        unpinned1: &'__pin mut (U),
        unpinned2: &'__pin mut (U),
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
    struct ProjectionRef<'__pin, T, U>
    where
        Struct<T, U>: '__pin,
    {
        pinned1: ::pin_project_lite::__private::Pin<&'__pin (T)>,
        pinned2: ::pin_project_lite::__private::Pin<&'__pin (T)>,
        unpinned1: &'__pin (U),
        unpinned2: &'__pin (U),
    }
    impl<T, U> Struct<T, U> {
        #[doc(hidden)]
        #[inline]
        fn project<'__pin>(
            self: ::pin_project_lite::__private::Pin<&'__pin mut Self>,
        ) -> Projection<'__pin, T, U> {
            unsafe {
                let Self { pinned1, pinned2, unpinned1, unpinned2 } = self
                    .get_unchecked_mut();
                Projection {
                    pinned1: ::pin_project_lite::__private::Pin::new_unchecked(pinned1),
                    pinned2: ::pin_project_lite::__private::Pin::new_unchecked(pinned2),
                    unpinned1: unpinned1,
                    unpinned2: unpinned2,
                }
            }
        }
        #[doc(hidden)]
        #[inline]
        fn project_ref<'__pin>(
            self: ::pin_project_lite::__private::Pin<&'__pin Self>,
        ) -> ProjectionRef<'__pin, T, U> {
            unsafe {
                let Self { pinned1, pinned2, unpinned1, unpinned2 } = self.get_ref();
                ProjectionRef {
                    pinned1: ::pin_project_lite::__private::Pin::new_unchecked(pinned1),
                    pinned2: ::pin_project_lite::__private::Pin::new_unchecked(pinned2),
                    unpinned1: unpinned1,
                    unpinned2: unpinned2,
                }
            }
        }
        #[doc(hidden)]
        #[inline]
        fn project_replace(
            self: ::pin_project_lite::__private::Pin<&mut Self>,
            replacement: Self,
        ) -> StructProjReplace<T, U> {
            unsafe {
                let __self_ptr: *mut Self = self.get_unchecked_mut();
                let __guard = ::pin_project_lite::__private::UnsafeOverwriteGuard::new(
                    __self_ptr,
                    replacement,
                );
                let Self { pinned1, pinned2, unpinned1, unpinned2 } = &mut *__self_ptr;
                let result = StructProjReplace {
                    pinned1: ::pin_project_lite::__private::PhantomData,
                    pinned2: ::pin_project_lite::__private::PhantomData,
                    unpinned1: ::pin_project_lite::__private::ptr::read(unpinned1),
                    unpinned2: ::pin_project_lite::__private::ptr::read(unpinned2),
                };
                {
                    (
                        ::pin_project_lite::__private::UnsafeDropInPlaceGuard::new(
                            pinned1,
                        ),
                        ::pin_project_lite::__private::UnsafeDropInPlaceGuard::new(
                            pinned2,
                        ),
                        (),
                        (),
                    );
                }
                result
            }
        }
    }
    #[allow(non_snake_case)]
    struct __Origin<'__pin, T, U> {
        __dummy_lifetime: ::pin_project_lite::__private::PhantomData<&'__pin ()>,
        pinned1: T,
        pinned2: T,
        unpinned1: ::pin_project_lite::__private::AlwaysUnpin<U>,
        unpinned2: ::pin_project_lite::__private::AlwaysUnpin<U>,
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
        let _ = &this.pinned1;
        let _ = &this.pinned2;
        let _ = &this.unpinned1;
        let _ = &this.unpinned2;
    }
};
fn main() {}
