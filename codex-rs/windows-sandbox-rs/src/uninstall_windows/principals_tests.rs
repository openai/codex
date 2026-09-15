//! Exercises profile deletion and its retry boundary with one disposable local user.

use std::io;
use std::os::windows::ffi::OsStringExt;
use std::os::windows::io::AsRawHandle;
use std::os::windows::io::FromRawHandle;
use std::os::windows::io::OwnedHandle;
use std::path::PathBuf;
use std::ptr::null;
use std::ptr::null_mut;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use pretty_assertions::assert_eq;
use rand::RngCore;
use windows_sys::Win32::Foundation::ERROR_ACCESS_DENIED;
use windows_sys::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
use windows_sys::Win32::NetworkManagement::NetManagement as network;
use windows_sys::Win32::System::Registry as registry;
use windows_sys::Win32::System::Threading as threading;

use super::SandboxUser;
use crate::winutil::local_user_flags;
use crate::winutil::resolve_sid;
use crate::winutil::set_local_user_flags;
use crate::winutil::string_from_sid_bytes;
use crate::winutil::to_wide;

struct ProfileFixture {
    user: SandboxUser,
    process: Option<OwnedHandle>,
}

impl ProfileFixture {
    fn stop(&mut self) -> Result<()> {
        if let Some(process) = self.process.take() {
            let handle = process.as_raw_handle() as _;
            unsafe {
                threading::TerminateProcess(handle, /*uexitcode*/ 0)
            };
            ensure!(
                unsafe {
                    threading::WaitForSingleObject(handle, /*dwmilliseconds*/ 10_000)
                } == WAIT_OBJECT_0,
                "owned profile fixture did not exit"
            );
            drop(process);
            // User Profile Service unloads the hive after the last owned process exits.
            let sid = string_from_sid_bytes(&self.user.sid).map_err(anyhow::Error::msg)?;
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                let mut key = 0;
                let status = unsafe {
                    registry::RegOpenKeyExW(
                        registry::HKEY_USERS,
                        to_wide(&sid).as_ptr(),
                        /*uloptions*/ 0,
                        registry::KEY_READ,
                        &mut key,
                    )
                };
                if status == ERROR_FILE_NOT_FOUND {
                    break;
                }
                ensure!(status == ERROR_SUCCESS, "read owned profile hive: {status}");
                unsafe { registry::RegCloseKey(key) };
                ensure!(Instant::now() < deadline, "owned profile did not unload");
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        Ok(())
    }
}

impl Drop for ProfileFixture {
    fn drop(&mut self) {
        let _ = self.stop();
        let result = if self.user.sid.is_empty() {
            super::remove_sandbox_principal(self.user.name)
        } else {
            self.user.remove()
        };
        if let Err(error) = result {
            eprintln!("unable to clean up owned profile fixture: {error:#}");
        }
    }
}

#[test]
fn profile_cleanup_preserves_account_until_profile_can_be_deleted() -> Result<()> {
    // Production names are static. This one unique name lives for this test process only.
    let name: &'static str =
        Box::leak(format!("CodexPrf{:08x}", rand::rngs::OsRng.next_u32()).into_boxed_str());
    let username = to_wide(name);
    let password = to_wide(format!("Cdx!a0{:016x}", rand::rngs::OsRng.next_u64()));
    let user_info = network::USER_INFO_1 {
        usri1_name: username.as_ptr().cast_mut(),
        usri1_password: password.as_ptr().cast_mut(),
        usri1_priv: network::USER_PRIV_USER,
        usri1_flags: network::UF_NORMAL_ACCOUNT
            | network::UF_SCRIPT
            | network::UF_DONT_EXPIRE_PASSWD,
        ..unsafe { std::mem::zeroed() }
    };
    let status = unsafe {
        network::NetUserAdd(
            null(),
            /*level*/ 1,
            (&raw const user_info).cast(),
            null_mut(),
        )
    };
    if status == ERROR_ACCESS_DENIED {
        eprintln!(
            "skipping native profile cleanup test: creating its disposable user requires elevation"
        );
        return Ok(());
    }
    ensure!(
        status == network::NERR_Success,
        "create owned profile fixture: {status}"
    );
    let mut fixture = ProfileFixture {
        user: SandboxUser {
            name,
            original_flags: user_info.usri1_flags,
            sid: Vec::new(),
        },
        process: None,
    };
    fixture.user.sid = resolve_sid(name)?;
    let sid = string_from_sid_bytes(&fixture.user.sid).map_err(anyhow::Error::msg)?;
    let profile_key = to_wide(format!(
        r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList\{sid}"
    ));
    let system_root = std::env::var_os("SystemRoot").context("SystemRoot is missing")?;
    let executable =
        PathBuf::from(&system_root).join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
    let executable = to_wide(executable);
    let mut command =
        to_wide("powershell.exe -NoProfile -NonInteractive -Command Start-Sleep -Seconds 600");
    let mut startup: threading::STARTUPINFOW = unsafe { std::mem::zeroed() };
    startup.cb = std::mem::size_of_val(&startup) as u32;
    let mut process: threading::PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
    let created = unsafe {
        threading::CreateProcessWithLogonW(
            username.as_ptr(),
            to_wide(".").as_ptr(),
            password.as_ptr(),
            threading::LOGON_WITH_PROFILE,
            executable.as_ptr(),
            command.as_mut_ptr(),
            threading::CREATE_NO_WINDOW,
            null(),
            to_wide(&system_root).as_ptr(),
            &startup,
            &mut process,
        )
    };
    ensure!(
        created != 0,
        "start owned profile fixture: {}",
        io::Error::last_os_error()
    );
    fixture.process = Some(unsafe { OwnedHandle::from_raw_handle(process.hProcess as _) });
    drop(unsafe { OwnedHandle::from_raw_handle(process.hThread as _) });
    let mut path = [0u16; 32768];
    let mut length = std::mem::size_of_val(&path) as u32;
    let status = unsafe {
        registry::RegGetValueW(
            registry::HKEY_LOCAL_MACHINE,
            profile_key.as_ptr(),
            to_wide("ProfileImagePath").as_ptr(),
            registry::RRF_RT_REG_SZ | registry::RRF_RT_REG_EXPAND_SZ,
            null_mut(),
            path.as_mut_ptr().cast(),
            &mut length,
        )
    };
    assert_eq!(status, ERROR_SUCCESS);
    let path = PathBuf::from(std::ffi::OsString::from_wide(
        &path[..length as usize / 2 - 1],
    ));
    assert!(path.is_dir());
    set_local_user_flags(name, user_info.usri1_flags | network::UF_ACCOUNTDISABLE)?;
    assert!(
        fixture.user.remove().is_err(),
        "loaded profile must block deletion"
    );
    assert_eq!(resolve_sid(name)?, fixture.user.sid);
    assert!(path.is_dir());

    fixture.stop()?;
    fixture.user.remove()?;
    assert_eq!(local_user_flags(name)?, None);
    assert!(!path.exists());
    let mut key = 0;
    let status = unsafe {
        registry::RegOpenKeyExW(
            registry::HKEY_LOCAL_MACHINE,
            profile_key.as_ptr(),
            /*uloptions*/ 0,
            registry::KEY_READ,
            &mut key,
        )
    };
    if status == ERROR_SUCCESS {
        unsafe { registry::RegCloseKey(key) };
    }
    assert_eq!(status, ERROR_FILE_NOT_FOUND);
    fixture.user.remove()?;
    Ok(())
}
