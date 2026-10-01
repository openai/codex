//! Exercises owned group decoding and bounded queries without changing token privileges.

use super::*;
use pretty_assertions::assert_eq;
use std::os::windows::io::FromRawHandle;
use std::os::windows::io::OwnedHandle;
use windows_sys::Win32::System::Threading::OpenProcessToken;

fn group_buffer(groups: &[TokenGroup]) -> Vec<u8> {
    let offset = std::mem::offset_of!(TOKEN_GROUPS, Groups);
    let stride = std::mem::size_of::<SID_AND_ATTRIBUTES>();
    let mut sid_offset = offset + groups.len() * stride;
    let mut buffer =
        vec![0u8; sid_offset + groups.iter().map(|group| group.sid.len()).sum::<usize>()];
    unsafe { std::ptr::write_unaligned(buffer.as_mut_ptr().cast::<u32>(), groups.len() as u32) };
    for (index, group) in groups.iter().enumerate() {
        buffer[sid_offset..sid_offset + group.sid.len()].copy_from_slice(&group.sid);
        let entry = SID_AND_ATTRIBUTES {
            Sid: unsafe { buffer.as_mut_ptr().add(sid_offset).cast() },
            Attributes: group.attributes,
        };
        unsafe {
            std::ptr::write_unaligned(
                buffer
                    .as_mut_ptr()
                    .add(offset + index * stride)
                    .cast::<SID_AND_ATTRIBUTES>(),
                entry,
            )
        };
        sid_offset += group.sid.len();
    }
    buffer
}

#[test]
fn owns_sids_and_preserves_duplicate_order_and_attributes() -> Result<()> {
    let expected = [0x4, 0x10, SE_GROUP_LOGON_ID]
        .into_iter()
        .map(|attributes| TokenGroup {
            sid: unsafe { world_sid().expect("world SID") },
            attributes,
        })
        .collect::<Vec<_>>();
    let actual = {
        let buffer = group_buffer(&expected);
        decode_token_groups(&buffer)?
    };
    assert_eq!(actual, expected);
    Ok(())
}

#[test]
fn rejects_truncated_group_layout() {
    assert!(decode_token_groups(&[]).is_err());
    let mut buffer = group_buffer(&[]);
    assert_eq!(
        decode_token_groups(&buffer).unwrap(),
        Vec::<TokenGroup>::new()
    );
    unsafe { std::ptr::write_unaligned(buffer.as_mut_ptr().cast::<u32>(), u32::MAX) };
    assert!(decode_token_groups(&buffer).is_err());
}

#[test]
fn rejects_malformed_and_out_of_buffer_sids() {
    let group = TokenGroup {
        sid: unsafe { world_sid().expect("world SID") },
        attributes: 4,
    };
    let mut buffer = group_buffer(&[group]);
    let offset = std::mem::offset_of!(TOKEN_GROUPS, Groups);
    let entry = unsafe {
        std::ptr::read_unaligned(buffer.as_ptr().add(offset).cast::<SID_AND_ATTRIBUTES>())
    };
    let sid_offset = (entry.Sid as usize) - (buffer.as_ptr() as usize);
    buffer[sid_offset] = 2;
    assert!(decode_token_groups(&buffer).is_err());
    buffer[sid_offset] = 1;
    buffer[sid_offset + 1] = 15;
    assert!(decode_token_groups(&buffer).is_err());
    for sid in [std::ptr::null_mut(), unsafe {
        buffer.as_mut_ptr().add(buffer.len() - 1).cast()
    }] {
        unsafe {
            std::ptr::write_unaligned(
                buffer.as_mut_ptr().add(offset).cast::<SID_AND_ATTRIBUTES>(),
                SID_AND_ATTRIBUTES {
                    Sid: sid,
                    Attributes: 4,
                },
            )
        };
        assert!(decode_token_groups(&buffer).is_err());
    }
}

#[test]
fn queries_current_token_with_a_caller_size_limit() -> Result<()> {
    let mut raw = std::ptr::null_mut();
    ensure!(unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw) } != 0);
    let _token = unsafe { OwnedHandle::from_raw_handle(raw as _) };
    assert!(
        unsafe {
            token_groups(raw, /*max_bytes*/ 0)
        }
        .is_err()
    );
    let groups = unsafe { token_groups(raw, u32::MAX) }?;
    assert!(!groups.is_empty());
    if let Some(logon) = groups
        .iter()
        .find(|group| group.attributes & SE_GROUP_LOGON_ID == SE_GROUP_LOGON_ID)
    {
        assert_eq!(unsafe { get_logon_sid_bytes(raw) }?, logon.sid);
    }
    Ok(())
}
