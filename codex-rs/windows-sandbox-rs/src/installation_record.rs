//! Persists the authenticated sandbox owner across service restarts and package updates.
//! The shared store keeps the service's existing key, bounded REG_SZ format, and semantics.

use std::io;
use std::mem::size_of;
use std::path::PathBuf;
use std::ptr;

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use serde::Deserialize;
use serde::Serialize;
use windows_sys::Win32::Foundation as foundation;
use windows_sys::Win32::System::Registry as registry;

use crate::winutil::to_wide;

// Package updates can replace the service key, so keep this record outside it.
const INSTALLATION_KEY: &str = r"SOFTWARE\OpenAI\Codex\WindowsSandboxService";
const INSTALLATION_VALUE: &str = "ProvisionedInstallation";
const MAX_VALUE_UNITS: usize = 4096;

#[derive(Clone, Deserialize, Serialize)]
pub struct DesktopInstallation {
    pub created_codex_home: bool,
    pub cache_home: PathBuf,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct InstallationRecord {
    pub user_sid: String,
    pub codex_home: PathBuf,
    pub session_id: u32,
    #[serde(default)]
    pub desktop_installation: Option<DesktopInstallation>,
}

pub fn load() -> Result<Option<InstallationRecord>> {
    load_from(INSTALLATION_KEY)
}

pub(crate) fn load_from(key: &str) -> Result<Option<InstallationRecord>> {
    let mut value = [0_u16; MAX_VALUE_UNITS];
    let mut value_length = std::mem::size_of_val(&value) as u32;
    let status = unsafe {
        registry::RegGetValueW(
            registry::HKEY_LOCAL_MACHINE,
            to_wide(key).as_ptr(),
            to_wide(INSTALLATION_VALUE).as_ptr(),
            registry::RRF_RT_REG_SZ,
            ptr::null_mut(),
            value.as_mut_ptr().cast(),
            &mut value_length,
        )
    };
    match status {
        foundation::ERROR_FILE_NOT_FOUND | foundation::ERROR_PATH_NOT_FOUND => return Ok(None),
        foundation::ERROR_SUCCESS => {}
        status => {
            return Err(io::Error::from_raw_os_error(status as i32))
                .context("read protected sandbox installation record");
        }
    }
    ensure!(
        value_length as usize <= std::mem::size_of_val(&value)
            && value_length.is_multiple_of(size_of::<u16>() as u32),
        "sandbox installation record has an invalid length"
    );
    let value = value[..value_length as usize / size_of::<u16>()]
        .strip_suffix(&[0])
        .context("sandbox installation record is not null-terminated")?;
    let value = String::from_utf16(value).context("decode sandbox installation record")?;
    let record = serde_json::from_str(&value).context("parse sandbox installation record")?;
    Ok(Some(record))
}

pub fn save(record: &InstallationRecord) -> Result<()> {
    save_to(INSTALLATION_KEY, record)
}

pub(crate) fn save_to(key: &str, record: &InstallationRecord) -> Result<()> {
    let value = to_wide(
        serde_json::to_string(record).context("serialize protected sandbox installation record")?,
    );
    ensure!(
        value.len() <= MAX_VALUE_UNITS,
        "sandbox installation record exceeds its size limit"
    );
    let status = unsafe {
        registry::RegSetKeyValueW(
            registry::HKEY_LOCAL_MACHINE,
            to_wide(key).as_ptr(),
            to_wide(INSTALLATION_VALUE).as_ptr(),
            registry::REG_SZ,
            value.as_ptr().cast(),
            (value.len() * size_of::<u16>()) as u32,
        )
    };
    if status == foundation::ERROR_SUCCESS {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(status as i32))
            .context("persist protected sandbox installation record")
    }
}

pub fn remove() -> Result<()> {
    let status = unsafe {
        registry::RegDeleteKeyW(
            registry::HKEY_LOCAL_MACHINE,
            to_wide(INSTALLATION_KEY).as_ptr(),
        )
    };
    match status {
        foundation::ERROR_SUCCESS
        | foundation::ERROR_FILE_NOT_FOUND
        | foundation::ERROR_PATH_NOT_FOUND => Ok(()),
        status => Err(io::Error::from_raw_os_error(status as i32))
            .context("remove protected sandbox installation record"),
    }
}
