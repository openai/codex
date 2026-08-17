#[cfg(target_os = "macos")]
use std::ffi::CStr;
#[cfg(target_os = "macos")]
use std::ffi::OsStr;
#[cfg(target_os = "macos")]
use std::fs;
#[cfg(target_os = "macos")]
use std::os::unix::ffi::OsStrExt;
#[cfg(target_os = "windows")]
use std::os::windows::io::AsRawHandle;
#[cfg(target_os = "windows")]
use std::os::windows::io::FromRawHandle;
#[cfg(target_os = "windows")]
use std::os::windows::io::OwnedHandle;
#[cfg(target_os = "macos")]
use std::path::Path;
#[cfg(target_os = "macos")]
use std::path::PathBuf;
#[cfg(target_os = "macos")]
use std::process::Stdio;
#[cfg(target_os = "macos")]
use std::time::Duration;

#[cfg(target_os = "macos")]
use serde_json::Value;
#[cfg(target_os = "macos")]
use tokio::process::Command;
#[cfg(target_os = "macos")]
use tokio::time::timeout;
#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER;
#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::ERROR_SUCCESS;
#[cfg(target_os = "windows")]
use windows_sys::Win32::Storage::Packaging::Appx::FindPackagesByPackageFamily;
#[cfg(target_os = "windows")]
use windows_sys::Win32::Storage::Packaging::Appx::GetPackageFamilyName;
#[cfg(target_os = "windows")]
use windows_sys::Win32::Storage::Packaging::Appx::PACKAGE_FILTER_HEAD;
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Threading::OpenProcess;
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Threading::PROCESS_QUERY_LIMITED_INFORMATION;

use super::super::CheckStatus;
use super::super::DoctorCheck;

pub(in crate::doctor) struct InstalledApp {
    pub(in crate::doctor) identity: &'static str,
    pub(in crate::doctor) version: String,
    #[cfg(target_os = "windows")]
    package_family: &'static str,
    #[cfg(target_os = "macos")]
    pub(in crate::doctor) bundle: PathBuf,
    #[cfg(target_os = "macos")]
    pub(in crate::doctor) build: u64,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::doctor) struct DiscoveryError;

pub(super) async fn installed_app() -> Result<Option<InstalledApp>, DiscoveryError> {
    #[cfg(target_os = "windows")]
    {
        installed_windows_app()
    }
    #[cfg(target_os = "macos")]
    {
        installed_macos_app().await
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Ok(None)
    }
}

#[cfg(target_os = "macos")]
pub(super) fn application_running(application: &InstalledApp, candidate_pid: u32) -> bool {
    process_executable_path(candidate_pid)
        .is_some_and(|path| path.starts_with(application.bundle.join("Contents/MacOS")))
}

#[cfg(target_os = "macos")]
fn process_executable_path(pid: u32) -> Option<PathBuf> {
    let pid = i32::try_from(pid).ok()?;
    let mut buffer = [0_u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
    let result =
        unsafe { libc::proc_pidpath(pid, buffer.as_mut_ptr().cast(), buffer.len() as u32) };
    if result <= 0 {
        return None;
    }
    let executable = CStr::from_bytes_until_nul(&buffer).ok()?;
    Some(PathBuf::from(OsStr::from_bytes(executable.to_bytes())))
}

#[cfg(target_os = "windows")]
pub(super) fn application_running(application: &InstalledApp, candidate_pid: u32) -> bool {
    process_package_family(candidate_pid).as_deref() == Some(application.package_family)
}

#[cfg(target_os = "windows")]
fn process_package_family(pid: u32) -> Option<String> {
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle == 0 {
        return None;
    }
    let process = unsafe { OwnedHandle::from_raw_handle(handle as _) };

    let mut family_length = 0;
    let result = unsafe {
        GetPackageFamilyName(
            process.as_raw_handle() as _,
            &mut family_length,
            std::ptr::null_mut(),
        )
    };
    if result != ERROR_INSUFFICIENT_BUFFER || family_length == 0 {
        return None;
    }

    let mut family = vec![0_u16; family_length as usize];
    let result = unsafe {
        GetPackageFamilyName(
            process.as_raw_handle() as _,
            &mut family_length,
            family.as_mut_ptr(),
        )
    };
    if result != ERROR_SUCCESS {
        return None;
    }

    let end = family.iter().position(|value| *value == 0)?;
    String::from_utf16(&family[..end]).ok()
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub(super) fn application_running(_application: &InstalledApp, _candidate_pid: u32) -> bool {
    false
}

pub(super) fn desktop_check(
    id: impl Into<String>,
    status: CheckStatus,
    summary: impl Into<String>,
) -> DoctorCheck {
    DoctorCheck::new(id, "desktop", status, summary)
}

#[cfg(target_os = "windows")]
fn installed_windows_app() -> Result<Option<InstalledApp>, DiscoveryError> {
    const PACKAGE_FAMILY: &str = "OpenAI.Codex_2p2nqsd0c76g0";

    let family = PACKAGE_FAMILY
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let mut count = 0;
    let mut length = 0;
    let result = unsafe {
        FindPackagesByPackageFamily(
            family.as_ptr(),
            PACKAGE_FILTER_HEAD,
            &mut count,
            std::ptr::null_mut(),
            &mut length,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if result == ERROR_SUCCESS && count == 0 {
        return Ok(None);
    }
    if result != ERROR_INSUFFICIENT_BUFFER {
        return Err(DiscoveryError);
    }

    let mut names = vec![std::ptr::null_mut(); count as usize];
    let mut buffer = vec![0_u16; length as usize];
    let result = unsafe {
        FindPackagesByPackageFamily(
            family.as_ptr(),
            PACKAGE_FILTER_HEAD,
            &mut count,
            names.as_mut_ptr(),
            &mut length,
            buffer.as_mut_ptr(),
            std::ptr::null_mut(),
        )
    };
    if result != ERROR_SUCCESS {
        return Err(DiscoveryError);
    }
    if count == 0 {
        return Ok(None);
    }
    let end = buffer
        .iter()
        .position(|value| *value == 0)
        .ok_or(DiscoveryError)?;
    let package = String::from_utf16(&buffer[..end]).map_err(|_| DiscoveryError)?;
    let version = package
        .strip_prefix("OpenAI.Codex_")
        .and_then(|name| name.split('_').next())
        .filter(|version| {
            version
                .split('.')
                .all(|part| !part.is_empty() && part.bytes().all(|digit| digit.is_ascii_digit()))
        })
        .ok_or(DiscoveryError)?;

    Ok(Some(InstalledApp {
        identity: "OpenAI.Codex",
        version: version.to_string(),
        package_family: PACKAGE_FAMILY,
    }))
}

#[cfg(target_os = "macos")]
async fn installed_macos_app() -> Result<Option<InstalledApp>, DiscoveryError> {
    let applications = std::iter::once(PathBuf::from("/Applications"))
        .chain(std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Applications")))
        .flat_map(|directory| {
            ["ChatGPT.app", "Codex.app"]
                .into_iter()
                .map(move |application| directory.join(application))
        });

    for bundle in applications {
        match fs::metadata(&bundle) {
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => continue,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err(DiscoveryError),
        }
        if let Some(application) = inspect_macos_bundle(&bundle).await? {
            return Ok(Some(application));
        }
    }

    Ok(None)
}

#[cfg(target_os = "macos")]
pub(in crate::doctor) async fn inspect_macos_bundle(
    bundle: &Path,
) -> Result<Option<InstalledApp>, DiscoveryError> {
    let mut command = Command::new("/usr/bin/plutil");
    command
        .args(["-convert", "json", "-o", "-"])
        .arg(bundle.join("Contents/Info.plist"))
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let output = timeout(Duration::from_secs(5), command.output())
        .await
        .map_err(|_| DiscoveryError)?
        .map_err(|_| DiscoveryError)?;
    if !output.status.success() || output.stdout.len() > 64 * 1024 {
        return Err(DiscoveryError);
    }
    let metadata: Value = serde_json::from_slice(&output.stdout).map_err(|_| DiscoveryError)?;
    if metadata.get("CFBundleIdentifier").and_then(Value::as_str) != Some("com.openai.codex") {
        return Ok(None);
    }
    let version = metadata
        .get("CFBundleShortVersionString")
        .or_else(|| metadata.get("CFBundleVersion"))
        .and_then(Value::as_str)
        .ok_or(DiscoveryError)?;
    let build = metadata
        .get("CFBundleVersion")
        .and_then(Value::as_str)
        .and_then(|value| value.parse().ok())
        .ok_or(DiscoveryError)?;

    Ok(Some(InstalledApp {
        identity: "com.openai.codex",
        version: version.to_string(),
        bundle: bundle.to_path_buf(),
        build,
    }))
}
