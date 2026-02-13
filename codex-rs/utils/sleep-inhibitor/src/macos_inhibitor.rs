use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use core_foundation::string::CFStringRef;
use std::sync::OnceLock;
use tracing::warn;

const MACOS_IDLE_SLEEP_ASSERTION_TYPE: &str = "PreventUserIdleSystemSleep";
const IOKIT_FRAMEWORK_BINARY: &[u8] = b"/System/Library/Frameworks/IOKit.framework/IOKit\0";
const IOPM_ASSERTION_CREATE_WITH_NAME_SYMBOL: &[u8] = b"IOPMAssertionCreateWithName\0";
const IOPM_ASSERTION_RELEASE_SYMBOL: &[u8] = b"IOPMAssertionRelease\0";
const IOKIT_ASSERTION_API_UNAVAILABLE: &str = "IOKit power assertion APIs are unavailable";

#[derive(Debug)]
pub(crate) struct MacSleepAssertion {
    id: IOPMAssertionID,
}

impl MacSleepAssertion {
    pub(crate) fn create(name: &str) -> Result<Self, MacSleepAssertionError> {
        let Some(api) = MacSleepApi::get() else {
            return Err(MacSleepAssertionError::ApiUnavailable(
                IOKIT_ASSERTION_API_UNAVAILABLE,
            ));
        };

        let assertion_type = CFString::new(MACOS_IDLE_SLEEP_ASSERTION_TYPE);
        let assertion_name = CFString::new(name);
        let mut id: IOPMAssertionID = 0;
        let result = unsafe {
            (api.create_with_name)(
                assertion_type.as_concrete_TypeRef(),
                K_IOPM_ASSERTION_LEVEL_ON,
                assertion_name.as_concrete_TypeRef(),
                &mut id,
            )
        };
        if result == K_IORETURN_SUCCESS {
            Ok(Self { id })
        } else {
            Err(MacSleepAssertionError::Iokit(result))
        }
    }
}

impl Drop for MacSleepAssertion {
    fn drop(&mut self) {
        let Some(api) = MacSleepApi::get() else {
            warn!(
                reason = IOKIT_ASSERTION_API_UNAVAILABLE,
                "Failed to release macOS sleep-prevention assertion"
            );
            return;
        };

        let result = unsafe { (api.release)(self.id) };
        if result != K_IORETURN_SUCCESS {
            warn!(
                iokit_error = result,
                "Failed to release macOS sleep-prevention assertion"
            );
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum MacSleepAssertionError {
    ApiUnavailable(&'static str),
    Iokit(IOReturn),
}

type IOPMAssertionCreateWithNameFn = unsafe extern "C" fn(
    assertion_type: CFStringRef,
    assertion_level: IOPMAssertionLevel,
    assertion_name: CFStringRef,
    assertion_id: *mut IOPMAssertionID,
) -> IOReturn;

type IOPMAssertionReleaseFn = unsafe extern "C" fn(assertion_id: IOPMAssertionID) -> IOReturn;

struct MacSleepApi {
    // Keep the dlopen handle alive for the lifetime of the loaded symbols.
    // This prevents accidental dlclose while function pointers are in use.
    _iokit_handle: usize,
    create_with_name: IOPMAssertionCreateWithNameFn,
    release: IOPMAssertionReleaseFn,
}

impl MacSleepApi {
    fn get() -> Option<&'static Self> {
        static API: OnceLock<Option<MacSleepApi>> = OnceLock::new();
        API.get_or_init(Self::load).as_ref()
    }

    fn load() -> Option<Self> {
        let handle = unsafe {
            libc::dlopen(
                IOKIT_FRAMEWORK_BINARY.as_ptr().cast(),
                libc::RTLD_LOCAL | libc::RTLD_LAZY,
            )
        };
        if handle.is_null() {
            warn!(framework = "IOKit", "Failed to open IOKit framework");
            return None;
        }

        let create_with_name = unsafe {
            libc::dlsym(
                handle,
                IOPM_ASSERTION_CREATE_WITH_NAME_SYMBOL.as_ptr().cast(),
            )
        };
        if create_with_name.is_null() {
            warn!(
                symbol = "IOPMAssertionCreateWithName",
                "Failed to load IOKit symbol"
            );
            let _ = unsafe { libc::dlclose(handle) };
            return None;
        }

        let release = unsafe { libc::dlsym(handle, IOPM_ASSERTION_RELEASE_SYMBOL.as_ptr().cast()) };
        if release.is_null() {
            warn!(
                symbol = "IOPMAssertionRelease",
                "Failed to load IOKit symbol"
            );
            let _ = unsafe { libc::dlclose(handle) };
            return None;
        }

        let create_with_name: IOPMAssertionCreateWithNameFn =
            unsafe { std::mem::transmute(create_with_name) };
        let release: IOPMAssertionReleaseFn = unsafe { std::mem::transmute(release) };

        Some(Self {
            _iokit_handle: handle as usize,
            create_with_name,
            release,
        })
    }
}

type IOPMAssertionID = u32;
type IOPMAssertionLevel = u32;
type IOReturn = i32;

const K_IOPM_ASSERTION_LEVEL_ON: IOPMAssertionLevel = 255;
const K_IORETURN_SUCCESS: IOReturn = 0;
