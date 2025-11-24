use crate::alloc::Box;
use core::any::TypeId;
use core::marker::PhantomData;
use core::mem::{self, MaybeUninit};
use core::ptr;

#[cfg(feature = "unstable-debug")]
use core::any;

pub struct Any {
    value: Value,
    drop: unsafe fn(&mut Value),
    type_id: TypeId,

    /// For panic messages only. Not used for comparison.
    #[cfg(feature = "unstable-debug")]
    type_name: &'static str,
}

union Value {
    ptr: *mut (),
    inline: [MaybeUninit<usize>; 2],
}

fn is_small<T>() -> bool {
    mem::size_of::<T>() <= mem::size_of::<Value>()
        && mem::align_of::<T>() <= mem::align_of::<Value>()
}

impl Any {
    // This is unsafe -- caller must not hold on to the Any beyond the lifetime
    // of T.
    //
    // Example of bad code:
    //
    //    let s = "bad".to_owned();
    //    let a = Any::new(&s);
    //    drop(s);
    //
    // Now `a.view()` and `a.take()` return references to a dead String.
    pub(crate) unsafe fn new<T>(t: T) -> Self {
        let value: Value;
        let drop: unsafe fn(&mut Value);
        let type_id = non_static_type_id::<T>();

        if is_small::<T>() {
            let mut inline = [MaybeUninit::uninit(); 2];
            unsafe { ptr::write(inline.as_mut_ptr().cast::<T>(), t) };
            value = Value { inline };
            unsafe fn inline_drop<T>(value: &mut Value) {
                unsafe { ptr::drop_in_place(value.inline.as_mut_ptr().cast::<T>()) }
            }
            drop = inline_drop::<T>;
        } else {
            let ptr = Box::into_raw(Box::new(t)).cast::<()>();
            value = Value { ptr };
            unsafe fn ptr_drop<T>(value: &mut Value) {
                mem::drop(unsafe { Box::from_raw(value.ptr.cast::<T>()) });
            }
            drop = ptr_drop::<T>;
        };

        Any {
            value,
            drop,
            type_id,
            #[cfg(feature = "unstable-debug")]
            type_name: any::type_name::<T>(),
        }
    }

    // This is unsafe -- caller is responsible that T is the correct type.
    pub(crate) unsafe fn view<T>(&mut self) -> &mut T {
        if self.type_id != non_static_type_id::<T>() {
            self.invalid_cast_to::<T>();
        }

        let ptr = if is_small::<T>() {
            unsafe { self.value.inline.as_mut_ptr().cast::<T>() }
        } else {
            unsafe { self.value.ptr.cast::<T>() }
        };

        unsafe { &mut *ptr }
    }

    // This is unsafe -- caller is responsible that T is the correct type.
    pub(crate) unsafe fn take<T>(mut self) -> T {
        if self.type_id != non_static_type_id::<T>() {
            self.invalid_cast_to::<T>();
        }

        if is_small::<T>() {
            let ptr = unsafe { self.value.inline.as_mut_ptr().cast::<T>() };
            let value = unsafe { ptr::read(ptr) };
            mem::forget(self);
            value
        } else {
            let ptr = unsafe { self.value.ptr.cast::<T>() };
            let box_t = unsafe { Box::from_raw(ptr) };
            mem::forget(self);
            *box_t
        }
    }

    #[cfg(not(feature = "unstable-debug"))]
    fn invalid_cast_to<T>(&self) -> ! {
        panic!("invalid cast; enable `unstable-debug` feature to debug");
    }

    #[cfg(feature = "unstable-debug")]
    fn invalid_cast_to<T>(&self) -> ! {
        let from = self.type_name;
        let to = any::type_name::<T>();
        panic!("invalid cast: {} to {}", from, to);
    }
}

impl Drop for Any {
    fn drop(&mut self) {
        unsafe { (self.drop)(&mut self.value) }
    }
}

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

fn non_static_type_id<T>() -> TypeId {
    let non_static_thing = &PhantomData::<T>;
    let thing = unsafe {
        mem::transmute::<&dyn NonStaticAny, &(dyn NonStaticAny + 'static)>(non_static_thing)
    };
    NonStaticAny::get_type_id(thing)
}

#[test]
fn test_non_static_type_id() {
    assert_eq!(non_static_type_id::<usize>(), non_static_type_id::<usize>());
    assert_eq!(
        non_static_type_id::<&str>(),
        non_static_type_id::<&'static str>()
    );

    assert_ne!(non_static_type_id::<u32>(), non_static_type_id::<[u8; 4]>());
    assert_ne!(
        non_static_type_id::<u32>(),
        non_static_type_id::<[u32; 2]>()
    );

    assert_ne!(non_static_type_id::<usize>(), non_static_type_id::<isize>());
    assert_ne!(
        non_static_type_id::<usize>(),
        non_static_type_id::<&usize>()
    );
    assert_ne!(
        non_static_type_id::<&usize>(),
        non_static_type_id::<&&usize>()
    );
    assert_ne!(
        non_static_type_id::<&usize>(),
        non_static_type_id::<&mut usize>()
    );

    struct A;
    struct B;
    assert_ne!(non_static_type_id::<A>(), non_static_type_id::<B>());
}
