//! Cross-platform helper for preventing idle sleep while a turn is running.
//!
//! On macOS this uses native IOKit power assertions instead of spawning
//! `caffeinate`, so assertion lifecycle is tied directly to Rust object lifetime.

#[cfg(target_os = "macos")]
use core_foundation::base::TCFType;
#[cfg(target_os = "macos")]
use core_foundation::string::CFString;
#[cfg(target_os = "macos")]
use core_foundation::string::CFStringRef;
#[cfg(target_os = "macos")]
use std::sync::OnceLock;
#[cfg(target_os = "macos")]
use tracing::warn;

#[cfg(target_os = "macos")]
const MACOS_IDLE_SLEEP_ASSERTION_TYPE: &str = "PreventUserIdleSystemSleep";
#[cfg(target_os = "macos")]
const ASSERTION_REASON: &str = "Codex is running an active turn";
#[cfg(target_os = "macos")]
const IOKIT_FRAMEWORK_BINARY: &[u8] = b"/System/Library/Frameworks/IOKit.framework/IOKit\0";
#[cfg(target_os = "macos")]
const IOPM_ASSERTION_CREATE_WITH_NAME_SYMBOL: &[u8] = b"IOPMAssertionCreateWithName\0";
#[cfg(target_os = "macos")]
const IOPM_ASSERTION_RELEASE_SYMBOL: &[u8] = b"IOPMAssertionRelease\0";
#[cfg(target_os = "macos")]
const IOKIT_ASSERTION_API_UNAVAILABLE: &str = "IOKit power assertion APIs are unavailable";

/// Keeps the machine awake while a turn is in progress when enabled.
#[derive(Debug)]
pub struct SleepInhibitor {
    enabled: bool,
    #[cfg(target_os = "macos")]
    assertion: Option<MacSleepAssertion>,
}

impl SleepInhibitor {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            #[cfg(target_os = "macos")]
            assertion: None,
        }
    }

    /// Update the active turn state; turns sleep prevention on/off as needed.
    pub fn set_turn_running(&mut self, turn_running: bool) {
        if !self.enabled {
            self.release();
            return;
        }

        if turn_running {
            self.acquire();
        } else {
            self.release();
        }
    }

    fn acquire(&mut self) {
        #[cfg(target_os = "macos")]
        {
            if self.assertion.is_some() {
                return;
            }
            match MacSleepAssertion::create(ASSERTION_REASON) {
                Ok(assertion) => {
                    self.assertion = Some(assertion);
                }
                Err(error) => match error {
                    MacSleepAssertionError::ApiUnavailable(reason) => {
                        warn!(reason, "Failed to create macOS sleep-prevention assertion");
                    }
                    MacSleepAssertionError::Iokit(code) => {
                        warn!(
                            iokit_error = code,
                            "Failed to create macOS sleep-prevention assertion"
                        );
                    }
                },
            }
        }
    }

    fn release(&mut self) {
        #[cfg(target_os = "macos")]
        {
            // Dropping the assertion releases it via `MacSleepAssertion::drop`.
            self.assertion = None;
        }
    }
}

#[cfg(target_os = "macos")]
#[derive(Debug)]
struct MacSleepAssertion {
    id: IOPMAssertionID,
}

#[cfg(target_os = "macos")]
impl MacSleepAssertion {
    fn create(name: &str) -> Result<Self, MacSleepAssertionError> {
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

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Copy)]
enum MacSleepAssertionError {
    ApiUnavailable(&'static str),
    Iokit(IOReturn),
}

#[cfg(target_os = "macos")]
type IOPMAssertionCreateWithNameFn = unsafe extern "C" fn(
    assertion_type: CFStringRef,
    assertion_level: IOPMAssertionLevel,
    assertion_name: CFStringRef,
    assertion_id: *mut IOPMAssertionID,
) -> IOReturn;

#[cfg(target_os = "macos")]
type IOPMAssertionReleaseFn = unsafe extern "C" fn(assertion_id: IOPMAssertionID) -> IOReturn;

#[cfg(target_os = "macos")]
struct MacSleepApi {
    // Keep the dlopen handle alive for the lifetime of the loaded symbols.
    // This prevents accidental dlclose while function pointers are in use.
    _iokit_handle: usize,
    create_with_name: IOPMAssertionCreateWithNameFn,
    release: IOPMAssertionReleaseFn,
}

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
type IOPMAssertionID = u32;
#[cfg(target_os = "macos")]
type IOPMAssertionLevel = u32;
#[cfg(target_os = "macos")]
type IOReturn = i32;

#[cfg(target_os = "macos")]
const K_IOPM_ASSERTION_LEVEL_ON: IOPMAssertionLevel = 255;
#[cfg(target_os = "macos")]
const K_IORETURN_SUCCESS: IOReturn = 0;

#[cfg(test)]
mod tests {
    use super::SleepInhibitor;

    #[test]
    fn sleep_inhibitor_toggles_without_panicking() {
        let mut inhibitor = SleepInhibitor::new(true);
        inhibitor.set_turn_running(true);
        inhibitor.set_turn_running(false);
    }

    #[test]
    fn sleep_inhibitor_disabled_does_not_panic() {
        let mut inhibitor = SleepInhibitor::new(false);
        inhibitor.set_turn_running(true);
        inhibitor.set_turn_running(false);
    }

    #[test]
    fn sleep_inhibitor_multiple_true_calls_are_idempotent() {
        let mut inhibitor = SleepInhibitor::new(true);
        inhibitor.set_turn_running(true);
        inhibitor.set_turn_running(true);
        inhibitor.set_turn_running(true);
        inhibitor.set_turn_running(false);
    }

    #[test]
    fn sleep_inhibitor_can_toggle_multiple_times() {
        let mut inhibitor = SleepInhibitor::new(true);
        inhibitor.set_turn_running(true);
        inhibitor.set_turn_running(false);
        inhibitor.set_turn_running(true);
        inhibitor.set_turn_running(false);
    }
}
