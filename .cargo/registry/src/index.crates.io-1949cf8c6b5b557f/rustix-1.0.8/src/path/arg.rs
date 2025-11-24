//! Convenient and efficient string argument passing.
//!
//! This module defines the `Arg` trait and implements it for several common
//! string types. This allows users to pass any of these string types directly
//! to rustix APIs with string arguments, and it allows rustix to implement
//! NUL-termination without the need for copying or dynamic allocation where
//! possible.

use crate::ffi::CStr;
use crate::io;
use crate::path::{DecInt, SMALL_PATH_BUFFER_SIZE};
#[cfg(feature = "alloc")]
use alloc::borrow::ToOwned as _;
use core::mem::MaybeUninit;
use core::{ptr, slice, str};
#[cfg(feature = "std")]
use std::ffi::{OsStr, OsString};
#[cfg(all(feature = "std", target_os = "hermit"))]
use std::os::hermit::ext::ffi::{OsStrExt, OsStringExt};
#[cfg(all(feature = "std", unix))]
use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};
#[cfg(all(feature = "std", target_os = "vxworks"))]
use std::os::vxworks::ext::ffi::{OsStrExt, OsStringExt};
#[cfg(all(
    feature = "std",
    target_os = "wasi",
    any(not(target_env = "p2"), wasip2)
))]
use std::os::wasi::ffi::{OsStrExt, OsStringExt};
#[cfg(feature = "std")]
use std::path::{Component, Components, Iter, Path, PathBuf};
#[cfg(feature = "alloc")]
use {crate::ffi::CString, alloc::borrow::Cow};
#[cfg(feature = "alloc")]
use {alloc::string::String, alloc::vec::Vec};

/// A trait for passing path arguments.
///
/// This is similar to [`AsRef`]`<`[`Path`]`>`, but is implemented for more
/// kinds of strings and can convert into more kinds of strings.
///
/// # Examples
///
/// ```
/// # #[cfg(any(feature = "fs", feature = "net"))]
/// use rustix::ffi::CStr;
/// use rustix::io;
/// # #[cfg(any(feature = "fs", feature = "net"))]
/// use rustix::path::Arg;
///
/// # #[cfg(any(feature = "fs", feature = "net"))]
/// pub fn touch<P: Arg>(path: P) -> io::Result<()> {
///     let path = path.into_c_str()?;
///     _touch(&path)
/// }
///
/// # #[cfg(any(feature = "fs", feature = "net"))]
/// fn _touch(path: &CStr) -> io::Result<()> {
///     // implementation goes here
///     Ok(())
/// }
/// ```
///
/// Users can then call `touch("foo")`, `touch(cstr!("foo"))`,
/// `touch(Path::new("foo"))`, or many other things.
///
/// [`AsRef`]: std::convert::AsRef
pub trait Arg {
    /// Returns a view of this string as a string slice.
    fn as_str(&self) -> io::Result<&str>;

    /// Returns a potentially-lossy rendering of this string as a
    /// `Cow<'_, str>`.
    #[cfg(feature = "alloc")]
    fn to_string_lossy(&self) -> Cow<'_, str>;

    /// Returns a view of this string as a maybe-owned [`CStr`].
    #[cfg(feature = "alloc")]
    fn as_cow_c_str(&self) -> io::Result<Cow<'_, CStr>>;

    /// Consumes `self` and returns a view of this string as a maybe-owned
    /// [`CStr`].
    #[cfg(feature = "alloc")]
    fn into_c_str<'b>(self) -> io::Result<Cow<'b, CStr>>
    where
        Self: 'b;

    /// Runs a closure with `self` passed in as a `&CStr`.
    fn into_with_c_str<T, F>(self, f: F) -> io::Result<T>
    where
        Self: Sized,
        F: FnOnce(&CStr) -> io::Result<T>;
}

/// Runs a closure on `arg` where `A` is mapped to a `&CStr`
pub fn option_into_with_c_str<T, F, A>(arg: Option<A>, f: F) -> io::Result<T>
where
    A: Arg + Sized,
    F: FnOnce(Option<&CStr>) -> io::Result<T>,
{
    if let Some(arg) = arg {
        arg.into_with_c_str(|p| f(Some(p)))
    } else {
        f(None)
    }
}

impl Arg for &str {
    #[inline]
    fn as_str(&self) -> io::Result<&str> {
        Ok(self)
    }

    #[cfg(feature = "alloc")]
    #[inline]
    fn to_string_lossy(&self) -> Cow<'_, str> {
        Cow::Borrowed(self)
    }

    #[cfg(feature = "alloc")]
    #[inline]
    fn as_cow_c_str(&self) -> io::Result<Cow<'_, CStr>> {
        Ok(Cow::Owned(
            CString::new(*self).map_err(|_cstr_err| io::Errno::INVAL)?,
        ))
    }

    #[cfg(feature = "alloc")]
    #[inline]
    fn into_c_str<'b>(self) -> io::Result<Cow<'b, CStr>>
    where
        Self: 'b,
    {
        Ok(Cow::Owned(
            CString::new(self).map_err(|_cstr_err| io::Errno::INVAL)?,
        ))
    }

    #[inline]
    fn into_with_c_str<T, F>(self, f: F) -> io::Result<T>
    where
        Self: Sized,
        F: FnOnce(&CStr) -> io::Result<T>,
    {
        with_c_str(self.as_bytes(), f)
    }
}

#[cfg(feature = "alloc")]
impl Arg for &String {
    #[inline]
    fn as_str(&self) -> io::Result<&str> {
        Ok(self)
    }

    #[cfg(feature = "alloc")]
    #[inline]
    fn to_string_lossy(&self) -> Cow<'_, str> {
        Cow::Borrowed(self)
    }

    #[cfg(feature = "alloc")]
    #[inline]
    fn as_cow_c_str(&self) -> io::Result<Cow<'_, CStr>> {
        Ok(Cow::Owned(
            CString::new(String::as_str(self)).map_err(|_cstr_err| io::Errno::INVAL)?,
        ))
    }

    #[cfg(feature = "alloc")]
    #[inline]
    fn into_c_str<'b>(self) -> io::Result<Cow<'b, CStr>>
    where
        Self: 'b,
    {
        self.as_str().into_c_str()
    }

    #[inline]
    fn into_with_c_str<T, F>(self, f: F) -> io::Result<T>
    where
        Self: Sized,
        F: FnOnce(&CStr) -> io::Result<T>,
    {
        with_c_str(self.as_bytes(), f)
    }
}

#[cfg(feature = "alloc")]
impl Arg for String {
    #[inline]
    fn as_str(&self) -> io::Result<&str> {
        Ok(self)
    }

    #[cfg(feature = "alloc")]
    #[inline]
    fn to_string_lossy(&self) -> Cow<'_, str> {
        Cow::Borrowed(self)
    }

    #[cfg(feature = "alloc")]
    #[inline]
    fn as_cow_c_str(&self) -> io::Result<Cow<'_, CStr>> {
        Ok(Cow::Owned(
            CString::new(self.as_str()).map_err(|_cstr_err| io::Errno::INVAL)?,
        ))
    }

    #[cfg(feature = "alloc")]
    #[inline]
    fn into_c_str<'b>(self) -> io::Result<Cow<'b, CStr>>
    where
        Self: 'b,
    {
        Ok(Cow::Owned(
            CString::new(self).map_err(|_cstr_err| io::Errno::INVAL)?,
        ))
    }

    #[inline]
    fn into_with_c_str<T, F>(self, f: F) -> io::Result<T>
    where
        Self: Sized,
        F: FnOnce(&CStr) -> io::Result<T>,
    {
        f(&CString::new(self).map_err(|_cstr_err| io::Errno::INVAL)?)
    }
}

#[cfg(feature = "std")]
#[cfg(any(not(target_os = "wasi"), not(target_env = "p2"), wasip2))]
impl Arg for &OsStr {
    #[inline]
    fn as_str(&self) -> io::Result<&str> {
        self.to_str().ok_or(io::Errno::INVAL)
    }

    #[inline]
    fn to_string_lossy(&self) -> Cow<'_, str> {
        OsStr::to_string_lossy(self)
    }

    #[inline]
    fn as_cow_c_str(&self) -> io::Result<Cow<'_, CStr>> {
        Ok(Cow::Owned(
            CString::new(self.as_bytes()).map_err(|_cstr_err| io::Errno::INVAL)?,
        ))
    }

    #[inline]
    fn into_c_str<'b>(self) -> io::Result<Cow<'b, CStr>>
    where
        Self: 'b,
    {
        Ok(Cow::Owned(
            CString::new(self.as_bytes()).map_err(|_cstr_err| io::Errno::INVAL)?,
        ))
    }

    #[inline]
    fn into_with_c_str<T, F>(self, f: F) -> io::Result<T>
    where
        Self: Sized,
        F: FnOnce(&CStr) -> io::Result<T>,
    {
        with_c_str(self.as_bytes(), f)
    }
}

#[cfg(feature = "std")]
#[cfg(any(not(target_os = "wasi"), not(target_env = "p2"), wasip2))]
impl Arg for &OsString {
    #[inline]
    fn as_str(&self) -> io::Result<&str> {
        OsString::as_os_str(self).to_str().ok_or(io::Errno::INVAL)
    }

    #[inline]
    fn to_string_lossy(&self) -> Cow<'_, str> {
        self.as_os_str().to_string_lossy()
    }

    #[inline]
    fn as_cow_c_str(&self) -> io::Result<Cow<'_, CStr>> {
        Ok(Cow::Owned(
            CString::new(OsString::as_os_str(self).as_bytes())
                .map_err(|_cstr_err| io::Errno::INVAL)?,
        ))
    }

    #[inline]
    fn into_c_str<'b>(self) -> io::Result<Cow<'b, CStr>>
    where
        Self: 'b,
    {
        self.as_os_str().into_c_str()
    }

    #[inline]
    fn into_with_c_str<T, F>(self, f: F) -> io::Result<T>
    where
        Self: Sized,
        F: FnOnce(&CStr) -> io::Result<T>,
    {
        with_c_str(self.as_bytes(), f)
    }
}

#[cfg(feature = "std")]
#[cfg(any(not(target_os = "wasi"), not(target_env = "p2"), wasip2))]
impl Arg for OsString {
    #[inline]
    fn as_str(&self) -> io::Result<&str> {
        self.as_os_str().to_str().ok_or(io::Errno::INVAL)
    }

    #[inline]
    fn to_string_lossy(&self) -> Cow<'_, str> {
        self.as_os_str().to_string_lossy()
    }

    #[inline]
    fn as_cow_c_str(&self) -> io::Result<Cow<'_, CStr>> {
        Ok(Cow::Owned(
            CString::new(self.as_bytes()).map_err(|_cstr_err| io::Errno::INVAL)?,
        ))
    }

    #[inline]
    fn into_c_str<'b>(self) -> io::Result<Cow<'b, CStr>>
    where
        Self: 'b,
    {
        Ok(Cow::Owned(
            CString::new(self.into_vec()).map_err(|_cstr_err| io::Errno::INVAL)?,
        ))
    }

    #[inline]
    fn into_with_c_str<T, F>(self, f: F) -> io::Result<T>
    where
        Self: Sized,
        F: FnOnce(&CStr) -> io::Result<T>,
    {
        f(&CString::new(self.into_vec()).map_err(|_cstr_err| io::Errno::INVAL)?)
    }
}

#[cfg(feature = "std")]
#[cfg(any(not(target_os = "wasi"), not(target_env = "p2"), wasip2))]
impl Arg for &Path {
    #[inline]
    fn as_str(&self) -> io::Result<&str> {
        self.as_os_str().to_str().ok_or(io::Errno::INVAL)
    }

    #[inline]
    fn to_string_lossy(&self) -> Cow<'_, str> {
        Path::to_string_lossy(self)
    }

    #[inline]
    fn as_cow_c_str(&self) -> io::Result<Cow<'_, CStr>> {
        Ok(Cow::Owned(
            CString::new(self.as_os_str().as_bytes()).map_err(|_cstr_err| io::Errno::INVAL)?,
        ))
    }

    #[inline]
    fn into_c_str<'b>(self) -> io::Result<Cow<'b, CStr>>
    where
        Self: 'b,
    {
        Ok(Cow::Owned(
            CString::new(self.as_os_str().as_bytes()).map_err(|_cstr_err| io::Errno::INVAL)?,
        ))
    }

    #[inline]
    fn into_with_c_str<T, F>(self, f: F) -> io::Result<T>
    where
        Self: Sized,
        F: FnOnce(&CStr) -> io::Result<T>,
    {
        with_c_str(self.as_os_str().as_bytes(), f)
    }
}

#[cfg(feature = "std")]
#[cfg(any(not(target_os = "wasi"), not(target_env = "p2"), wasip2))]
impl Arg for &PathBuf {
    #[inline]
    fn as_str(&self) -> io::Result<&str> {
        PathBuf::as_path(self)
            .as_os_str()
            .to_str()
            .ok_or(io::Errno::INVAL)
    }

    #[inline]
    fn to_string_lossy(&self) -> Cow<'_, str> {
        self.as_path().to_string_lossy()
    }

    #[inline]
    fn as_cow_c_str(&self) -> io::Result<Cow<'_, CStr>> {
        Ok(Cow::Owned(
            CString::new(PathBuf::as_path(self).as_os_str().as_bytes())
                .map_err(|_cstr_err| io::Errno::INVAL)?,
        ))
    }

    #[inline]
    fn into_c_str<'b>(self) -> io::Result<Cow<'b, CStr>>
    where
        Self: 'b,
    {
        self.as_path().into_c_str()
    }

    #[inline]
    fn into_with_c_str<T, F>(self, f: F) -> io::Result<T>
    where
        Self: Sized,
        F: FnOnce(&CStr) -> io::Result<T>,
    {
        with_c_str(self.as_os_str().as_bytes(), f)
    }
}

#[cfg(feature = "std")]
#[cfg(any(not(target_os = "wasi"), not(target_env = "p2"), wasip2))]
impl Arg for PathBuf {
    #[inline]
    fn as_str(&self) -> io::Result<&str> {
        self.as_os_str().to_str().ok_or(io::Errno::INVAL)
    }

    #[inline]
    fn to_string_lossy(&self) -> Cow<'_, str> {
        self.as_os_str().to_string_lossy()
    }

    #[inline]
    fn as_cow_c_str(&self) -> io::Result<Cow<'_, CStr>> {
        Ok(Cow::Owned(
            CString::new(self.as_os_str().as_bytes()).map_err(|_cstr_err| io::Errno::INVAL)?,
        ))
    }

    #[inline]
    fn into_c_str<'b>(self) -> io::Result<Cow<'b, CStr>>
    where
        Self: 'b,
    {
        Ok(Cow::Owned(
            CString::new(self.into_os_string().into_vec()).map_err(|_cstr_err| io::Errno::INVAL)?,
        ))
    }

    #[inline]
    fn into_with_c_str<T, F>(self, f: F) -> io::Result<T>
    where
        Self: Sized,
        F: FnOnce(&CStr) -> io::Result<T>,
    {
        f(
            &CString::new(self.into_os_string().into_vec())
                .map_err(|_cstr_err| io::Errno::INVAL)?,
        )
    }
}

impl Arg for &CStr {
    #[inline]
    fn as_str(&self) -> io::Result<&str> {
        self.to_str().map_err(|_utf8_err| io::Errno::INVAL)
    }

    #[cfg(feature = "alloc")]
    #[inline]
    fn to_string_lossy(&self) -> Cow<'_, str> {
        CStr::to_string_lossy(self)
    }

    #[cfg(feature = "alloc")]
    #[inline]
    fn as_cow_c_str(&self) -> io::Result<Cow<'_, CStr>> {
        Ok(Cow::Borrowed(self))
    }

    #[cfg(feature = "alloc")]
    #[inline]
    fn into_c_str<'b>(self) -> io::Result<Cow<'b, CStr>>
    where
        Self: 'b,
    {
        Ok(Cow::Borrowed(self))
    }

    #[inline]
    fn into_with_c_str<T, F>(self, f: F) -> io::Result<T>
    where
        Self: Sized,
        F: FnOnce(&CStr) -> io::Result<T>,
    {
        f(self)
    }
}

#[cfg(feature = "alloc")]
impl Arg for &CString {
    #[inline]
    fn as_str(&self) -> io::Result<&str> {
        unimplemented!()
    }

    #[inline]
    fn to_string_lossy(&self) -> Cow<'_, str> {
        unimplemented!()
    }

    #[inline]
    fn as_cow_c_str(&self) -> io::Result<Cow<'_, CStr>> {
        Ok(Cow::Borrowed(self))
    }

    #[inline]
    fn into_c_str<'b>(self) -> io::Result<Cow<'b, CStr>>
    where
        Self: 'b,
    {
        Ok(Cow::Borrowed(self))
    }

    #[inline]
    fn into_with_c_str<T, F>(self, f: F) -> io::Result<T>
    where
        Self: Sized,
        F: FnOnce(&CStr) -> io::Result<T>,
    {
        f(self)
    }
}

#[cfg(feature = "alloc")]
impl Arg for CString {
    #[inline]
    fn as_str(&self) -> io::Result<&str> {
        self.to_str().map_err(|_utf8_err| io::Errno::INVAL)
    }

    #[inline]
    fn to_string_lossy(&self) -> Cow<'_, str> {
        CStr::to_string_lossy(self)
    }

    #[inline]
    fn as_cow_c_str(&self) -> io::Result<Cow<'_, CStr>> {
        Ok(Cow::Borrowed(self))
    }

    #[inline]
    fn into_c_str<'b>(self) -> io::Result<Cow<'b, CStr>>
    where
        Self: 'b,
    {
        Ok(Cow::Owned(self))
    }

    #[inline]
    fn into_with_c_str<T, F>(self, f: F) -> io::Result<T>
    where
        Self: Sized,
        F: FnOnce(&CStr) -> io::Result<T>,
    {
        f(&self)
    }
}

#[cfg(feature = "alloc")]
impl<'a> Arg for Cow<'a, str> {
    #[inline]
    fn as_str(&self) -> io::Result<&str> {
        Ok(self)
    }

    #[inline]
    fn to_string_lossy(&self) -> Cow<'_, str> {
        Cow::Borrowed(self)
    }

    #[inline]
    fn as_cow_c_str(&self) -> io::Result<Cow<'_, CStr>> {
        Ok(Cow::Owned(
            CString::new(self.as_ref()).map_err(|_cstr_err| io::Errno::INVAL)?,
        ))
    }

    #[inline]
    fn into_c_str<'b>(self) -> io::Result<Cow<'b, CStr>>
    where
        Self: 'b,
    {
        Ok(Cow::Owned(
            match self {
                Cow::Owned(s) => CString::new(s),
                Cow::Borrowed(s) => CString::new(s),
            }
            .map_err(|_cstr_err| io::Errno::INVAL)?,
        ))
    }

    #[inline]
    fn into_with_c_str<T, F>(self, f: F) -> io::Result<T>
    where
        Self: Sized,
        F: FnOnce(&CStr) -> io::Result<T>,
    {
        with_c_str(self.as_bytes(), f)
    }
}

#[cfg(feature = "std")]
#[cfg(any(not(target_os = "wasi"), not(target_env = "p2"), wasip2))]
impl<'a> Arg for Cow<'a, OsStr> {
    #[inline]
    fn as_str(&self) -> io::Result<&str> {
        (**self).to_str().ok_or(io::Errno::INVAL)
    }

    #[inline]
    fn to_string_lossy(&self) -> Cow<'_, str> {
        (**self).to_string_lossy()
    }

    #[inline]
    fn as_cow_c_str(&self) -> io::Result<Cow<'_, CStr>> {
        Ok(Cow::Owned(
            CString::new(self.as_bytes()).map_err(|_cstr_err| io::Errno::INVAL)?,
        ))
    }

    #[inline]
    fn into_c_str<'b>(self) -> io::Result<Cow<'b, CStr>>
    where
        Self: 'b,
    {
        Ok(Cow::Owned(
            match self {
                Cow::Owned(os) => CString::new(os.into_vec()),
                Cow::Borrowed(os) => CString::new(os.as_bytes()),
            }
            .map_err(|_cstr_err| io::Errno::INVAL)?,
        ))
    }

    #[inline]
    fn into_with_c_str<T, F>(self, f: F) -> io::Result<T>
    where
        Self: Sized,
        F: FnOnce(&CStr) -> io::Result<T>,
    {
        with_c_str(self.as_bytes(), f)
    }
}

#[cfg(feature = "alloc")]
impl<'a> Arg for Cow<'a, CStr> {
    #[inline]
    fn as_str(&self) -> io::Result<&str> {
        self.to_str().map_err(|_utf8_err| io::Errno::INVAL)
    }

    #[inline]
    fn to_string_lossy(&self) -> Cow<'_, str> {
        let borrow: &CStr = core::borrow::Borrow::borrow(self);
        borrow.to_string_lossy()
    }

    #[inline]
    fn as_cow_c_str(&self) -> io::Result<Cow<'_, CStr>> {
        Ok(Cow::Borrowed(self))
    }

    #[inline]
    fn into_c_str<'b>(self) -> io::Result<Cow<'b, CStr>>
    where
        Self: 'b,
    {
        Ok(self)
    }

    #[inline]
    fn into_with_c_str<T, F>(self, f: F) -> io::Result<T>
    where
        Self: Sized,
        F: FnOnce(&CStr) -> io::Result<T>,
    {
        f(&self)
    }
}

#[cfg(feature = "std")]
#[cfg(any(not(target_os = "wasi"), not(target_env = "p2"), wasip2))]
impl<'a> Arg for Component<'a> {
    #[inline]
    fn as_str(&self) -> io::Result<&str> {
        self.as_os_str().to_str().ok_or(io::Errno::INVAL)
    }

    #[inline]
    fn to_string_lossy(&self) -> Cow<'_, str> {
        self.as_os_str().to_string_lossy()
    }

    #[inline]
    fn as_cow_c_str(&self) -> io::Result<Cow<'_, CStr>> {
        Ok(Cow::Owned(
            CString::new(self.as_os_str().as_bytes()).map_err(|_cstr_err| io::Errno::INVAL)?,
        ))
    }

    #[inline]
    fn into_c_str<'b>(self) -> io::Result<Cow<'b, CStr>>
    where
        Self: 'b,
    {
        Ok(Cow::Owned(
            CString::new(self.as_os_str().as_bytes()).map_err(|_cstr_err| io::Errno::INVAL)?,
        ))
    }

    #[inline]
    fn into_with_c_str<T, F>(self, f: F) -> io::Result<T>
    where
        Self: Sized,
        F: FnOnce(&CStr) -> io::Result<T>,
    {
        with_c_str(self.as_os_str().as_bytes(), f)
    }
}

#[cfg(feature = "std")]
#[cfg(any(not(target_os = "wasi"), not(target_env = "p2"), wasip2))]
impl<'a> Arg for Components<'a> {
    #[inline]
    fn as_str(&self) -> io::Result<&str> {
        self.as_path().to_str().ok_or(io::Errno::INVAL)
    }

    #[inline]
    fn to_string_lossy(&self) -> Cow<'_, str> {
        self.as_path().to_string_lossy()
    }

    #[inline]
    fn as_cow_c_str(&self) -> io::Result<Cow<'_, CStr>> {
        Ok(Cow::Owned(
            CString::new(self.as_path().as_os_str().as_bytes())
                .map_err(|_cstr_err| io::Errno::INVAL)?,
        ))
    }

    #[inline]
    fn into_c_str<'b>(self) -> io::Result<Cow<'b, CStr>>
    where
        Self: 'b,
    {
        Ok(Cow::Owned(
            CString::new(self.as_path().as_os_str().as_bytes())
                .map_err(|_cstr_err| io::Errno::INVAL)?,
        ))
    }

    #[inline]
    fn into_with_c_str<T, F>(self, f: F) -> io::Result<T>
    where
        Self: Sized,
        F: FnOnce(&CStr) -> io::Result<T>,
    {
        with_c_str(self.as_path().as_os_str().as_bytes(), f)
    }
}

#[cfg(feature = "std")]
#[cfg(any(not(target_os = "wasi"), not(target_env = "p2"), wasip2))]
impl<'a> Arg for Iter<'a> {
    #[inline]
    fn as_str(&self) -> io::Result<&str> {
        self.as_path().to_str().ok_or(io::Errno::INVAL)
    }

    #[inline]
    fn to_string_lossy(&self) -> Cow<'_, str> {
        self.as_path().to_string_lossy()
    }

    #[inline]
    fn as_cow_c_str(&self) -> io::Result<Cow<'_, CStr>> {
        Ok(Cow::Owned(
            CString::new(self.as_path().as_os_str().as_bytes())
                .map_err(|_cstr_err| io::Errno::INVAL)?,
        ))
    }

    #[inline]
    fn into_c_str<'b>(self) -> io::Result<Cow<'b, CStr>>
    where
        Self: 'b,
    {
        Ok(Cow::Owned(
            CString::new(self.as_path().as_os_str().as_bytes())
                .map_err(|_cstr_err| io::Errno::INVAL)?,
        ))
    }

    #[inline]
    fn into_with_c_str<T, F>(self, f: F) -> io::Result<T>
    where
        Self: Sized,
        F: FnOnce(&CStr) -> io::Result<T>,
    {
        with_c_str(self.as_path().as_os_str().as_bytes(), f)
    }
}

impl Arg for &[u8] {
    #[inline]
    fn as_str(&self) -> io::Result<&str> {
        str::from_utf8(self).map_err(|_utf8_err| io::Errno::INVAL)
    }

    #[cfg(feature = "alloc")]
    #[inline]
    fn to_string_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(self)
    }

    #[cfg(feature = "alloc")]
    #[inline]
    fn as_cow_c_str(&self) -> io::Result<Cow<'_, CStr>> {
        Ok(Cow::Owned(
            CString::new(*self).map_err(|_cstr_err| io::Errno::INVAL)?,
        ))
    }

    #[cfg(feature = "alloc")]
    #[inline]
    fn into_c_str<'b>(self) -> io::Result<Cow<'b, CStr>>
    where
        Self: 'b,
    {
        Ok(Cow::Owned(
            CString::new(self).map_err(|_cstr_err| io::Errno::INVAL)?,
        ))
    }

    #[inline]
    fn into_with_c_str<T, F>(self, f: F) -> io::Result<T>
    where
        Self: Sized,
        F: FnOnce(&CStr) -> io::Result<T>,
    {
        with_c_str(self, f)
    }
}

#[cfg(feature = "alloc")]
impl Arg for &Vec<u8> {
    #[inline]
    fn as_str(&self) -> io::Result<&str> {
        str::from_utf8(self).map_err(|_utf8_err| io::Errno::INVAL)
    }

    #[cfg(feature = "alloc")]
    #[inline]
    fn to_string_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(self)
    }

    #[cfg(feature = "alloc")]
    #[inline]
    fn as_cow_c_str(&self) -> io::Result<Cow<'_, CStr>> {
        Ok(Cow::Owned(
            CString::new(self.as_slice()).map_err(|_cstr_err| io::Errno::INVAL)?,
        ))
    }

    #[cfg(feature = "alloc")]
    #[inline]
    fn into_c_str<'b>(self) -> io::Result<Cow<'b, CStr>>
    where
        Self: 'b,
    {
        Ok(Cow::Owned(
            CString::new(self.as_slice()).map_err(|_cstr_err| io::Errno::INVAL)?,
        ))
    }

    #[inline]
    fn into_with_c_str<T, F>(self, f: F) -> io::Result<T>
    where
        Self: Sized,
        F: FnOnce(&CStr) -> io::Result<T>,
    {
        with_c_str(self, f)
    }
}

#[cfg(feature = "alloc")]
#[cfg(any(not(target_os = "wasi"), not(target_env = "p2"), wasip2))]
impl Arg for Vec<u8> {
    #[inline]
    fn as_str(&self) -> io::Result<&str> {
        str::from_utf8(self).map_err(|_utf8_err| io::Errno::INVAL)
    }

    #[cfg(feature = "alloc")]
    #[inline]
    fn to_string_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(self)
    }

    #[cfg(feature = "alloc")]
    #[inline]
    fn as_cow_c_str(&self) -> io::Result<Cow<'_, CStr>> {
        Ok(Cow::Owned(
            CString::new(self.as_slice()).map_err(|_cstr_err| io::Errno::INVAL)?,
        ))
    }

    #[cfg(feature = "alloc")]
    #[inline]
    fn into_c_str<'b>(self) -> io::Result<Cow<'b, CStr>>
    where
        Self: 'b,
    {
        Ok(Cow::Owned(
            CString::new(self).map_err(|_cstr_err| io::Errno::INVAL)?,
        ))
    }

    #[inline]
    fn into_with_c_str<T, F>(self, f: F) -> io::Result<T>
    where
        Self: Sized,
        F: FnOnce(&CStr) -> io::Result<T>,
    {
        f(&CString::new(self).map_err(|_cstr_err| io::Errno::INVAL)?)
    }
}

impl Arg for DecInt {
    #[inline]
    fn as_str(&self) -> io::Result<&str> {
        Ok(self.as_str())
    }

    #[cfg(feature = "alloc")]
    #[inline]
    fn to_string_lossy(&self) -> Cow<'_, str> {
        Cow::Borrowed(self.as_str())
    }

    #[cfg(feature = "alloc")]
    #[inline]
    fn as_cow_c_str(&self) -> io::Result<Cow<'_, CStr>> {
        Ok(Cow::Borrowed(self.as_c_str()))
    }

    #[cfg(feature = "alloc")]
    #[inline]
    fn into_c_str<'b>(self) -> io::Result<Cow<'b, CStr>>
    where
        Self: 'b,
    {
        Ok(Cow::Owned(self.as_c_str().to_owned()))
    }

    #[inline]
    fn into_with_c_str<T, F>(self, f: F) -> io::Result<T>
    where
        Self: Sized,
        F: FnOnce(&CStr) -> io::Result<T>,
    {
        f(self.as_c_str())
    }
}

/// Runs a closure with `bytes` passed in as a `&CStr`.
#[allow(unsafe_code, clippy::int_plus_one)]
#[inline]
fn with_c_str<T, F>(bytes: &[u8], f: F) -> io::Result<T>
where
    F: FnOnce(&CStr) -> io::Result<T>,
{
    // Most paths are less than `SMALL_PATH_BUFFER_SIZE` long. The rest can go
    // through the dynamic allocation path. If you're opening many files in a
    // directory with a long path, consider opening the directory and using
    // `openat` to open the files under it, which will avoid this, and is often
    // faster in the OS as well.

    // Test with `>=` so that we have room for the trailing NUL.
    if bytes.len() >= SMALL_PATH_BUFFER_SIZE {
        return with_c_str_slow_path(bytes, f);
    }

    // Taken from
    // <https://github.com/rust-lang/rust/blob/a00f8ba7fcac1b27341679c51bf5a3271fa82df3/library/std/src/sys/common/small_c_string.rs>
    let mut buf = MaybeUninit::<[u8; SMALL_PATH_BUFFER_SIZE]>::uninit();
    let buf_ptr = buf.as_mut_ptr().cast::<u8>();

    // This helps test our safety condition below.
    debug_assert!(bytes.len() + 1 <= SMALL_PATH_BUFFER_SIZE);

    // SAFETY: `bytes.len() < SMALL_PATH_BUFFER_SIZE` which means we have space
    // for `bytes.len() + 1` `u8`s:
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), buf_ptr, bytes.len());
        buf_ptr.add(bytes.len()).write(b'\0');
    }

    // SAFETY: We just wrote the bytes above and they will remain valid for the
    // duration of `f` because `buf` doesn't get dropped until the end of the
    // function.
    match CStr::from_bytes_with_nul(unsafe { slice::from_raw_parts(buf_ptr, bytes.len() + 1) }) {
        Ok(s) => f(s),
        Err(_) => Err(io::Errno::INVAL),
    }
}

/// The slow path which handles any length. In theory OS's only support up to
/// `PATH_MAX`, but we let the OS enforce that.
#[allow(unsafe_code, clippy::int_plus_one)]
#[cold]
fn with_c_str_slow_path<T, F>(bytes: &[u8], f: F) -> io::Result<T>
where
    F: FnOnce(&CStr) -> io::Result<T>,
{
    #[cfg(feature = "alloc")]
    {
        f(&CString::new(bytes).map_err(|_cstr_err| io::Errno::INVAL)?)
    }

    #[cfg(not(feature = "alloc"))]
    {
        #[cfg(all(
            libc,
            not(any(
                target_os = "espidf",
                target_os = "horizon",
                target_os = "hurd",
                target_os = "vita",
                target_os = "wasi"
            ))
        ))]
        const LARGE_PATH_BUFFER_SIZE: usize = libc::PATH_MAX as usize;
        #[cfg(linux_raw)]
        const LARGE_PATH_BUFFER_SIZE: usize = linux_raw_sys::general::PATH_MAX as usize;
        #[cfg(any(
            target_os = "espidf",
            target_os = "horizon",
            target_os = "hurd",
            target_os = "vita",
            target_os = "wasi"
        ))]
        const LARGE_PATH_BUFFER_SIZE: usize = 4096 as usize; // TODO: upstream this

        // Taken from
        // <https://github.com/rust-lang/rust/blob/a00f8ba7fcac1b27341679c51bf5a3271fa82df3/library/std/src/sys/common/small_c_string.rs>
        let mut buf = MaybeUninit::<[u8; LARGE_PATH_BUFFER_SIZE]>::uninit();
        let buf_ptr = buf.as_mut_ptr().cast::<u8>();

        // This helps test our safety condition below.
        if bytes.len() + 1 > LARGE_PATH_BUFFER_SIZE {
            return Err(io::Errno::NAMETOOLONG);
        }

        // SAFETY: `bytes.len() < LARGE_PATH_BUFFER_SIZE` which means we have
        // space for `bytes.len() + 1` `u8`s:
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), buf_ptr, bytes.len());
            buf_ptr.add(bytes.len()).write(b'\0');
        }

        // SAFETY: We just wrote the bytes above and they will remain valid for
        // the duration of `f` because `buf` doesn't get dropped until the end
        // of the function.
        match CStr::from_bytes_with_nul(unsafe { slice::from_raw_parts(buf_ptr, bytes.len() + 1) })
        {
            Ok(s) => f(s),
            Err(_) => Err(io::Errno::INVAL),
        }
    }
}
