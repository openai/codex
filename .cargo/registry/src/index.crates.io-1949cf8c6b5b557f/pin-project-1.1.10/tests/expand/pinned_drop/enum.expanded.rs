use std::pin::Pin;
use pin_project::{pin_project, pinned_drop};
#[pin(__private(PinnedDrop, project = EnumProj, project_ref = EnumProjRef))]
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
enum EnumProj<'pin, T, U>
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
enum EnumProjRef<'pin, T, U>
where
    Enum<T, U>: 'pin,
{
    Struct { pinned: ::pin_project::__private::Pin<&'pin (T)>, unpinned: &'pin (U) },
    Tuple(::pin_project::__private::Pin<&'pin (T)>, &'pin (U)),
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
        ) -> EnumProj<'pin, T, U> {
            unsafe {
                match self.get_unchecked_mut() {
                    Self::Struct { pinned, unpinned } => {
                        EnumProj::Struct {
                            pinned: _pin_project::__private::Pin::new_unchecked(pinned),
                            unpinned,
                        }
                    }
                    Self::Tuple(_0, _1) => {
                        EnumProj::Tuple(
                            _pin_project::__private::Pin::new_unchecked(_0),
                            _1,
                        )
                    }
                    Self::Unit => EnumProj::Unit,
                }
            }
        }
        #[allow(dead_code)]
        #[inline]
        fn project_ref<'pin>(
            self: _pin_project::__private::Pin<&'pin Self>,
        ) -> EnumProjRef<'pin, T, U> {
            unsafe {
                match self.get_ref() {
                    Self::Struct { pinned, unpinned } => {
                        EnumProjRef::Struct {
                            pinned: _pin_project::__private::Pin::new_unchecked(pinned),
                            unpinned,
                        }
                    }
                    Self::Tuple(_0, _1) => {
                        EnumProjRef::Tuple(
                            _pin_project::__private::Pin::new_unchecked(_0),
                            _1,
                        )
                    }
                    Self::Unit => EnumProjRef::Unit,
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
    impl<T, U> _pin_project::__private::Drop for Enum<T, U> {
        #[allow(clippy::missing_inline_in_public_items)]
        fn drop(&mut self) {
            unsafe {
                let __pinned_self = _pin_project::__private::Pin::new_unchecked(self);
                _pin_project::__private::PinnedDrop::drop(__pinned_self);
            }
        }
    }
};
#[doc(hidden)]
impl<T, U> ::pin_project::__private::PinnedDrop for Enum<T, U> {
    unsafe fn drop(self: Pin<&mut Self>) {
        #[allow(
            clippy::missing_const_for_fn,
            clippy::needless_pass_by_value,
            clippy::single_call_fn
        )]
        fn __drop_inner<T, U>(__self: Pin<&mut Enum<T, U>>) {
            fn __drop_inner() {}
            let _ = __self;
        }
        __drop_inner(self);
    }
}
fn main() {}
