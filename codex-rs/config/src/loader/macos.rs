use super::merge_requirements_with_remote_sandbox_config;
use crate::config_requirements::ConfigRequirementsToml;
use crate::config_requirements::ConfigRequirementsWithSources;
use crate::config_requirements::RequirementSource;
use crate::config_toml::ConfigToml;
use crate::diagnostics::ConfigDiagnosticSource;
use crate::diagnostics::config_error_from_toml_for_source;
use crate::diagnostics::io_error_from_config_error;
use crate::strict_config::config_error_from_ignored_toml_value_fields_for_source_name;
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use codex_utils_absolute_path::AbsolutePathBufGuard;
use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use core_foundation::string::CFStringRef;
use std::ffi::c_void;
use std::io;
use std::path::Path;
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
    strict_config: bool,
    base_dir: &Path,
) -> io::Result<Option<ManagedAdminConfigLayer>> {
    if let Some(encoded) = override_base64 {
        let trimmed = encoded.trim();
        return if trimmed.is_empty() {
            Ok(None)
        } else {
            parse_managed_config_base64(trimmed, strict_config, base_dir).map(Some)
        };
    }

    let base_dir = base_dir.to_path_buf();
    match task::spawn_blocking(move || load_managed_admin_config(strict_config, &base_dir)).await {
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

fn load_managed_admin_config(
    strict_config: bool,
    base_dir: &Path,
) -> io::Result<Option<ManagedAdminConfigLayer>> {
    load_managed_preference(MANAGED_PREFERENCES_CONFIG_KEY)?
        .as_deref()
        .map(str::trim)
        .map(|encoded| parse_managed_config_base64(encoded, strict_config, base_dir))
        .transpose()
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

        merge_requirements_with_remote_sandbox_config(
            target,
            managed_preferences_requirements_source(),
            parse_managed_requirements_base64(trimmed)?,
        );
        return Ok(());
    }

    match task::spawn_blocking(load_managed_admin_requirements).await {
        Ok(result) => {
            if let Some(requirements) = result? {
                merge_requirements_with_remote_sandbox_config(
                    target,
                    managed_preferences_requirements_source(),
                    requirements,
                );
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

fn parse_managed_config_base64(
    encoded: &str,
    strict_config: bool,
    base_dir: &Path,
) -> io::Result<ManagedAdminConfigLayer> {
    let raw_toml = decode_managed_preferences_base64(encoded)?;
    let source_name =
        format!("{MANAGED_PREFERENCES_APPLICATION_ID}:{MANAGED_PREFERENCES_CONFIG_KEY}");
    let parsed = toml::from_str::<TomlValue>(&raw_toml).map_err(|err| {
        tracing::error!("Failed to parse managed config TOML: {err}");
        if strict_config {
            let config_error = config_error_from_toml_for_source(
                ConfigDiagnosticSource::DisplayName(&source_name),
                &raw_toml,
                err.clone(),
            );
            io_error_from_config_error(io::ErrorKind::InvalidData, config_error, Some(err))
        } else {
            io::Error::new(io::ErrorKind::InvalidData, err)
        }
    })?;

    validate_managed_config_toml_strictly_if_requested(
        strict_config,
        &source_name,
        &raw_toml,
        &parsed,
        base_dir,
    )?;
    match parsed {
        TomlValue::Table(parsed) => Ok(ManagedAdminConfigLayer {
            config: TomlValue::Table(parsed),
            raw_toml,
        }),
        other => {
            tracing::error!("Managed config TOML must have a table at the root, found {other:?}",);
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "managed config root must be a table",
            ))
        }
    }
}

fn validate_managed_config_toml_strictly_if_requested(
    strict_config: bool,
    source_name: &str,
    raw_toml: &str,
    parsed: &TomlValue,
    base_dir: &Path,
) -> io::Result<()> {
    if !strict_config {
        return Ok(());
    }

    let _guard = AbsolutePathBufGuard::new(base_dir);
    if let Some(config_error) = config_error_from_ignored_toml_value_fields_for_source_name::<
        ConfigToml,
    >(source_name, raw_toml, parsed.clone())
    {
        Err(io_error_from_config_error(
            io::ErrorKind::InvalidData,
            config_error,
            /*source*/ None,
        ))
    } else {
        Ok(())
    }
}

fn parse_managed_requirements_base64(encoded: &str) -> io::Result<ConfigRequirementsToml> {
    toml::from_str::<ConfigRequirementsToml>(&decode_managed_preferences_base64(encoded)?).map_err(
        |err| {
            tracing::error!("Failed to parse managed requirements TOML: {err}");
            io::Error::new(io::ErrorKind::InvalidData, err)
        },
    )
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
