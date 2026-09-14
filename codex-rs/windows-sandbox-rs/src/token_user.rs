//! Owns the token-user SID query used by sandbox token construction.

use anyhow::Result;
use anyhow::anyhow;
use std::ffi::c_void;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Security::CopySid;
use windows_sys::Win32::Security::GetLengthSid;
use windows_sys::Win32::Security::GetTokenInformation;
use windows_sys::Win32::Security::TOKEN_USER;
use windows_sys::Win32::Security::TokenUser;

pub(crate) unsafe fn get_user_sid_bytes(h_token: HANDLE) -> Result<Vec<u8>> {
    let mut needed: u32 = 0;
    GetTokenInformation(h_token, TokenUser, std::ptr::null_mut(), 0, &mut needed);
    if needed == 0 {
        return Err(anyhow!("TokenUser size query returned 0"));
    }
    let mut user_buf: Vec<u8> = vec![0u8; needed as usize];
    let ok = GetTokenInformation(
        h_token,
        TokenUser,
        user_buf.as_mut_ptr() as *mut c_void,
        needed,
        &mut needed,
    );
    if ok == 0 || (needed as usize) < std::mem::size_of::<TOKEN_USER>() {
        return Err(anyhow!(
            "GetTokenInformation(TokenUser) failed: {}",
            GetLastError()
        ));
    }
    let token_user: TOKEN_USER = std::ptr::read_unaligned(user_buf.as_ptr() as *const TOKEN_USER);
    let sid_len = GetLengthSid(token_user.User.Sid);
    if sid_len == 0 {
        return Err(anyhow!(
            "GetLengthSid(TokenUser) failed: {}",
            GetLastError()
        ));
    }
    let mut user_sid_bytes = vec![0u8; sid_len as usize];
    if CopySid(
        sid_len,
        user_sid_bytes.as_mut_ptr() as *mut c_void,
        token_user.User.Sid,
    ) == 0
    {
        return Err(anyhow!("CopySid(TokenUser) failed: {}", GetLastError()));
    }
    Ok(user_sid_bytes)
}

#[cfg(test)]
#[path = "token_user_tests.rs"]
mod tests;
