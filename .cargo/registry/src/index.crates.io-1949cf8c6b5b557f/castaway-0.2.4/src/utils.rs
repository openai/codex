//! Low-level utility functions.

use core::{
    any::{type_name, TypeId},
    marker::PhantomData,
    mem, ptr,
};

/// Determine if two static, generic types are equal to each other.
#[inline(always)]
pub(crate) fn type_eq<T: 'static, U: 'static>() -> bool {
    // Reduce the chance of `TypeId` collisions causing a problem by also
    // verifying the layouts match and the type names match. Since `T` and `U`
    // are known at compile time the compiler should optimize away these extra
    // checks anyway.
    mem::size_of::<T>() == mem::size_of::<U>()
        && mem::align_of::<T>() == mem::align_of::<U>()
        && mem::needs_drop::<T>() == mem::needs_drop::<U>()
        && TypeId::of::<T>() == TypeId::of::<U>()
        && type_name::<T>() == type_name::<U>()
}

/// Determine if two generic types which may not be static are equal to each
/// other.
///
/// This function must be used with extreme discretion, as no lifetime checking
/// is done. Meaning, this function considers `Struct<'a>` to be equal to
/// `Struct<'b>`, even if either `'a` or `'b` outlives the other.
#[inline(always)]
pub(crate) fn type_eq_non_static<T: ?Sized, U: ?Sized>() -> bool {
    non_static_type_id::<T>() == non_static_type_id::<U>()
}

/// Produces type IDs that are compatible with `TypeId::of::<T>`, but without
/// `T: 'static` bound.
fn non_static_type_id<T: ?Sized>() -> TypeId {
    trait NonStaticAny {
        fn get_type_id(&self) -> TypeId
        where
            Self: 'static;
    }

    impl<T: ?Sized> NonStaticAny for PhantomData<T> {
        fn get_type_id(&self) -> TypeId
        where
            Self: 'static,
        {
            TypeId::of::<T>()
        }
    }

    let phantom_data = PhantomData::<T>;
    NonStaticAny::get_type_id(unsafe {
        mem::transmute::<&dyn NonStaticAny, &(dyn NonStaticAny + 'static)>(&phantom_data)
    })
}

/// Reinterprets the bits of a value of one type as another type.
///
/// Similar to [`std::mem::transmute`], except that it makes no compile-time
/// guarantees about the layout of `T` or `U`, and is therefore even **more**
/// dangerous than `transmute`. Extreme caution must be taken when using this
/// function; it is up to the caller to assert that `T` and `U` have the same
/// size and layout and that it is safe to do this conversion. Which it probably
/// isn't, unless `T` and `U` are identical.
///
/// # Panics
///
/// This function panics if `T` and `U` have different sizes.
///
/// # Safety
///
/// It is up to the caller to uphold the following invariants:
///
/// - `T` must have the same alignment as `U`
/// - `T` must be safe to transmute into `U`
#[inline(always)]
pub(crate) unsafe fn transmute_unchecked<T, U>(value: T) -> U {
    // Assert is necessary to avoid miscompilation caused by a bug in LLVM.
    // Without it `castaway::cast!(123_u8, (u8, u8))` returns `Ok(...)` on
    // release build profile. `assert` shouldn't be replaced by `assert_eq`
    // because with `assert_eq` Rust 1.70 and 1.71 will still miscompile it.
    //
    // See https://github.com/rust-lang/rust/issues/127286 for details.
    assert!(
        mem::size_of::<T>() == mem::size_of::<U>(),
        "cannot transmute_unchecked if Dst and Src have different size"
    );

    let dest = ptr::read(&value as *const T as *const U);
    mem::forget(value);
    dest
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_static_type_comparisons() {
        assert!(type_eq_non_static::<u8, u8>());
        assert!(type_eq_non_static::<&'static u8, &'static u8>());
        assert!(type_eq_non_static::<&u8, &'static u8>());

        assert!(!type_eq_non_static::<u8, i8>());
        assert!(!type_eq_non_static::<u8, &'static u8>());
    }
}
