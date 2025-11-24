use crate::ffi;
use super::*;
use super::check;
use crate::strings::{Signature, Path, Member, ErrorName, Interface};
use std::{ptr, any, mem};
use std::ffi::CStr;
use std::os::raw::{c_void, c_char, c_int};
use std::fs::File;
use std::borrow::Cow;


fn arg_append_basic<T>(i: *mut ffi::DBusMessageIter, arg_type: ArgType, v: T) {
    let p = &v as *const _ as *const c_void;
    unsafe {
        check("dbus_message_iter_append_basic", ffi::dbus_message_iter_append_basic(i, arg_type as c_int, p));
    };
}

fn arg_get_basic<T>(i: *mut ffi::DBusMessageIter, arg_type: ArgType) -> Option<T> {
    unsafe {
        if ffi::dbus_message_iter_get_arg_type(i) != arg_type as c_int { return None };
        let mut c = mem::MaybeUninit::uninit();
        ffi::dbus_message_iter_get_basic(i, &mut c as *mut _ as *mut c_void);
        Some(c.assume_init())
    }
}

fn arg_append_str(i: *mut ffi::DBusMessageIter, arg_type: ArgType, v: &CStr) {
    let p = v.as_ptr();
    let q = &p as *const _ as *const c_void;
    unsafe {
        check("dbus_message_iter_append_basic", ffi::dbus_message_iter_append_basic(i, arg_type as c_int, q));
    };
}

unsafe fn arg_get_str<'a>(i: *mut ffi::DBusMessageIter, arg_type: ArgType) -> Option<&'a CStr> {
    if ffi::dbus_message_iter_get_arg_type(i) != arg_type as c_int { return None };
    let mut p = ptr::null_mut();
    ffi::dbus_message_iter_get_basic(i, &mut p as *mut _ as *mut c_void);
    Some(CStr::from_ptr(p as *const c_char))
}




// Implementation for basic types.

macro_rules! integer_impl {
    ($t: ident, $s: ident, $f: expr, $i: ident, $ii: expr, $u: ident, $uu: expr, $fff: ident, $ff: expr) => {

impl Arg for $t {
    const ARG_TYPE: ArgType = ArgType::$s;
    #[inline]
    fn signature() -> Signature<'static> { unsafe { Signature::from_slice_unchecked($f) } }
}

impl Append for $t {
    fn append_by_ref(&self, i: &mut IterAppend) { arg_append_basic(&mut i.0, ArgType::$s, *self) }
}

impl<'a> Get<'a> for $t {
    fn get(i: &mut Iter) -> Option<Self> { arg_get_basic(&mut i.0, ArgType::$s) }
}

impl RefArg for $t {
    #[inline]
    fn arg_type(&self) -> ArgType { ArgType::$s }
    #[inline]
    fn signature(&self) -> Signature<'static> { unsafe { Signature::from_slice_unchecked($f) } }
    #[inline]
    fn append(&self, i: &mut IterAppend) { arg_append_basic(&mut i.0, ArgType::$s, *self) }
    #[inline]
    fn as_any(&self) -> &dyn any::Any { self }
    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn any::Any { self }
    #[inline]
    fn as_i64(&self) -> Option<i64> { let $i = *self; $ii }
    #[inline]
    fn as_u64(&self) -> Option<u64> { let $u = *self; $uu }
    #[inline]
    fn as_f64(&self) -> Option<f64> { let $fff = *self; $ff }
    #[inline]
    fn box_clone(&self) -> Box<dyn RefArg + 'static> { Box::new(self.clone()) }
    fn array_clone(v: &[Self]) -> Option<Box<dyn RefArg + 'static>> where Self: Sized { Some(Box::new(v.to_vec())) }
}

impl DictKey for $t {}
unsafe impl FixedArray for $t {}

}} // End of macro_rules

integer_impl!(u8, Byte, "y\0", i, Some(i as i64),    u, Some(u as u64), f, Some(f as f64));
integer_impl!(i16, Int16, "n\0", i, Some(i as i64),  _u, None,          f, Some(f as f64));
integer_impl!(u16, UInt16, "q\0", i, Some(i as i64), u, Some(u as u64), f, Some(f as f64));
integer_impl!(i32, Int32, "i\0", i, Some(i as i64),  _u, None,          f, Some(f as f64));
integer_impl!(u32, UInt32, "u\0", i, Some(i as i64), u, Some(u as u64), f, Some(f as f64));
integer_impl!(i64, Int64, "x\0", i, Some(i),         _u, None,          _f, None);
integer_impl!(u64, UInt64, "t\0", _i, None,          u, Some(u as u64), _f, None);


macro_rules! refarg_impl {
    ($t: ty, $i: ident, $ii: expr, $ss: expr, $uu: expr, $ff: expr) => {

impl RefArg for $t {
    #[inline]
    fn arg_type(&self) -> ArgType { <$t as Arg>::ARG_TYPE }
    #[inline]
    fn signature(&self) -> Signature<'static> { <$t as Arg>::signature() }
    #[inline]
    fn append(&self, i: &mut IterAppend) { <$t as Append>::append_by_ref(self, i) }
    #[inline]
    fn as_any(&self) -> &dyn any::Any { self }
    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn any::Any { self }
    #[inline]
    fn as_i64(&self) -> Option<i64> { let $i = self; $ii }
    #[inline]
    fn as_u64(&self) -> Option<u64> { let $i = self; $uu }
    #[inline]
    fn as_f64(&self) -> Option<f64> { let $i = self; $ff }
    #[inline]
    fn as_str(&self) -> Option<&str> { let $i = self; $ss }
    #[inline]
    fn box_clone(&self) -> Box<dyn RefArg + 'static> { Box::new(self.clone()) }
    fn array_clone(v: &[Self]) -> Option<Box<dyn RefArg + 'static>> where Self: Sized { Some(Box::new(v.to_vec())) }

}

    }
}


impl Arg for bool {
    const ARG_TYPE: ArgType = ArgType::Boolean;
    fn signature() -> Signature<'static> { unsafe { Signature::from_slice_unchecked("b\0") } }
}
impl Append for bool {
    fn append_by_ref(&self, i: &mut IterAppend) { arg_append_basic(&mut i.0, ArgType::Boolean, if *self {1} else {0}) }
}
impl DictKey for bool {}
impl<'a> Get<'a> for bool {
    fn get(i: &mut Iter) -> Option<Self> { arg_get_basic::<u32>(&mut i.0, ArgType::Boolean).map(|q| q != 0) }
}

refarg_impl!(bool, _i, Some(if *_i { 1 } else { 0 }), None, Some(if *_i { 1 as u64 } else { 0 as u64 }), Some(if *_i { 1 as f64 } else { 0 as f64 }));

impl Arg for f64 {
    const ARG_TYPE: ArgType = ArgType::Double;
    fn signature() -> Signature<'static> { unsafe { Signature::from_slice_unchecked("d\0") } }
}
impl Append for f64 {
    fn append_by_ref(&self, i: &mut IterAppend) { arg_append_basic(&mut i.0, ArgType::Double, *self) }
}
impl DictKey for f64 {}
impl<'a> Get<'a> for f64 {
    fn get(i: &mut Iter) -> Option<Self> { arg_get_basic(&mut i.0, ArgType::Double) }
}
unsafe impl FixedArray for f64 {}

refarg_impl!(f64, _i, None, None, None, Some(*_i));

/// Represents a D-Bus string.
impl<'a> Arg for &'a str {
    const ARG_TYPE: ArgType = ArgType::String;
    fn signature() -> Signature<'static> { unsafe { Signature::from_slice_unchecked("s\0") } }
}

impl<'a> Append for &'a str {
    fn append_by_ref(&self, i: &mut IterAppend) {
        let b: &[u8] = self.as_bytes();
        let v: Cow<[u8]> = if !b.is_empty() && b[b.len()-1] == 0 { Cow::Borrowed(b) }
        else {
            let mut bb: Vec<u8> = b.into();
            bb.push(0);
            Cow::Owned(bb)
        };
        let z = unsafe { CStr::from_ptr(v.as_ptr() as *const c_char) };
        arg_append_str(&mut i.0, ArgType::String, &z)
    }
}
impl<'a> DictKey for &'a str {}
impl<'a> Get<'a> for &'a str {
    fn get(i: &mut Iter<'a>) -> Option<&'a str> { unsafe { arg_get_str(&mut i.0, ArgType::String) }
        .and_then(|s| s.to_str().ok()) }
}

impl<'a> Append for Cow<'a, str> {
    fn append_by_ref(&self, i: &mut IterAppend) {
        (&**self).append_by_ref(i)
    }
}

impl<'a> Arg for String {
    const ARG_TYPE: ArgType = ArgType::String;
    fn signature() -> Signature<'static> { unsafe { Signature::from_slice_unchecked("s\0") } }
}
impl<'a> Append for String {
    fn append(mut self, i: &mut IterAppend) {
        self.push_str("\0");
        let s: &str = &self;
        s.append(i)
    }
    fn append_by_ref(&self, i: &mut IterAppend) {
        (&**self).append_by_ref(i)
    }
}
impl<'a> DictKey for String {}
impl<'a> Get<'a> for String {
    fn get(i: &mut Iter<'a>) -> Option<String> { <&str>::get(i).map(String::from) }
}

refarg_impl!(String, _i, None, Some(&_i), None, None);

/// Represents a D-Bus string.
impl<'a> Arg for &'a CStr {
    const ARG_TYPE: ArgType = ArgType::String;
    fn signature() -> Signature<'static> { unsafe { Signature::from_slice_unchecked("s\0") } }
}

/*
/// Note: Will give D-Bus errors in case the CStr is not valid UTF-8.
impl<'a> Append for &'a CStr {
    fn append(self, i: &mut IterAppend) {
        arg_append_str(&mut i.0, Self::arg_type(), &self)
    }
}
*/

impl<'a> DictKey for &'a CStr {}
impl<'a> Get<'a> for &'a CStr {
    fn get(i: &mut Iter<'a>) -> Option<&'a CStr> { unsafe { arg_get_str(&mut i.0, Self::ARG_TYPE) }}
}

impl Arg for OwnedFd {
    const ARG_TYPE: ArgType = ArgType::UnixFd;
    fn signature() -> Signature<'static> { unsafe { Signature::from_slice_unchecked("h\0") } }
}
impl Append for OwnedFd {
    #[cfg(unix)]
    fn append_by_ref(&self, i: &mut IterAppend) {
        arg_append_basic(&mut i.0, ArgType::UnixFd, self.as_raw_fd())
    }
    #[cfg(windows)]
    fn append_by_ref(&self, _i: &mut IterAppend) {
        panic!("File descriptor passing not available on Windows");
    }
}
impl DictKey for OwnedFd {}
impl<'a> Get<'a> for OwnedFd {
    #[cfg(unix)]
    fn get(i: &mut Iter) -> Option<Self> {
        arg_get_basic(&mut i.0, ArgType::UnixFd).map(|fd| unsafe { OwnedFd::from_raw_fd(fd) })
    }
    #[cfg(windows)]
    fn get(_i: &mut Iter) -> Option<Self> {
        None
    }
}

#[cfg(all(unix, feature = "io-lifetimes"))]
impl Arg for io_lifetimes::OwnedFd {
    const ARG_TYPE: ArgType = ArgType::UnixFd;
    fn signature() -> Signature<'static> { unsafe { Signature::from_slice_unchecked("h\0") } }
}
#[cfg(all(unix, feature = "io-lifetimes"))]
impl Append for io_lifetimes::OwnedFd {
    fn append_by_ref(&self, i: &mut IterAppend) {
        arg_append_basic(&mut i.0, ArgType::UnixFd, self.as_raw_fd())
    }
}
#[cfg(all(unix, feature = "io-lifetimes"))]
impl DictKey for io_lifetimes::OwnedFd {}
#[cfg(all(unix, feature = "io-lifetimes"))]
impl<'a> Get<'a> for io_lifetimes::OwnedFd {
    fn get(i: &mut Iter) -> Option<Self> {
        arg_get_basic(&mut i.0, ArgType::UnixFd).map(|fd| unsafe { io_lifetimes::OwnedFd::from_raw_fd(fd) })
    }
}

#[cfg(unix)]
impl RefArg for OwnedFd {
    #[inline]
    fn arg_type(&self) -> ArgType { <Self as Arg>::ARG_TYPE }
    #[inline]
    fn signature(&self) -> Signature<'static> { <Self as Arg>::signature() }
    #[inline]
    fn append(&self, i: &mut IterAppend) { <Self as Append>::append_by_ref(self, i) }
    #[inline]
    fn as_any(&self) -> &dyn any::Any { self }
    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn any::Any { self }
    #[inline]
    fn as_i64(&self) -> Option<i64> { Some(self.as_raw_fd() as i64) }
    #[inline]
    fn box_clone(&self) -> Box<dyn RefArg + 'static> { Box::new(self.try_clone().unwrap()) }
}

#[cfg(windows)]
refarg_impl!(OwnedFd, _i, None, None, None, None);

impl Arg for File {
    const ARG_TYPE: ArgType = ArgType::UnixFd;
    fn signature() -> Signature<'static> { unsafe { Signature::from_slice_unchecked("h\0") } }
}
impl Append for File {
    #[cfg(unix)]
    fn append_by_ref(&self, i: &mut IterAppend) {
        arg_append_basic(&mut i.0, ArgType::UnixFd, self.as_raw_fd())
    }
    #[cfg(windows)]
    fn append_by_ref(&self, _i: &mut IterAppend) {
        panic!("File descriptor passing not available on Windows");
    }
}
impl DictKey for File {}
impl<'a> Get<'a> for File {
    #[cfg(unix)]
    fn get(i: &mut Iter) -> Option<Self> {
        arg_get_basic(&mut i.0, ArgType::UnixFd).map(|fd| unsafe { File::from_raw_fd(fd) })
    }
    #[cfg(windows)]
    fn get(_i: &mut Iter) -> Option<Self> {
        None
    }
}

impl RefArg for File {
    #[inline]
    fn arg_type(&self) -> ArgType { <File as Arg>::ARG_TYPE }
    #[inline]
    fn signature(&self) -> Signature<'static> { <File as Arg>::signature() }
    #[inline]
    fn append(&self, i: &mut IterAppend) { <File as Append>::append_by_ref(self, i) }
    #[inline]
    fn as_any(&self) -> &dyn any::Any { self }
    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn any::Any { self }
    #[cfg(unix)]
    #[inline]
    fn as_i64(&self) -> Option<i64> { Some(self.as_raw_fd() as i64) }
    #[inline]
    fn box_clone(&self) -> Box<dyn RefArg + 'static> { Box::new(self.try_clone().unwrap()) }
}

#[cfg(all(unix, feature = "io-lifetimes"))]
impl RefArg for io_lifetimes::OwnedFd {
    #[inline]
    fn arg_type(&self) -> ArgType { <Self as Arg>::ARG_TYPE }
    #[inline]
    fn signature(&self) -> Signature<'static> { <Self as Arg>::signature() }
    #[inline]
    fn append(&self, i: &mut IterAppend) { <Self as Append>::append_by_ref(self, i) }
    #[inline]
    fn as_any(&self) -> &dyn any::Any { self }
    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn any::Any { self }
    #[cfg(unix)]
    #[inline]
    fn as_i64(&self) -> Option<i64> { Some(self.as_raw_fd() as i64) }
    #[inline]
    fn box_clone(&self) -> Box<dyn RefArg + 'static> { Box::new(self.try_clone().unwrap()) }
}


macro_rules! string_impl {
    ($t: ident, $s: ident, $f: expr) => {

impl<'a> Arg for $t<'a> {
    const ARG_TYPE: ArgType = ArgType::$s;
    fn signature() -> Signature<'static> { unsafe { Signature::from_slice_unchecked($f) } }
}

impl RefArg for $t<'static> {
    fn arg_type(&self) -> ArgType { ArgType::$s }
    fn signature(&self) -> Signature<'static> { unsafe { Signature::from_slice_unchecked($f) } }
    fn append(&self, i: &mut IterAppend) { arg_append_str(&mut i.0, ArgType::$s, self.as_cstr()) }
    #[inline]
    fn as_any(&self) -> &dyn any::Any { self }
    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn any::Any { self }
    #[inline]
    fn as_str(&self) -> Option<&str> { Some(self) }
    #[inline]
    fn box_clone(&self) -> Box<dyn RefArg + 'static> { Box::new(self.clone().into_static()) }
    fn array_clone(v: &[Self]) -> Option<Box<dyn RefArg + 'static>> where Self: Sized { Some(Box::new(v.to_vec())) }
}

impl<'a> DictKey for $t<'a> {}

impl<'a> Append for $t<'a> {
    fn append_by_ref(&self, i: &mut IterAppend) {
        arg_append_str(&mut i.0, ArgType::$s, self.as_cstr())
    }
}

/*

Unfortunately, this does not work because it conflicts with getting a $t<'static>.

impl<'a> Get<'a> for $t<'a> {
    fn get(i: &mut Iter<'a>) -> Option<$t<'a>> { unsafe { arg_get_str(&mut i.0, ArgType::$s) }
        .map(|s| unsafe { $t::from_slice_unchecked(s.to_bytes_with_nul()) } ) }
}
*/

impl<'a> Get<'a> for $t<'static> {
    fn get(i: &mut Iter<'a>) -> Option<$t<'static>> { unsafe {
        let c = arg_get_str(&mut i.0, ArgType::$s)?;
        let s = std::str::from_utf8(c.to_bytes_with_nul()).ok()?;
        Some($t::from_slice_unchecked(s).into_static())
    }}
}


    }
}

string_impl!(Interface, String, "s\0");
string_impl!(ErrorName, String, "s\0");
string_impl!(Member, String, "s\0");
string_impl!(Path, ObjectPath, "o\0");
string_impl!(Signature, Signature, "g\0");
