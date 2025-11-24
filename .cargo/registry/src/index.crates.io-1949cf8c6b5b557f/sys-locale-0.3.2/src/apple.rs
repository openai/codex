use alloc::{string::String, vec::Vec};
use core::ffi::c_void;

type CFIndex = isize;
type Boolean = u8;
type CFStringEncoding = u32;

#[allow(non_upper_case_globals)]
const kCFStringEncodingUTF8: CFStringEncoding = 0x08000100;

#[repr(C)]
#[derive(Clone, Copy)]
struct CFRange {
    pub location: CFIndex,
    pub length: CFIndex,
}

type CFTypeRef = *const c_void;

#[repr(C)]
struct __CFArray(c_void);
type CFArrayRef = *const __CFArray;

#[repr(C)]
struct __CFString(c_void);
type CFStringRef = *const __CFString;

// Most of these definitions come from `core-foundation-sys`, but we want this crate
// to be `no_std` and `core-foundation-sys` isn't currently.
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFArrayGetCount(theArray: CFArrayRef) -> CFIndex;
    fn CFArrayGetValueAtIndex(theArray: CFArrayRef, idx: CFIndex) -> *const c_void;

    fn CFStringGetLength(theString: CFStringRef) -> CFIndex;
    fn CFStringGetBytes(
        theString: CFStringRef,
        range: CFRange,
        encoding: CFStringEncoding,
        lossByte: u8,
        isExternalRepresentation: Boolean,
        buffer: *mut u8,
        maxBufLen: CFIndex,
        usedBufLen: *mut CFIndex,
    ) -> CFIndex;

    fn CFRelease(cf: CFTypeRef);

    fn CFLocaleCopyPreferredLanguages() -> CFArrayRef;
}

pub(crate) fn get() -> impl Iterator<Item = String> {
    let preferred_langs = get_languages();
    let mut idx = 0;

    #[allow(clippy::as_conversions)]
    core::iter::from_fn(move || unsafe {
        let (langs, num_langs) = preferred_langs.as_ref()?;

        // 0 to N-1 inclusive
        if idx >= *num_langs {
            return None;
        }

        // SAFETY: The current index has been checked that its still within bounds of the array.
        // XXX: We don't retain the strings because we know we have total ownership of the backing array.
        let locale = CFArrayGetValueAtIndex(langs.0, idx) as CFStringRef;
        idx += 1;

        // SAFETY: `locale` is a valid CFString pointer because the array will always contain a value.
        let str_len = CFStringGetLength(locale);

        let range = CFRange {
            location: 0,
            length: str_len,
        };

        let mut capacity = 0;
        // SAFETY:
        // - `locale` is a valid CFString
        // - The supplied range is within the length of the string.
        // - `capacity` is writable.
        // Passing NULL and `0` is correct for the buffer to get the
        // encoded output length.
        CFStringGetBytes(
            locale,
            range,
            kCFStringEncodingUTF8,
            0,
            false as Boolean,
            core::ptr::null_mut(),
            0,
            &mut capacity,
        );

        // Guard against a zero-sized allocation, if that were to somehow occur.
        if capacity == 0 {
            return None;
        }

        // Note: This is the number of bytes (u8) that will be written to
        // the buffer, not the number of codepoints they would contain.
        let mut buffer = Vec::with_capacity(capacity as usize);

        // SAFETY:
        // - `locale` is a valid CFString
        // - The supplied range is within the length of the string.
        // - `buffer` is writable and has sufficent capacity to receive the data.
        // - `maxBufLen` is correctly based on `buffer`'s available capacity.
        // - `out_len` is writable.
        let mut out_len = 0;
        CFStringGetBytes(
            locale,
            range,
            kCFStringEncodingUTF8,
            0,
            false as Boolean,
            buffer.as_mut_ptr(),
            capacity as CFIndex,
            &mut out_len,
        );

        // Sanity check that both calls to `CFStringGetBytes`
        // were equivalent. If they weren't, the system is doing
        // something very wrong...
        assert!(out_len <= capacity);

        // SAFETY: The system has written `out_len` elements, so they are
        // initialized and inside the buffer's capacity bounds.
        buffer.set_len(out_len as usize);

        // This should always contain UTF-8 since we told the system to
        // write UTF-8 into the buffer, but the value is small enough that
        // using `from_utf8_unchecked` isn't worthwhile.
        String::from_utf8(buffer).ok()
    })
}

fn get_languages() -> Option<(CFArray, CFIndex)> {
    unsafe {
        // SAFETY: This function is safe to call and has no invariants. Any value inside the
        // array will be owned by us.
        let langs = CFLocaleCopyPreferredLanguages();
        if !langs.is_null() {
            let langs = CFArray(langs);
            // SAFETY: The returned array is a valid CFArray object.
            let count = CFArrayGetCount(langs.0);
            if count != 0 {
                Some((langs, count))
            } else {
                None
            }
        } else {
            None
        }
    }
}

struct CFArray(CFArrayRef);

impl Drop for CFArray {
    fn drop(&mut self) {
        // SAFETY: This wrapper contains a valid CFArray.
        unsafe { CFRelease(self.0.cast()) }
    }
}
