use std::sync::Once;

use android_system_properties::AndroidSystemProperties;

use crate::ffi_utils::android_timezone_property_name;

pub(crate) fn get_timezone_inner() -> Result<String, crate::GetTimezoneError> {
    let key = android_timezone_property_name();

    get_properties()
        .and_then(|properties| properties.get_from_cstr(key))
        .ok_or(crate::GetTimezoneError::OsError)
}

fn get_properties() -> Option<&'static AndroidSystemProperties> {
    static INITIALIZED: Once = Once::new();
    static mut PROPERTIES: Option<AndroidSystemProperties> = None;

    INITIALIZED.call_once(|| {
        let properties = AndroidSystemProperties::new();
        // SAFETY: `INITIALIZED` is synchronizing. The variable is only assigned to once.
        unsafe { PROPERTIES = Some(properties) };
    });

    // SAFETY: `INITIALIZED` is synchronizing. The variable is only assigned to once.
    unsafe { PROPERTIES.as_ref() }
}
