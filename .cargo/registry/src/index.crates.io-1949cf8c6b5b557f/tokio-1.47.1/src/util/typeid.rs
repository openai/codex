use std::{
    any::TypeId,
    marker::PhantomData,
    mem::{self, ManuallyDrop},
};

// SAFETY: this function does not compare lifetimes. Values returned as `Ok`
// may have their lifetimes extended.
pub(super) unsafe fn try_transmute<Src, Target: 'static>(x: Src) -> Result<Target, Src> {
    if nonstatic_typeid::<Src>() == TypeId::of::<Target>() {
        let x = ManuallyDrop::new(x);
        Ok(mem::transmute_copy::<Src, Target>(&x))
    } else {
        Err(x)
    }
}

// https://github.com/dtolnay/typeid/blob/b06a3c08a0eaccc7df6091ade1ae4e3fb53609d5/src/lib.rs#L197-L222
#[inline(always)]
fn nonstatic_typeid<T>() -> TypeId
where
    T: ?Sized,
{
    trait NonStaticAny {
        fn get_type_id(&self) -> TypeId
        where
            Self: 'static;
    }

    impl<T: ?Sized> NonStaticAny for PhantomData<T> {
        #[inline(always)]
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
