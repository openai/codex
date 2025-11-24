use std::pin::Pin;
use pin_project::{pin_project, pinned_drop};
#[pin(__private(PinnedDrop))]
struct Struct<T, U> {
    #[pin]
    pinned: T,
    unpinned: U,
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
    #[allow(dead_code, clippy::missing_docs_in_private_items, clippy::mut_mut)]
    struct __StructProjection<'pin, T, U>
    where
        Struct<T, U>: 'pin,
    {
        pinned: ::pin_project::__private::Pin<&'pin mut (T)>,
        unpinned: &'pin mut (U),
    }
    #[allow(dead_code, clippy::missing_docs_in_private_items, clippy::ref_option_ref)]
    struct __StructProjectionRef<'pin, T, U>
    where
        Struct<T, U>: 'pin,
    {
        pinned: ::pin_project::__private::Pin<&'pin (T)>,
        unpinned: &'pin (U),
    }
    impl<T, U> Struct<T, U> {
        #[allow(dead_code)]
        #[inline]
        fn project<'pin>(
            self: _pin_project::__private::Pin<&'pin mut Self>,
        ) -> __StructProjection<'pin, T, U> {
            unsafe {
                let Self { pinned, unpinned } = self.get_unchecked_mut();
                __StructProjection {
                    pinned: _pin_project::__private::Pin::new_unchecked(pinned),
                    unpinned,
                }
            }
        }
        #[allow(dead_code)]
        #[inline]
        fn project_ref<'pin>(
            self: _pin_project::__private::Pin<&'pin Self>,
        ) -> __StructProjectionRef<'pin, T, U> {
            unsafe {
                let Self { pinned, unpinned } = self.get_ref();
                __StructProjectionRef {
                    pinned: _pin_project::__private::Pin::new_unchecked(pinned),
                    unpinned,
                }
            }
        }
    }
    #[forbid(unaligned_references, safe_packed_borrows)]
    fn __assert_not_repr_packed<T, U>(this: &Struct<T, U>) {
        let _ = &this.pinned;
        let _ = &this.unpinned;
    }
    #[allow(missing_debug_implementations, unnameable_types)]
    struct __Struct<'pin, T, U> {
        __pin_project_use_generics: _pin_project::__private::AlwaysUnpin<
            'pin,
            (
                _pin_project::__private::PhantomData<T>,
                _pin_project::__private::PhantomData<U>,
            ),
        >,
        __field0: T,
    }
    impl<'pin, T, U> _pin_project::__private::Unpin for Struct<T, U>
    where
        _pin_project::__private::PinnedFieldsOf<
            __Struct<'pin, T, U>,
        >: _pin_project::__private::Unpin,
    {}
    #[doc(hidden)]
    unsafe impl<'pin, T, U> _pin_project::UnsafeUnpin for Struct<T, U>
    where
        _pin_project::__private::PinnedFieldsOf<
            __Struct<'pin, T, U>,
        >: _pin_project::__private::Unpin,
    {}
    impl<T, U> _pin_project::__private::Drop for Struct<T, U> {
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
impl<T, U> ::pin_project::__private::PinnedDrop for Struct<T, U> {
    unsafe fn drop(self: Pin<&mut Self>) {
        #[allow(
            clippy::missing_const_for_fn,
            clippy::needless_pass_by_value,
            clippy::single_call_fn
        )]
        fn __drop_inner<T, U>(__self: Pin<&mut Struct<T, U>>) {
            fn __drop_inner() {}
            let _ = __self;
        }
        __drop_inner(self);
    }
}
fn main() {}
