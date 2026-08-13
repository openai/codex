#![cfg(target_os = "windows")]

use anyhow::Context;
use anyhow::Result;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use windows_sys::Win32::Foundation::FreeLibrary;
use windows_sys::Win32::System::LibraryLoader::FindResourceW;
use windows_sys::Win32::System::LibraryLoader::LOAD_LIBRARY_AS_DATAFILE;
use windows_sys::Win32::System::LibraryLoader::LOAD_LIBRARY_AS_IMAGE_RESOURCE;
use windows_sys::Win32::System::LibraryLoader::LoadLibraryExW;
use windows_sys::Win32::System::LibraryLoader::LoadResource;
use windows_sys::Win32::System::LibraryLoader::LockResource;
use windows_sys::Win32::System::LibraryLoader::SizeofResource;
use windows_sys::Win32::UI::WindowsAndMessaging::CREATEPROCESS_MANIFEST_RESOURCE_ID;
use windows_sys::Win32::UI::WindowsAndMessaging::RT_MANIFEST;

/// The setup executable must expose an asInvoker manifest through the Windows resource API.
#[test]
fn setup_helper_embeds_as_invoker_manifest() -> Result<()> {
    let setup_executable = std::env::var_os("CARGO_BIN_EXE_codex-windows-sandbox-setup")
        .or_else(|| std::env::var_os("CARGO_BIN_EXE_codex_windows_sandbox_setup"))
        .map(PathBuf::from)
        .or_else(|| option_env!("CARGO_BIN_EXE_codex-windows-sandbox-setup").map(PathBuf::from))
        .context("locate the Windows sandbox setup executable")?;
    std::fs::metadata(&setup_executable)
        .with_context(|| format!("find setup helper {}", setup_executable.display()))?;
    let setup_path = setup_executable
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let module = unsafe {
        LoadLibraryExW(
            setup_path.as_ptr(),
            /*hfile*/ 0,
            LOAD_LIBRARY_AS_DATAFILE | LOAD_LIBRARY_AS_IMAGE_RESOURCE,
        )
    };
    if module == 0 {
        return Err(io::Error::last_os_error())
            .with_context(|| format!("load setup helper {}", setup_executable.display()));
    }

    let resource = unsafe {
        FindResourceW(
            module,
            std::ptr::without_provenance(CREATEPROCESS_MANIFEST_RESOURCE_ID as usize),
            std::ptr::without_provenance(RT_MANIFEST as usize),
        )
    };
    if resource == 0 {
        return Err(io::Error::last_os_error()).context("find numeric RT_MANIFEST resource ID 1");
    }

    let resource_size = unsafe { SizeofResource(module, resource) };
    let loaded_resource = unsafe { LoadResource(module, resource) };
    if loaded_resource.is_null() {
        return Err(io::Error::last_os_error()).context("load setup helper manifest resource");
    }
    let resource_data = unsafe { LockResource(loaded_resource) };
    if resource_data.is_null() {
        return Err(io::Error::last_os_error()).context("read setup helper manifest resource");
    }
    let manifest_bytes =
        unsafe { std::slice::from_raw_parts(resource_data.cast::<u8>(), resource_size as usize) };
    let manifest = std::str::from_utf8(manifest_bytes).context("decode setup helper manifest")?;
    assert!(
        manifest.contains("requestedExecutionLevel level=\"asInvoker\""),
        "setup helper manifest does not request asInvoker: {manifest}",
    );
    assert!(
        manifest.contains("uiAccess=\"false\""),
        "setup helper manifest does not disable UI access: {manifest}",
    );

    if unsafe { FreeLibrary(module) } == 0 {
        return Err(io::Error::last_os_error()).context("unload setup helper resource module");
    }

    Ok(())
}
