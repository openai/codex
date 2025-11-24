use crate::ffi_utils::buffer::{tzname_buf, MAX_LEN};

pub(crate) fn get_timezone_inner() -> Result<String, crate::GetTimezoneError> {
    get_timezone().ok_or(crate::GetTimezoneError::OsError)
}

#[inline]
fn get_timezone() -> Option<String> {
    let mut buf = tzname_buf();
    // Get system time zone, and borrow its name.
    let tz = system_time_zone::SystemTimeZone::new()?;
    let name = tz.name()?;

    // If the name is encoded in UTF-8, copy it directly.
    let name = if let Some(name) = name.as_utf8() {
        name
    } else {
        // Otherwise convert the name to UTF-8.
        name.to_utf8(&mut buf)?
    };

    if name.is_empty() || name.len() > MAX_LEN {
        // The name should not be empty, or excessively long.
        return None;
    } else {
        Some(name.to_owned())
    }
}

mod system_time_zone {
    //! create a safe wrapper around `CFTimeZoneRef`

    use core_foundation_sys::base::{CFRelease, CFTypeRef};
    use core_foundation_sys::timezone::{
        CFTimeZoneCopySystem, CFTimeZoneGetName, CFTimeZoneRef, CFTimeZoneResetSystem,
    };

    pub(crate) struct SystemTimeZone(CFTimeZoneRef);

    impl Drop for SystemTimeZone {
        fn drop(&mut self) {
            // SAFETY: `SystemTimeZone` is only ever created with a valid `CFTimeZoneRef`.
            unsafe { CFRelease(self.0 as CFTypeRef) };
        }
    }

    impl SystemTimeZone {
        /// Creates a new `SystemTimeZone` by querying the current Darwin system
        /// timezone.
        ///
        /// This function implicitly calls `CFTimeZoneResetSystem` to invalidate
        /// the cached timezone and ensure we always retrieve the current system
        /// timezone.
        ///
        /// Due to CoreFoundation's internal caching mechanism, subsequent calls
        /// to `CFTimeZoneCopySystem` do not reflect system timezone changes
        /// made while the process is running. Thus, we explicitly call
        /// `CFTimeZoneResetSystem` first to invalidate the cached value and
        /// ensure we always retrieve the current system timezone.
        pub(crate) fn new() -> Option<Self> {
            // SAFETY:
            // - Both `CFTimeZoneResetSystem` and `CFTimeZoneCopySystem` are FFI
            //   calls to macOS CoreFoundation.
            // - `CFTimeZoneResetSystem` safely invalidates the cached timezone
            //   without any external invariants.
            // - The pointer returned by `CFTimeZoneCopySystem` is managed and
            //   released properly within `Drop`.
            let v: CFTimeZoneRef = unsafe {
                // First, clear the potentially cached timezone. This call will
                // take the global lock on the timezone data.
                //
                // See <https://github.com/strawlab/iana-time-zone/issues/145#issuecomment-2745934606>
                // for context on why we reset the timezone here.
                CFTimeZoneResetSystem();

                // Fetch the current value. This will likely allocate. This call
                // will again take the global lock on the timezone data.
                CFTimeZoneCopySystem()
            };
            if v.is_null() {
                None
            } else {
                Some(SystemTimeZone(v))
            }
        }

        /// Get the time zone name as a [`StringRef`].
        ///
        /// The lifetime of the `StringRef` is bound to our lifetime. Mutable
        /// access is also prevented by taking a reference to `self`.
        ///
        /// [`StringRef`]: super::string_ref::StringRef
        pub(crate) fn name(&self) -> Option<super::string_ref::StringRef<'_, Self>> {
            // SAFETY: `SystemTimeZone` is only ever created with a valid `CFTimeZoneRef`.
            let string = unsafe { CFTimeZoneGetName(self.0) };
            if string.is_null() {
                None
            } else {
                // SAFETY: here we ensure that `string` is a valid pointer.
                Some(unsafe { super::string_ref::StringRef::new(string, self) })
            }
        }
    }
}

mod string_ref {
    //! create safe wrapper around `CFStringRef`

    use core::convert::TryInto;

    use core_foundation_sys::base::{Boolean, CFRange};
    use core_foundation_sys::string::{
        kCFStringEncodingUTF8, CFStringGetBytes, CFStringGetCStringPtr, CFStringGetLength,
        CFStringRef,
    };

    pub(crate) struct StringRef<'a, T> {
        string: CFStringRef,
        // We exclude mutable access to the parent by taking a reference to the
        // parent (rather than, for example, just using a marker to enforce the
        // parent's lifetime).
        _parent: &'a T,
    }

    impl<'a, T> StringRef<'a, T> {
        // SAFETY: `string` must be valid pointer
        pub(crate) unsafe fn new(string: CFStringRef, _parent: &'a T) -> Self {
            Self { string, _parent }
        }

        pub(crate) fn as_utf8(&self) -> Option<&'a str> {
            // SAFETY: `StringRef` is only ever created with a valid `CFStringRef`.
            let v = unsafe { CFStringGetCStringPtr(self.string, kCFStringEncodingUTF8) };
            if !v.is_null() {
                // SAFETY: `CFStringGetCStringPtr()` returns NUL-terminated
                // strings and will return NULL if the internal representation
                // of the `CFString`` is not compatible with the requested
                // encoding.
                let v = unsafe { std::ffi::CStr::from_ptr(v) };
                if let Ok(v) = v.to_str() {
                    return Some(v);
                }
            }
            None
        }

        pub(crate) fn to_utf8<'b>(&self, buf: &'b mut [u8]) -> Option<&'b str> {
            // SAFETY: `StringRef` is only ever created with a valid `CFStringRef`.
            let length = unsafe { CFStringGetLength(self.string) };

            let mut buf_bytes = 0;
            let range = CFRange {
                location: 0,
                length,
            };

            // SAFETY: `StringRef` is only ever created with a valid `CFStringRef`.
            let converted_bytes = unsafe {
                CFStringGetBytes(
                    self.string,
                    range,
                    kCFStringEncodingUTF8,
                    b'\0',
                    false as Boolean,
                    buf.as_mut_ptr(),
                    buf.len() as isize,
                    &mut buf_bytes,
                )
            };
            if converted_bytes != length {
                return None;
            }

            let len = buf_bytes.try_into().ok()?;
            let s = buf.get(..len)?;
            std::str::from_utf8(s).ok()
        }
    }
}
