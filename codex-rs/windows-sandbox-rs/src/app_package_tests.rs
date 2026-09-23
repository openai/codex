//! Runtime opt-in and bounded OS package-name results, without installed-state changes.

use super::authorize_runner_receipt;
use super::query_package_name;
use super::requested_value;
use crate::installation_record::InstallationRecord;
use crate::runtime_ownership::RuntimeAccountRegistration;
use crate::runtime_ownership::RuntimeRegistration;
use crate::runtime_ownership::SandboxRuntimeAccount;
use pretty_assertions::assert_eq;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::Path;
use windows_sys::Win32::Foundation::APPMODEL_ERROR_NO_PACKAGE;
use windows_sys::Win32::Foundation::ERROR_ACCESS_DENIED;
use windows_sys::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER;
use windows_sys::Win32::Foundation::ERROR_SUCCESS;

#[test]
fn only_exact_opt_in_requests_registered_core() {
    let values = [
        None,
        Some(""),
        Some("0"),
        Some("1"),
        Some("true"),
        Some(" 1"),
        Some("1 "),
    ];
    assert_eq!(
        values.map(|value| requested_value(value.map(OsStr::new))),
        [false, false, false, true, false, false, false]
    );
    assert!(!requested_value(Some(&OsString::from_wide(&[0xd800]))));
}

#[test]
fn runner_receipt_distinguishes_incomplete_setup_from_owner_and_removal() {
    let owner = "S-1-5-21-100";
    let home = Path::new(r"C:\Users\owner\.codex");
    let package = "OpenAI.Codex_current";
    let runtime = RuntimeRegistration {
        package_family: "OpenAI.Codex_family".to_owned(),
        accounts: Vec::new(),
        metadata_roots: Vec::new(),
        ready_package: None,
        retiring: None,
    };
    let mut record = InstallationRecord {
        user_sid: owner.to_owned(),
        codex_home: home.to_owned(),
        session_id: 1,
        desktop_installation: None,
        runtime: Some(runtime),
    };
    let incomplete =
        "registered Core setup is not complete for the installed app; retry sandbox setup";
    assert_eq!(
        authorize_runner_receipt(&record, owner, home, package)
            .unwrap_err()
            .to_string(),
        incomplete
    );
    record.runtime.as_mut().unwrap().retiring = Some("removing".to_owned());
    for (candidate_owner, candidate_home, message) in [
        (
            "S-1-5-21-200",
            home,
            "registered Core setup belongs to another owner",
        ),
        (
            owner,
            Path::new(r"C:\Users\other\.codex"),
            "registered Core setup belongs to another Codex home",
        ),
        (owner, home, "registered Core setup is being removed"),
    ] {
        assert_eq!(
            authorize_runner_receipt(&record, candidate_owner, candidate_home, package)
                .unwrap_err()
                .to_string(),
            message
        );
    }
    let runtime = record.runtime.as_mut().unwrap();
    runtime.retiring = None;
    runtime.accounts = [
        SandboxRuntimeAccount::Offline,
        SandboxRuntimeAccount::Online,
    ]
    .into_iter()
    .map(|account| RuntimeAccountRegistration {
        cleanup_logon_pending: false,
        account,
        user_sid: format!("{account:?}"),
        alias_path: Some(home.join(account.username())),
    })
    .collect();
    runtime.ready_package = Some(package.to_owned());
    assert!(authorize_runner_receipt(&record, owner, home, package).is_ok());
    assert_eq!(
        authorize_runner_receipt(&record, owner, home, "OpenAI.Codex_updated")
            .unwrap_err()
            .to_string(),
        incomplete
    );
}

#[test]
fn package_query_distinguishes_absence_from_failure_and_bounds_allocation() {
    assert_eq!(
        query_package_name(|_, _| APPMODEL_ERROR_NO_PACKAGE, /*max_length*/ 256).unwrap(),
        None
    );
    assert!(query_package_name(|_, _| ERROR_ACCESS_DENIED, /*max_length*/ 256).is_err());
    for max_length in [256, 32768] {
        assert!(
            query_package_name(
                |length, buffer| {
                    assert!(buffer.is_null());
                    unsafe { *length = max_length + 1 };
                    ERROR_INSUFFICIENT_BUFFER
                },
                max_length
            )
            .is_err()
        );
    }
}

#[test]
fn package_query_validates_returned_length_termination_and_utf16() {
    for (value, returned, expected) in [
        (vec![65, 0], 2, Some("A")),
        (vec![65, 0], 0, None),
        (vec![65, 0], 3, None),
        (vec![65, 66], 2, None),
        (vec![65, 0, 66, 0], 4, None),
        (vec![0xd800, 0], 2, None),
    ] {
        let actual = query_package_name(
            |length, buffer| {
                if buffer.is_null() {
                    unsafe { *length = value.len() as u32 };
                    ERROR_INSUFFICIENT_BUFFER
                } else {
                    unsafe {
                        std::ptr::copy_nonoverlapping(value.as_ptr(), buffer, value.len());
                        *length = returned;
                    }
                    ERROR_SUCCESS
                }
            },
            /*max_length*/ 256,
        );
        match expected {
            Some(name) => assert_eq!(actual.unwrap(), Some(name.to_owned())),
            None => assert!(actual.is_err()),
        }
    }
    assert!(
        query_package_name(
            |length, buffer| {
                unsafe { *length = 2 };
                if buffer.is_null() {
                    ERROR_INSUFFICIENT_BUFFER
                } else {
                    ERROR_ACCESS_DENIED
                }
            },
            /*max_length*/ 256
        )
        .is_err()
    );
}
