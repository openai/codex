use pin_project::pin_project;
#[pin(__private(project = Proj, project_ref = ProjRef, project_replace = ProjOwn))]
enum Enum<T, U> {
    Struct { #[pin] pinned: T, unpinned: U },
    Tuple(#[pin] T, U),
    Unit,
}
#[allow(
    dead_code,
    deprecated,
    explicit_outlives_requirements,
    single_use_lifetimes,
    unreachable_pub,
    unused_tuple_struct_fields,
    clippy::unknown_clippy_lints,
    clippy::absolute_paths,
    clippy::min_ident_chars,
    clippy::pattern_type_mismatch,
    clippy::pub_with_shorthand,
    clippy::redundant_pub_crate,
    clippy::single_char_lifetime_names,
    clippy::type_repetition_in_bounds,
    clippy::missing_docs_in_private_items,
    clippy::mut_mut
)]
enum Proj<'pin, T, U>
where
    Enum<T, U>: 'pin,
{
    Struct {
        pinned: ::pin_project::__private::Pin<&'pin mut (T)>,
        unpinned: &'pin mut (U),
    },
    Tuple(::pin_project::__private::Pin<&'pin mut (T)>, &'pin mut (U)),
    Unit,
}
#[allow(
    dead_code,
    deprecated,
    explicit_outlives_requirements,
    single_use_lifetimes,
    unreachable_pub,
    unused_tuple_struct_fields,
    clippy::unknown_clippy_lints,
    clippy::absolute_paths,
    clippy::min_ident_chars,
    clippy::pattern_type_mismatch,
    clippy::pub_with_shorthand,
    clippy::redundant_pub_crate,
    clippy::single_char_lifetime_names,
    clippy::type_repetition_in_bounds,
    clippy::missing_docs_in_private_items,
    clippy::ref_option_ref
)]
enum ProjRef<'pin, T, U>
where
    Enum<T, U>: 'pin,
{
    Struct { pinned: ::pin_project::__private::Pin<&'pin (T)>, unpinned: &'pin (U) },
    Tuple(::pin_project::__private::Pin<&'pin (T)>, &'pin (U)),
    Unit,
}
#[allow(
    dead_code,
    deprecated,
    explicit_outlives_requirements,
    single_use_lifetimes,
    unreachable_pub,
    unused_tuple_struct_fields,
    clippy::unknown_clippy_lints,
    clippy::absolute_paths,
    clippy::min_ident_chars,
    clippy::pattern_type_mismatch,
    clippy::pub_with_shorthand,
    clippy::redundant_pub_crate,
    clippy::single_char_lifetime_names,
    clippy::type_repetition_in_bounds,
    variant_size_differences,
    clippy::large_enum_variant,
    clippy::missing_docs_in_private_items
)]
enum ProjOwn<T, U> {
    Struct { pinned: ::pin_project::__private::PhantomData<T>, unpinned: U },
    Tuple(::pin_project::__private::PhantomData<T>, U),
    Unit,
}
#[allow(
    unused_qualifications,
    deprecated,
    explicit_outlives_requirements,
    single_use_lifetimes,
    unreachable_pub,
    unused_tuple_struct_fields,
    clippy::unknown_clippy_lints,
    clippy::absolute_paths,
    clippy::min_ident_chars,
    clippy::pattern_type_mismatch,
    clippy::pub_with_shorthand,
    clippy::redundant_pub_crate,
    clippy::single_char_lifetime_names,
    clippy::type_repetition_in_bounds,
    clippy::elidable_lifetime_names,
    clippy::missing_const_for_fn,
    clippy::needless_lifetimes,
    clippy::semicolon_if_nothing_returned,
    clippy::use_self,
    clippy::used_underscore_binding
)]
const _: () = {
    #[allow(unused_extern_crates)]
    extern crate pin_project as _pin_project;
    impl<T, U> Enum<T, U> {
        #[allow(dead_code)]
        #[inline]
        fn project<'pin>(
            self: _pin_project::__private::Pin<&'pin mut Self>,
        ) -> Proj<'pin, T, U> {
            unsafe {
                match self.get_unchecked_mut() {
                    Self::Struct { pinned, unpinned } => {
                        Proj::Struct {
                            pinned: _pin_project::__private::Pin::new_unchecked(pinned),
                            unpinned,
                        }
                    }
                    Self::Tuple(_0, _1) => {
                        Proj::Tuple(_pin_project::__private::Pin::new_unchecked(_0), _1)
                    }
                    Self::Unit => Proj::Unit,
                }
            }
        }
        #[allow(dead_code)]
        #[inline]
        fn project_ref<'pin>(
            self: _pin_project::__private::Pin<&'pin Self>,
        ) -> ProjRef<'pin, T, U> {
            unsafe {
                match self.get_ref() {
                    Self::Struct { pinned, unpinned } => {
                        ProjRef::Struct {
                            pinned: _pin_project::__private::Pin::new_unchecked(pinned),
                            unpinned,
                        }
                    }
                    Self::Tuple(_0, _1) => {
                        ProjRef::Tuple(
                            _pin_project::__private::Pin::new_unchecked(_0),
                            _1,
                        )
                    }
                    Self::Unit => ProjRef::Unit,
                }
            }
        }
        #[allow(dead_code)]
        #[inline]
        fn project_replace(
            self: _pin_project::__private::Pin<&mut Self>,
            __replacement: Self,
        ) -> ProjOwn<T, U> {
            unsafe {
                let __self_ptr: *mut Self = self.get_unchecked_mut();
                let __guard = _pin_project::__private::UnsafeOverwriteGuard::new(
                    __self_ptr,
                    __replacement,
                );
                match &mut *__self_ptr {
                    Self::Struct { pinned, unpinned } => {
                        let __result = ProjOwn::Struct {
                            pinned: _pin_project::__private::PhantomData,
                            unpinned: _pin_project::__private::ptr::read(unpinned),
                        };
                        {
                            let __guard = _pin_project::__private::UnsafeDropInPlaceGuard::new(
                                pinned,
                            );
                        }
                        __result
                    }
                    Self::Tuple(_0, _1) => {
                        let __result = ProjOwn::Tuple(
                            _pin_project::__private::PhantomData,
                            _pin_project::__private::ptr::read(_1),
                        );
                        {
                            let __guard = _pin_project::__private::UnsafeDropInPlaceGuard::new(
                                _0,
                            );
                        }
                        __result
                    }
                    Self::Unit => {
                        let __result = ProjOwn::Unit;
                        {}
                        __result
                    }
                }
            }
        }
    }
    #[allow(missing_debug_implementations, unnameable_types)]
    struct __Enum<'pin, T, U> {
        __pin_project_use_generics: _pin_project::__private::AlwaysUnpin<
            'pin,
            (
                _pin_project::__private::PhantomData<T>,
                _pin_project::__private::PhantomData<U>,
            ),
        >,
        __field0: T,
        __field1: T,
    }
    impl<'pin, T, U> _pin_project::__private::Unpin for Enum<T, U>
    where
        _pin_project::__private::PinnedFieldsOf<
            __Enum<'pin, T, U>,
        >: _pin_project::__private::Unpin,
    {}
    #[doc(hidden)]
    unsafe impl<'pin, T, U> _pin_project::UnsafeUnpin for Enum<T, U>
    where
        _pin_project::__private::PinnedFieldsOf<
            __Enum<'pin, T, U>,
        >: _pin_project::__private::Unpin,
    {}
    trait EnumMustNotImplDrop {}
    #[allow(clippy::drop_bounds, drop_bounds)]
    impl<T: _pin_project::__private::Drop> EnumMustNotImplDrop for T {}
    impl<T, U> EnumMustNotImplDrop for Enum<T, U> {}
    #[doc(hidden)]
    impl<T, U> _pin_project::__private::PinnedDrop for Enum<T, U> {
        unsafe fn drop(self: _pin_project::__private::Pin<&mut Self>) {}
    }
};
fn main() {}
