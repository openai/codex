use super::ConfigRequirementsToml;
use super::ConfigRequirementsWithSources;
use super::RequirementSource;
use crate::config::ConfigToml;
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use codex_config::sanitize_toml_value;
use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use core_foundation::string::CFStringRef;
use std::ffi::c_void;
use std::io;
use tokio::task;
use toml::Value as TomlValue;

const MANAGED_PREFERENCES_APPLICATION_ID: &str = "com.openai.codex";
const MANAGED_PREFERENCES_CONFIG_KEY: &str = "config_toml_base64";
const MANAGED_PREFERENCES_REQUIREMENTS_KEY: &str = "requirements_toml_base64";

#[derive(Debug, Clone)]
pub(super) struct ManagedAdminConfigLayer {
    pub config: TomlValue,
    pub raw_toml: String,
}

pub(super) fn managed_preferences_requirements_source() -> RequirementSource {
    RequirementSource::MdmManagedPreferences {
        domain: MANAGED_PREFERENCES_APPLICATION_ID.to_string(),
        key: MANAGED_PREFERENCES_REQUIREMENTS_KEY.to_string(),
    }
}

pub(crate) async fn load_managed_admin_config_layer(
    override_base64: Option<&str>,
) -> io::Result<Option<ManagedAdminConfigLayer>> {
    if let Some(encoded) = override_base64 {
        let trimmed = encoded.trim();
        return if trimmed.is_empty() {
            Ok(None)
        } else {
            parse_managed_config_base64(trimmed)
        };
    }

    match task::spawn_blocking(load_managed_admin_config).await {
        Ok(result) => result,
        Err(join_err) => {
            if join_err.is_cancelled() {
                tracing::error!("Managed config load task was cancelled");
            } else {
                tracing::error!("Managed config load task failed: {join_err}");
            }
            Err(io::Error::other("Failed to load managed config"))
        }
    }
}

fn load_managed_admin_config() -> io::Result<Option<ManagedAdminConfigLayer>> {
    load_managed_preference(MANAGED_PREFERENCES_CONFIG_KEY)?
        .as_deref()
        .map(str::trim)
        .map(parse_managed_config_base64)
        .transpose()
        .map(Option::flatten)
}

pub(crate) async fn load_managed_admin_requirements_toml(
    target: &mut ConfigRequirementsWithSources,
    override_base64: Option<&str>,
) -> io::Result<()> {
    if let Some(encoded) = override_base64 {
        let trimmed = encoded.trim();
        if trimmed.is_empty() {
            return Ok(());
        }

        if let Some(requirements) = parse_managed_requirements_base64(trimmed)? {
            target.merge_unset_fields(managed_preferences_requirements_source(), requirements);
        }
        return Ok(());
    }

    match task::spawn_blocking(load_managed_admin_requirements).await {
        Ok(result) => {
            if let Some(requirements) = result? {
                target.merge_unset_fields(managed_preferences_requirements_source(), requirements);
            }
            Ok(())
        }
        Err(join_err) => {
            if join_err.is_cancelled() {
                tracing::error!("Managed requirements load task was cancelled");
            } else {
                tracing::error!("Managed requirements load task failed: {join_err}");
            }
            Err(io::Error::other("Failed to load managed requirements"))
        }
    }
}

fn load_managed_admin_requirements() -> io::Result<Option<ConfigRequirementsToml>> {
    load_managed_preference(MANAGED_PREFERENCES_REQUIREMENTS_KEY)?
        .as_deref()
        .map(str::trim)
        .map(parse_managed_requirements_base64)
        .transpose()
        .map(Option::flatten)
}

fn load_managed_preference(key_name: &str) -> io::Result<Option<String>> {
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFPreferencesCopyAppValue(key: CFStringRef, application_id: CFStringRef) -> *mut c_void;
    }

    let value_ref = unsafe {
        CFPreferencesCopyAppValue(
            CFString::new(key_name).as_concrete_TypeRef(),
            CFString::new(MANAGED_PREFERENCES_APPLICATION_ID).as_concrete_TypeRef(),
        )
    };

    if value_ref.is_null() {
        tracing::debug!(
            "Managed preferences for {MANAGED_PREFERENCES_APPLICATION_ID} key {key_name} not found",
        );
        return Ok(None);
    }

    let value = unsafe { CFString::wrap_under_create_rule(value_ref as _) }.to_string();
    Ok(Some(value))
}

fn parse_managed_config_base64(encoded: &str) -> io::Result<Option<ManagedAdminConfigLayer>> {
    let raw_toml = match decode_managed_preferences_base64(encoded) {
        Ok(raw_toml) => raw_toml,
        Err(err) => {
            tracing::warn!(
                error = %err,
                "Ignoring invalid MDM managed config payload",
            );
            return Ok(None);
        }
    };
    match toml::from_str::<TomlValue>(&raw_toml) {
        Ok(TomlValue::Table(parsed)) => {
            let sanitized = match sanitize_toml_value::<ConfigToml>(TomlValue::Table(parsed)) {
                Ok(sanitized) => sanitized,
                Err(err) => {
                    tracing::warn!(
                        error = %err,
                        "Ignoring invalid MDM managed config payload",
                    );
                    return Ok(None);
                }
            };
            for dropped_entry in &sanitized.dropped_entries {
                tracing::warn!(
                    dropped_entry = %dropped_entry,
                    "Ignoring invalid MDM managed config entry",
                );
            }
            Ok(Some(ManagedAdminConfigLayer {
                config: sanitized.value,
                raw_toml,
            }))
        }
        Ok(other) => {
            tracing::warn!(
                managed_value = ?other,
                "Ignoring invalid MDM managed config payload: root must be a table",
            );
            Ok(None)
        }
        Err(err) => {
            tracing::warn!(
                error = %err,
                "Ignoring invalid MDM managed config payload",
            );
            Ok(None)
        }
    }
}

fn parse_managed_requirements_base64(encoded: &str) -> io::Result<Option<ConfigRequirementsToml>> {
    let raw_toml = match decode_managed_preferences_base64(encoded) {
        Ok(raw_toml) => raw_toml,
        Err(err) => {
            tracing::warn!(
                error = %err,
                "Ignoring invalid MDM managed requirements payload",
            );
            return Ok(None);
        }
    };
    let parsed = match toml::from_str::<TomlValue>(&raw_toml) {
        Ok(TomlValue::Table(parsed)) => TomlValue::Table(parsed),
        Ok(other) => {
            tracing::warn!(
                managed_value = ?other,
                "Ignoring invalid MDM managed requirements payload: root must be a table",
            );
            return Ok(None);
        }
        Err(err) => {
            tracing::warn!(
                error = %err,
                "Ignoring invalid MDM managed requirements payload",
            );
            return Ok(None);
        }
    };
    let sanitized = match sanitize_toml_value::<ConfigRequirementsToml>(parsed) {
        Ok(sanitized) => sanitized,
        Err(err) => {
            tracing::warn!(
                error = %err,
                "Ignoring invalid MDM managed requirements payload",
            );
            return Ok(None);
        }
    };
    for dropped_entry in &sanitized.dropped_entries {
        tracing::warn!(
            dropped_entry = %dropped_entry,
            "Ignoring invalid MDM managed requirements entry",
        );
    }
    let requirements = sanitized
        .value
        .try_into()
        .map_err(|err: toml::de::Error| io::Error::new(io::ErrorKind::InvalidData, err))?;
    Ok(Some(requirements))
}

fn decode_managed_preferences_base64(encoded: &str) -> io::Result<String> {
    String::from_utf8(BASE64_STANDARD.decode(encoded.as_bytes()).map_err(|err| {
        tracing::error!("Failed to decode managed value as base64: {err}",);
        io::Error::new(io::ErrorKind::InvalidData, err)
    })?)
    .map_err(|err| {
        tracing::error!("Managed value base64 contents were not valid UTF-8: {err}",);
        io::Error::new(io::ErrorKind::InvalidData, err)
    })
}
