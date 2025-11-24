use crate::r#gen::SchemaGenerator;
use crate::schema::Schema;
use crate::JsonSchema;

macro_rules! wrapper_impl {
    ($($desc:tt)+) => {
        forward_impl!(($($desc)+ where T: JsonSchema) => T);
    };
}

wrapper_impl!(<'a, T: ?Sized> JsonSchema for &'a T);
wrapper_impl!(<'a, T: ?Sized> JsonSchema for &'a mut T);
wrapper_impl!(<T: ?Sized> JsonSchema for Box<T>);
wrapper_impl!(<T: ?Sized> JsonSchema for std::rc::Rc<T>);
wrapper_impl!(<T: ?Sized> JsonSchema for std::rc::Weak<T>);
wrapper_impl!(<T: ?Sized> JsonSchema for std::sync::Arc<T>);
wrapper_impl!(<T: ?Sized> JsonSchema for std::sync::Weak<T>);
wrapper_impl!(<T: ?Sized> JsonSchema for std::sync::Mutex<T>);
wrapper_impl!(<T: ?Sized> JsonSchema for std::sync::RwLock<T>);
wrapper_impl!(<T: ?Sized> JsonSchema for std::cell::Cell<T>);
wrapper_impl!(<T: ?Sized> JsonSchema for std::cell::RefCell<T>);
wrapper_impl!(<'a, T: ?Sized + ToOwned> JsonSchema for std::borrow::Cow<'a, T>);
wrapper_impl!(<T> JsonSchema for std::num::Wrapping<T>);
wrapper_impl!(<T> JsonSchema for std::cmp::Reverse<T>);
