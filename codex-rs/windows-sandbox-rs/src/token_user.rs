//! Copies token user identities with bounded Windows queries and owned SID storage.

use anyhow::Result;
use anyhow::anyhow;
use anyhow::ensure;
use std::ffi::c_void;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Security::GetTokenInformation;
use windows_sys::Win32::Security::IsValidSid;
use windows_sys::Win32::Security::TOKEN_USER;
use windows_sys::Win32::Security::TokenUser;

/// Copies a token's user SID into owned storage after a bounded TokenUser query.
///
/// # Safety
/// The caller must keep a token handle with TOKEN_QUERY access valid during this call.
pub unsafe fn get_user_sid_bytes(h_token: HANDLE) -> Result<Vec<u8>> {
    let mut needed: u32 = 0;
    GetTokenInformation(h_token, TokenUser, std::ptr::null_mut(), 0, &mut needed);
    ensure!(
        needed as usize >= std::mem::size_of::<TOKEN_USER>() && needed <= 4096,
        "invalid TokenUser query size"
    );
    let mut user_buf: Vec<u8> = vec![0u8; needed as usize];
    let ok = GetTokenInformation(
        h_token,
        TokenUser,
        user_buf.as_mut_ptr() as *mut c_void,
        needed,
        &mut needed,
    );
    if ok == 0 {
        return Err(anyhow!(
            "GetTokenInformation(TokenUser) failed: {}",
            GetLastError()
        ));
    }
    ensure!(
        needed as usize <= user_buf.len(),
        "invalid TokenUser result size"
    );
    decode_token_user(&user_buf[..needed as usize])
}

fn decode_token_user(buffer: &[u8]) -> Result<Vec<u8>> {
    ensure!(
        buffer.len() >= std::mem::size_of::<TOKEN_USER>(),
        "truncated TokenUser"
    );
    let user = unsafe { std::ptr::read_unaligned(buffer.as_ptr().cast::<TOKEN_USER>()) };
    copy_token_sid(buffer, user.User.Sid)
}

fn copy_token_sid(buffer: &[u8], pointer: *mut c_void) -> Result<Vec<u8>> {
    // Bound the header and every subauthority before calling a SID API.
    let offset = (pointer as usize).wrapping_sub(buffer.as_ptr() as usize);
    ensure!(
        buffer.len() >= 8 && offset <= buffer.len() - 8,
        "invalid token SID pointer"
    );
    ensure!(buffer[offset] == 1, "invalid token SID revision");
    let length = 8 + usize::from(buffer[offset + 1]) * 4;
    ensure!(
        length <= 68 && length <= buffer.len() - offset,
        "invalid token SID size"
    );
    ensure!(unsafe { IsValidSid(pointer) } != 0, "invalid token SID");
    Ok(buffer[offset..offset + length].to_vec())
}

#[cfg(test)]
#[path = "token_user_tests.rs"]
mod tests;
