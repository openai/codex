//! Exercises retained-logon admission and ownership using read-only current-process tokens.

use super::super::principals::DisabledSandboxUsers;
use super::RetainedLogons;
use crate::token::get_user_sid_bytes;
use anyhow::Result;
use anyhow::ensure;
use pretty_assertions::assert_eq;
use std::os::windows::io::AsHandle;
use std::os::windows::io::AsRawHandle;
use std::os::windows::io::FromRawHandle;
use std::os::windows::io::OwnedHandle;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Foundation::LUID;
use windows_sys::Win32::Security::TOKEN_QUERY;
use windows_sys::Win32::System::Threading::GetCurrentProcess;
use windows_sys::Win32::System::Threading::OpenProcessToken;

fn current_process_token() -> Result<OwnedHandle> {
    let mut raw = 0;
    ensure!(
        unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw) } != 0,
        "open current-process token: {}",
        std::io::Error::last_os_error()
    );
    Ok(unsafe { OwnedHandle::from_raw_handle(raw as _) })
}

#[test]
fn matches_only_the_complete_retained_authentication_id() {
    let retained = RetainedLogons {
        _tokens: Vec::new(),
        ids: vec![LUID {
            LowPart: 17,
            HighPart: 4,
        }],
        sids: Vec::new(),
    };
    let observed = [
        LUID {
            LowPart: 17,
            HighPart: 4,
        },
        LUID {
            LowPart: 18,
            HighPart: 4,
        },
        LUID {
            LowPart: 17,
            HighPart: 5,
        },
        LUID {
            LowPart: 0,
            HighPart: 0,
        },
    ]
    .map(|id| retained.contains(id));
    assert_eq!(observed, [true, false, false, false]);
    assert!(!RetainedLogons::default().contains(LUID {
        LowPart: 17,
        HighPart: 4
    }));
}

#[test]
fn rejects_more_than_two_tokens_before_admission() -> Result<()> {
    let token = current_process_token()?;
    let error = RetainedLogons::capture(&[token.as_handle(); 3], &DisabledSandboxUsers::default())
        .err()
        .expect("three token inputs must be rejected");
    assert_eq!(error.to_string(), "too many retained cleanup tokens");
    Ok(())
}

#[test]
fn rejects_a_token_without_a_captured_managed_sid() -> Result<()> {
    let token = current_process_token()?;
    assert!(
        RetainedLogons::capture(&[token.as_handle()], &DisabledSandboxUsers::default()).is_err()
    );
    Ok(())
}

#[test]
fn rejects_distinct_handles_for_the_same_managed_sid() -> Result<()> {
    let token = current_process_token()?;
    let sid = unsafe { get_user_sid_bytes(token.as_raw_handle() as HANDLE) }?;
    let users = DisabledSandboxUsers::for_token_test(sid);
    let duplicate = token.as_handle().try_clone_to_owned()?;
    RetainedLogons::capture(&[token.as_handle()], &users)?;
    assert!(RetainedLogons::capture(&[token.as_handle(), duplicate.as_handle()], &users).is_err());
    Ok(())
}

#[test]
fn retained_token_remains_queryable_after_the_borrowed_handle_is_closed() -> Result<()> {
    let (retained, expected_sid) = {
        let token = current_process_token()?;
        let sid = unsafe { get_user_sid_bytes(token.as_raw_handle() as HANDLE) }?;
        let users = DisabledSandboxUsers::for_token_test(sid.clone());
        (RetainedLogons::capture(&[token.as_handle()], &users)?, sid)
    };
    assert_eq!(retained._tokens.len(), 1);
    let actual_sid = unsafe { get_user_sid_bytes(retained._tokens[0].as_raw_handle() as HANDLE) }?;
    assert_eq!(actual_sid, expected_sid);
    assert!(retained.contains_sid(&expected_sid));
    assert!(!retained.contains_sid(&[]));
    Ok(())
}
