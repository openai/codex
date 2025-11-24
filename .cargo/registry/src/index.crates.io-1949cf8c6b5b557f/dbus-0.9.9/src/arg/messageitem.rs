//! MessageItem - old, enum design that is used as parameters and return values from
//! method calls, or as data added to a signal.
//!
//! Note that the newer generic design (see `arg` module) is, in general, both faster
//! and smaller than MessageItem, and should be your first hand choice
//! whenever applicable. There is also a trait object design called `RefArg` in
//! case the generic design is too inflexible.

use crate::strings::{Signature, Path, Interface, BusName};

use crate::arg;
use crate::arg::{Iter, IterAppend, Arg, ArgType};
use crate::arg::OwnedFd;
use std::ffi::CStr;
use std::{ops, any};

use crate::{ffidisp::Connection, Message, Error};
use std::collections::BTreeMap;
use std::convert::TryFrom;

#[derive(Debug,Copy,Clone)]
/// Errors that can happen when creating a MessageItem::Array.
pub enum ArrayError {
    /// The array is empty.
    EmptyArray,
    /// The array is composed of different element types.
    DifferentElementTypes,
    /// The supplied signature is not a valid array signature
    InvalidSignature,
}


/// OwnedFd wrapper for MessageItem
#[cfg(feature = "stdfd")]
#[derive(Debug)]
pub struct MessageItemFd(pub OwnedFd);

#[cfg(feature = "stdfd")]
mod messageitem_fd_impl {
    use super::*;
    impl Clone for MessageItemFd {
        fn clone(&self) -> Self { MessageItemFd(self.0.try_clone().unwrap()) }
    }

    impl PartialEq for MessageItemFd {
        fn eq(&self, _rhs: &Self) -> bool { false }
    }

    impl PartialOrd for MessageItemFd {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            use std::os::unix::io::AsRawFd;
            let a = self.0.as_raw_fd();
            let b = other.0.as_raw_fd();
            a.partial_cmp(&b)
        }
    }

    impl From<OwnedFd> for MessageItem { fn from(i: OwnedFd) -> MessageItem { MessageItem::UnixFd(MessageItemFd(i)) } }

    impl<'a> TryFrom<&'a MessageItem> for &'a OwnedFd {
        type Error = ();
        fn try_from(i: &'a MessageItem) -> Result<&'a OwnedFd,()> { if let MessageItem::UnixFd(ref b) = i { Ok(&b.0) } else { Err(()) } }
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
/// An array of MessageItem where every MessageItem is of the same type.
pub struct MessageItemArray {
    v: Vec<MessageItem>,
    // signature includes the "a"!
    sig: Signature<'static>,
}

impl MessageItemArray {
    /// Creates a new array where every element has the supplied signature.
    ///
    /// Signature is the full array signature, not the signature of the element.
    pub fn new(v: Vec<MessageItem>, sig: Signature<'static>) -> Result<MessageItemArray, ArrayError> {
        let a = MessageItemArray {v: v, sig: sig };
        if a.sig.as_bytes()[0] != ffi::DBUS_TYPE_ARRAY as u8 { return Err(ArrayError::InvalidSignature) }
        {
            let esig = a.element_signature();
            for i in &a.v {
                if i.signature().as_cstr() != esig { return Err(ArrayError::DifferentElementTypes) }
            }
        }
        Ok(a)
    }

    fn element_signature(&self) -> &CStr {
        let z = &self.sig.as_cstr().to_bytes_with_nul()[1..];
        unsafe { CStr::from_bytes_with_nul_unchecked(z) }
    }

    fn make_sig(m: &MessageItem) -> Signature<'static> {
        Signature::new(format!("a{}", m.signature())).unwrap()
    }

    /// Signature of array (full array signature)
    pub fn signature(&self) -> &Signature<'static> { &self.sig }

    /// Consumes the MessageItemArray in order to allow you to modify the individual items of the array.
    pub fn into_vec(self) -> Vec<MessageItem> { self.v }
}

impl ops::Deref for MessageItemArray {
    type Target = [MessageItem];
    fn deref(&self) -> &Self::Target { &self.v }
}

impl arg::Append for MessageItemArray {
    fn append_by_ref(&self, i: &mut IterAppend) {
        i.append_container(ArgType::Array, Some(self.element_signature()), |s| {
            for a in &self.v { a.append_by_ref(s) }
        });
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
/// An array of MessageItem where every MessageItem is of the same type.
pub struct MessageItemDict {
    v: Vec<(MessageItem, MessageItem)>,
    // signature includes the "a"!
    sig: Signature<'static>,
}

impl MessageItemDict {
    /// Creates a new dict where every key and value elements have the supplied signature.
    pub fn new(v: Vec<(MessageItem, MessageItem)>, keysig: Signature<'static>, valuesig: Signature<'static>) -> Result<MessageItemDict, ArrayError> {
        let sig = Signature::from(format!("a{{{}{}}}", keysig, valuesig));
        let a = MessageItemDict {v: v, sig: sig };
        for (k, v) in &a.v {
            if keysig != k.signature() || valuesig != v.signature() {
                return Err(ArrayError::DifferentElementTypes);
            }
        }
        Ok(a)
    }

    fn element_signature(&self) -> &CStr {
        let z = &self.sig.as_cstr().to_bytes_with_nul()[1..];
        unsafe { CStr::from_bytes_with_nul_unchecked(z) }
    }

    /// Signature of array (full array signature)
    pub fn signature(&self) -> &Signature<'static> { &self.sig }

    /// Consumes the MessageItemDict in order to allow you to modify the individual items of the dict.
    pub fn into_vec(self) -> Vec<(MessageItem, MessageItem)> { self.v }
}

impl ops::Deref for MessageItemDict {
    type Target = [(MessageItem, MessageItem)];
    fn deref(&self) -> &Self::Target { &self.v }
}

impl arg::Append for MessageItemDict {
    fn append_by_ref(&self, i: &mut IterAppend) {
        i.append_container(ArgType::Array, Some(self.element_signature()), |s| {
            for (k, v) in &self.v {
                s.append_container(ArgType::DictEntry, None, |ss| {
                    k.append_by_ref(ss);
                    v.append_by_ref(ss);
                });
            }
        });
    }
}

/// MessageItem - used as parameters and return values from
/// method calls, or as data added to a signal (old, enum version).
///
/// Note that the newer generic design (see `arg` module) is both faster
/// and less error prone than MessageItem, and should be your first hand choice
/// whenever applicable.
#[derive(Debug, PartialEq, PartialOrd, Clone)]
pub enum MessageItem {
    /// A D-Bus array requires all elements to be of the same type.
    /// All elements must match the Signature.
    Array(MessageItemArray),
    /// A D-Bus struct allows for values of different types.
    Struct(Vec<MessageItem>),
    /// A D-Bus variant is a wrapper around another `MessageItem`, which
    /// can be of any type.
    Variant(Box<MessageItem>),
    /// A D-Bus dictionary. All keys and values are required to be of the same type.
    /// Not all types can be dictionary keys, but all can be dictionary values.
    Dict(MessageItemDict),
    /// A D-Bus objectpath requires its content to be a valid objectpath,
    /// so this cannot be any string.
    ObjectPath(Path<'static>),
    /// A D-Bus signature requires its content to be a valid type signature,
    /// so this cannot be any string.
    Signature(Signature<'static>),
    /// A D-Bus String is zero terminated, so no \0 s in the String, please.
    /// (D-Bus strings are also - like Rust strings - required to be valid UTF-8.)
    Str(String),
    /// A D-Bus boolean type.
    Bool(bool),
    /// A D-Bus unsigned 8 bit type.
    Byte(u8),
    /// A D-Bus signed 16 bit type.
    Int16(i16),
    /// A D-Bus signed 32 bit type.
    Int32(i32),
    /// A D-Bus signed 64 bit type.
    Int64(i64),
    /// A D-Bus unsigned 16 bit type.
    UInt16(u16),
    /// A D-Bus unsigned 32 bit type.
    UInt32(u32),
    /// A D-Bus unsigned 64 bit type.
    UInt64(u64),
    /// A D-Bus IEEE-754 double-precision floating point type.
    Double(f64),
    /// D-Bus allows for sending file descriptors, which can be used to
    /// set up SHM, unix pipes, or other communication channels.
    #[cfg(not(feature = "stdfd"))]
    UnixFd(OwnedFd),
    /// D-Bus allows for sending file descriptors, which can be used to
    /// set up SHM, unix pipes, or other communication channels.
    #[cfg(feature = "stdfd")]
    UnixFd(MessageItemFd),
}

impl MessageItem {
    /// Get the D-Bus Signature for this MessageItem.
    pub fn signature(&self) -> Signature<'static> {
        use crate::arg::Variant;
        match self {
            MessageItem::Str(_) => <String as Arg>::signature(),
            MessageItem::Bool(_) => <bool as Arg>::signature(),
            MessageItem::Byte(_) => <u8 as Arg>::signature(),
            MessageItem::Int16(_) => <i16 as Arg>::signature(),
            MessageItem::Int32(_) => <i32 as Arg>::signature(),
            MessageItem::Int64(_) => <i64 as Arg>::signature(),
            MessageItem::UInt16(_) => <u16 as Arg>::signature(),
            MessageItem::UInt32(_) => <u32 as Arg>::signature(),
            MessageItem::UInt64(_) => <u64 as Arg>::signature(),
            MessageItem::Double(_) => <f64 as Arg>::signature(),
            MessageItem::Array(ref a) => a.sig.clone(),
            MessageItem::Struct(ref s) => Signature::new(format!("({})", s.iter().fold(String::new(), |s, i| s + &*i.signature()))).unwrap(),
            MessageItem::Variant(_) => <Variant<u8> as Arg>::signature(),
            MessageItem::Dict(ref a) => a.sig.clone(),
            MessageItem::ObjectPath(_) => <Path as Arg>::signature(),
            MessageItem::Signature(_) => <Signature as Arg>::signature(),
            MessageItem::UnixFd(_) => <std::fs::File as Arg>::signature(),
        }
    }

    /// Get the arg type of this MessageItem.
    pub fn arg_type(&self) -> arg::ArgType {
        match self {
            MessageItem::Str(_) => ArgType::String,
            MessageItem::Bool(_) => ArgType::Boolean,
            MessageItem::Byte(_) => ArgType::Byte,
            MessageItem::Int16(_) => ArgType::Int16,
            MessageItem::Int32(_) => ArgType::Int32,
            MessageItem::Int64(_) => ArgType::Int64,
            MessageItem::UInt16(_) => ArgType::UInt16,
            MessageItem::UInt32(_) => ArgType::UInt32,
            MessageItem::UInt64(_) => ArgType::UInt64,
            MessageItem::Double(_) => ArgType::Double,
            MessageItem::Array(_) => ArgType::Array,
            MessageItem::Struct(_) => ArgType::Struct,
            MessageItem::Variant(_) => ArgType::Variant,
            MessageItem::Dict(_) => ArgType::Array,
            MessageItem::ObjectPath(_) => ArgType::ObjectPath,
            MessageItem::Signature(_) => ArgType::Signature,
            MessageItem::UnixFd(_) => ArgType::UnixFd,
        }
    }

    /// Creates a (String, Variant) dictionary from an iterator with Result passthrough (an Err will abort and return that Err)
    pub fn from_dict<E, I: Iterator<Item=Result<(String, MessageItem),E>>>(i: I) -> Result<MessageItem, E> {
        let mut v = Vec::new();
        for r in i {
            let (s, vv) = r?;
            v.push((s.into(), Box::new(vv).into()));
        }
        Ok(MessageItem::Dict(MessageItemDict::new(v, Signature::new("s").unwrap(), Signature::new("v").unwrap()).unwrap()))
    }

    /// Creates an MessageItem::Array from a list of MessageItems.
    ///
    /// Note: This requires `v` to be non-empty. See also
    /// `MessageItem::from(&[T])`, which can handle empty arrays as well.
    pub fn new_array(v: Vec<MessageItem>) -> Result<MessageItem, ArrayError> {
        if v.is_empty() {
            return Err(ArrayError::EmptyArray);
        }
        let s = MessageItemArray::make_sig(&v[0]);
        Ok(MessageItem::Array(MessageItemArray::new(v, s)?))
    }

    /// Creates an MessageItem::Dict from a list of MessageItem pairs.
    ///
    /// Note: This requires `v` to be non-empty. See also
    /// `MessageItem::from(&[(T1, T2)])`, which can handle empty arrays as well.
    pub fn new_dict(v: Vec<(MessageItem, MessageItem)>) -> Result<MessageItem, ArrayError> {
        if v.is_empty() {
            return Err(ArrayError::EmptyArray);
        }
        let (s1, s2) = (v[0].0.signature(), v[0].1.signature());
        Ok(MessageItem::Dict(MessageItemDict::new(v, s1, s2)?))
    }

    /// Get the inner value of a `MessageItem`
    ///
    /// # Example
    /// ```
    /// use dbus::arg::messageitem::MessageItem;
    /// let m: MessageItem = 5i64.into();
    /// let s: i64 = m.inner().unwrap();
    /// assert_eq!(s, 5i64);
    /// ```
    pub fn inner<'a, T: TryFrom<&'a MessageItem>>(&'a self) -> Result<T, T::Error> {
        T::try_from(self)
    }

    /// Get the underlying `MessageItem` of a `MessageItem::Variant`
    ///
    /// Nested `MessageItem::Variant`s are unwrapped recursively until a
    /// non-`Variant` is found.
    ///
    /// # Example
    /// ```
    /// use dbus::arg::messageitem::MessageItem;
    /// let nested = MessageItem::Variant(Box::new(6i64.into()));
    /// let flat: MessageItem = 6i64.into();
    /// assert_ne!(&nested, &flat);
    /// assert_eq!(nested.peel(), &flat);
    /// ```
    pub fn peel(&self) -> &Self {
        let mut current = self;

        while let MessageItem::Variant(b) = current {
            current = &*b;
        }

        current
    }

    fn new_array2<D, I>(i: I) -> MessageItem
    where D: Into<MessageItem>, D: Default, I: Iterator<Item=D> {
        let v: Vec<MessageItem> = i.map(|ii| ii.into()).collect();
        let s = {
            let d;
            let t = if v.is_empty() { d = D::default().into(); &d } else { &v[0] };
            MessageItemArray::make_sig(t)
        };
        MessageItem::Array(MessageItemArray::new(v, s).unwrap())
    }

    fn new_dict2<K, V, I>(i: I) -> MessageItem
    where K: Into<MessageItem> + Default, V: Into<MessageItem> + Default, I: Iterator<Item=(K, V)> {
        let v: Vec<(MessageItem, MessageItem)> = i.map(|(k, v)| (k.into(), v.into())).collect();
        let (kt, vt) = if v.is_empty() {
            let kd = K::default().into();
            let vd = V::default().into();
            (kd.signature(), vd.signature())
        } else { (v[0].0.signature(), v[0].1.signature()) };
        MessageItem::Dict(MessageItemDict::new(v, kt, vt).unwrap())
    }
}

macro_rules! msgitem_convert {
    ($t: ty, $s: ident) => {
        impl From<$t> for MessageItem { fn from(i: $t) -> MessageItem { MessageItem::$s(i) } }

        impl<'a> TryFrom<&'a MessageItem> for $t {
            type Error = ();
            fn try_from(i: &'a MessageItem) -> Result<$t,()> {
                if let MessageItem::$s(b) = i.peel() { Ok(*b) } else { Err(()) }
            }
        }
    }
}

msgitem_convert!(u8, Byte);
msgitem_convert!(u64, UInt64);
msgitem_convert!(u32, UInt32);
msgitem_convert!(u16, UInt16);
msgitem_convert!(i16, Int16);
msgitem_convert!(i32, Int32);
msgitem_convert!(i64, Int64);
msgitem_convert!(f64, Double);
msgitem_convert!(bool, Bool);



/// Create a `MessageItem::Array`.
impl<'a, T> From<&'a [T]> for MessageItem
where T: Into<MessageItem> + Clone + Default {
    fn from(i: &'a [T]) -> MessageItem {
        MessageItem::new_array2(i.iter().cloned())
    }
}

/// Create a `MessageItem::Dict`.
impl<'a, T1, T2> From<&'a [(T1, T2)]> for MessageItem
where T1: Into<MessageItem> + Clone + Default, T2: Into<MessageItem> + Clone + Default {
    fn from(i: &'a [(T1, T2)]) -> MessageItem {
        MessageItem::new_dict2(i.iter().cloned())
    }
}

impl<'a> From<&'a str> for MessageItem { fn from(i: &str) -> MessageItem { MessageItem::Str(i.to_string()) } }

impl From<String> for MessageItem { fn from(i: String) -> MessageItem { MessageItem::Str(i) } }

impl From<Path<'static>> for MessageItem { fn from(i: Path<'static>) -> MessageItem { MessageItem::ObjectPath(i) } }

impl From<Signature<'static>> for MessageItem { fn from(i: Signature<'static>) -> MessageItem { MessageItem::Signature(i) } }

#[cfg(not(feature = "stdfd"))]
impl From<OwnedFd> for MessageItem { fn from(i: OwnedFd) -> MessageItem { MessageItem::UnixFd(i) } }

#[cfg(unix)]
impl From<std::fs::File> for MessageItem {
    fn from(i: std::fs::File) -> MessageItem {
        use std::os::unix::io::{FromRawFd, IntoRawFd};
        let fd = unsafe { OwnedFd::from_raw_fd(i.into_raw_fd()) };
        fd.into()
    }
}

/// Create a `MessageItem::Variant`
impl From<Box<MessageItem>> for MessageItem {
    fn from(i: Box<MessageItem>) -> MessageItem { MessageItem::Variant(i) }
}

impl<'a> TryFrom<&'a MessageItem> for &'a str {
    type Error = ();
    fn try_from(i: &'a MessageItem) -> Result<&'a str, Self::Error> {
        match i.peel() {
            MessageItem::Str(ref b) => Ok(b),
            MessageItem::ObjectPath(ref b) => Ok(b),
            MessageItem::Signature(ref b) => Ok(b),
            _ => Err(()),
        }
    }
}

impl<'a> TryFrom<&'a MessageItem> for &'a String {
    type Error = ();
    fn try_from(i: &'a MessageItem) -> Result<&'a String,()> { if let MessageItem::Str(b) = i.peel() { Ok(b) } else { Err(()) } }
}

impl<'a> TryFrom<&'a MessageItem> for &'a Path<'static> {
    type Error = ();
    fn try_from(i: &'a MessageItem) -> Result<&'a Path<'static>,()> { if let MessageItem::ObjectPath(b) = i.peel() { Ok(b) } else { Err(()) } }
}

impl<'a> TryFrom<&'a MessageItem> for &'a Signature<'static> {
    type Error = ();
    fn try_from(i: &'a MessageItem) -> Result<&'a Signature<'static>,()> { if let MessageItem::Signature(b) = i.peel() { Ok(b) } else { Err(()) } }
}

impl<'a> TryFrom<&'a MessageItem> for &'a Box<MessageItem> {
    type Error = ();
    fn try_from(i: &'a MessageItem) -> Result<&'a Box<MessageItem>,()> { if let MessageItem::Variant(b) = i { Ok(b) } else { Err(()) } }
}

impl<'a> TryFrom<&'a MessageItem> for &'a Vec<MessageItem> {
    type Error = ();
    fn try_from(i: &'a MessageItem) -> Result<&'a Vec<MessageItem>,()> {
        match i.peel() {
            MessageItem::Array(b) => Ok(&b.v),
            MessageItem::Struct(b) => Ok(b),
            _ => Err(()),
        }
    }
}

impl<'a> TryFrom<&'a MessageItem> for &'a [MessageItem] {
    type Error = ();
    fn try_from(i: &'a MessageItem) -> Result<&'a [MessageItem],()> { i.inner::<&Vec<MessageItem>>().map(|s| &**s) }
}

#[cfg(not(feature = "stdfd"))]
impl<'a> TryFrom<&'a MessageItem> for &'a OwnedFd {
    type Error = ();
    fn try_from(i: &'a MessageItem) -> Result<&'a OwnedFd,()> { if let MessageItem::UnixFd(ref b) = i { Ok(b) } else { Err(()) } }
}

impl<'a> TryFrom<&'a MessageItem> for &'a [(MessageItem, MessageItem)] {
    type Error = ();
    fn try_from(i: &'a MessageItem) -> Result<&'a [(MessageItem, MessageItem)],()> {
        if let MessageItem::Dict(ref d) = i { Ok(&*d.v) } else { Err(()) }
    }
}


impl arg::Append for MessageItem {
    fn append_by_ref(&self, i: &mut IterAppend) {
        match self {
            MessageItem::Str(a) => a.append_by_ref(i),
            MessageItem::Bool(a) => a.append_by_ref(i),
            MessageItem::Byte(a) => a.append_by_ref(i),
            MessageItem::Int16(a) => a.append_by_ref(i),
            MessageItem::Int32(a) => a.append_by_ref(i),
            MessageItem::Int64(a) => a.append_by_ref(i),
            MessageItem::UInt16(a) => a.append_by_ref(i),
            MessageItem::UInt32(a) => a.append_by_ref(i),
            MessageItem::UInt64(a) => a.append_by_ref(i),
            MessageItem::Double(a) => a.append_by_ref(i),
            MessageItem::Array(a) => a.append_by_ref(i),
            MessageItem::Struct(a) => i.append_container(ArgType::Struct, None, |s| {
                for v in a { v.append_by_ref(s); }
            }),
            MessageItem::Variant(a) => {
                i.append_container(ArgType::Variant, Some(a.signature().as_cstr()), |s| a.append_by_ref(s))
            },
            MessageItem::Dict(a) => a.append_by_ref(i),
            MessageItem::ObjectPath(a) => a.append_by_ref(i),
            MessageItem::Signature(a) => a.append_by_ref(i),
            #[cfg(not(feature = "stdfd"))]
            MessageItem::UnixFd(a) => a.append_by_ref(i),
            #[cfg(feature = "stdfd")]
            MessageItem::UnixFd(a) => a.0.append_by_ref(i),
        }
    }
}

impl<'a> arg::Get<'a> for MessageItem {
    fn get(i: &mut Iter<'a>) -> Option<Self> {
        Some(match i.arg_type() {
            ArgType::Array => {
                let mut s = i.recurse(ArgType::Array).unwrap();
                if i.signature().as_bytes()[1] == b'{' { // Dict
                    let mut v = vec!();
                    while s.arg_type() == ArgType::DictEntry {
                        let mut ss = s.recurse(ArgType::DictEntry).unwrap();
                        let kk = MessageItem::get(&mut ss).unwrap();
                        ss.next();
                        let vv = MessageItem::get(&mut ss).unwrap();
                        v.push((kk, vv));
                        s.next();
                    };
                    MessageItem::Dict(MessageItemDict { v: v, sig:  i.signature() })
                } else {
                    let mut v = vec!();
                    while let Some(mi) = MessageItem::get(&mut s) { v.push(mi); s.next(); };
                    MessageItem::Array(MessageItemArray { v: v, sig: i.signature() })
                }
            },
            ArgType::Variant => MessageItem::Variant({
                let mut s = i.recurse(ArgType::Variant).unwrap();
                Box::new(MessageItem::get(&mut s).unwrap())
            }),
            ArgType::Boolean => MessageItem::Bool(i.get::<bool>().unwrap()),
            ArgType::Invalid => return None,
            ArgType::String => MessageItem::Str(i.get::<String>().unwrap()),
            ArgType::DictEntry => return None,
            ArgType::Byte => MessageItem::Byte(i.get::<u8>().unwrap()),
            ArgType::Int16 => MessageItem::Int16(i.get::<i16>().unwrap()),
            ArgType::UInt16 => MessageItem::UInt16(i.get::<u16>().unwrap()),
            ArgType::Int32 => MessageItem::Int32(i.get::<i32>().unwrap()),
            ArgType::UInt32 => MessageItem::UInt32(i.get::<u32>().unwrap()),
            ArgType::Int64 => MessageItem::Int64(i.get::<i64>().unwrap()),
            ArgType::UInt64 => MessageItem::UInt64(i.get::<u64>().unwrap()),
            ArgType::Double => MessageItem::Double(i.get::<f64>().unwrap()),
            #[cfg(not(feature = "stdfd"))]
            ArgType::UnixFd => MessageItem::UnixFd(i.get::<OwnedFd>().unwrap()),
            #[cfg(feature = "stdfd")]
            ArgType::UnixFd => MessageItem::UnixFd(MessageItemFd(i.get::<OwnedFd>().unwrap())),
            ArgType::Struct => MessageItem::Struct({
                let mut s = i.recurse(ArgType::Struct).unwrap();
                let mut v = vec!();
                while let Some(mi) = MessageItem::get(&mut s) { v.push(mi); s.next(); };
                v
            }),
            ArgType::ObjectPath => MessageItem::ObjectPath(i.get::<Path>().unwrap().into_static()),
            ArgType::Signature => MessageItem::Signature(i.get::<Signature>().unwrap().into_static()),
        })
    }
}

impl arg::RefArg for MessageItem {
    fn arg_type(&self) -> ArgType { MessageItem::arg_type(&self) }
    fn signature(&self) -> Signature<'static> { MessageItem::signature(&self) }
    fn append(&self, i: &mut IterAppend) { arg::Append::append_by_ref(self, i) }
    #[inline]
    fn as_any(&self) -> &dyn any::Any where Self: 'static { self }
    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn any::Any where Self: 'static { self }
    #[inline]
    fn box_clone(&self) -> Box<dyn arg::RefArg + 'static> { Box::new(self.clone()) }
}


impl arg::Append for arg::Variant<MessageItem> {
    fn append_by_ref(&self, i: &mut IterAppend) {
        let z = &self.0;
        let asig = z.signature();
        let sig = asig.as_cstr();
        i.append_container(ArgType::Variant, Some(&sig), |s| z.append_by_ref(s));
    }
}


/// Client side properties - get and set properties on a remote application.
pub struct Props<'a> {
    name: BusName<'a>,
    path: Path<'a>,
    interface: Interface<'a>,
    timeout_ms: i32,
    conn: &'a Connection,
}

impl<'a> Props<'a> {
    /// Create a new Props.
    pub fn new<N, P, I>(conn: &'a Connection, name: N, path: P, interface: I, timeout_ms: i32) -> Props<'a>
    where N: Into<BusName<'a>>, P: Into<Path<'a>>, I: Into<Interface<'a>> {
        Props {
            name: name.into(),
            path: path.into(),
            interface: interface.into(),
            timeout_ms: timeout_ms,
            conn: conn,
        }
    }

    /// Get a single property's value.
    pub fn get(&self, propname: &str) -> Result<MessageItem, Error> {
        let mut m = Message::method_call(&self.name, &self.path,
            &"org.freedesktop.DBus.Properties".into(), &"Get".into());
        m.append_items(&[self.interface.to_string().into(), propname.to_string().into()]);
        let mut r = self.conn.send_with_reply_and_block(m, self.timeout_ms)?;
        let reply = r.as_result()?.get_items();
        if reply.len() == 1 {
            if let MessageItem::Variant(ref v) = reply[0] {
                return Ok((**v).clone())
            }
       }
       let f = format!("Invalid reply for property get {}: '{:?}'", propname, reply);
       Err(Error::new_custom("InvalidReply", &f))
    }

    /// Set a single property's value.
    pub fn set(&self, propname: &str, value: MessageItem) -> Result<(), Error> {
        let mut m = Message::method_call(&self.name, &self.path,
            &"org.freedesktop.DBus.Properties".into(), &"Set".into());
        m.append_items(&[self.interface.to_string().into(), propname.to_string().into(), Box::new(value).into()]);
        let mut r = self.conn.send_with_reply_and_block(m, self.timeout_ms)?;
        r.as_result()?;
        Ok(())
    }

    /// Get a map of all the properties' names and their values.
    pub fn get_all(&self) -> Result<BTreeMap<String, MessageItem>, Error> {
        let mut m = Message::method_call(&self.name, &self.path,
            &"org.freedesktop.DBus.Properties".into(), &"GetAll".into());
        m.append_items(&[self.interface.to_string().into()]);
        let mut r = self.conn.send_with_reply_and_block(m, self.timeout_ms)?;
        let reply = r.as_result()?.get_items();

        (|| {
            if reply.len() != 1 { return Err(()) };
            let mut tree = BTreeMap::new();
            let a: &[(MessageItem, MessageItem)] = reply[0].inner()?;
            for (k, v) in a.iter() {
                let (k, v): (&String, &Box<MessageItem>) = (k.inner()?, v.inner()?);
                tree.insert(k.clone(), *v.clone());
            }
            Ok(tree)
        })().map_err(|_| {
            let f = format!("Invalid reply for property GetAll: '{:?}'", reply);
            Error::new_custom("InvalidReply", &f)
        })
    }
}

/// Wrapper around Props that keeps a map of fetched properties.
pub struct PropHandler<'a> {
    p: Props<'a>,
    map: BTreeMap<String, MessageItem>,
}

impl<'a> PropHandler<'a> {
    /// Create a new PropHandler from a Props.
    pub fn new(p: Props) -> PropHandler {
        PropHandler { p: p, map: BTreeMap::new() }
    }

    /// Get a map of all the properties' names and their values.
    pub fn get_all(&mut self) -> Result<(), Error> {
        self.map = self.p.get_all()?;
        Ok(())
    }

    /// Get a mutable reference to the PropHandler's fetched properties.
    pub fn map_mut(&mut self) -> &mut BTreeMap<String, MessageItem> { &mut self.map }

    /// Get a reference to the PropHandler's fetched properties.
    pub fn map(&self) -> &BTreeMap<String, MessageItem> { &self.map }

    /// Get a single property's value.
    pub fn get(&mut self, propname: &str) -> Result<&MessageItem, Error> {
        let v = self.p.get(propname)?;
        self.map.insert(propname.to_string(), v);
        Ok(self.map.get(propname).unwrap())
    }

    /// Set a single property's value.
    pub fn set(&mut self, propname: &str, value: MessageItem) -> Result<(), Error> {
        self.p.set(propname, value.clone())?;
        self.map.insert(propname.to_string(), value);
        Ok(())
    }
}


#[cfg(test)]
mod test {
    extern crate tempfile;

    use crate::{Message, MessageType, Path, Signature};
    use crate::arg::messageitem::MessageItem;
    use crate::ffidisp::{Connection, BusType};

    #[test]
    fn unix_fd() {
        use std::io::prelude::*;
        use std::io::SeekFrom;
        use std::fs::OpenOptions;

        let c = Connection::get_private(BusType::Session).unwrap();
        c.register_object_path("/hello").unwrap();
        let mut m = Message::new_method_call(&c.unique_name(), "/hello", "com.example.hello", "Hello").unwrap();
        let tempdir = tempfile::Builder::new().prefix("dbus-rs-test").tempdir().unwrap();
        let mut filename = tempdir.path().to_path_buf();
        filename.push("test");
        println!("Creating file {:?}", filename);
        let mut file = OpenOptions::new().create(true).read(true).write(true).open(&filename).unwrap();
        file.write_all(b"z").unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();
        #[cfg(unix)]
        {
            m.append_items(&[file.into()]);
        }
        println!("Sending {:?}", m.get_items());
        c.send(m).unwrap();

        loop { for n in c.incoming(1000) {
            if n.msg_type() == MessageType::MethodCall {
                #[cfg(unix)]
                {
                    use std::os::unix::io::AsRawFd;
                    let z: crate::arg::OwnedFd = n.read1().unwrap();
                    println!("Got {:?}", z);
                    let mut q: libc::c_char = 100;
                    assert_eq!(1, unsafe { libc::read(z.as_raw_fd(), &mut q as *mut _ as *mut libc::c_void, 1) });
                    assert_eq!(q, 'z' as libc::c_char);
                }
                return;
            } else {
                println!("Got {:?}", n);
            }
        }}
    }

    #[test]
    fn message_types() {
        let c = Connection::get_private(BusType::Session).unwrap();
        c.register_object_path("/hello").unwrap();
        let mut m = Message::new_method_call(&c.unique_name(), "/hello", "com.example.hello", "Hello").unwrap();
        m.append_items(&[
            2000u16.into(),
            MessageItem::new_array(vec!(129u8.into())).unwrap(),
            ["Hello", "world"][..].into(),
            987654321u64.into(),
            (-1i32).into(),
            format!("Hello world").into(),
            (-3.14f64).into(),
            MessageItem::Struct(vec!(256i16.into())),
            Path::new("/some/path").unwrap().into(),
            MessageItem::new_dict(vec!((123543u32.into(), true.into()).into())).unwrap()
        ]);
        let sending = format!("{:?}", m.get_items());
        println!("Sending {}", sending);
        c.send(m).unwrap();

        loop { for n in c.incoming(1000) {
            if n.msg_type() == MessageType::MethodCall {
                let receiving = format!("{:?}", n.get_items());
                println!("Receiving {}", receiving);
                assert_eq!(sending, receiving);
                return;
            } else {
                println!("Got {:?}", n);
            }
        }}
    }

    #[test]
    fn dict_of_dicts() {
        use std::collections::BTreeMap;

        let officeactions: BTreeMap<&'static str, MessageItem> = BTreeMap::new();
        let mut officethings = BTreeMap::new();
        officethings.insert("pencil", 2u16.into());
        officethings.insert("paper", 5u16.into());
        let mut homethings = BTreeMap::new();
        homethings.insert("apple", 11u16.into());
        let mut homeifaces = BTreeMap::new();
        homeifaces.insert("getThings", homethings);
        let mut officeifaces = BTreeMap::new();
        officeifaces.insert("getThings", officethings);
        officeifaces.insert("getActions", officeactions);
        let mut paths = BTreeMap::new();
        paths.insert("/hello/office", officeifaces);
        paths.insert("/hello/home", homeifaces);

        println!("Original treemap: {:?}", paths);
        let m = MessageItem::new_dict(paths.iter().map(
            |(path, ifaces)| (MessageItem::ObjectPath(Path::new(*path).unwrap()),
                MessageItem::new_dict(ifaces.iter().map(
                    |(iface, props)| (iface.to_string().into(),
                        MessageItem::from_dict::<(),_>(props.iter().map(
                            |(name, value)| Ok((name.to_string(), value.clone()))
                        )).unwrap()
                    ).into()
                ).collect()).unwrap()
            ).into()
        ).collect()).unwrap();
        println!("As MessageItem: {:?}", m);
        assert_eq!(&*m.signature(), "a{oa{sa{sv}}}");

        let c = Connection::get_private(BusType::Session).unwrap();
        c.register_object_path("/hello").unwrap();
        let mut msg = Message::new_method_call(&c.unique_name(), "/hello", "org.freedesktop.DBusObjectManager", "GetManagedObjects").unwrap();
        msg.append_items(&[m]);
        let sending = format!("{:?}", msg.get_items());
        println!("Sending {}", sending);
        c.send(msg).unwrap();

        loop { for n in c.incoming(1000) {
            if n.msg_type() == MessageType::MethodCall {
                let receiving = format!("{:?}", n.get_items());
                println!("Receiving {}", receiving);
                assert_eq!(sending, receiving);
                return;
            } else {
                println!("Got {:?}", n);
            }
        } }
    }

    #[test]
    fn issue24() {
        let c = Connection::get_private(BusType::Session).unwrap();
        let mut m = Message::new_method_call("org.test.rust", "/", "org.test.rust", "Test").unwrap();

        let a = MessageItem::from("test".to_string());
        let b = MessageItem::from("test".to_string());
        let foo = MessageItem::Struct(vec!(a, b));
        let bar = foo.clone();

        let args = [MessageItem::new_array(vec!(foo, bar)).unwrap()];
        println!("{:?}", args);

        m.append_items(&args);
        c.send(m).unwrap();
    }

    /* Unfortunately org.freedesktop.DBus has no properties we can use for testing, but hostname1 should be around on most distros. */
    #[cfg(unix)]
    #[test]
    fn test_get_hostname1_prop() {
        use super::Props;

        let c = Connection::new_system().unwrap();
        let p = Props::new(&c, "org.freedesktop.hostname1", "/org/freedesktop/hostname1",
            "org.freedesktop.hostname1", 10000);

        /* Let's use both the get and getall methods and see if we get the same result */
        let v = p.get("StaticHostname").unwrap();
        let vall = p.get_all().unwrap();
        let v2 = vall.get("StaticHostname").unwrap();

        assert_eq!(&v, &*v2);
        match v {
            MessageItem::Str(ref s) => { println!("StaticHostname is {}", s); }
            _ => { panic!("Invalid Get: {:?}", v); }
        };
    }

    #[test]
    fn message_listnames() {
        let c = Connection::get_private(BusType::Session).unwrap();
        let m = Message::method_call(&"org.freedesktop.DBus".into(), &"/".into(),
            &"org.freedesktop.DBus".into(), &"ListNames".into());
        let r = c.send_with_reply_and_block(m, 2000).unwrap();
        let reply = r.get_items();
        println!("{:?}", reply);
    }

    #[test]
    fn message_namehasowner() {
        let c = Connection::get_private(BusType::Session).unwrap();
        let mut m = Message::new_method_call("org.freedesktop.DBus", "/", "org.freedesktop.DBus", "NameHasOwner").unwrap();
        m.append_items(&[MessageItem::Str("org.freedesktop.DBus".to_string())]);
        let r = c.send_with_reply_and_block(m, 2000).unwrap();
        let reply = r.get_items();
        println!("{:?}", reply);
        assert_eq!(reply, vec!(MessageItem::Bool(true)));
    }

    #[test]
    fn message_inner_str() {
        let ob = MessageItem::ObjectPath("/path".into());
        assert_eq!("/path", ob.inner::<&str>().unwrap());

        let ob = MessageItem::ObjectPath("/path".into());
        assert_ne!("/path/another", ob.inner::<&str>().unwrap());

        let ob = MessageItem::Str("String".into());
        assert_eq!("String", ob.inner::<&str>().unwrap());

        let ob = MessageItem::Str("String".into());
        assert_ne!("StringDiff", ob.inner::<&str>().unwrap());

        let ob = MessageItem::Signature(Signature::make::<i32>());
        assert_eq!("i", ob.inner::<&str>().unwrap());

        let ob = MessageItem::Signature(Signature::make::<u32>());
        assert_ne!("i", ob.inner::<&str>().unwrap());

    }

    #[test]
    fn message_peel() {
        let flat_str = MessageItem::Str("foobar".into());
        assert_eq!(flat_str.peel(), &flat_str);

        let flat_path = MessageItem::ObjectPath("/path".into());
        assert_eq!(flat_path.peel(), &flat_path);

        let flat_sig = MessageItem::Signature(Signature::make::<i32>());
        assert_eq!(flat_sig.peel(), &flat_sig);

        let flat_int = MessageItem::Int32(1234);
        assert_eq!(flat_int.peel(), &flat_int);

        let layered_str = MessageItem::Variant(Box::new(flat_str));
        assert_eq!(layered_str.peel(), &MessageItem::Str("foobar".into()));

        let layered_path = MessageItem::Variant(Box::new(flat_path));
        assert_eq!(layered_path.peel(), &MessageItem::ObjectPath("/path".into()));

        let layered_sig = MessageItem::Variant(Box::new(flat_sig));
        assert_eq!(layered_sig.peel(), &MessageItem::Signature(Signature::make::<i32>()));

        let layered_int = MessageItem::Variant(Box::new(flat_int));
        assert_eq!(layered_int.peel(), &MessageItem::Int32(1234));

        let very_deep =
            MessageItem::Variant(Box::new(
            MessageItem::Variant(Box::new(
            MessageItem::Variant(Box::new(
            MessageItem::Variant(Box::new(
            MessageItem::Variant(Box::new(
            MessageItem::Variant(Box::new(
            MessageItem::Variant(Box::new(
            MessageItem::Variant(Box::new(
            MessageItem::Variant(Box::new(
            MessageItem::Variant(Box::new(
            MessageItem::Int32(1234)
            ))))))))))))))))))));

        assert_eq!(very_deep.peel(), &MessageItem::Int32(1234));

    }

    #[test]
    fn inner_from_variant() {
        let msg_u8 = MessageItem::Variant(Box::new(3u8.into()));
        assert_eq!(msg_u8.inner::<u8>().unwrap(), 3u8);

        let msg_u16 = MessageItem::Variant(Box::new(4u16.into()));
        assert_eq!(msg_u16.inner::<u16>().unwrap(), 4u16);

        let msg_u32 = MessageItem::Variant(Box::new(5u32.into()));
        assert_eq!(msg_u32.inner::<u32>().unwrap(), 5u32);

        let msg_u64 = MessageItem::Variant(Box::new(6u64.into()));
        assert_eq!(msg_u64.inner::<u64>().unwrap(), 6u64);

        let msg_i16 = MessageItem::Variant(Box::new(4i16.into()));
        assert_eq!(msg_i16.inner::<i16>().unwrap(), 4i16);

        let msg_i32 = MessageItem::Variant(Box::new(5i32.into()));
        assert_eq!(msg_i32.inner::<i32>().unwrap(), 5i32);

        let msg_i64 = MessageItem::Variant(Box::new(6i64.into()));
        assert_eq!(msg_i64.inner::<i64>().unwrap(), 6i64);

        let msg_f64 = MessageItem::Variant(Box::new(6.5f64.into()));
        assert_eq!(msg_f64.inner::<f64>().unwrap(), 6.5f64);

        let msg_bool = MessageItem::Variant(Box::new(false.into()));
        assert_eq!(msg_bool.inner::<bool>().unwrap(), false);

        let msg_string = MessageItem::Variant(Box::new("asdf".to_string().into()));
        assert_eq!(msg_string.inner::<&String>().unwrap(), "asdf");

        let path: Path = "/path".into();
        let msg_path = MessageItem::Variant(Box::new(MessageItem::ObjectPath(path.clone())));
        assert_eq!(msg_path.inner::<&Path>().unwrap(), &path);

        let sig: Signature = "a{si}".into();
        let msg_sig = MessageItem::Variant(Box::new(MessageItem::Signature(sig.clone())));
        assert_eq!(msg_sig.inner::<&Signature>().unwrap(), &sig);

        assert_eq!(msg_string.inner::<&str>().unwrap(), "asdf");
        assert_eq!(msg_path.inner::<&str>().unwrap(), "/path");
        assert_eq!(msg_sig.inner::<&str>().unwrap(), "a{si}");

    }

}
