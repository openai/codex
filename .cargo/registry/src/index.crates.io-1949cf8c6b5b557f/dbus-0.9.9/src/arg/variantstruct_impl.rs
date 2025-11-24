use super::*;
use crate::Signature;
use std::any;
use std::collections::VecDeque;

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
/// A simple wrapper to specify a D-Bus variant.
///
/// See the argument guide and module level documentation for details and examples.
pub struct Variant<T>(pub T);

impl Variant<Box<dyn RefArg>> {
    /// Creates a new refarg from an Iter. Mainly for internal use.
    pub fn new_refarg<'a>(i: &mut Iter<'a>) -> Option<Self> {
        i.recurse(ArgType::Variant).and_then(|mut si| si.get_refarg()).map(Variant)
    }
}

impl<T:Default> Default for Variant<T> {
    fn default() -> Self { Variant(T::default()) }
}


impl<T> Arg for Variant<T> {
    const ARG_TYPE: ArgType = ArgType::Variant;
    fn signature() -> Signature<'static> { unsafe { Signature::from_slice_unchecked("v\0") } }
}

impl<T: Arg + Append> Append for Variant<T> {
    fn append_by_ref(&self, i: &mut IterAppend) {
        let z = &self.0;
        i.append_container(ArgType::Variant, Some(T::signature().as_cstr()), |s| z.append_by_ref(s));
    }
}

impl Append for Variant<Box<dyn RefArg>> {
    fn append_by_ref(&self, i: &mut IterAppend) {
        let z = &self.0;
        i.append_container(ArgType::Variant, Some(z.signature().as_cstr()), |s| z.append(s));
    }
}

impl<'a, T: Get<'a>> Get<'a> for Variant<T> {
    fn get(i: &mut Iter<'a>) -> Option<Variant<T>> {
        i.recurse(ArgType::Variant).and_then(|mut si| si.get().map(Variant))
    }
}

impl<'a> Get<'a> for Variant<Iter<'a>> {
    fn get(i: &mut Iter<'a>) -> Option<Variant<Iter<'a>>> {
        i.recurse(ArgType::Variant).map(Variant)
    }
}
/*
impl<'a> Get<'a> for Variant<Box<dyn RefArg>> {
    fn get(i: &mut Iter<'a>) -> Option<Variant<Box<dyn RefArg>>> {
        i.recurse(ArgType::Variant).and_then(|mut si| si.get_refarg().map(|v| Variant(v)))
    }
}
*/
impl<T: RefArg> RefArg for Variant<T> {
    fn arg_type(&self) -> ArgType { ArgType::Variant }
    fn signature(&self) -> Signature<'static> { unsafe { Signature::from_slice_unchecked("v\0") } }
    fn append(&self, i: &mut IterAppend) {
        let z = &self.0;
        i.append_container(ArgType::Variant, Some(z.signature().as_cstr()), |s| z.append(s));
    }
    #[inline]
    fn as_any(&self) -> &dyn any::Any where T: 'static { self }
    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn any::Any where T: 'static { self }
    #[inline]
    fn as_i64(&self) -> Option<i64> { self.0.as_i64() }
    #[inline]
    fn as_u64(&self) -> Option<u64> { self.0.as_u64() }
    #[inline]
    fn as_f64(&self) -> Option<f64> { self.0.as_f64() }
    #[inline]
    fn as_str(&self) -> Option<&str> { self.0.as_str() }
    #[inline]
    fn as_iter<'a>(&'a self) -> Option<Box<dyn Iterator<Item=&'a dyn RefArg> + 'a>> {
        use std::iter;
        let z: &dyn RefArg = &self.0;
        Some(Box::new(iter::once(z)))
    }
    #[inline]
    fn box_clone(&self) -> Box<dyn RefArg + 'static> { Box::new(Variant(self.0.box_clone())) }
    #[inline]
    fn as_static_inner(&self, index: usize) -> Option<&(dyn RefArg + 'static)> where Self: 'static {
        if index == 0 { Some(&self.0) } else { None }
    }
}

macro_rules! struct_impl {
    ( $($n: ident $t: ident,)+ ) => {

/// Tuples are represented as D-Bus structs.
impl<$($t: Arg),*> Arg for ($($t,)*) {
    const ARG_TYPE: ArgType = ArgType::Struct;
    fn signature() -> Signature<'static> {
        let mut s = String::from("(");
        $( s.push_str(&$t::signature()); )*
        s.push_str(")");
        Signature::from(s)
    }
}

impl<$($t: Append),*> Append for ($($t,)*) {
    fn append_by_ref(&self, i: &mut IterAppend) {
        let ( $($n,)*) = self;
        i.append_container(ArgType::Struct, None, |s| { $( $n.append_by_ref(s); )* });
    }
}

impl<'a, $($t: Get<'a>),*> Get<'a> for ($($t,)*) {
    fn get(i: &mut Iter<'a>) -> Option<Self> {
        let si = i.recurse(ArgType::Struct);
        if si.is_none() { return None; }
        let mut si = si.unwrap();
        let mut _valid_item = true;
        $(
            if !_valid_item { return None; }
            let $n: Option<$t> = si.get();
            if $n.is_none() { return None; }
            _valid_item = si.next();
        )*
        Some(($( $n.unwrap(), )* ))
    }
}

impl<$($t: RefArg),*> RefArg for ($($t,)*) {
    fn arg_type(&self) -> ArgType { ArgType::Struct }
    fn signature(&self) -> Signature<'static> {
        let &( $(ref $n,)*) = self;
        let mut s = String::from("(");
        $( s.push_str(&$n.signature()); )*
        s.push_str(")");
        Signature::from(s)
    }
    fn append(&self, i: &mut IterAppend) {
        let &( $(ref $n,)*) = self;
        i.append_container(ArgType::Struct, None, |s| { $( $n.append(s); )* });
    }
    fn as_any(&self) -> &dyn any::Any where Self: 'static { self }
    fn as_any_mut(&mut self) -> &mut dyn any::Any where Self: 'static { self }
    fn as_iter<'a>(&'a self) -> Option<Box<dyn Iterator<Item=&'a dyn RefArg> + 'a>> {
        let &( $(ref $n,)*) = self;
        let v = vec!(
        $( $n as &dyn RefArg, )*
        );
        Some(Box::new(v.into_iter()))
    }
    fn as_static_inner(&self, index: usize) -> Option<&(dyn RefArg + 'static)> where Self: 'static {
        let &( $(ref $n,)*) = self;
        let arr = [ $($n as &dyn RefArg,)*];
        arr.get(index).map(|x| *x)
    }
    #[inline]
    fn box_clone(&self) -> Box<dyn RefArg + 'static> {
        let &( $(ref $n,)*) = self;
        let mut z = VecDeque::new();
        $( z.push_back($n.box_clone()); )*
        Box::new(z)
    }
}


}} // macro_rules end

struct_impl!(a A,);
struct_impl!(a A, b B,);
struct_impl!(a A, b B, c C,);
struct_impl!(a A, b B, c C, d D,);
struct_impl!(a A, b B, c C, d D, e E,);
struct_impl!(a A, b B, c C, d D, e E, f F,);
struct_impl!(a A, b B, c C, d D, e E, f F, g G,);
struct_impl!(a A, b B, c C, d D, e E, f F, g G, h H,);
struct_impl!(a A, b B, c C, d D, e E, f F, g G, h H, i I,);
struct_impl!(a A, b B, c C, d D, e E, f F, g G, h H, i I, j J,);
struct_impl!(a A, b B, c C, d D, e E, f F, g G, h H, i I, j J, k K,);
struct_impl!(a A, b B, c C, d D, e E, f F, g G, h H, i I, j J, k K, l L,);

impl RefArg for VecDeque<Box<dyn RefArg>> {
    fn arg_type(&self) -> ArgType { ArgType::Struct }
    fn signature(&self) -> Signature<'static> {
        let mut s = String::from("(");
        for z in self {
            s.push_str(&z.signature());
        }
        s.push_str(")");
        Signature::from(s)
    }
    fn append(&self, i: &mut IterAppend) {
        i.append_container(ArgType::Struct, None, |s| {
            for z in self { z.append(s); }
        });
    }
    #[inline]
    fn as_any(&self) -> &dyn any::Any where Self: 'static { self }
    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn any::Any where Self: 'static { self }
    fn as_iter<'a>(&'a self) -> Option<Box<dyn Iterator<Item=&'a dyn RefArg> + 'a>> {
        Some(Box::new(self.iter().map(|b| &**b)))
    }
    #[inline]
    fn as_static_inner(&self, index: usize) -> Option<&(dyn RefArg + 'static)> where Self: 'static {
        self.get(index).map(|x| x as &dyn RefArg)
    }
    #[inline]
    fn box_clone(&self) -> Box<dyn RefArg + 'static> {
        let t: VecDeque<Box<dyn RefArg + 'static>> = self.iter().map(|x| x.box_clone()).collect();
        Box::new(t)
    }
}
