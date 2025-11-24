//! This module contains strings with a specific format, such as a valid
//! Interface name, a valid Error name, etc.
//!
//! (The internal representation of these strings are `Cow<str>`, but with a \0 byte
//! at the end to use it libdbus calls without extra allocations. This is usually nothing
//! you have to worry about.)

use std::{str, fmt, ops, default, hash};
use std::ffi::{CStr, CString};
use std::borrow::{Borrow, Cow};
use std::os::raw::c_char;

#[cfg(not(feature = "no-string-validation"))]
use crate::Error;
#[cfg(not(feature = "no-string-validation"))]
use crate::ffi;

macro_rules! cstring_wrapper {
    ($t: ident, $s: ident) => {

impl<'m> $t<'m> {
    #[cfg(feature = "no-string-validation")]
    fn check_valid(_: *const c_char) -> Result<(), String> { Ok(()) }

    #[cfg(not(feature = "no-string-validation"))]
    fn check_valid(c: *const c_char) -> Result<(), String> {
        let mut e = Error::empty();
        let b = unsafe { ffi::$s(c, e.get_mut()) };
        if b != 0 { Ok(()) } else { Err(e.message().unwrap().into()) }
    }

    /// Creates a new instance of this struct.
    ///
    /// Note: If the no-string-validation feature is activated, this string
    /// will not be checked for conformance with the D-Bus specification.
    pub fn new<S: Into<String>>(s: S) -> Result<$t<'m>, String> {
        let mut s = s.into();
        s.push_str("\0");
        unsafe { $t::check_valid(CStr::from_bytes_with_nul_unchecked(s.as_bytes()).as_ptr() as *const c_char)?; }
        Ok(Self(Cow::Owned(s)))
    }

    /// Creates a new instance of this struct. If you end it with \0,
    /// it can borrow the slice without extra allocation.
    ///
    /// Note: If the no-string-validation feature is activated, this string
    /// will not be checked for conformance with the D-Bus specification.
    pub fn from_slice(s: &'m str) -> Result<$t<'m>, String> {
        let ss = s.as_bytes();
        if ss.len() == 0 || ss[ss.len()-1] != 0 { return $t::new(s) };
        $t::check_valid(s.as_ptr() as *const c_char).map(|_| {
            unsafe { Self::from_slice_unchecked(s) }
        })
    }

    /// This function creates a new instance of this struct, without checking.
    /// It's up to you to guarantee that s ends with a \0 and is valid.
    pub const unsafe fn from_slice_unchecked(s: &'m str) -> $t<'m> {
        let ss = s.as_bytes();
        debug_assert!(ss[ss.len()-1] == 0);
        $t(Cow::Borrowed(s))
    }

    /// View this struct as a CStr.
    ///
    /// Note: As of dbus 0.9, this is made private to be able to make it easier for a potential
    /// native implementation using "str" instead of "cstr".
    pub (crate) fn as_cstr(&self) -> &CStr {
        unsafe {
            CStr::from_bytes_with_nul_unchecked(self.0.as_bytes())
        }
    }

    #[allow(dead_code)]
    pub (crate) fn as_ptr(&self) -> *const c_char { self.as_cstr().as_ptr() }

    /// Makes sure this string does not contain borrows.
    pub fn into_static(self) -> $t<'static> {
        $t(Cow::Owned(self.0.into_owned()))
    }

    /// Converts this struct to a CString.
    pub fn into_cstring(self) -> CString {
        // Change this when https://github.com/rust-lang/rust/issues/73179 is on stable.
        let mut x: Vec<u8> = self.0.into_owned().into();
        x.pop();
        CString::new(x).unwrap()
    }
}

/*
/// #Panics
///
/// If given string is not valid.
/// impl<S: Into<Vec<u8>>> From<S> for $t { fn from(s: S) -> $t { $t::new(s).unwrap() } }
*/

/// #Panics
///
/// If given string is not valid.
impl<'m> From<String> for $t<'m> { fn from(s: String) -> $t<'m> { $t::new(s).unwrap() } }

/// #Panics
///
/// If given string is not valid.
impl<'m> From<&'m String> for $t<'m> { fn from(s: &'m String) -> $t<'m> { $t::from_slice(s).unwrap() } }

/// #Panics
///
/// If given string is not valid.
impl<'m> From<&'m str> for $t<'m> { fn from(s: &'m str) -> $t<'m> { $t::from_slice(s).unwrap() } }

/// #Panics
///
/// If given string is not valid.
impl<'m> From<&'m CStr> for $t<'m> {
    fn from(s: &'m CStr) -> $t<'m> {
        let x = str::from_utf8(s.to_bytes_with_nul()).unwrap();
        $t::from_slice(x).unwrap()
    }
}

impl<'m> From<$t<'m>> for CString { fn from(s: $t<'m>) -> CString { s.into_cstring() } }


/// #Panics
///
/// If given string is not valid.
impl<'m> From<Cow<'m, str>> for $t<'m> {
    fn from(s: Cow<'m, str>) -> $t<'m> {
        match s {
            Cow::Borrowed(z) => z.into(),
            Cow::Owned(z) => z.into(),
        }
    }
}

impl<'inner, 'm: 'inner> From<&'m $t<'inner>> for $t<'m> {
    fn from(borrow: &'m $t<'inner>) -> $t<'m> {
        $t(Cow::Borrowed(borrow.0.borrow()))
    }
}

impl<'m> ops::Deref for $t<'m> {
    type Target = str;
    fn deref(&self) -> &str { self.0.split_at(self.0.len()-1).0 }
}

impl<'m> fmt::Display for $t<'m> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <str as fmt::Display>::fmt(&*self, f)
    }
}

/*
As of dbus 0.9, this has been removed to prepare for a potential native implementation.
impl<'m> AsRef<CStr> for $t<'m> {
    fn as_ref(&self) -> &CStr { &self.0 }
}
*/

impl<'m> hash::Hash for $t<'m> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl PartialEq<str> for $t<'_> {
    fn eq(&self, other: &str) -> bool {
        &**self == other
    }
}

impl PartialEq<&'_ str> for $t<'_> {
    fn eq(&self, other: &&str) -> bool {
        &**self == *other
    }
}

}}

/// A wrapper around a string that is guaranteed to be
/// a valid (single) D-Bus type signature.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct Signature<'a>(Cow<'a, str>);

cstring_wrapper!(Signature, dbus_signature_validate_single);

impl Signature<'static> {
    /// Makes a D-Bus signature that corresponds to A.
    pub fn make<A: super::arg::Arg>() -> Signature<'static> { A::signature() }
}

/// A wrapper around a string that is guaranteed to be
/// a valid D-Bus object path.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct Path<'a>(Cow<'a, str>);

cstring_wrapper!(Path, dbus_validate_path);

// This is needed so one can make arrays of paths easily
impl<'a> default::Default for Path<'a> {
    fn default() -> Path<'a> { Path(Cow::Borrowed("/\0")) }
}

/// A wrapper around a string that is guaranteed to be
/// a valid D-Bus member, i e, a signal or method name.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct Member<'a>(Cow<'a, str>);

cstring_wrapper!(Member, dbus_validate_member);

/// A wrapper around a string that is guaranteed to be
/// a valid D-Bus interface name.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct Interface<'a>(Cow<'a, str>);

cstring_wrapper!(Interface, dbus_validate_interface);

/// A wrapper around a string that is guaranteed to be
/// a valid D-Bus bus name.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct BusName<'a>(Cow<'a, str>);

cstring_wrapper!(BusName, dbus_validate_bus_name);

/// A wrapper around a string that is guaranteed to be
/// a valid D-Bus error name.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct ErrorName<'a>(Cow<'a, str>);

cstring_wrapper!(ErrorName, dbus_validate_error_name);

#[test]
fn some_path() {
    let p1: Path = "/valid".into();
    let p2 = Path::new("##invalid##");
    assert_eq!(p1, Path(Cow::Borrowed("/valid\0")));
    #[cfg(not(feature = "no-string-validation"))]
    assert_eq!(p2, Err("Object path was not valid: '##invalid##'".into()));
    #[cfg(feature = "no-string-validation")]
    assert_eq!(p2, Ok(Path(Cow::Borrowed("##invalid##\0"))));
}

#[test]
fn reborrow_path() {
    let p1 = Path::from("/valid");
    let p2 = p1.clone();
    {
        let p2_borrow: &Path = &p2;
        let p3 = Path::from(p2_borrow);
        // Check path created from borrow
        assert_eq!(p2, p3);
    }
    // Check path that was previously borrowed
    assert_eq!(p1, p2);
}

#[test]
fn make_sig() {
    assert_eq!(&*Signature::make::<(&str, u8)>(), "(sy)");
}

#[test]
fn partial_eq_str() {
    assert_eq!(Signature::new("s").unwrap(), "s");
    assert!(Signature::new("s").unwrap() != "s\0");
    assert_eq!(Path::new("/hello").unwrap(), "/hello");
}
