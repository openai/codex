#![allow(dead_code)]

use crate::{Signature, arg::TypeMismatchError, arg::Variant};
use std::{fmt, any};
use std::sync::Arc;
// use std::rc::Rc;
use std::collections::{HashMap, VecDeque};

use super::{Iter, IterAppend, ArgType};

/// Types that can represent a D-Bus message argument implement this trait.
///
/// Types should also implement either Append or Get to be useful.
pub trait Arg {
    /// The corresponding D-Bus argument type code.
    const ARG_TYPE: ArgType;
    /// The corresponding D-Bus type signature for this type.
    fn signature() -> Signature<'static>;
}

/// Helper trait to introspect many arguments.
pub trait ArgAll {
    /// A tuple of &static str. Used for introspection.
    #[allow(non_camel_case_types)] // Note: This should be changed for 0.9 - but for now, don't break backwards compatibility
    type strs;
    /// Enumerates all arguments with their signatures (introspection helper method).
    fn strs_sig<F: FnMut(&'static str, Signature<'static>)>(a: Self::strs, f: F);
}

/// Types that can be appended to a message as arguments implement this trait.
pub trait Append {
    /// Performs the append operation by consuming self.
    fn append(self, ia: &mut IterAppend) where Self: Sized { self.append_by_ref(ia) }

    /// Performs the append operation by borrowing self.
    fn append_by_ref(&self, _: &mut IterAppend);
}

/// Helper trait to append many arguments to a message.
pub trait AppendAll {
    /// Performs the append operation by borrowing self.
    fn append(&self, _: &mut IterAppend);
}

/// Types that can be retrieved from a message as arguments implement this trait.
pub trait Get<'a>: Sized {
    /// Performs the get operation.
    fn get(i: &mut Iter<'a>) -> Option<Self>;
}

/// Helper trait to read all arguments from a message.
pub trait ReadAll: Sized {
    /// Performs the read operation.
    fn read(i: &mut Iter) -> Result<Self, TypeMismatchError>;
}


/// Object safe version of Arg + Append + Get.
pub trait RefArg: fmt::Debug + Send + Sync {
    /// The corresponding D-Bus argument type code.
    fn arg_type(&self) -> ArgType;
    /// The corresponding D-Bus type signature for this type.
    fn signature(&self) -> Signature<'static>;
    /// Performs the append operation.
    fn append(&self, _: &mut IterAppend);
    /// Transforms this argument to Any (which can be downcasted to read the current value).
    ///
    /// See the argument guide's reference section for which types you can cast to.
    fn as_any(&self) -> &dyn any::Any where Self: 'static;
    /// Transforms this argument to Any (which can be downcasted to read the current value).
	///
    /// See the argument guide's reference section for which types you can cast to.
    /// # Panic
    /// Will panic if the interior cannot be made mutable, e g, if encapsulated
    /// inside a Rc with a reference count > 1.
    fn as_any_mut(&mut self) -> &mut dyn any::Any where Self: 'static;
    /// Try to read the argument as an i64.
    ///
    /// Works for: Boolean, Byte, Int16, UInt16, Int32, UInt32, Int64, UnixFd.
    #[inline]
    fn as_i64(&self) -> Option<i64> { None }
    /// Try to read the argument as an u64.
    ///
    /// Works for: Boolean, Byte, Int16, UInt16, Int32, UInt32, UInt64.
    #[inline]
    fn as_u64(&self) -> Option<u64> { None }
    /// Try to read the argument as an f64.
    ///
    /// Works for: Boolean, Byte, Int16, UInt16, Int32, UInt32, Double.
    #[inline]
    fn as_f64(&self) -> Option<f64> { None }
    /// Try to read the argument as a str.
    ///
    /// Works for: String, ObjectPath, Signature.
    #[inline]
    fn as_str(&self) -> Option<&str> { None }
    /// Try to read the argument as an iterator.
    ///
    /// Works for: Array/Dict, Struct, Variant.
    /// For Dicts, keys and values are interleaved.
    #[inline]
    fn as_iter<'a>(&'a self) -> Option<Box<dyn Iterator<Item=&'a dyn RefArg> + 'a>> { None }
    /// Try to read the inner of an argument, as another argument, specifying an index.
    ///
    /// Works for: Variant, Array, Struct, Dict.
    /// For Dicts, even indices gets a key, odd indices gets a value.
    #[inline]
    fn as_static_inner(&self, _index: usize) -> Option<&(dyn RefArg + 'static)> where Self: 'static { None }
    /// Deep clone of the RefArg, causing the result to be 'static.
    ///
    /// Usable as an escape hatch in case of lifetime problems with RefArg.
    ///
    /// In case of complex types (Array, Dict, Struct), the clone is not guaranteed
    /// to have the same internal representation as the original.
    fn box_clone(&self) -> Box<dyn RefArg + 'static>;

    /// Deep clone of an array.
    ///
    /// This method is used internally by box_clone.
    fn array_clone(_arg: &[Self]) -> Option<Box<dyn RefArg + 'static>> where Self: Sized { None }
}

impl<'a> Get<'a> for Box<dyn RefArg> {
    fn get(i: &mut Iter<'a>) -> Option<Self> { i.get_refarg() }
}

/// Cast a RefArg as a specific type (shortcut for any + downcast)
///
/// See the argument guide's reference section for which types you can cast to.
#[inline]
pub fn cast<'a, T: 'static>(a: &'a (dyn RefArg + 'static)) -> Option<&'a T> { a.as_any().downcast_ref() }

/// Cast a RefArg as a specific type (shortcut for any_mut + downcast_mut)
///
/// See the argument guide's reference section for which types you can cast to.
///
/// # Panic
/// Will panic if the interior cannot be made mutable, e g, if encapsulated
/// inside a Rc with a reference count > 1.
#[inline]
pub fn cast_mut<'a, T: 'static>(a: &'a mut (dyn RefArg + 'static)) -> Option<&'a mut T> { a.as_any_mut().downcast_mut() }

/// The type typically used for a dictionary of properties.
pub type PropMap = HashMap<String, Variant<Box<dyn RefArg + 'static>>>;


/// Descend into a hashmap returned by e g "Properties::get_all" to retrieve the value of a property.
///
/// Shortcut for get + cast. Returns None both if the property does not exist, or if it was of a different type.
/// See the argument guide's reference section for which types you can cast to.
pub fn prop_cast<'a, T: 'static>(map: &'a PropMap, key: &str) -> Option<&'a T> {
    map.get(key).and_then(|v| cast(&v.0))
}

/// If a type implements this trait, it means the size and alignment is the same
/// as in D-Bus. This means that you can quickly append and get slices of this type.
///
/// Note: Booleans do not implement this trait because D-Bus booleans are 4 bytes and Rust booleans are 1 byte.
pub unsafe trait FixedArray: Arg + 'static + Clone + Copy {}

/// Types that can be used as keys in a dict type implement this trait.
pub trait DictKey: Arg {}



/// Simple lift over reference to value - this makes some iterators more ergonomic to use
impl<'a, T: Arg> Arg for &'a T {
    const ARG_TYPE: ArgType = T::ARG_TYPE;
    fn signature() -> Signature<'static> { T::signature() }
}
impl<'a, T: Append> Append for &'a T {
    fn append_by_ref(&self, i: &mut IterAppend) { (&**self).append_by_ref(i) }
}
impl<'a, T: DictKey> DictKey for &'a T {}

impl<'a, T: RefArg + ?Sized> RefArg for &'a T {
    #[inline]
    fn arg_type(&self) -> ArgType { (&**self).arg_type() }
    #[inline]
    fn signature(&self) -> Signature<'static> { (&**self).signature() }
    #[inline]
    fn append(&self, i: &mut IterAppend) { (&**self).append(i) }
    #[inline]
    fn as_any(&self) -> &dyn any::Any where T: 'static { (&**self).as_any() }
    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn any::Any where T: 'static { unreachable!() }
    #[inline]
    fn as_i64(&self) -> Option<i64> { (&**self).as_i64() }
    #[inline]
    fn as_u64(&self) -> Option<u64> { (&**self).as_u64() }
    #[inline]
    fn as_f64(&self) -> Option<f64> { (&**self).as_f64() }
    #[inline]
    fn as_str(&self) -> Option<&str> { (&**self).as_str() }
    #[inline]
    fn as_iter<'b>(&'b self) -> Option<Box<dyn Iterator<Item=&'b dyn RefArg> + 'b>> { (&**self).as_iter() }
    #[inline]
    fn as_static_inner(&self, index: usize) -> Option<&(dyn RefArg + 'static)> where Self: 'static { (&**self).as_static_inner(index) }
    #[inline]
    fn box_clone(&self) -> Box<dyn RefArg + 'static> { (&**self).box_clone() }
}



macro_rules! deref_impl {
    ($t: ident, $ss: ident, $make_mut: expr) => {

impl<T: RefArg + ?Sized> RefArg for $t<T> {
    #[inline]
    fn arg_type(&self) -> ArgType { (&**self).arg_type() }
    #[inline]
    fn signature(&self) -> Signature<'static> { (&**self).signature() }
    #[inline]
    fn append(&self, i: &mut IterAppend) { (&**self).append(i) }
    #[inline]
    fn as_any(&self) -> &dyn any::Any where T: 'static { (&**self).as_any() }
    #[inline]
    fn as_any_mut(&mut $ss) -> &mut dyn any::Any where T: 'static { $make_mut.as_any_mut() }
    #[inline]
    fn as_i64(&self) -> Option<i64> { (&**self).as_i64() }
    #[inline]
    fn as_u64(&self) -> Option<u64> { (&**self).as_u64() }
    #[inline]
    fn as_f64(&self) -> Option<f64> { (&**self).as_f64() }
    #[inline]
    fn as_str(&self) -> Option<&str> { (&**self).as_str() }
    #[inline]
    fn as_iter<'a>(&'a self) -> Option<Box<dyn Iterator<Item=&'a dyn RefArg> + 'a>> { (&**self).as_iter() }
    #[inline]
    fn as_static_inner(&self, index: usize) -> Option<&(dyn RefArg + 'static)> where Self: 'static { (&**self).as_static_inner(index) }
    #[inline]
    fn box_clone(&self) -> Box<dyn RefArg + 'static> { (&**self).box_clone() }
}
impl<T: DictKey> DictKey for $t<T> {}

impl<T: Arg> Arg for $t<T> {
    const ARG_TYPE: ArgType = T::ARG_TYPE;
    fn signature() -> Signature<'static> { T::signature() }
}
impl<'a, T: Get<'a>> Get<'a> for $t<T> {
    fn get(i: &mut Iter<'a>) -> Option<Self> { T::get(i).map($t::new) }
}

    }
}

impl<T: Append> Append for Box<T> {
    fn append_by_ref(&self, i: &mut IterAppend) { (&**self).append_by_ref(i) }
}

deref_impl!(Box, self, &mut **self );
// deref_impl!(Rc, self, Rc::get_mut(self).unwrap());
deref_impl!(Arc, self, Arc::get_mut(self).unwrap());

macro_rules! argall_impl {
    ($($n: ident $t: ident $s: ty,)+) => {

impl<$($t: Arg),*> ArgAll for ($($t,)*) {
    type strs = ($(&'static $s,)*);
    fn strs_sig<Q: FnMut(&'static str, Signature<'static>)>(z: Self::strs, mut q: Q) {
        let ( $($n,)*) = z;
        $( q($n, $t::signature()); )*
    }
}

impl<$($t: Append),*> AppendAll for ($($t,)*) {
    fn append(&self, ia: &mut IterAppend) {
        let ( $($n,)*) = self;
        $( ia.append($n); )*
    }
}

impl<$($t: Arg + for<'z> Get<'z>),*> ReadAll for ($($t,)*) {
    fn read(ii: &mut Iter) -> Result<Self, TypeMismatchError> {
        $( let $n = ii.read()?; )*
        Ok(($( $n, )* ))
    }
}


    }
}

impl ArgAll for () {
    type strs = ();
    fn strs_sig<F: FnMut(&'static str, Signature<'static>)>(_: Self::strs, _: F) {}
}

impl AppendAll for () {
    fn append(&self, _: &mut IterAppend) {}
}

impl ReadAll for () {
    fn read(_: &mut Iter) -> Result<Self, TypeMismatchError> {
        Ok(())
    }
}


/// This is a fallback for methods that have tons of arguments.
/// Usually we'll use a tuple because it is more ergonomic, but AppendAll is only
/// implemented for tuples up to a certain size.
impl AppendAll for VecDeque<Box<dyn RefArg>> {
    fn append(&self, ia: &mut IterAppend) {
        for arg in self {
            arg.append(ia);
        }
    }
}

/// This is a fallback for methods that have tons of arguments.
/// Usually we'll use a tuple because it is more ergonomic, but ReadAll is only
/// implemented for tuples up to a certain size.
impl ReadAll for VecDeque<Box<dyn RefArg>> {
    fn read(ii: &mut Iter) -> Result<Self, TypeMismatchError> {
        let mut r = VecDeque::new();
        while let Some(arg) = ii.get_refarg() {
            r.push_back(arg);
            ii.next();
        }
        Ok(r)
    }
}


argall_impl!(a A str,);
argall_impl!(a A str, b B str,);
argall_impl!(a A str, b B str, c C str,);
argall_impl!(a A str, b B str, c C str, d D str,);
argall_impl!(a A str, b B str, c C str, d D str, e E str,);
argall_impl!(a A str, b B str, c C str, d D str, e E str, f F str,);
argall_impl!(a A str, b B str, c C str, d D str, e E str, f F str, g G str,);
argall_impl!(a A str, b B str, c C str, d D str, e E str, f F str, g G str, h H str,);
argall_impl!(a A str, b B str, c C str, d D str, e E str, f F str, g G str, h H str, i I str,);
argall_impl!(a A str, b B str, c C str, d D str, e E str, f F str, g G str, h H str, i I str, j J str,);
argall_impl!(a A str, b B str, c C str, d D str, e E str, f F str, g G str, h H str, i I str, j J str, k K str,);
argall_impl!(a A str, b B str, c C str, d D str, e E str, f F str, g G str, h H str, i I str, j J str, k K str, l L str,);
argall_impl!(a A str, b B str, c C str, d D str, e E str, f F str, g G str, h H str, i I str, j J str, k K str, l L str, m M str,);
argall_impl!(a A str, b B str, c C str, d D str, e E str, f F str, g G str, h H str, i I str, j J str, k K str, l L str, m M str, n N str,);
argall_impl!(a A str, b B str, c C str, d D str, e E str, f F str, g G str, h H str, i I str, j J str, k K str, l L str, m M str, n N str, o O str,);
argall_impl!(a A str, b B str, c C str, d D str, e E str, f F str, g G str, h H str, i I str, j J str, k K str, l L str, m M str, n N str, o O str, p P str,);
argall_impl!(a A str, b B str, c C str, d D str, e E str, f F str, g G str, h H str, i I str, j J str, k K str, l L str, m M str, n N str, o O str, p P str, r R str,);
argall_impl!(a A str, b B str, c C str, d D str, e E str, f F str, g G str, h H str, i I str, j J str, k K str, l L str, m M str, n N str, o O str, p P str, r R str, s S str,);
argall_impl!(a A str, b B str, c C str, d D str, e E str, f F str, g G str, h H str, i I str, j J str, k K str, l L str, m M str, n N str, o O str, p P str, r R str, s S str, t T str,);
argall_impl!(a A str, b B str, c C str, d D str, e E str, f F str, g G str, h H str, i I str, j J str, k K str, l L str, m M str, n N str, o O str, p P str, r R str, s S str, t T str, u U str,);
argall_impl!(a A str, b B str, c C str, d D str, e E str, f F str, g G str, h H str, i I str, j J str, k K str, l L str, m M str, n N str, o O str, p P str, r R str, s S str, t T str, u U str, v V str,);
argall_impl!(a A str, b B str, c C str, d D str, e E str, f F str, g G str, h H str, i I str, j J str, k K str, l L str, m M str, n N str, o O str, p P str, r R str, s S str, t T str, u U str, v V str, w W str,);
argall_impl!(a A str, b B str, c C str, d D str, e E str, f F str, g G str, h H str, i I str, j J str, k K str, l L str, m M str, n N str, o O str, p P str, r R str, s S str, t T str, u U str, v V str, w W str, x X str,);
argall_impl!(a A str, b B str, c C str, d D str, e E str, f F str, g G str, h H str, i I str, j J str, k K str, l L str, m M str, n N str, o O str, p P str, r R str, s S str, t T str, u U str, v V str, w W str, x X str, y Y str,);
argall_impl!(a A str, b B str, c C str, d D str, e E str, f F str, g G str, h H str, i I str, j J str, k K str, l L str, m M str, n N str, o O str, p P str, r R str, s S str, t T str, u U str, v V str, w W str, x X str, y Y str, z Z str,);


#[cfg(test)]
mod test {
    use crate::{channel::{Channel, BusType}, Message, Path, Signature};
    use crate::message::MessageType;
    use crate::arg::{Array, Variant, Dict, Iter, ArgType, TypeMismatchError, RefArg, cast};

    use std::collections::HashMap;

    #[test]
    fn refarg() {
        let c = Channel::get_private(BusType::Session).unwrap();
        let m = Message::new_method_call(c.unique_name().unwrap(), "/mooh", "com.example.hello", "Hello").unwrap();

        let mut vv: Vec<Variant<Box<dyn RefArg>>> = vec!();
        vv.push(Variant(Box::new(5i32)));
        vv.push(Variant(Box::new(String::from("Hello world"))));
        let m = m.append_ref(&vv);

        let (f1, f2) = (false, 7u64);
        let mut v: Vec<&dyn RefArg> = vec!();
        v.push(&f1);
        v.push(&f2);
        let m = m.append_ref(&v);
        let vi32 = vec![7i32, 9i32];
        let vstr: Vec<String> = ["This", "is", "dbus", "rs"].iter().map(|&s| s.into()).collect();
        let m = m.append_ref(&[&vi32 as &dyn RefArg, &vstr as &dyn RefArg]);
        let mut map = HashMap::new();
        map.insert(true, String::from("Yes"));
        map.insert(false, String::from("No"));
        let m = m.append_ref(&[&map as &dyn RefArg, &1.5f64 as &dyn RefArg]);

        c.send(m).unwrap();

        loop {
            if let Some(m) = c.blocking_pop_message(std::time::Duration::from_millis(1000)).unwrap() {
                if m.msg_type() != MessageType::MethodCall { continue; }

                let rv: Vec<Box<dyn RefArg + 'static>> = m.iter_init().collect();
                println!("Receiving {:?}", rv);
                let rv0: &Variant<Box<dyn RefArg>> = cast(&rv[0]).unwrap();
                let rv00: &i32 = cast(&rv0.0).unwrap();
                assert_eq!(rv00, &5i32);
                assert_eq!(Some(&false), rv[2].as_any().downcast_ref::<bool>());
                assert_eq!(Some(&vi32), rv[4].as_any().downcast_ref::<Vec<i32>>());
                assert_eq!(Some(&vstr), rv[5].as_any().downcast_ref::<Vec<String>>());
                let mut diter = rv[6].as_iter().unwrap();
                {
                    let mut mmap: HashMap<bool, String> = HashMap::new();
                    while let Some(k) = diter.next() {
                        let x: String = diter.next().unwrap().as_str().unwrap().into();
                        mmap.insert(*cast::<bool>(&k.box_clone()).unwrap(), x);
                    }
                    assert_eq!(mmap[&true], "Yes");
                }
                let mut iter = rv[6].as_iter().unwrap();
                assert!(iter.next().unwrap().as_i64().is_some());
                assert!(iter.next().unwrap().as_str().is_some());
                assert!(iter.next().unwrap().as_str().is_none());
                assert!(iter.next().unwrap().as_i64().is_none());
                assert!(iter.next().is_none());
                assert!(rv[7].as_f64().unwrap() > 1.0);
                assert!(rv[7].as_f64().unwrap() < 2.0);
                break;
            }
        }
    }

    #[test]
    fn message_types() {
        let c = Channel::get_private(BusType::Session).unwrap();

        let m = Message::new_method_call(c.unique_name().unwrap(), "/hello", "com.example.hello", "Hello").unwrap();
        let m = m.append1(2000u16);
        let m = m.append1(&Array::new(&vec![129u8, 5, 254]));
        let m = m.append2(Variant(&["Hello", "world"][..]), &[32768u16, 16u16, 12u16][..]);
        let m = m.append3(-1i32, &*format!("Hello world"), -3.14f64);
        let m = m.append1((256i16, Variant(18_446_744_073_709_551_615u64)));
        let m = m.append2(Path::new("/a/valid/path").unwrap(), &Signature::new("a{sv}").unwrap());
        let mut z = HashMap::new();
        z.insert(123543u32, true);
        z.insert(0u32, false);
        let m = m.append1(Dict::new(&z));
        let sending = format!("{:?}", m.iter_init());
        println!("Sending {}", sending);
        c.send(m).unwrap();

        loop {
            if let Some(m) = c.blocking_pop_message(std::time::Duration::from_millis(1000)).unwrap() {
                if m.msg_type() != MessageType::MethodCall { continue; }
                use super::Arg;
                let receiving = format!("{:?}", m.iter_init());
                println!("Receiving {}", receiving);
                assert_eq!(sending, receiving);

                assert_eq!(2000u16, m.get1().unwrap());
                assert_eq!(m.get2(), (Some(2000u16), Some(&[129u8, 5, 254][..])));
                assert_eq!(m.read2::<u16, bool>().unwrap_err(),
                    TypeMismatchError { position: 1, found: ArgType::Array, expected: ArgType::Boolean });

                let mut g = m.iter_init();
                let e = g.read::<u32>().unwrap_err();
                assert_eq!(e.pos(), 0);
                assert_eq!(e.expected_arg_type(), ArgType::UInt32);
                assert_eq!(e.found_arg_type(), ArgType::UInt16);

                assert!(g.next() && g.next());
                let v: Variant<Iter> = g.get().unwrap();
                let mut viter = v.0;
                assert_eq!(viter.arg_type(), Array::<&str,()>::ARG_TYPE);
                let a: Array<&str, _> = viter.get().unwrap();
                assert_eq!(a.collect::<Vec<&str>>(), vec!["Hello", "world"]);

                assert!(g.next());
                assert_eq!(g.get::<u16>(), None); // It's an array, not a single u16
                assert!(g.next() && g.next() && g.next() && g.next());

                assert_eq!(g.get(), Some((256i16, Variant(18_446_744_073_709_551_615u64))));
                assert!(g.next());
                assert_eq!(g.get(), Some(Path::new("/a/valid/path").unwrap()));
                assert!(g.next());
                assert_eq!(g.get(), Some(Signature::new("a{sv}").unwrap()));
                assert!(g.next());
                let d: Dict<u32, bool, _> = g.get().unwrap();
                let z2: HashMap<_, _> = d.collect();
                assert_eq!(z, z2);
                break;
            }
        }
    }

    #[test]
    fn cast_vecs() {
        let c = Channel::get_private(BusType::Session).unwrap();

        let m = Message::new_method_call(c.unique_name().unwrap(), "/hello", "com.example.hello", "Hello").unwrap();
        macro_rules! append_array {
            ($m:expr, $t:ty) => {
                $m.append1(Variant(&Array::<&$t, _>::new(&vec![Default::default()])))
            };
        }
        let m = append_array!(m, bool);
        let m = append_array!(m, u8);
        let m = append_array!(m, u16);
        let m = append_array!(m, i16);
        let m = append_array!(m, u32);
        let m = append_array!(m, i32);
        let m = append_array!(m, f64);
        let m = append_array!(m, String);
        c.send(m).unwrap();
        loop {
            if let Some(m) = c.blocking_pop_message(std::time::Duration::from_millis(1000)).unwrap() {
                if m.msg_type() != MessageType::MethodCall {
                    continue;
                }
                let mut i = m.iter_init();
                let mut i2 = m.iter_init();

                macro_rules! check_array {
                    ($t:ty) => {
                        let array: Variant<Box<dyn RefArg>> = i.read().unwrap();
                        assert_eq!(
                            cast::<Vec<$t>>(&(array.0)),
                            Some(&vec![Default::default()]),
                            "a variant containing an array of {0} should be castable to a Vec<{0}>",
                            std::any::type_name::<$t>()
                        );
                        let refarg = i2.get_refarg().unwrap();
                        println!("refarg {:?}", refarg);
                        let cloned = refarg.box_clone();
                        println!("cloned: {:?}", cloned);
                        let st_inner = refarg.as_static_inner(0).unwrap();
                        println!("st_inner {:?}", st_inner);
                        i2.next();
                        assert_eq!(cast::<Vec<$t>>(st_inner), Some(&vec![Default::default()]));
                        let cl_inner = refarg.as_static_inner(0).unwrap();
                        assert_eq!(cast::<Vec<$t>>(cl_inner), Some(&vec![Default::default()]));
                    };
                }
                check_array!(bool);
                check_array!(u8);
                check_array!(u16);
                check_array!(i16);
                check_array!(u32);
                check_array!(i32);
                check_array!(f64);
                check_array!(String);
                break;
            }
        }
    }

    #[test]
    fn cast_dicts() {
        let c = Channel::get_private(BusType::Session).unwrap();

        let m = Message::new_method_call(
            c.unique_name().unwrap(),
            "/hello",
            "com.example.hello",
            "Hello",
        )
        .unwrap();
        macro_rules! append_dict_variant {
            ($m:expr, $k:ty, $v:ty) => {{
                let mut map: HashMap<$k, Variant<Box<dyn RefArg>>> = HashMap::new();
                map.insert(Default::default(), Variant(Box::new(<$v>::default())));
                $m.append1(Variant(&map))
            }};
        }
        let m = append_dict_variant!(m, bool, bool);
        let m = append_dict_variant!(m, u8, u8);
        let m = append_dict_variant!(m, u16, u16);
        let m = append_dict_variant!(m, i16, i16);
        let m = append_dict_variant!(m, u32, u32);
        let m = append_dict_variant!(m, i32, i32);
        let m = append_dict_variant!(m, u64, u64);
        let m = append_dict_variant!(m, i64, i64);
        let m = append_dict_variant!(m, u8, f64);
        let m = append_dict_variant!(m, String, String);
        c.send(m).unwrap();
        loop {
            if let Some(m) = c
                .blocking_pop_message(std::time::Duration::from_millis(1000))
                .unwrap()
            {
                if m.msg_type() != MessageType::MethodCall {
                    continue;
                }
                let mut i = m.iter_init();
                let mut i2 = m.iter_init();

                macro_rules! check_dict_variant {
                    ($k:ty, $v:ty) => {
                        let map: Variant<Box<dyn RefArg>> = i.read().unwrap();
                        let expected_key: $k = Default::default();
                        let expected_value: $v = Default::default();
                        let cast_map = cast::<HashMap<$k, Variant<Box<dyn RefArg>>>>(&map.0);
                        assert!(cast_map.is_some(),
                            "a variant containing a dict of {0} to Variant({1}) should be castable to a HashMap<{0}, Variant<Box<dyn RefArg>>>",
                            std::any::type_name::<$k>(),
                            std::any::type_name::<$v>()
                        );
                        let cast_map_value = cast_map.unwrap().get(&expected_key).unwrap();
                        assert_eq!(
                            cast::<$v>(&cast_map_value.0),
                            Some(&expected_value),
                            "a variant {0:?} containing a {1} should be castable to {1}",
                            cast_map_value,
                            std::any::type_name::<$v>()
                        );
                        let refarg = i2.get_refarg().unwrap();
                        println!("refarg {:?}", refarg);
                        let st_inner = refarg.as_static_inner(0).unwrap();
                        println!("st_inner {:?}", st_inner);
                        i2.next();
                        assert!(cast::<HashMap<$k, Variant<Box<dyn RefArg>>>>(st_inner).is_some());
                    };
                }
                check_dict_variant!(bool, bool);
                check_dict_variant!(u8, u8);
                check_dict_variant!(u16, u16);
                check_dict_variant!(i16, i16);
                check_dict_variant!(u32, u32);
                check_dict_variant!(i32, i32);
                check_dict_variant!(u64, u64);
                check_dict_variant!(i64, i64);
                check_dict_variant!(u8, f64);
                check_dict_variant!(String, String);
                break;
            }
        }
    }
}
