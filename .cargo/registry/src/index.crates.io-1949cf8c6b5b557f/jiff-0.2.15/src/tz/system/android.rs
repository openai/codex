use std::{
    ffi::{c_char, c_void},
    sync::OnceLock,
};

use alloc::vec::Vec;

use core::{ffi::CStr, mem, ptr::NonNull};

use crate::tz::{TimeZone, TimeZoneDatabase};

/// Attempts to find the default "system" time zone.
pub(super) fn get(db: &TimeZoneDatabase) -> Option<TimeZone> {
    static PROPERTY_NAME: &str = "persist.sys.timezone\0";

    static GETTER: OnceLock<Option<PropertyGetter>> = OnceLock::new();
    let Some(getter) = GETTER.get_or_init(|| PropertyGetter::new()) else {
        // We don't emit any messages here because `PropertyGetter::new()` will
        // have already done so.
        return None;
    };
    let tzname = getter.get(cstr(PROPERTY_NAME))?;
    let Some(tzname) = core::str::from_utf8(&tzname).ok() else {
        warn!(
            "found `{PROPERTY_NAME}` name `{name}` on Android, \
             but it's not valid UTF-8",
            name = crate::util::escape::Bytes(&tzname),
        );
        return None;
    };
    let tz = match db.get(tzname) {
        Ok(tz) => tz,
        Err(_err) => {
            warn!(
                "found `{PROPERTY_NAME}` name `{tzname}` on Android, \
                 but could not find it in time zone database {db:?}",
            );
            return None;
        }
    };
    debug!(
        "found system time zone `{tzname}` from Android property \
         `{PROPERTY_NAME}` and found entry for it in time zone \
         database {db:?}",
    );
    Some(tz)
}

/// Given a path to a system default TZif file, return its corresponding
/// time zone.
///
/// This doesn't do any symlink shenanigans like in other Unix environments,
/// although we could consider doing that. I think probably this is very
/// unlikely to be used on Android, although it can be by setting `TZ`.
pub(super) fn read(_db: &TimeZoneDatabase, path: &str) -> Option<TimeZone> {
    match super::read_unnamed_tzif_file(path) {
        Ok(tz) => Some(tz),
        Err(_err) => {
            trace!("failed to read {path} as unnamed time zone: {_err}");
            None
        }
    }
}

/// An abstraction for safely reading Android system properties.
///
/// Initialization of this should only be done once. Initialization
/// dynamically loads `libc`. But it does not read any properties. Instead,
/// we permit the time zone property to be looked up repeatedly, in case it
/// changes. So, our `libc` library handle remains invariant, but the values
/// of properties can change over the lifetime of the process.
///
/// This copies the technique used by the `android-system-properties` crate.
/// Namely, we use `dlopen` instead of hard-coding our linking requirements
/// since this is apparently more flexible. I guess in the past, the hard-coded
/// extern functions broke, but this technique doesn't. Or at least, it "fails
/// gracefully" since this won't result in build errors but just runtime
/// errors that result in no time zone being found (and thus will result in an
/// automatic fallback to UTC).
///
/// Our implementation of this idea is perhaps a bit simpler than what
/// `android-system-properties` does though. We don't bother supporting >10
/// year old versions of Android. (Although support for that could be added
/// if there was a real need.) Also, when this fails, we give better error
/// messages via `dlerror()` in the logs.
struct PropertyGetter {
    /// A `dlopen` handle to `libc.so`.
    ///
    /// Note that since this is a bespoke property getter and we only ever
    /// create a single instance in a process global static, this never gets
    /// dropped. So we don't bother writing a `Drop` impl that calls `dlclose`.
    /// Because it never gets dropped and because we load the symbols right
    /// away at construction time, we never actually end up using it after
    /// construction. We keep it around in case we want to refactor to this
    /// to actually drop it for some reason.
    _libc: NonNull<c_void>,
    system_property_find: SystemPropertyFind,
    system_property_read: SystemPropertyRead,
}

// SAFETY: It is presumably safe to call functions derived from `dlsym`
// symbols from multiple threads simultaneously. And it is presumably safe
// to call Android's property getter APIs from multiple threads simultaneously.
// This isn't technically documented (as far as I can see), but it would be
// crazytown if this weren't true.
unsafe impl Send for PropertyGetter {}
// SAFETY: It is presumably safe to call functions derived from `dlsym`
// symbols from multiple threads simultaneously. And it is presumably safe
// to call Android's property getter APIs from multiple threads simultaneously.
// This isn't technically documented (as far as I can see), but it would be
// crazytown if this weren't true.
unsafe impl Sync for PropertyGetter {}

impl PropertyGetter {
    /// Creates a new property getter by `dlopen`'ing `libc.so`.
    ///
    /// If this fails for whatever reason, `None` is returned and WARN-level
    /// log messages are emitted stating the reason for failure if it is known.
    fn new() -> Option<PropertyGetter> {
        // SAFETY: OK because we provide a valid NUL terminated string.
        let handle = unsafe { dlopen(cstr("libc.so\0").as_ptr(), 0) };
        let Some(libc) = NonNull::new(handle) else {
            let _msg = dlerror_message();
            warn!(
                "could not open libc.so via `dlopen`: {err}",
                err = crate::util::escape::Bytes(&_msg),
            );
            return None;
        };

        // SAFETY: Our `SystemPropertyFind` type definition matches what is
        // declared in `include/sys/system_properties.h` on Android.
        let system_property_find: SystemPropertyFind =
            unsafe { load_symbol(libc, cstr("__system_property_find\0"))? };

        // SAFETY: Our `SystemPropertyRead` type definition matches what is
        // declared in `include/sys/system_properties.h` on Android.
        let system_property_read: SystemPropertyRead = unsafe {
            load_symbol(libc, cstr("__system_property_read_callback\0"))?
        };

        Some(PropertyGetter {
            _libc: libc,
            system_property_find,
            system_property_read,
        })
    }

    /// Reads the given property name into the `Vec<u8>` returned.
    ///
    /// If the property doesn't exist, then `None` is returned and a WARN-level
    /// log message is emitted explaining why.
    fn get(&self, name: &CStr) -> Option<Vec<u8>> {
        unsafe extern "C" fn callback(
            buf: *mut c_void,
            _name: *const c_char,
            value: *const c_char,
            _serial: u32,
        ) {
            let buf = buf.cast::<Vec<u8>>();
            // SAFETY: The implied contract of `__system_property_read_callback`
            // is that `value` is a valid NUL terminated C string.
            let value = unsafe { CStr::from_ptr(value) };
            // SAFETY: We passed a valid `*mut Vec<u8>` to the callback, so
            // casting it back to it is safe.
            unsafe {
                (*buf).extend_from_slice(value.to_bytes());
            }
        }

        // SAFETY: `name` is a valid NUL terminated string and
        // `system_property_find` is a valid function read from `dlsym`
        // according to the declaration in `include/sys/system_properties.h`.
        let prop_info = unsafe { (self.system_property_find)(name.as_ptr()) };
        if prop_info.is_null() {
            warn!(
                "Android property name `{name}` not found",
                name = crate::util::escape::Bytes(name.to_bytes()),
            );
            return None;
        }

        // N.B. A `prop_info` is an opaque pointer[1]... which means the
        // implementation is probably allocating something in order to create
        // it... right? But there's no API to free the pointer returned. And
        // no other indication as to its lifetime. Once again, C is awful.
        //
        // [1]: https://android.googlesource.com/platform/bionic/+/master/libc/include/sys/system_properties.h#44

        let mut buf = Vec::new();
        // SAFETY: `name` is a valid NUL terminated string and
        // `system_property_find` is a valid function read from `dlsym`
        // according to the declaration in `include/sys/system_properties.h`.
        unsafe {
            let buf: *mut Vec<u8> = &mut buf;
            (self.system_property_read)(
                prop_info,
                callback,
                buf.cast::<c_void>(),
            );
        }
        if buf.is_empty() {
            warn!(
                "reading Android property `{name}` resulted in empty value",
                name = crate::util::escape::Bytes(name.to_bytes()),
            );
            return None;
        }
        Some(buf)
    }
}

/// Loads a function symbol, of type `F`, from the given `dlopen` handle.
///
/// If this fails, then `handle` is closed and `None` is returned and a
/// WARN-level log message is emitted with an error message if possible.
///
/// # Safety
///
/// Callers must ensure that `F` is a function type that matches the ABI of
/// the `symbol` in `handle`.
unsafe fn load_symbol<F>(handle: NonNull<c_void>, symbol: &CStr) -> Option<F> {
    let sym =
        // SAFETY: We know `handle` is non-null.
        unsafe { dlsym(handle.as_ptr(), symbol.as_ptr()) };
    if sym.is_null() {
        // SAFETY: We know `handle` is non-null.
        let _ = unsafe { dlclose(handle.as_ptr()) };
        let _msg = dlerror_message();
        warn!(
            "could not load `{symbol}` \
             symbol from `libc.so: {err}",
            symbol = crate::util::escape::Bytes(symbol.to_bytes()),
            err = crate::util::escape::Bytes(&_msg),
        );
        return None;
    }
    // SAFETY: The safety obligation here is forwarded to the caller. They
    // must guarantee that `F` is an appropriate type.
    // declared in `include/sys/system_properties.h` on Android.
    let function = unsafe { mem::transmute_copy::<*mut c_void, F>(&sym) };
    Some(function)
}

/// Returns the error message given by `dlerror`.
///
/// Callers should only use this when they expect an error to have occurred
/// with one of the `dl*` APIs. If `dlerror` returns a null pointer, then a
/// generic "unknwon error" message is returned.
fn dlerror_message() -> Vec<u8> {
    // SAFETY: I believe `dlerror()` is always safe to call.
    let msg = unsafe { dlerror() };
    if msg.is_null() {
        return b"unknown error".to_vec();
    }
    // SAFETY: We've verified that `msg` is not null and the contract of
    // `dlerror` says that it returns a NUL terminated C string. Moreover,
    // we do not hold on to this string and instead copy it to the heap
    // immediately.
    //
    // One wonders if `dlerror()` is actually sound in this context. While
    // Jiff can guarantee that itself will call `dlerror()` in only one
    // thread, Jiff can't prevent other parts of the process from calling
    // `dlerror()`. In particular, `dlerror(3)` says:
    //
    // > The message returned by dlerror() may reside in a statically allocated
    // > buffer that is overwritten by subsequent dlerror() calls.
    //
    // But no mention is made about whether this statically allocated buffer
    // is written to in a thread safe way. Or whether the string returned to
    // the caller can be unceremoniously overwritten by a simultaneously
    // executing thread.
    //
    // However, in practice, this could be sound if the libc in use is doing
    // something sensible like using a thread local. Then I believe this is
    // fine.
    //
    // My goodness C is awful. If this turns out to be unsound, then since
    // `dlerror()` isn't essential, we'll probably just have to stop using it.
    // So dumb.
    //
    // Note that in theory the error path should never be exercised.
    let cstr = unsafe { CStr::from_ptr(msg) };
    cstr.to_bytes().to_vec()
}

/// Creates a C string "literal" and returns it as a raw pointer.
///
/// This panics if the string given contains a NUL byte.
fn cstr(string: &'static str) -> &'static CStr {
    CStr::from_bytes_with_nul(string.as_bytes()).unwrap()
}

// We just define the FFI bindings ourselves instead of bringing in libc for
// this. We're only doing it for one platform, so it doesn't seem like a huge
// deal. But if this turns out to be a problem in practice, I'm fine accepting
// a target specific dependency on `libc` for Android.
extern "C" {
    fn dlopen(filename: *const c_char, flag: i32) -> *mut c_void;
    fn dlclose(handle: *mut c_void) -> i32;
    fn dlerror() -> *mut c_char;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

// These types come from:
// https://android.googlesource.com/platform/bionic/+/master/libc/include/sys/system_properties.h
type PropInfo = c_void;
type SystemPropertyFind =
    unsafe extern "C" fn(*const c_char) -> *const PropInfo;
type SystemPropertyRead = unsafe extern "C" fn(
    *const PropInfo,
    SystemPropertyReadCallback,
    *mut c_void,
);
type SystemPropertyReadCallback =
    unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char, u32);
