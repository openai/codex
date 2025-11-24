use crate::JsonSchema;
use crate::_alloc_prelude::*;

macro_rules! wrapper_impl {
    ($($desc:tt)+) => {
        forward_impl!(($($desc)+ where T: JsonSchema) => T);
    };
}

wrapper_impl!(<'a, T: ?Sized> JsonSchema for &'a T);
wrapper_impl!(<'a, T: ?Sized> JsonSchema for &'a mut T);
wrapper_impl!(<T: ?Sized> JsonSchema for Box<T>);
wrapper_impl!(<T: ?Sized> JsonSchema for alloc::rc::Rc<T>);
wrapper_impl!(<T: ?Sized> JsonSchema for alloc::rc::Weak<T>);
wrapper_impl!(<T: ?Sized> JsonSchema for alloc::sync::Arc<T>);
wrapper_impl!(<T: ?Sized> JsonSchema for alloc::sync::Weak<T>);
#[cfg(feature = "std")]
wrapper_impl!(<T: ?Sized> JsonSchema for std::sync::Mutex<T>);
#[cfg(feature = "std")]
wrapper_impl!(<T: ?Sized> JsonSchema for std::sync::RwLock<T>);
wrapper_impl!(<T: ?Sized> JsonSchema for core::cell::Cell<T>);
wrapper_impl!(<T: ?Sized> JsonSchema for core::cell::RefCell<T>);
wrapper_impl!(<'a, T: ?Sized + ToOwned> JsonSchema for alloc::borrow::Cow<'a, T>);
wrapper_impl!(<T> JsonSchema for core::num::Wrapping<T>);
wrapper_impl!(<T> JsonSchema for core::cmp::Reverse<T>);
