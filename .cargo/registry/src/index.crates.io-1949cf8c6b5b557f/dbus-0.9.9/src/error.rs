use crate::arg::TypeMismatchError;
use std::ffi::CString;
use std::{ptr, fmt};
use crate::{arg, to_c_str, c_str_to_slice, init_dbus, Message};
use crate::strings::ErrorName;
use std::error::Error as stdError;
use std::result::Result as stdResult;

/// Alias for a [`Result`](stdResult) containing [`dbus::Error`](Error) by default.
///
/// Can still be used with different error types.
pub type Result<T, E = Error> = stdResult<T, E>;

/// D-Bus Error wrapper.
///
/// This is a wrapper around the libc dbus error object.
pub struct Error {
    e: ffi::DBusError,
}

unsafe impl Send for Error {}

// Note! For this Sync impl to be safe, it requires that no functions that take &self,
// actually calls into FFI. All functions that call into FFI with a ffi::DBusError
// must take &mut self.

unsafe impl Sync for Error {}

impl Error {

    /// Create a new custom D-Bus Error.
    pub fn new_custom<'a, N: Into<ErrorName<'a>>>(name: N, message: &str) -> Error {
        let n = to_c_str(&name.into());
        let m = to_c_str(&message.replace("%","%%"));
        let mut e = Error::empty();

        unsafe { ffi::dbus_set_error(e.get_mut(), n.as_ptr(), m.as_ptr()) };
        e
    }

    /// Create a new generic D-Bus Error with "org.freedesktop.DBus.Error.Failed" as the Error name.
    pub fn new_failed(message: &str) -> Error {
        Error::new_custom("org.freedesktop.DBus.Error.Failed", message)
    }

    pub (crate) fn empty() -> Error {
        init_dbus();
        let mut e = ffi::DBusError {
            name: ptr::null(),
            message: ptr::null(),
            dummy: 0,
            padding1: ptr::null()
        };
        unsafe { ffi::dbus_error_init(&mut e); }
        Error{ e: e }
    }

    /// Error name/type, e g 'org.freedesktop.DBus.Error.Failed'
    pub fn name(&self) -> Option<&str> {
        c_str_to_slice(&self.e.name)
    }

    /// Custom message, e g 'Could not find a matching object path'
    pub fn message(&self) -> Option<&str> {
        c_str_to_slice(&self.e.message)
    }

    pub (crate) fn get_mut(&mut self) -> &mut ffi::DBusError { &mut self.e }
}

impl Drop for Error {
    fn drop(&mut self) {
        unsafe { ffi::dbus_error_free(&mut self.e); }
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "D-Bus error: {} ({})", self.message().unwrap_or(""),
            self.name().unwrap_or(""))
    }
}

impl stdError for Error {
    fn description(&self) -> &str { "D-Bus error" }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        if let Some(x) = self.message() {
             write!(f, "{}", x)
        } else { Ok(()) }
    }
}

impl From<arg::TypeMismatchError> for Error {
    fn from(t: arg::TypeMismatchError) -> Error {
        Error::new_custom("org.freedesktop.DBus.Error.Failed", &format!("{}", t))
    }
}


impl From<MethodErr> for Error {
    fn from(t: MethodErr) -> Error {
        Error::new_custom(t.errorname(), t.description())
    }
}


#[derive(Clone, Debug, PartialOrd, Ord, PartialEq, Eq)]
/// A D-Bus Method Error, containing an error name and a description.
///
/// Unlike the "Error" struct, this is a Rust native struct.
pub struct MethodErr(ErrorName<'static>, String);

impl MethodErr {
    /// Create an Invalid Args MethodErr.
    pub fn invalid_arg<T: fmt::Debug + ?Sized>(a: &T) -> MethodErr {
        ("org.freedesktop.DBus.Error.InvalidArgs", format!("Invalid argument {:?}", a)).into()
    }
    /// Create a MethodErr that there are not enough arguments given.
    pub fn no_arg() -> MethodErr {
        ("org.freedesktop.DBus.Error.InvalidArgs", "Not enough arguments").into()
    }
    /// Create a MethodErr that the method failed in the way specified.
    pub fn failed<T: fmt::Display + ?Sized>(a: &T) -> MethodErr {
        ("org.freedesktop.DBus.Error.Failed", a.to_string()).into()
    }

    /// Create a MethodErr that the Object path was unknown.
    pub fn no_path<T: fmt::Display + ?Sized>(a: &T) -> MethodErr {
        ("org.freedesktop.DBus.Error.UnknownObject", format!("Unknown object path {}", a)).into()
    }

    /// Create a MethodErr that the Interface was unknown.
    pub fn no_interface<T: fmt::Display + ?Sized>(a: &T) -> MethodErr {
        ("org.freedesktop.DBus.Error.UnknownInterface", format!("Unknown interface {}", a)).into()
    }
    /// Create a MethodErr that the Method was unknown.
    pub fn no_method<T: fmt::Display + ?Sized>(a: &T) -> MethodErr {
        ("org.freedesktop.DBus.Error.UnknownMethod", format!("Unknown method {}", a)).into()
    }
    /// Create a MethodErr that the Property was unknown.
    pub fn no_property<T: fmt::Display + ?Sized>(a: &T) -> MethodErr {
        ("org.freedesktop.DBus.Error.UnknownProperty", format!("Unknown property {}", a)).into()
    }
    /// Create a MethodErr that the Property was read-only.
    pub fn ro_property<T: fmt::Display + ?Sized>(a: &T) -> MethodErr {
        ("org.freedesktop.DBus.Error.PropertyReadOnly", format!("Property {} is read only", a)).into()
    }

    /// Error name accessor
    pub fn errorname(&self) -> &ErrorName<'static> { &self.0 }
    /// Description accessor
    pub fn description(&self) -> &str { &self.1 }

    /// Creates an error reply from a method call message.
    ///
    /// Note: You normally don't need to use this function,
    /// as it is called internally from Tree::handle.
    pub fn to_message(&self, msg: &Message) -> Message {
        msg.error(&self.0, &CString::new(&*self.1).unwrap())
    }
}

impl fmt::Display for MethodErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

impl stdError for MethodErr {}

impl From<TypeMismatchError> for MethodErr {
    fn from(t: TypeMismatchError) -> MethodErr { ("org.freedesktop.DBus.Error.Failed", format!("{}", t)).into() }
}

impl<T: Into<ErrorName<'static>>, M: Into<String>> From<(T, M)> for MethodErr {
    fn from((t, m): (T, M)) -> MethodErr { MethodErr(t.into(), m.into()) }
}

impl From<Error> for MethodErr {
    fn from(t: Error) -> MethodErr {
        let n = t.name().unwrap_or("org.freedesktop.DBus.Error.Failed");
        let m = t.message().unwrap_or("Unknown error");
        MethodErr(String::from(n).into(), m.into())
    }
}
