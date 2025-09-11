#[cfg(target_os = "macos")]
use base64::Engine;
#[cfg(target_os = "macos")]
use base64::prelude::BASE64_STANDARD;
use dirs::home_dir;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::thread;
use toml::Value as TomlValue;

const CONFIG_TOML_FILE: &str = "config.toml";

pub fn load_config_as_toml(codex_home: &Path) -> io::Result<TomlValue> {
    let user_config_path = codex_home.join(CONFIG_TOML_FILE);
    let global_config_path = home_dir().map(|mut path| {
        path.push(".codex");
        path.push("global.toml");
        path
    });
    let system_config_path = PathBuf::from("/etc/opt/codex/config.toml");

    thread::scope(|scope| {
        let user_handle = scope.spawn(|| read_config_from_path(&user_config_path, true));
        let global_handle = scope.spawn(move || match global_config_path {
            Some(path) => read_config_from_path(&path, false),
            None => Ok(None),
        });
        let system_handle = scope.spawn(move || read_config_from_path(&system_config_path, false));
        let managed_handle = scope.spawn(load_managed_admin_config);

        let user_config = join_config_result(user_handle, "user config.toml")?;
        let global_config = join_config_result(global_handle, "~/.codex/global.toml")?;
        let system_config = join_config_result(system_handle, "/etc/opt/codex/config.toml")?;
        let managed_config = join_config_result(managed_handle, "managed preferences")?;

        let mut merged = user_config.unwrap_or_else(default_empty_table);

        for overlay in [global_config, system_config, managed_config]
            .into_iter()
            .flatten()
        {
            merge_toml_values(&mut merged, &overlay);
        }

        Ok(merged)
    })
}

fn default_empty_table() -> TomlValue {
    TomlValue::Table(Default::default())
}

fn join_config_result(
    handle: thread::ScopedJoinHandle<'_, io::Result<Option<TomlValue>>>,
    label: &str,
) -> io::Result<Option<TomlValue>> {
    match handle.join() {
        Ok(result) => result,
        Err(panic) => {
            if let Some(msg) = panic.downcast_ref::<&str>() {
                tracing::error!("Configuration loader for {label} panicked: {msg}");
            } else if let Some(msg) = panic.downcast_ref::<String>() {
                tracing::error!("Configuration loader for {label} panicked: {msg}");
            } else {
                tracing::error!("Configuration loader for {label} panicked");
            }
            Err(io::Error::other(format!(
                "Failed to load {label} configuration"
            )))
        }
    }
}

fn read_config_from_path(path: &Path, log_missing_as_info: bool) -> io::Result<Option<TomlValue>> {
    match std::fs::read_to_string(path) {
        Ok(contents) => match toml::from_str::<TomlValue>(&contents) {
            Ok(value) => Ok(Some(value)),
            Err(err) => {
                tracing::error!("Failed to parse {}: {err}", path.display());
                Err(io::Error::new(io::ErrorKind::InvalidData, err))
            }
        },
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            if log_missing_as_info {
                tracing::info!("{} not found, using defaults", path.display());
            } else {
                tracing::debug!("{} not found", path.display());
            }
            Ok(None)
        }
        Err(err) => {
            tracing::error!("Failed to read {}: {err}", path.display());
            Err(err)
        }
    }
}

fn merge_toml_values(base: &mut TomlValue, overlay: &TomlValue) {
    if let TomlValue::Table(overlay_table) = overlay
        && let TomlValue::Table(base_table) = base
    {
        for (key, value) in overlay_table {
            if let Some(existing) = base_table.get_mut(key) {
                merge_toml_values(existing, value);
            } else {
                base_table.insert(key.clone(), value.clone());
            }
        }
        return;
    }

    *base = overlay.clone();
}

fn load_managed_admin_config() -> io::Result<Option<TomlValue>> {
    load_managed_admin_config_impl()
}

#[cfg(target_os = "macos")]
fn load_managed_admin_config_impl() -> io::Result<Option<TomlValue>> {
    use core_foundation::base::TCFType;
    use core_foundation::string::CFString;
    use core_foundation::string::CFStringRef;
    use std::ffi::c_void;

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

    let decoded = BASE64_STANDARD.decode(trimmed.as_bytes()).map_err(|err| {
        tracing::error!("Failed to decode managed preferences as base64: {err}");
        io::Error::new(io::ErrorKind::InvalidData, err)
    })?;

    let decoded_str = String::from_utf8(decoded).map_err(|err| {
        tracing::error!("Managed preferences base64 contents were not valid UTF-8: {err}");
        io::Error::new(io::ErrorKind::InvalidData, err)
    })?;

    match toml::from_str::<TomlValue>(&decoded_str) {
        Ok(parsed) => Ok(Some(parsed)),
        Err(err) => {
            tracing::error!("Failed to parse managed preferences TOML: {err}");
            Err(io::Error::new(io::ErrorKind::InvalidData, err))
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn load_managed_admin_config_impl() -> io::Result<Option<TomlValue>> {
    Ok(None)
}
