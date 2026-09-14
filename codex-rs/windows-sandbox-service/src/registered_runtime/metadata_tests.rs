//! Tests that metadata ACE edits preserve unrelated, inherited, and broader permissions.

use super::*;

fn ace_bytes(acl: &mut [u32]) -> Vec<Vec<u8>> {
    let acl = acl.as_mut_ptr().cast::<security::ACL>();
    (0..u32::from(unsafe { (*acl).AceCount }))
        .map(|index| {
            let mut raw = ptr::null_mut();
            assert_ne!(unsafe { security::GetAce(acl, index, &mut raw) }, 0);
            let length = unsafe { (*raw.cast::<security::ACE_HEADER>()).AceSize } as usize;
            unsafe { std::slice::from_raw_parts(raw.cast::<u8>(), length) }.to_vec()
        })
        .collect()
}

fn existing_acl(sid: &LocalSid) -> Vec<u32> {
    let mut acl = vec![0u32; 256];
    let raw = acl.as_mut_ptr().cast();
    assert_ne!(
        unsafe { security::InitializeAcl(raw, 1024, security::ACL_REVISION_DS) },
        0
    );
    assert_ne!(
        unsafe {
            security::AddAccessDeniedAceEx(
                raw,
                security::ACL_REVISION_DS,
                0,
                filesystem::FILE_WRITE_DATA,
                sid.as_ptr(),
            )
        },
        0
    );
    assert_ne!(
        unsafe {
            security::AddAccessAllowedAceEx(
                raw,
                security::ACL_REVISION_DS,
                0,
                filesystem::FILE_READ_DATA | filesystem::FILE_READ_ATTRIBUTES,
                sid.as_ptr(),
            )
        },
        0
    );
    assert_ne!(
        unsafe {
            security::AddAccessAllowedAceEx(
                raw,
                security::ACL_REVISION_DS,
                security::INHERITED_ACE,
                filesystem::FILE_READ_ATTRIBUTES,
                sid.as_ptr(),
            )
        },
        0
    );
    acl
}

#[test]
fn metadata_grant_and_cleanup_preserve_broader_and_inherited_aces() -> Result<()> {
    let sid = LocalSid::from_string("S-1-5-21-1-2-3-1001")?;
    let mut acl = existing_acl(&sid);
    let before = ace_bytes(&mut acl);
    assert_eq!(metadata_ace(&mut acl, &sid)?, None);
    insert_metadata_ace(&mut acl, &sid)?;
    assert_eq!(metadata_ace(&mut acl, &sid)?, Some(2));
    assert_ne!(unsafe { security::IsValidAcl(acl.as_mut_ptr().cast()) }, 0);
    assert_ne!(
        unsafe { security::DeleteAce(acl.as_mut_ptr().cast(), 2) },
        0
    );
    assert_eq!(ace_bytes(&mut acl), before);
    Ok(())
}

#[test]
fn metadata_cleanup_handles_duplicates_without_touching_another_principal() -> Result<()> {
    let sid = LocalSid::from_string("S-1-5-21-1-2-3-1001")?;
    let other = LocalSid::from_string("S-1-5-21-1-2-3-1002")?;
    let mut acl = existing_acl(&sid);
    let before = ace_bytes(&mut acl);
    insert_metadata_ace(&mut acl, &sid)?;
    assert_eq!(metadata_ace(&mut acl, &other)?, None);
    insert_metadata_ace(&mut acl, &sid)?;
    while let Some(index) = metadata_ace(&mut acl, &sid)? {
        assert_ne!(
            unsafe { security::DeleteAce(acl.as_mut_ptr().cast(), index) },
            0
        );
    }
    assert_eq!(ace_bytes(&mut acl), before);
    Ok(())
}
