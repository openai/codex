//! Native account repair regression; uses a disposable account and requires elevation.

use super::ensure_local_user;
use crate::local_user_flags;
use crate::set_local_user_flags;
use crate::to_wide;
use anyhow::Result;
use pretty_assertions::assert_eq;
use windows_sys::Win32::NetworkManagement::NetManagement::NetApiBufferFree;
use windows_sys::Win32::NetworkManagement::NetManagement::NetUserDel;
use windows_sys::Win32::NetworkManagement::NetManagement::NetUserGetInfo;
use windows_sys::Win32::NetworkManagement::NetManagement::NetUserSetInfo;
use windows_sys::Win32::NetworkManagement::NetManagement::UF_ACCOUNTDISABLE;
use windows_sys::Win32::NetworkManagement::NetManagement::UF_DONT_EXPIRE_PASSWD;
use windows_sys::Win32::NetworkManagement::NetManagement::UF_PASSWORD_EXPIRED;
use windows_sys::Win32::NetworkManagement::NetManagement::USER_INFO_4;

struct TestAccount(String);

impl Drop for TestAccount {
    fn drop(&mut self) {
        unsafe { NetUserDel(std::ptr::null(), to_wide(&self.0).as_ptr()) };
    }
}

#[test]
#[ignore = "requires administrator rights to create and remove a disposable local account"]
fn repairs_existing_account_password_without_changing_account_policy() -> Result<()> {
    let name = format!("CdxExpiry{}", std::process::id());
    anyhow::ensure!(
        local_user_flags(&name)?.is_none(),
        "test account already exists"
    );
    // Setup can create the account before returning an error.
    let account = TestAccount(name.clone());
    let mut log = Vec::new();
    ensure_local_user(&name, "Old-Test-Password-93!", UF_ACCOUNTDISABLE, &mut log)?;
    let initial_flags = local_user_flags(&account.0)?.unwrap();
    set_local_user_flags(&account.0, initial_flags & !UF_DONT_EXPIRE_PASSWD)?;
    // A level-1008 flags write does not set the dedicated must-change field.
    // https://learn.microsoft.com/windows/win32/api/lmaccess/ns-lmaccess-user_info_4
    let mut buffer = std::ptr::null_mut();
    let status = unsafe {
        NetUserGetInfo(
            std::ptr::null(),
            to_wide(&account.0).as_ptr(),
            /*level*/ 4,
            &mut buffer,
        )
    };
    anyhow::ensure!(status == 0, "read USER_INFO_4 failed: {status}");
    let status = unsafe {
        (*buffer.cast::<USER_INFO_4>()).usri4_password_expired = 1;
        let status = NetUserSetInfo(
            std::ptr::null(),
            to_wide(&account.0).as_ptr(),
            /*level*/ 4,
            buffer,
            std::ptr::null_mut(),
        );
        NetApiBufferFree(buffer.cast());
        status
    };
    anyhow::ensure!(status == 0, "expire test account password failed: {status}");
    let expired_flags = local_user_flags(&account.0)?.unwrap();
    assert_eq!(expired_flags & UF_PASSWORD_EXPIRED, UF_PASSWORD_EXPIRED);

    ensure_local_user(
        &account.0,
        "New-Test-Password-94!",
        /*new_user_flags*/ 0,
        &mut log,
    )?;

    assert_eq!(
        local_user_flags(&account.0)?.unwrap(),
        expired_flags & !UF_PASSWORD_EXPIRED
    );
    drop(account);
    assert_eq!(local_user_flags(&name)?, None);
    Ok(())
}
