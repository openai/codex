use super::*;
use crate::{Message, ffi};
use crate::strings::{Signature, Path};
use std::marker::PhantomData;
use std::{ptr, mem, any, fmt};
use super::check;
use std::ffi::{CString};
use std::os::raw::{c_void, c_int};
use std::collections::{HashMap, BTreeMap};
use std::hash::{Hash, BuildHasher};
use std::borrow::Cow;

// Map DBus-Type -> Alignment. Copied from _dbus_marshal_write_fixed_multi in
// http://dbus.freedesktop.org/doc/api/html/dbus-marshal-basic_8c_source.html#l01020
// Note that Rust booleans are one byte, dbus booleans are four bytes!
const FIXED_ARRAY_ALIGNMENTS: [(ArgType, usize); 9] = [
    (ArgType::Byte, 1),
    (ArgType::Int16, 2),
    (ArgType::UInt16, 2),
    (ArgType::UInt32, 4),
    (ArgType::Int32, 4),
    (ArgType::Boolean, 4),
    (ArgType::Int64, 8),
    (ArgType::UInt64, 8),
    (ArgType::Double, 8)
];

/// Represents a D-Bus array.
impl<'a, T: Arg> Arg for &'a [T] {
    const ARG_TYPE: ArgType = ArgType::Array;
    fn signature() -> Signature<'static> { Signature::from(format!("a{}", T::signature())) }
}

fn array_append<T: Arg, F: FnMut(&T, &mut IterAppend)>(z: &[T], i: &mut IterAppend, mut f: F) {
    let zptr = z.as_ptr();
    let zlen = z.len() as i32;

    // Can we do append_fixed_array?
    let a = (T::ARG_TYPE, mem::size_of::<T>());
    let can_fixed_array = (zlen > 1) && (z.len() == zlen as usize) && FIXED_ARRAY_ALIGNMENTS.iter().any(|&v| v == a);

    i.append_container(ArgType::Array, Some(T::signature().as_cstr()), |s|
        if can_fixed_array { unsafe { check("dbus_message_iter_append_fixed_array",
            ffi::dbus_message_iter_append_fixed_array(&mut s.0, a.0 as c_int, &zptr as *const _ as *const c_void, zlen)) }}
        else { for arg in z { f(arg, s); }}
    );
}

/// Appends a D-Bus array. Note: In case you have a large array of a type that implements FixedArray,
/// using this method will be more efficient than using an Array.
impl<'a, T: Arg + Append + Clone> Append for &'a [T] {
    fn append_by_ref(&self, i: &mut IterAppend) {
        array_append(self, i, |arg, s| arg.clone().append(s));
    }
}

impl<'a, T: Arg + Append + Clone> Append for Cow<'a, [T]> {
    fn append_by_ref(&self, i: &mut IterAppend) {
        (&**self).append_by_ref(i)
    }
}

impl<'a, T: Arg + RefArg> RefArg for &'a [T] {
    fn arg_type(&self) -> ArgType { ArgType::Array }
    fn signature(&self) -> Signature<'static> { Signature::from(format!("a{}", <T as Arg>::signature())) }
    fn append(&self, i: &mut IterAppend) {
        array_append(self, i, |arg, s| RefArg::append(arg,s));
    }
    #[inline]
    fn as_any(&self) -> &dyn any::Any where Self: 'static { self }
    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn any::Any where Self: 'static { self }
    #[inline]
    fn as_static_inner(&self, index: usize) -> Option<&(dyn RefArg + 'static)> where Self: 'static {
        self.get(index).map(|x| x as &dyn RefArg)
    }

    fn box_clone(&self) -> Box<dyn RefArg + 'static> {
        T::array_clone(self).unwrap_or_else(|| {
            Box::new(InternalArray {
                inner_sig: <T as Arg>::signature(),
                data: self.iter().map(|x| x.box_clone()).collect(),
            })
        })
    }
}

impl<T: Arg + RefArg> RefArg for Vec<T> {
    fn arg_type(&self) -> ArgType { ArgType::Array }
    fn signature(&self) -> Signature<'static> { Signature::from(format!("a{}", <T as Arg>::signature())) }
    fn append(&self, i: &mut IterAppend) {
        array_append(&self, i, |arg, s| RefArg::append(arg,s));
    }
    #[inline]
    fn as_any(&self) -> &dyn any::Any where Self: 'static { self }
    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn any::Any where Self: 'static { self }
    fn as_iter<'a>(&'a self) -> Option<Box<dyn Iterator<Item=&'a dyn RefArg> + 'a>> {
        Some(Box::new(self.iter().map(|b| b as &dyn RefArg)))
    }
    #[inline]
    fn as_static_inner(&self, index: usize) -> Option<&(dyn RefArg + 'static)> where Self: 'static {
        self.get(index).map(|x| x as &dyn RefArg)
    }
    #[inline]
    fn box_clone(&self) -> Box<dyn RefArg + 'static> { (&**self).box_clone() }
}


impl<'a, T: FixedArray> Get<'a> for &'a [T] {
    fn get(i: &mut Iter<'a>) -> Option<&'a [T]> {
        debug_assert!(FIXED_ARRAY_ALIGNMENTS.iter().any(|&v| v == (T::ARG_TYPE, mem::size_of::<T>())));
        i.recurse(Self::ARG_TYPE).and_then(|mut si| unsafe {
            let etype = ffi::dbus_message_iter_get_element_type(&mut i.0);

            if etype != T::ARG_TYPE as c_int { return None };

            let mut v: *mut T = ptr::null_mut();
            let mut i = 0;
            ffi::dbus_message_iter_get_fixed_array(&mut si.0, &mut v as *mut _ as *mut c_void, &mut i);
            if v.is_null() {
                assert_eq!(i, 0);
                Some(&[][..])
            } else {
                Some(::std::slice::from_raw_parts(v, i as usize))
            }
        })
    }
}


#[derive(Copy, Clone, Debug)]
/// Append a D-Bus dict type (i e, an array of dict entries).
///
/// See the argument guide and module level documentation for details and alternatives.
pub struct Dict<'a, K: DictKey, V: Arg, I>(I, PhantomData<(&'a Message, *const K, *const V)>);

impl<'a, K: DictKey, V: Arg, I> Dict<'a, K, V, I> {
    fn entry_sig() -> String { format!("{{{}{}}}", K::signature(), V::signature()) }
}

impl<'a, K: 'a + DictKey, V: 'a + Append + Arg, I: Iterator<Item=(K, V)>> Dict<'a, K, V, I> {
    /// Creates a new Dict from an iterator.
    pub fn new<J: IntoIterator<IntoIter=I, Item=(K, V)>>(j: J) -> Dict<'a, K, V, I> { Dict(j.into_iter(), PhantomData) }
}

impl<'a, K: DictKey, V: Arg, I> Arg for Dict<'a, K, V, I> {
    const ARG_TYPE: ArgType = ArgType::Array;
    fn signature() -> Signature<'static> {
        Signature::from(format!("a{}", Self::entry_sig())) }
}

impl<'a, K: 'a + DictKey + Append, V: 'a + Append + Arg, I: Iterator<Item=(K, V)> + Clone> Append for Dict<'a, K, V, I> {
    fn append_by_ref(&self, i: &mut IterAppend) {
        let z = self.0.clone();
        i.append_container(Self::ARG_TYPE, Some(&CString::new(Self::entry_sig()).unwrap()), |s| for (k, v) in z {
            s.append_container(ArgType::DictEntry, None, |ss| {
                k.append_by_ref(ss);
                v.append_by_ref(ss);
            })
        });
    }
}


impl<'a, K: DictKey + Get<'a>, V: Arg + Get<'a>> Get<'a> for Dict<'a, K, V, Iter<'a>> {
    fn get(i: &mut Iter<'a>) -> Option<Self> {
        i.recurse(Self::ARG_TYPE).map(|si| Dict(si, PhantomData))
        // TODO: Verify full element signature?
    }
}

impl<'a, K: DictKey + Get<'a>, V: Arg + Get<'a>> Iterator for Dict<'a, K, V, Iter<'a>> {
    type Item = (K, V);
    fn next(&mut self) -> Option<(K, V)> {
        let i = self.0.recurse(ArgType::DictEntry).and_then(|mut si| {
            let k = si.get();
            if k.is_none() { return None };
            assert!(si.next());
            let v = si.get();
            if v.is_none() { return None };
            Some((k.unwrap(), v.unwrap()))
        });
        self.0.next();
        i
    }
}

impl<K: DictKey, V: Arg, S: BuildHasher> Arg for HashMap<K, V, S> {
    const ARG_TYPE: ArgType = ArgType::Array;
    fn signature() -> Signature<'static> {
        Signature::from(format!("a{{{}{}}}", K::signature(), V::signature())) }
}

impl<K: DictKey + Append + Eq + Hash, V: Arg + Append, S: BuildHasher> Append for HashMap<K, V, S> {
    fn append_by_ref(&self, i: &mut IterAppend) {
        Dict::new(self.iter()).append_by_ref(i);
    }
}

impl<'a, K: DictKey + Get<'a> + Eq + Hash, V: Arg + Get<'a>, S: BuildHasher + Default> Get<'a> for HashMap<K, V, S> {
    fn get(i: &mut Iter<'a>) -> Option<Self> {
        // TODO: Full element signature is not verified.
        Dict::get(i).map(|d| d.collect())
    }
}

impl<K: DictKey + RefArg + Eq + Hash, V: RefArg + Arg, S: BuildHasher + Send + Sync> RefArg for HashMap<K, V, S> {
    fn arg_type(&self) -> ArgType { ArgType::Array }
    fn signature(&self) -> Signature<'static> { format!("a{{{}{}}}", <K as Arg>::signature(), <V as Arg>::signature()).into() }
    fn append(&self, i: &mut IterAppend) {
        let sig = CString::new(format!("{{{}{}}}", <K as Arg>::signature(), <V as Arg>::signature())).unwrap();
        i.append_container(ArgType::Array, Some(&sig), |s| for (k, v) in self {
            s.append_container(ArgType::DictEntry, None, |ss| {
                k.append(ss);
                v.append(ss);
            })
        });
    }
    #[inline]
    fn as_any(&self) -> &dyn any::Any where Self: 'static { self }
    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn any::Any where Self: 'static { self }
    fn as_iter<'b>(&'b self) -> Option<Box<dyn Iterator<Item=&'b dyn RefArg> + 'b>> {
        Some(Box::new(self.iter().flat_map(|(k, v)| vec![k as &dyn RefArg, v as &dyn RefArg].into_iter())))
    }
    #[inline]
    fn box_clone(&self) -> Box<dyn RefArg + 'static> {
        Box::new(InternalDict {
            outer_sig: self.signature(),
            data: self.iter().map(|(k, v)| (k.box_clone(), v.box_clone())).collect(),
        })
    }
}

impl<K: DictKey, V: Arg> Arg for BTreeMap<K, V> {
    const ARG_TYPE: ArgType = ArgType::Array;
    fn signature() -> Signature<'static> {
        Signature::from(format!("a{{{}{}}}", K::signature(), V::signature())) }
}

impl<K: DictKey + Append + Eq + Ord, V: Arg + Append> Append for BTreeMap<K, V> {
    fn append_by_ref(&self, i: &mut IterAppend) {
        Dict::new(self.iter()).append_by_ref(i);
    }
}

impl<'a, K: DictKey + Get<'a> + Eq + Ord, V: Arg + Get<'a>> Get<'a> for BTreeMap<K, V> {
    fn get(i: &mut Iter<'a>) -> Option<Self> {
        // TODO: Full element signature is not verified.
        Dict::get(i).map(|d| d.collect())
    }
}

impl<K: DictKey + RefArg + Eq + Ord, V: RefArg + Arg> RefArg for BTreeMap<K, V> {
    fn arg_type(&self) -> ArgType { ArgType::Array }
    fn signature(&self) -> Signature<'static> { format!("a{{{}{}}}", <K as Arg>::signature(), <V as Arg>::signature()).into() }
    fn append(&self, i: &mut IterAppend) {
        let sig = CString::new(format!("{{{}{}}}", <K as Arg>::signature(), <V as Arg>::signature())).unwrap();
        i.append_container(ArgType::Array, Some(&sig), |s| for (k, v) in self {
            s.append_container(ArgType::DictEntry, None, |ss| {
                k.append(ss);
                v.append(ss);
            })
        });
    }
    #[inline]
    fn as_any(&self) -> &dyn any::Any where Self: 'static { self }
    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn any::Any where Self: 'static { self }
    fn as_iter<'b>(&'b self) -> Option<Box<dyn Iterator<Item=&'b dyn RefArg> + 'b>> {
        Some(Box::new(self.iter().flat_map(|(k, v)| vec![k as &dyn RefArg, v as &dyn RefArg].into_iter())))
    }
    fn box_clone(&self) -> Box<dyn RefArg + 'static> {
        Box::new(InternalDict {
            outer_sig: self.signature(),
            data: self.iter().map(|(k, v)| (k.box_clone(), v.box_clone())).collect(),
        })
    }
}


impl<T: Arg> Arg for Vec<T> {
    const ARG_TYPE: ArgType = ArgType::Array;
    fn signature() -> Signature<'static> { Signature::from(format!("a{}", T::signature())) }
}

impl<T: Arg + Append> Append for Vec<T> {
    fn append_by_ref(&self, i: &mut IterAppend) {
        Array::new(self).append_by_ref(i);
    }
}

impl<'a, T: Arg + Get<'a>> Get<'a> for Vec<T> {
    fn get(i: &mut Iter<'a>) -> Option<Self> {
        <Array<T, Iter<'a>>>::get(i).map(|a| a.collect())
    }
}


#[derive(Copy, Clone, Debug)]
/// Represents a D-Bus Array. Maximum flexibility (wraps an iterator of items to append).
///
/// See the argument guide and module level documentation for details and alternatives.
pub struct Array<'a, T, I>(I, PhantomData<(fn() -> T, &'a ())>);

impl<'a, T: 'a, I: Iterator<Item=T>> Array<'a, T, I> {
    /// Creates a new Array from an iterator.
    pub fn new<J: IntoIterator<IntoIter=I, Item=T>>(j: J) -> Array<'a, T, I> { Array(j.into_iter(), PhantomData) }
}

impl<'a, T: Arg, I> Arg for Array<'a, T, I> {
    const ARG_TYPE: ArgType = ArgType::Array;
    fn signature() -> Signature<'static> { Signature::from(format!("a{}", T::signature())) }
}

impl<'a, T: 'a + Arg + Append, I: Iterator<Item=T> + Clone> Append for Array<'a, T, I> {
    fn append_by_ref(&self, i: &mut IterAppend) {
        let z = self.0.clone();
        i.append_container(ArgType::Array, Some(T::signature().as_cstr()), |s| for arg in z { arg.append_by_ref(s) });
    }
}

impl<'a, T: Arg + Get<'a>> Get<'a> for Array<'a, T, Iter<'a>> {
    fn get(i: &mut Iter<'a>) -> Option<Array<'a, T, Iter<'a>>> {
        i.recurse(Self::ARG_TYPE).map(|si| Array(si, PhantomData))
        // TODO: Verify full element signature?
    }
}

impl<'a, T: Get<'a>> Iterator for Array<'a, T, Iter<'a>> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        let i = self.0.get();
        self.0.next();
        i
    }
}

// Due to the strong typing here; RefArg is implemented only for T's that are both Arg and RefArg.
// We need Arg for this to work for empty arrays (we can't get signature from first element if there is no elements).
// We need RefArg for box_clone.
impl<'a, T, I> RefArg for Array<'static, T, I>
where
    T: 'a + Arg + RefArg,
    I: fmt::Debug + Clone + Send + Sync + Iterator<Item=&'a T>
{
    fn arg_type(&self) -> ArgType { ArgType::Array }
    fn signature(&self) -> Signature<'static> { Signature::from(format!("a{}", <T as Arg>::signature())) }
    fn append(&self, i: &mut IterAppend) {
        let z = self.0.clone();
        i.append_container(ArgType::Array, Some(<T as Arg>::signature().as_cstr()), |s|
            for arg in z { RefArg::append(arg, s); }
        );
    }
    #[inline]
    fn as_any(&self) -> &dyn any::Any where Self: 'static { self }
    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn any::Any where Self: 'static { self }

    fn box_clone(&self) -> Box<dyn RefArg + 'static> {
        Box::new(InternalArray {
            inner_sig: <T as Arg>::signature(),
            data: self.0.clone().map(|x| x.box_clone()).collect(),
        })
    }
}

fn get_fixed_array_refarg<T: FixedArray + Clone + RefArg>(i: &mut Iter) -> Box<dyn RefArg> {
    let s = <&[T]>::get(i).unwrap();
    Box::new(s.to_vec())
}

fn get_var_array_refarg<'a, T: 'static + RefArg + Arg, F: FnMut(&mut Iter<'a>) -> Option<T>>
    (i: &mut Iter<'a>, mut f: F) -> Box<dyn RefArg> {
    let mut v: Vec<T> = vec!(); // dbus_message_iter_get_element_count might be O(n), better not use it
    let mut si = i.recurse(ArgType::Array).unwrap();
    while let Some(q) = f(&mut si) { v.push(q); si.next(); }
    Box::new(v)
}

fn get_dict_refarg<'a, K, V, KF, VF>(i: &mut Iter<'a>, mut kf: KF, mut vf: VF) -> Box<dyn RefArg>
where
    K: DictKey + 'static + RefArg + Clone + Eq + Hash,
    V: RefArg + Arg + 'static,
    KF: FnMut(&mut Iter<'a>) -> Option<K>,
    VF: FnMut(&mut Iter<'a>) -> Option<V>,
{
    let mut data: HashMap<K, V> = HashMap::new();
    let mut si = i.recurse(ArgType::Array).unwrap();
    while let Some(mut d) = si.recurse(ArgType::DictEntry) {
        let k = kf(&mut d).unwrap();
        d.next();
        let v = vf(&mut d).unwrap();
        data.insert(k, v);
        si.next();
    }
    Box::new(data)
}

#[derive(Debug)]
struct InternalDict<K> {
   data: Vec<(K, Box<dyn RefArg>)>,
   outer_sig: Signature<'static>,
}

fn get_internal_dict_refarg<'a, K, F: FnMut(&mut Iter<'a>) -> Option<K>>(
    i: &mut Iter<'a>,
    mut f: F,
) -> Box<dyn RefArg>
where
    K: DictKey + 'static + RefArg + Clone,
{
    let mut data = vec![];
    let outer_sig = i.signature();
    let mut si = i.recurse(ArgType::Array).unwrap();
    while let Some(mut d) = si.recurse(ArgType::DictEntry) {
        let k = f(&mut d).unwrap();
        d.next();
        data.push((k, d.get_refarg().unwrap()));
        si.next();
    }
    Box::new(InternalDict { data, outer_sig })
}

// This only happens from box_clone
impl RefArg for InternalDict<Box<dyn RefArg>> {
    fn arg_type(&self) -> ArgType { ArgType::Array }
    fn signature(&self) -> Signature<'static> { self.outer_sig.clone() }
    fn append(&self, i: &mut IterAppend) {
        let inner_sig = &self.outer_sig.as_cstr().to_bytes_with_nul()[1..];
        let inner_sig = CStr::from_bytes_with_nul(inner_sig).unwrap();
        i.append_container(ArgType::Array, Some(inner_sig), |s| for (k, v) in &self.data {
            s.append_container(ArgType::DictEntry, None, |ss| {
                k.append(ss);
                v.append(ss);
            })
        });
    }
    #[inline]
    fn as_any(&self) -> &dyn any::Any where Self: 'static { self }
    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn any::Any where Self: 'static { self }
    fn as_iter<'b>(&'b self) -> Option<Box<dyn Iterator<Item=&'b dyn RefArg> + 'b>> {
        Some(Box::new(self.data.iter().flat_map(|(k, v)| vec![k as &dyn RefArg, v as &dyn RefArg].into_iter())))
    }
    fn as_static_inner(&self, index: usize) -> Option<&(dyn RefArg + 'static)> where Self: 'static {
        let (k, v) = self.data.get(index / 2)?;
        Some(if index & 1 != 0 { v } else { k })
    }
    #[inline]
    fn box_clone(&self) -> Box<dyn RefArg + 'static> {
        Box::new(InternalDict {
            data: self.data.iter().map(|(k, v)| (k.box_clone(), v.box_clone())).collect(),
            outer_sig: self.outer_sig.clone(),
        })
    }
}


impl<K: DictKey + RefArg + Clone + 'static> RefArg for InternalDict<K> {
    fn arg_type(&self) -> ArgType { ArgType::Array }
    fn signature(&self) -> Signature<'static> { self.outer_sig.clone() }
    fn append(&self, i: &mut IterAppend) {
        let inner_sig = &self.outer_sig.as_cstr().to_bytes_with_nul()[1..];
        let inner_sig = CStr::from_bytes_with_nul(inner_sig).unwrap();
        i.append_container(ArgType::Array, Some(inner_sig), |s| for (k, v) in &self.data {
            s.append_container(ArgType::DictEntry, None, |ss| {
                k.append(ss);
                v.append(ss);
            })
        });
    }
    #[inline]
    fn as_any(&self) -> &dyn any::Any where Self: 'static { self }
    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn any::Any where Self: 'static { self }
    fn as_iter<'b>(&'b self) -> Option<Box<dyn Iterator<Item=&'b dyn RefArg> + 'b>> {
        Some(Box::new(self.data.iter().flat_map(|(k, v)| vec![k as &dyn RefArg, v as &dyn RefArg].into_iter())))
    }
    fn as_static_inner(&self, index: usize) -> Option<&(dyn RefArg + 'static)> where Self: 'static {
        let (k, v) = self.data.get(index / 2)?;
        Some(if index & 1 != 0 { v } else { k })
    }
    #[inline]
    fn box_clone(&self) -> Box<dyn RefArg + 'static> {
        Box::new(InternalDict {
            data: self.data.iter().map(|(k, v)| (k.clone(), v.box_clone())).collect(),
            outer_sig: self.outer_sig.clone(),
        })
    }
}


// Fallback for Arrays of Arrays and Arrays of Structs.
// We store the signature manually here and promise that it is correct for all elements
// has that signature.
#[derive(Debug)]
struct InternalArray {
   data: Vec<Box<dyn RefArg>>,
   inner_sig: Signature<'static>,
}

fn get_internal_array(i: &mut Iter) -> Box<dyn RefArg> {
    let mut si = i.recurse(ArgType::Array).unwrap();
    let inner_sig = si.signature();
    let data = si.collect::<Vec<_>>();
    Box::new(InternalArray { data, inner_sig })
}

impl RefArg for InternalArray {
    fn arg_type(&self) -> ArgType { ArgType::Array }
    fn signature(&self) -> Signature<'static> { Signature::from(format!("a{}", self.inner_sig)) }
    fn append(&self, i: &mut IterAppend) {
        i.append_container(ArgType::Array, Some(self.inner_sig.as_cstr()), |s|
            for arg in &self.data { RefArg::append(arg,s) }
        );
    }
    #[inline]
    fn as_any(&self) -> &dyn any::Any where Self: 'static { self }
    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn any::Any where Self: 'static { self }
    fn as_iter<'a>(&'a self) -> Option<Box<dyn Iterator<Item=&'a dyn RefArg> + 'a>> {
        Some(Box::new(self.data.iter().map(|b| b as &dyn RefArg)))
    }
    fn as_static_inner(&self, index: usize) -> Option<&(dyn RefArg + 'static)> where Self: 'static {
        self.data.get(index).map(|x| x as &dyn RefArg)
    }
    #[inline]
    fn box_clone(&self) -> Box<dyn RefArg + 'static> {
        Box::new(InternalArray {
            data: self.data.iter().map(|x| x.box_clone()).collect(),
            inner_sig: self.inner_sig.clone(),
        })
    }
}

fn get_dict_refarg_for_value_type<'a, K, KF>(
    value_type: ArgType,
    i: &mut Iter<'a>,
    kf: KF,
) -> Box<dyn RefArg>
where
    K: DictKey + 'static + RefArg + Clone + Eq + Hash,
    KF: FnMut(&mut Iter<'a>) -> Option<K>,
{
    match value_type {
        ArgType::Variant => {
            get_dict_refarg::<K, Variant<Box<dyn RefArg>>, KF, _>(i, kf, Variant::new_refarg)
        }
        // Most of the following could also use get_dict_refarg to convert to a typed HashMap, but
        // doing so results in a large binary size increase due to all the generic instances being
        // instantiated.
        ArgType::Byte
        | ArgType::Int16
        | ArgType::UInt16
        | ArgType::Int32
        | ArgType::UInt32
        | ArgType::Int64
        | ArgType::UInt64
        | ArgType::Double
        | ArgType::String
        | ArgType::ObjectPath
        | ArgType::Signature
        | ArgType::Boolean
        | ArgType::UnixFd
        | ArgType::Array
        | ArgType::Struct => get_internal_dict_refarg::<K, KF>(i, kf),
        ArgType::DictEntry => panic!("Can't have DictEntry as value for dictionary"),
        ArgType::Invalid => panic!("Array with invalid dictvalue"),
    }
}

pub fn get_array_refarg(i: &mut Iter) -> Box<dyn RefArg> {
    debug_assert!(i.arg_type() == ArgType::Array);
    let etype = ArgType::from_i32(unsafe { ffi::dbus_message_iter_get_element_type(&mut i.0) } as i32).unwrap();

    let x = match etype {
        ArgType::Byte => get_fixed_array_refarg::<u8>(i),
        ArgType::Int16 => get_fixed_array_refarg::<i16>(i),
        ArgType::UInt16 => get_fixed_array_refarg::<u16>(i),
        ArgType::Int32 => get_fixed_array_refarg::<i32>(i),
        ArgType::UInt32 => get_fixed_array_refarg::<u32>(i),
        ArgType::Int64 => get_fixed_array_refarg::<i64>(i),
        ArgType::UInt64 => get_fixed_array_refarg::<u64>(i),
        ArgType::Double => get_fixed_array_refarg::<f64>(i),
        ArgType::String => get_var_array_refarg::<String, _>(i, |si| si.get()),
        ArgType::ObjectPath => get_var_array_refarg::<Path<'static>, _>(i, |si| si.get::<Path>().map(|s| s.into_static())),
        ArgType::Signature => get_var_array_refarg::<Signature<'static>, _>(i, |si| si.get::<Signature>().map(|s| s.into_static())),
        ArgType::Variant => get_var_array_refarg::<Variant<Box<dyn RefArg>>, _>(i, |si| Variant::new_refarg(si)),
        ArgType::Boolean => get_var_array_refarg::<bool, _>(i, |si| si.get()),
        ArgType::Invalid => panic!("Array with Invalid ArgType"),
        ArgType::Array => get_internal_array(i),
        ArgType::DictEntry => {
            let key = ArgType::from_i32(i.signature().as_bytes()[2] as i32).unwrap(); // The third character, after "a{", is our key.
            let value = ArgType::from_i32(i.signature().as_bytes()[3] as i32).unwrap(); // The fourth character, after "a{", is our value.
            match key {
                ArgType::Byte => get_dict_refarg_for_value_type::<u8, _>(value, i, Iter::get),
                ArgType::Int16 => get_dict_refarg_for_value_type::<i16, _>(value, i, Iter::get),
                ArgType::UInt16 => get_dict_refarg_for_value_type::<u16, _>(value, i, Iter::get),
                ArgType::Int32 => get_dict_refarg_for_value_type::<i32, _>(value, i, Iter::get),
                ArgType::UInt32 => get_dict_refarg_for_value_type::<u32, _>(value, i, Iter::get),
                ArgType::Int64 => get_dict_refarg_for_value_type::<i64, _>(value, i, Iter::get),
                ArgType::UInt64 => get_dict_refarg_for_value_type::<u64, _>(value, i, Iter::get),
                ArgType::Double => get_internal_dict_refarg::<f64, _>(i, Iter::get),
                ArgType::Boolean => get_dict_refarg_for_value_type::<bool, _>(value, i, Iter::get),
                // ArgType::UnixFd => get_dict_refarg::<OwnedFd, _>(i),
                ArgType::String => get_dict_refarg_for_value_type::<String, _>(value, i, Iter::get),
                ArgType::ObjectPath => {
                    get_dict_refarg_for_value_type::<Path<'static>, _>(value, i, |si| {
                        si.get::<Path>().map(|s| s.into_static())
                    })
                }
                ArgType::Signature => {
                    get_dict_refarg_for_value_type::<Signature<'static>, _>(value, i, |si| {
                        si.get::<Signature>().map(|s| s.into_static())
                    })
                }
                _ => panic!("Array with invalid dictkey ({:?})", key),
            }
        }
        ArgType::UnixFd => get_var_array_refarg::<std::fs::File, _>(i, |si| si.get()),
        ArgType::Struct => get_internal_array(i),
    };

    debug_assert_eq!(i.signature(), x.signature());
    x
}
