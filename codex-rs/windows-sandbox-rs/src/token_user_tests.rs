//! Verifies owned token-user SID results and rejects malformed query buffers.

use super::*;
use crate::token::world_sid;
use pretty_assertions::assert_eq;
use std::os::windows::io::FromRawHandle;
use std::os::windows::io::OwnedHandle;
use windows_sys::Win32::Security::SID_AND_ATTRIBUTES;
use windows_sys::Win32::Security::TOKEN_QUERY;
use windows_sys::Win32::System::Threading::GetCurrentProcess;
use windows_sys::Win32::System::Threading::OpenProcessToken;

fn user_buffer(sid: &[u8]) -> Vec<u8> {
    let offset = std::mem::size_of::<TOKEN_USER>();
    let mut buffer = vec![0; offset + sid.len()];
    buffer[offset..].copy_from_slice(sid);
    unsafe {
        let user = TOKEN_USER {
            User: SID_AND_ATTRIBUTES {
                Sid: buffer.as_mut_ptr().add(offset).cast(),
                Attributes: 0,
            },
        };
        std::ptr::write_unaligned(buffer.as_mut_ptr().cast::<TOKEN_USER>(), user);
    }
    buffer
}

#[test]
fn user_sid_is_owned_after_the_query_buffer_is_dropped() -> Result<()> {
    let expected = unsafe { world_sid() }?;
    let actual = {
        let buffer = user_buffer(&expected);
        decode_token_user(&buffer)?
    };
    assert_eq!(actual, expected);
    Ok(())
}

#[test]
fn rejects_truncated_user_and_malformed_sid() -> Result<()> {
    assert!(decode_token_user(&[]).is_err());
    let mut buffer = user_buffer(&unsafe { world_sid() }?);
    let offset = std::mem::size_of::<TOKEN_USER>();
    buffer[offset] = 2;
    assert!(decode_token_user(&buffer).is_err());
    buffer[offset] = 1;
    buffer[offset + 1] = 15;
    assert!(decode_token_user(&buffer).is_err());
    for pointer in [std::ptr::null_mut(), unsafe {
        buffer.as_mut_ptr().add(buffer.len() - 1).cast()
    }] {
        unsafe {
            std::ptr::write_unaligned(
                buffer.as_mut_ptr().cast::<TOKEN_USER>(),
                TOKEN_USER {
                    User: SID_AND_ATTRIBUTES {
                        Sid: pointer,
                        Attributes: 0,
                    },
                },
            );
        }
        assert!(decode_token_user(&buffer).is_err());
    }
    Ok(())
}

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
