use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use tracing::warn;

#[allow(
    dead_code,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    clippy::all
)]
mod iokit {
    #[link(name = "IOKit", kind = "framework")]
    unsafe extern "C" {}

    include!("iokit_bindings.rs");
}

type IOPMAssertionID = iokit::IOPMAssertionID;
type IOPMAssertionLevel = iokit::IOPMAssertionLevel;
type IOReturn = iokit::IOReturn;

const ASSERTION_REASON: &str = "Codex is running an active turn";
// Apple exposes this assertion type as a `CFSTR(...)` macro, so bindgen cannot generate a
// reusable `CFStringRef` constant for it.
const ASSERTION_TYPE_PREVENT_USER_IDLE_SYSTEM_SLEEP: &str = "PreventUserIdleSystemSleep";

#[derive(Debug, Default)]
pub(crate) struct SleepInhibitor {
    assertion: Option<MacSleepAssertion>,
}

impl SleepInhibitor {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn acquire(&mut self) {
        if self.assertion.is_some() {
            return;
        }

        match MacSleepAssertion::create(ASSERTION_REASON) {
            Ok(assertion) => {
                self.assertion = Some(assertion);
            }
            Err(error) => {
                warn!(
                    iokit_error = error,
                    "Failed to create macOS sleep-prevention assertion"
                );
            }
        }
    }

    pub(crate) fn release(&mut self) {
        self.assertion = None;
    }
}

#[derive(Debug)]
struct MacSleepAssertion {
    id: IOPMAssertionID,
}

impl MacSleepAssertion {
    fn create(name: &str) -> Result<Self, IOReturn> {
        let assertion_type = CFString::new(ASSERTION_TYPE_PREVENT_USER_IDLE_SYSTEM_SLEEP);
        let assertion_name = CFString::new(name);
        let mut id: IOPMAssertionID = 0;
        // `core-foundation` and the generated bindings each declare an opaque `__CFString` type,
        // so we cast to the bindgen pointer aliases before crossing the FFI boundary.
        let assertion_type_ref: iokit::CFStringRef = assertion_type.as_concrete_TypeRef().cast();
        let assertion_name_ref: iokit::CFStringRef = assertion_name.as_concrete_TypeRef().cast();
        let result = unsafe {
            // SAFETY: `assertion_type_ref` and `assertion_name_ref` are valid `CFStringRef`s and
            // `&mut id` is a valid out-pointer for `IOPMAssertionCreateWithName` to initialize.
            iokit::IOPMAssertionCreateWithName(
                assertion_type_ref,
                iokit::kIOPMAssertionLevelOn as IOPMAssertionLevel,
                assertion_name_ref,
                &mut id,
            )
        };
        if result == iokit::kIOReturnSuccess as IOReturn {
            Ok(Self { id })
        } else {
            Err(result)
        }
    }
}

impl Drop for MacSleepAssertion {
    fn drop(&mut self) {
        let result = unsafe {
            // SAFETY: `self.id` was returned by `IOPMAssertionCreateWithName` and this `Drop`
            // implementation releases it exactly once when the owning assertion is dropped.
            iokit::IOPMAssertionRelease(self.id)
        };
        if result != iokit::kIOReturnSuccess as IOReturn {
            warn!(
                iokit_error = result,
                "Failed to release macOS sleep-prevention assertion"
            );
        }
    }
}
