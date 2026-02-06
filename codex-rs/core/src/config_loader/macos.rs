use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use core_foundation::string::CFStringRef;
use std::ffi::c_void;
use std::io;
use tokio::task;
use toml::Value as TomlValue;

const MANAGED_PREFERENCES_APPLICATION_ID: &str = "com.openai.codex";
const MANAGED_PREFERENCES_CONFIG_KEY: &str = "config_toml_base64";

pub(crate) async fn load_managed_admin_config_layer(
    override_base64: Option<&str>,
) -> io::Result<Option<TomlValue>> {
    if let Some(encoded) = override_base64 {
        let trimmed = encoded.trim();
        return if trimmed.is_empty() {
            Ok(None)
        } else {
            parse_managed_preferences_base64(trimmed).map(Some)
        };
    }

    const LOAD_ERROR: &str = "Failed to load managed preferences configuration";

    match task::spawn_blocking(load_managed_admin_config).await {
        Ok(result) => result,
        Err(join_err) => {
            if join_err.is_cancelled() {
                tracing::error!("Managed preferences load task was cancelled");
            } else {
                tracing::error!("Managed preferences load task failed: {join_err}");
            }
            Err(io::Error::other(LOAD_ERROR))
        }
    }
}

fn load_managed_admin_config() -> io::Result<Option<TomlValue>> {
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFPreferencesCopyAppValue(key: CFStringRef, application_id: CFStringRef) -> *mut c_void;
    }

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

fn parse_managed_preferences_base64(encoded: &str) -> io::Result<TomlValue> {
    let decoded = BASE64_STANDARD.decode(encoded.as_bytes()).map_err(|err| {
        tracing::error!("Failed to decode managed preferences as base64: {err}");
        io::Error::new(io::ErrorKind::InvalidData, err)
    })?;

    let decoded_str = String::from_utf8(decoded).map_err(|err| {
        tracing::error!("Managed preferences base64 contents were not valid UTF-8: {err}");
        io::Error::new(io::ErrorKind::InvalidData, err)
    })?;

    match toml::from_str::<TomlValue>(&decoded_str) {
        Ok(TomlValue::Table(parsed)) => Ok(TomlValue::Table(parsed)),
        Ok(other) => {
            tracing::error!(
                "Managed preferences TOML must have a table at the root, found {other:?}",
            );
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "managed preferences root must be a table",
            ))
        }
        Err(err) => {
            tracing::error!("Failed to parse managed preferences TOML: {err}");
            Err(io::Error::new(io::ErrorKind::InvalidData, err))
        }
    }
}
