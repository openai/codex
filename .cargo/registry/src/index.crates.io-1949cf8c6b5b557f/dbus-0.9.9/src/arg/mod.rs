//! Types and traits for easily getting a message's arguments, or appending a message with arguments.
//!
//! Also see the argument's guide (in the examples directory) for details about which Rust
//! types correspond to which D-Bus types.
//!
//! A message has `read1`, `read2` etc, and `append1`, `append2` etc, which is one
//! starting point into this module's types.


mod msgarg;
mod basic_impl;
mod variantstruct_impl;
mod array_impl;

pub mod messageitem;

pub use self::msgarg::{Arg, FixedArray, Get, DictKey, Append, RefArg, AppendAll, ReadAll, ArgAll,
    cast, cast_mut, prop_cast, PropMap};
pub use self::array_impl::{Array, Dict};
pub use self::variantstruct_impl::Variant;

use std::{fmt, mem, ptr, error};
use crate::{ffi, Message, Signature, Path};
use std::ffi::{CStr, CString};
use std::os::raw::{c_void, c_int};
#[cfg(unix)]
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::collections::VecDeque;

fn check(f: &str, i: u32) { if i == 0 { panic!("D-Bus error: '{}' failed", f) }}

fn ffi_iter() -> ffi::DBusMessageIter {
    // Safe because DBusMessageIter contains only fields that are allowed to be zeroed (i e no references or similar)
    unsafe { mem::zeroed() }
}

#[cfg(feature = "stdfd")]
pub use std::os::unix::io::OwnedFd;

/// An RAII wrapper around Fd to ensure that file descriptor is closed
/// when the scope ends. Enable the `stdfd` feature to use std's OwnedFd instead.
#[cfg(not(feature = "stdfd"))]
#[derive(Debug, PartialEq, PartialOrd)]
pub struct OwnedFd {
    #[cfg(unix)]
    fd: std::os::unix::io::RawFd
}

#[cfg(all(unix,not(feature = "stdfd")))]
mod owned_fd_impl {
    use super::OwnedFd;
    use std::os::unix::io::{RawFd, AsRawFd, FromRawFd, IntoRawFd};

    impl OwnedFd {
        /// Create a new OwnedFd from a RawFd.
        ///
        /// This function is unsafe, because you could potentially send in an invalid file descriptor,
        /// or close it during the lifetime of this struct. This could potentially be unsound.
        pub unsafe fn new(fd: RawFd) -> OwnedFd {
            OwnedFd { fd: fd }
        }

        /// Convert an OwnedFD back into a RawFd.
        pub fn into_fd(self) -> RawFd {
            let s = self.fd;
            ::std::mem::forget(self);
            s
        }

        /// Tries to clone the fd.
        pub fn try_clone(&self) -> Result<Self, &'static str> {
            let x = unsafe { libc::dup(self.fd) };
            if x == -1 { Err("Duplicating file descriptor failed") }
            else { Ok(unsafe { OwnedFd::new(x) }) }
        }
    }

    impl Drop for OwnedFd {
        fn drop(&mut self) {
            unsafe { libc::close(self.fd); }
        }
    }

    impl AsRawFd for OwnedFd {
        fn as_raw_fd(&self) -> RawFd {
            self.fd
        }
    }

    impl IntoRawFd for OwnedFd {
        fn into_raw_fd(self) -> RawFd {
            self.into_fd()
        }
    }

    impl FromRawFd for OwnedFd {
        unsafe fn from_raw_fd(fd: RawFd) -> Self { OwnedFd::new(fd) }
    }
}

#[cfg(not(feature = "stdfd"))]
impl Clone for OwnedFd {
    #[cfg(unix)]
    fn clone(&self) -> OwnedFd {
        self.try_clone().unwrap()
    }

    #[cfg(windows)]
    fn clone(&self) -> OwnedFd {
        OwnedFd {}
    }
}


#[derive(Clone, Copy)]
/// Helper struct for appending one or more arguments to a Message.
pub struct IterAppend<'a>(ffi::DBusMessageIter, &'a Message);

impl<'a> IterAppend<'a> {
    /// Creates a new IterAppend struct.
    pub fn new(m: &'a mut Message) -> IterAppend<'a> {
        let mut i = ffi_iter();
        unsafe { ffi::dbus_message_iter_init_append(m.ptr(), &mut i) };
        IterAppend(i, m)
    }

    /// Appends the argument.
    pub fn append<T: Append>(&mut self, a: T) { a.append(self) }

    fn append_container<F: FnOnce(&mut IterAppend<'a>)>(&mut self, arg_type: ArgType, sig: Option<&CStr>, f: F) {
        let mut s = IterAppend(ffi_iter(), self.1);
        let p = sig.map(|s| s.as_ptr()).unwrap_or(ptr::null());
        check("dbus_message_iter_open_container",
            unsafe { ffi::dbus_message_iter_open_container(&mut self.0, arg_type as c_int, p, &mut s.0) });
        f(&mut s);
        check("dbus_message_iter_close_container",
            unsafe { ffi::dbus_message_iter_close_container(&mut self.0, &mut s.0) });
    }

    /// Low-level function to append a variant.
    ///
    /// Use in case the `Variant` struct is not flexible enough -
    /// the easier way is to just call e g "append1" on a message and supply a `Variant` parameter.
    ///
    /// In order not to get D-Bus errors: during the call to "f" you need to call "append" on
    /// the supplied `IterAppend` exactly once,
    /// and with a value which has the same signature as inner_sig.
    pub fn append_variant<F: FnOnce(&mut IterAppend<'a>)>(&mut self, inner_sig: &Signature, f: F) {
        self.append_container(ArgType::Variant, Some(inner_sig.as_cstr()), f)
    }

    /// Low-level function to append an array.
    ///
    /// Use in case the `Array` struct is not flexible enough -
    /// the easier way is to just call e g "append1" on a message and supply an `Array` parameter.
    ///
    /// In order not to get D-Bus errors: during the call to "f", you should only call "append" on
    /// the supplied `IterAppend` with values which has the same signature as inner_sig.
    pub fn append_array<F: FnOnce(&mut IterAppend<'a>)>(&mut self, inner_sig: &Signature, f: F) {
        self.append_container(ArgType::Array, Some(inner_sig.as_cstr()), f)
    }

    /// Low-level function to append a struct.
    ///
    /// Use in case tuples are not flexible enough -
    /// the easier way is to just call e g "append1" on a message and supply a tuple parameter.
    pub fn append_struct<F: FnOnce(&mut IterAppend<'a>)>(&mut self, f: F) {
        self.append_container(ArgType::Struct, None, f)
    }

    /// Low-level function to append a dict entry.
    ///
    /// Use in case the `Dict` struct is not flexible enough -
    /// the easier way is to just call e g "append1" on a message and supply a `Dict` parameter.
    ///
    /// In order not to get D-Bus errors: during the call to "f", you should call "append" once
    /// for the key, then once for the value. You should only call this function for a subiterator
    /// you got from calling "append_dict", and signatures need to match what you specified in "append_dict".
    pub fn append_dict_entry<F: FnOnce(&mut IterAppend<'a>)>(&mut self, f: F) {
        self.append_container(ArgType::DictEntry, None, f)
    }

    /// Low-level function to append a dict.
    ///
    /// Use in case the `Dict` struct is not flexible enough -
    /// the easier way is to just call e g "append1" on a message and supply a `Dict` parameter.
    ///
    /// In order not to get D-Bus errors: during the call to "f", you should only call "append_dict_entry"
    /// for the subiterator - do this as many times as the number of dict entries.
    pub fn append_dict<F: FnOnce(&mut IterAppend<'a>)>(&mut self, key_sig: &Signature, value_sig: &Signature, f: F) {
        let sig = format!("{{{}{}}}", key_sig, value_sig);
        self.append_container(Array::<bool,()>::ARG_TYPE, Some(&CString::new(sig).unwrap()), f);
    }
}



#[derive(Clone, Copy)]
/// Helper struct for retrieve one or more arguments from a Message.
pub struct Iter<'a>(ffi::DBusMessageIter, &'a Message, u32);

impl<'a> Iter<'a> {
    /// Creates a new struct for iterating over the arguments of a message, starting with the first argument.
    pub fn new(m: &'a Message) -> Iter<'a> {
        let mut i = ffi_iter();
        unsafe { ffi::dbus_message_iter_init(m.ptr(), &mut i) };
        Iter(i, m, 0)
    }

    /// Returns the current argument, if T is the argument type. Otherwise returns None.
    pub fn get<T: Get<'a>>(&mut self) -> Option<T> {
        T::get(self)
    }

    /// Returns the current argument as a trait object.
    ///
    /// See the argument guide's reference part for information on what types hide inside the RefArg.
    pub fn get_refarg(&mut self) -> Option<Box<dyn RefArg + 'static>> {
        Some(match self.arg_type() {
            ArgType::Array => array_impl::get_array_refarg(self),
            ArgType::Variant => Box::new(Variant::new_refarg(self).unwrap()),
            ArgType::Boolean => Box::new(self.get::<bool>().unwrap()),
            ArgType::Invalid => return None,
            ArgType::String => Box::new(self.get::<String>().unwrap()),
            ArgType::DictEntry => unimplemented!(),
            ArgType::Byte => Box::new(self.get::<u8>().unwrap()),
            ArgType::Int16 => Box::new(self.get::<i16>().unwrap()),
            ArgType::UInt16 => Box::new(self.get::<u16>().unwrap()),
            ArgType::Int32 => Box::new(self.get::<i32>().unwrap()),
            ArgType::UInt32 => Box::new(self.get::<u32>().unwrap()),
            ArgType::Int64 => Box::new(self.get::<i64>().unwrap()),
            ArgType::UInt64 => Box::new(self.get::<u64>().unwrap()),
            ArgType::Double => Box::new(self.get::<f64>().unwrap()),
            ArgType::UnixFd => Box::new(self.get::<std::fs::File>().unwrap()),
            ArgType::Struct => Box::new(self.recurse(ArgType::Struct).unwrap().collect::<VecDeque<_>>()),
            ArgType::ObjectPath => Box::new(self.get::<Path>().unwrap().into_static()),
            ArgType::Signature => Box::new(self.get::<Signature>().unwrap().into_static()),
        })
    }

    /// Returns the type signature for the current argument.
    pub fn signature(&mut self) -> Signature<'static> {
        unsafe {
            let c = ffi::dbus_message_iter_get_signature(&mut self.0);
            assert!(c != ptr::null_mut());
            let cc = CStr::from_ptr(c);
            let r = Signature::new(std::str::from_utf8(cc.to_bytes()).unwrap());
            ffi::dbus_free(c as *mut c_void);
            r.unwrap()
        }
    }

    /// The raw arg_type for the current item.
    ///
    /// Unlike Arg::arg_type, this requires access to self and is not a static method.
    /// You can match this against Arg::arg_type for different types to understand what type the current item is.
    /// In case you're past the last argument, this function will return 0.
    pub fn arg_type(&mut self) -> ArgType {
        let s = unsafe { ffi::dbus_message_iter_get_arg_type(&mut self.0) };
        ArgType::from_i32(s as i32).unwrap()
    }

    /// Returns false if there are no more items.
    pub fn next(&mut self) -> bool {
        self.2 += 1;
        unsafe { ffi::dbus_message_iter_next(&mut self.0) != 0 }
    }

    /// Wrapper around `get` and `next`. Calls `get`, and then `next` if `get` succeeded.
    ///
    /// Also returns a `Result` rather than an `Option` to give an error if successful.
    ///
    /// # Example
    /// ```ignore
    /// struct ServiceBrowserItemNew {
    ///     interface: i32,
    ///     protocol: i32,
    ///     name: String,
    ///     item_type: String,
    ///     domain: String,
    ///     flags: u32,
    /// }
    ///
    /// fn service_browser_item_new_msg(m: &Message) -> Result<ServiceBrowserItemNew, TypeMismatchError> {
    ///     let mut iter = m.iter_init();
    ///     Ok(ServiceBrowserItemNew {
    ///         interface: iter.read()?,
    ///         protocol: iter.read()?,
    ///         name: iter.read()?,
    ///         item_type: iter.read()?,
    ///         domain: iter.read()?,
    ///         flags: iter.read()?,
    ///     })
    /// }
    /// ```
    pub fn read<T: Arg + Get<'a>>(&mut self) -> Result<T, TypeMismatchError> {
        let r = self.get().ok_or_else(||
             TypeMismatchError { expected: T::ARG_TYPE, found: self.arg_type(), position: self.2 })?;
        self.next();
        Ok(r)
    }

    /// If the current argument is a container of the specified arg_type, then a new
    /// Iter is returned which is for iterating over the contents inside the container.
    ///
    /// Primarily for internal use (the "get" function is more ergonomic), but could be
    /// useful for recursing into containers with unknown types.
    pub fn recurse(&mut self, arg_type: ArgType) -> Option<Iter<'a>> {
        let containers = [ArgType::Array, ArgType::DictEntry, ArgType::Struct, ArgType::Variant];
        if !containers.iter().any(|&t| t == arg_type) { return None; }

        let mut subiter = ffi_iter();
        unsafe {
            if ffi::dbus_message_iter_get_arg_type(&mut self.0) != arg_type as c_int { return None };
            ffi::dbus_message_iter_recurse(&mut self.0, &mut subiter)
        }
        Some(Iter(subiter, self.1, 0))
    }
}

impl<'a> fmt::Debug for Iter<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut z = self.clone();
        let mut t = f.debug_tuple("Iter");
        loop {
            t.field(&z.arg_type());
            if !z.next() { break }
        }
        t.finish()
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = Box<dyn RefArg + 'static>;
    fn next(&mut self) -> Option<Self::Item> {
        let r = self.get_refarg();
        if r.is_some() { self.next(); }
        r
    }
}

/// Type of Argument
///
/// use this to figure out, e g, which type of argument is at the current position of Iter.
#[repr(u8)]
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub enum ArgType {
    /// Dicts are Arrays of dict entries, so Dict types will have Array as ArgType.
    Array = ffi::DBUS_TYPE_ARRAY as u8,
    /// Variant
    Variant = ffi::DBUS_TYPE_VARIANT as u8,
    /// bool
    Boolean = ffi::DBUS_TYPE_BOOLEAN as u8,
    /// Invalid arg type - this is also the ArgType returned when there are no more arguments available.
    Invalid = ffi::DBUS_TYPE_INVALID as u8,
    /// String
    String = ffi::DBUS_TYPE_STRING as u8,
    /// Dict entry; you'll usually not encounter this one as dicts are arrays of dict entries.
    DictEntry = ffi::DBUS_TYPE_DICT_ENTRY as u8,
    /// u8
    Byte = ffi::DBUS_TYPE_BYTE as u8,
    /// i16
    Int16 = ffi::DBUS_TYPE_INT16 as u8,
    /// u16
    UInt16 = ffi::DBUS_TYPE_UINT16 as u8,
    /// i32
    Int32 = ffi::DBUS_TYPE_INT32 as u8,
    /// u32
    UInt32 = ffi::DBUS_TYPE_UINT32 as u8,
    /// i64
    Int64 = ffi::DBUS_TYPE_INT64 as u8,
    /// u64
    UInt64 = ffi::DBUS_TYPE_UINT64 as u8,
    /// f64
    Double = ffi::DBUS_TYPE_DOUBLE as u8,
    /// File
    UnixFd = ffi::DBUS_TYPE_UNIX_FD as u8,
    /// Use tuples or Vec<Box<dyn RefArg>> to read/write structs.
    Struct = ffi::DBUS_TYPE_STRUCT as u8,
    /// Path
    ObjectPath = ffi::DBUS_TYPE_OBJECT_PATH as u8,
    /// Signature
    Signature = ffi::DBUS_TYPE_SIGNATURE as u8,
}

const ALL_ARG_TYPES: [(ArgType, &str); 18] =
    [(ArgType::Variant, "Variant"),
    (ArgType::Array, "Array/Dict"),
    (ArgType::Struct, "Struct"),
    (ArgType::String, "String"),
    (ArgType::DictEntry, "Dict entry"),
    (ArgType::ObjectPath, "Path"),
    (ArgType::Signature, "Signature"),
    (ArgType::UnixFd, "File"),
    (ArgType::Boolean, "bool"),
    (ArgType::Byte, "u8"),
    (ArgType::Int16, "i16"),
    (ArgType::Int32, "i32"),
    (ArgType::Int64, "i64"),
    (ArgType::UInt16, "u16"),
    (ArgType::UInt32, "u32"),
    (ArgType::UInt64, "u64"),
    (ArgType::Double, "f64"),
    (ArgType::Invalid, "nothing")];

impl ArgType {
    /// A str corresponding to the name of a Rust type.
    pub fn as_str(self) -> &'static str {
        ALL_ARG_TYPES.iter().skip_while(|a| a.0 != self).next().unwrap().1
    }

    /// Returns a Vec of all possible argtypes.
    pub fn all() -> Vec<Self> {
        ALL_ARG_TYPES.iter().map(|x| x.0).collect()
    }

    /// Converts an i32 to an ArgType (or an error).
    pub fn from_i32(i: i32) -> Result<ArgType, String> {
        for &(a, _) in &ALL_ARG_TYPES {
            if a as i32 == i { return Ok(a); }
        }
        Err(format!("Invalid ArgType {} ({})", i, i as u8 as char))
    }
}


/// Error struct to indicate a D-Bus argument type mismatch.
///
/// Might be returned from `iter::read()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypeMismatchError {
    expected: ArgType,
    found: ArgType,
    position: u32,
}

impl TypeMismatchError {
    /// The ArgType we were trying to read, but failed
    pub fn expected_arg_type(&self) -> ArgType { self.expected }

    /// The ArgType we should have been trying to read, if we wanted the read to succeed
    pub fn found_arg_type(&self) -> ArgType { self.found }

    /// At what argument was the error found?
    ///
    /// Returns 0 for first argument, 1 for second argument, etc.
    pub fn pos(&self) -> u32 { self.position }
}

impl error::Error for TypeMismatchError {
    fn description(&self) -> &str { "D-Bus argument type mismatch" }
    fn cause(&self) -> Option<&dyn error::Error> { None }
}

impl fmt::Display for TypeMismatchError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "D-Bus argument type mismatch at position {}: expected {}, found {}",
            self.position, self.expected.as_str(),
            if self.expected == self.found { "same but still different somehow" } else { self.found.as_str() }
        )
    }
}


#[allow(dead_code)]
fn test_compile() {
    let mut msg = Message::new_signal("/", "a.b", "C").unwrap();
    let mut q = IterAppend::new(&mut msg);

    q.append(5u8);
    q.append(Array::new(&[5u8, 6, 7]));
    q.append((8u8, &[9u8, 6, 7][..]));
    q.append(Variant((6u8, 7u8)));
}
