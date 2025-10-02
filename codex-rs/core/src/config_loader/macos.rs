#![cfg(target_os = "macos")]

use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use core_foundation::string::CFStringRef;
use std::ffi::c_void;
use std::io;
use toml::Value as TomlValue;

#[cfg(test)]
mod test_support {
    /// Test-only helpers that provide deterministic managed preferences.
    use super::*;
    use std::panic::AssertUnwindSafe;
    use std::panic::catch_unwind;
    use std::panic::resume_unwind;
    use std::sync::Mutex;
    use std::sync::OnceLock;

    static TEST_MANAGED_PREFERENCES_OVERRIDE: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    static TEST_MANAGED_PREFERENCES_SERIALIZER: OnceLock<Mutex<()>> = OnceLock::new();

    fn test_managed_preferences_override_storage() -> &'static Mutex<Option<String>> {
        TEST_MANAGED_PREFERENCES_OVERRIDE.get_or_init(|| Mutex::new(None))
    }

    fn test_managed_preferences_serializer() -> &'static Mutex<()> {
        TEST_MANAGED_PREFERENCES_SERIALIZER.get_or_init(|| Mutex::new(()))
    }

    fn replace_test_managed_preferences_override(value: Option<String>) -> Option<String> {
        let mut guard = match test_managed_preferences_override_storage().lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        std::mem::replace(&mut *guard, value)
    }

    pub(super) fn current_test_managed_preferences_override() -> Option<String> {
        match test_managed_preferences_override_storage().lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    pub(super) fn with_test_managed_preferences_override<R>(
        value: Option<&str>,
        f: impl FnOnce() -> R + std::panic::UnwindSafe,
    ) -> R {
        let serializer_guard = match test_managed_preferences_serializer().lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        let previous =
            replace_test_managed_preferences_override(value.map(std::string::ToString::to_string));
        let result = catch_unwind(AssertUnwindSafe(f));
        replace_test_managed_preferences_override(previous);
        drop(serializer_guard);
        match result {
            Ok(output) => output,
            Err(payload) => resume_unwind(payload),
        }
    }

    pub(super) fn with_cleared_test_managed_preferences<R>(
        f: impl FnOnce() -> R + std::panic::UnwindSafe,
    ) -> R {
        with_test_managed_preferences_override(None, f)
    }

    pub(super) fn with_encoded_test_managed_preferences<R>(
        encoded: &str,
        f: impl FnOnce() -> R + std::panic::UnwindSafe,
    ) -> R {
        with_test_managed_preferences_override(Some(encoded), f)
    }
}

#[cfg(test)]
use test_support::current_test_managed_preferences_override;
#[cfg(test)]
pub(super) use test_support::with_cleared_test_managed_preferences;
#[cfg(test)]
pub(super) use test_support::with_encoded_test_managed_preferences;
#[cfg(test)]
pub(super) use test_support::with_test_managed_preferences_override;

pub(super) fn load_managed_admin_config() -> io::Result<Option<TomlValue>> {
    #[cfg(test)]
    {
        if let Some(encoded) = current_test_managed_preferences_override() {
            let trimmed = encoded.trim();
            if trimmed.is_empty() {
                return Ok(None);
            }
            return parse_managed_preferences_base64(trimmed).map(Some);
        }
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFPreferencesCopyAppValue(key: CFStringRef, application_id: CFStringRef) -> *mut c_void;
    }

    const MANAGED_PREFERENCES_APPLICATION_ID: &str = "com.openai.codex";
    const MANAGED_PREFERENCES_CONFIG_KEY: &str = "config_toml_base64";

    let application_id = CFString::new(MANAGED_PREFERENCES_APPLICATION_ID);
    let key = CFString::new(MANAGED_PREFERENCES_CONFIG_KEY);

    let value_ref = unsafe {
        CFPreferencesCopyAppValue(
            key.as_concrete_TypeRef(),
            application_id.as_concrete_TypeRef(),
        )
    };

    if value_ref.is_null() {
        tracing::debug!(
            "Managed preferences for {} key {} not found",
            MANAGED_PREFERENCES_APPLICATION_ID,
            MANAGED_PREFERENCES_CONFIG_KEY
        );
        return Ok(None);
    }

    let value = unsafe { CFString::wrap_under_create_rule(value_ref as _) };
    let contents = value.to_string();
    let trimmed = contents.trim();

    parse_managed_preferences_base64(trimmed).map(Some)
}

pub(super) fn parse_managed_preferences_base64(encoded: &str) -> io::Result<TomlValue> {
    let decoded = BASE64_STANDARD.decode(encoded.as_bytes()).map_err(|err| {
        tracing::error!("Failed to decode managed preferences as base64: {err}");
        io::Error::new(io::ErrorKind::InvalidData, err)
    })?;

    let decoded_str = String::from_utf8(decoded).map_err(|err| {
        tracing::error!("Managed preferences base64 contents were not valid UTF-8: {err}");
        io::Error::new(io::ErrorKind::InvalidData, err)
    })?;

    match toml::from_str::<TomlValue>(&decoded_str) {
        Ok(parsed) => Ok(parsed),
        Err(err) => {
            tracing::error!("Failed to parse managed preferences TOML: {err}");
            Err(io::Error::new(io::ErrorKind::InvalidData, err))
        }
    }
}
