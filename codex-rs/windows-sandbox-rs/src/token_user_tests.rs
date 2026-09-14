//! Checks the existing token-user query against real process and invalid handles.

use super::get_user_sid_bytes;
use anyhow::Result;
use anyhow::ensure;
use std::os::windows::io::FromRawHandle;
use std::os::windows::io::OwnedHandle;
use windows_sys::Win32::Security::IsValidSid;
use windows_sys::Win32::Security::TOKEN_QUERY;
use windows_sys::Win32::System::Threading::GetCurrentProcess;
use windows_sys::Win32::System::Threading::OpenProcessToken;

#[test]
fn queries_current_user_and_rejects_invalid_token() -> Result<()> {
    let mut raw = 0;
    ensure!(unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw) } != 0);
    let _token = unsafe { OwnedHandle::from_raw_handle(raw as _) };
    let user = unsafe { get_user_sid_bytes(raw) }?;
    assert!(unsafe { IsValidSid(user.as_ptr() as _) } != 0);
    assert!(unsafe { get_user_sid_bytes(0) }.is_err());
    Ok(())
}
