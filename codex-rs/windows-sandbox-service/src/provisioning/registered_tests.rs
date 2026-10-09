//! Password-repair admission. Native tokens require the disposable Windows Sandbox fixture.

use std::os::windows::io::FromRawHandle;
use std::os::windows::io::OwnedHandle;

use super::credentials_need_repair;
use crate::registered_runtime::registered_packages;
use anyhow::Result;
use anyhow::anyhow;
use codex_windows_sandbox::InstallationRecord;
use codex_windows_sandbox::RuntimeAccountRegistration;
use codex_windows_sandbox::RuntimeRegistration;
use codex_windows_sandbox::SandboxAccountCredentialMismatch;
use codex_windows_sandbox::SandboxRuntimeAccount;
use codex_windows_sandbox::SandboxRuntimeAccount::Offline;
use codex_windows_sandbox::SandboxRuntimeAccount::Online;
use codex_windows_sandbox::resolve_sid;
use codex_windows_sandbox::string_from_sid_bytes;
use codex_windows_sandbox::to_wide;
use pretty_assertions::assert_eq;
use windows_sys::Win32::Security::LOGON32_LOGON_INTERACTIVE;
use windows_sys::Win32::Security::LOGON32_PROVIDER_DEFAULT;
use windows_sys::Win32::Security::LogonUserW;

fn recorded_installation(accounts: &[SandboxRuntimeAccount]) -> InstallationRecord {
    InstallationRecord {
        user_sid: "S-1-5-21-0-0-0-1000".into(),
        codex_home: std::path::PathBuf::from(r"C:\codex-registered-test"),
        session_id: 0,
        desktop_installation: None,
        runtime: Some(RuntimeRegistration {
            package_family: "Codex.SandboxFixture_8wekyb3d8bbwe".into(),
            accounts: accounts
                .iter()
                .map(|&account| RuntimeAccountRegistration {
                    cleanup_logon_pending: false,
                    account,
                    user_sid: format!("test-{account:?}"),
                    alias_path: None,
                })
                .collect(),
            metadata_roots: Vec::new(),
            ready_package: None,
            retiring: None,
        }),
    }
}

#[test]
fn unrelated_errors_never_admit_password_repair() {
    for records in [vec![], vec![Offline], vec![Offline, Online]] {
        let record = recorded_installation(&records);
        for policy_failure in [Offline, Online] {
            let error = credentials_need_repair(
                &record,
                |account| {
                    if account == policy_failure {
                        Err(anyhow!("policy"))
                    } else {
                        Err(anyhow!(SandboxAccountCredentialMismatch))
                    }
                },
                registered_packages,
            )
            .unwrap_err();
            assert_eq!(error.to_string(), "policy");
        }
    }
}

#[test]
fn malformed_records_never_admit_password_repair() {
    for (accounts, expected) in [
        (
            vec![Offline, Offline],
            "runtime registration record contains duplicate accounts",
        ),
        (
            vec![Online, Online],
            "runtime registration record contains duplicate accounts",
        ),
        (
            vec![Offline, Online, Offline],
            "runtime registration record has too many accounts",
        ),
    ] {
        let error = credentials_need_repair(
            &recorded_installation(&accounts),
            |_| Err(anyhow!(SandboxAccountCredentialMismatch)),
            registered_packages,
        )
        .unwrap_err();
        assert_eq!(error.to_string(), expected);
    }
}

// Only in a disposable Windows Sandbox with newly created Offline/Online accounts.
// The fixture sets SANDBOX_REPAIR_TEST_GROUPS to healthy, administrator, or missing;
// never read or modify real credential stores.
#[test]
#[ignore = "requires disposable Windows Sandbox accounts and group fixture"]
fn native_successful_sibling_validation() -> Result<()> {
    assert_eq!(
        std::env::var("USERNAME").as_deref(),
        Ok("WDAGUtilityAccount")
    );
    // Match PackageLifecycle::new before making native PackageManager queries.
    unsafe {
        windows::Win32::System::WinRT::RoInitialize(
            windows::Win32::System::WinRT::RO_INIT_MULTITHREADED,
        )?
    };
    let groups = std::env::var("SANDBOX_REPAIR_TEST_GROUPS")?;
    let mut record = recorded_installation(&[Offline, Online]);
    for entry in &mut record.runtime_mut()?.accounts {
        entry.user_sid = string_from_sid_bytes(&resolve_sid(entry.account.username())?)
            .map_err(anyhow::Error::msg)?;
    }

    if groups == "healthy" {
        // The fixture just made these accounts: no logon or LoadUserProfileW has run.
        // Query both real SIDs, before profiles exist, through the production WinRT API.
        assert!(credentials_need_repair(
            &recorded_installation(&[]),
            |_| Err(anyhow!(SandboxAccountCredentialMismatch)),
            registered_packages,
        )?);

        // An absent record must not mask a same-family registration or query error
        // for either unrecorded account. Don't install packages to induce the failure.
        for unrecorded in [Offline, Online] {
            let known = record
                .runtime()?
                .accounts
                .iter()
                .find(|entry| entry.account == unrecorded)
                .unwrap();
            for accounts in [
                vec![],
                vec![if unrecorded == Offline {
                    Online
                } else {
                    Offline
                }],
            ] {
                let mut partial = record.clone();
                partial
                    .runtime_mut()?
                    .accounts
                    .retain(|entry| accounts.contains(&entry.account));
                for query_failed in [false, true] {
                    let error = credentials_need_repair(
                        &partial,
                        |_| Err(anyhow!(SandboxAccountCredentialMismatch)),
                        |sid, family| {
                            if sid == known.user_sid {
                                assert_eq!(family, record.runtime()?.package_family);
                                if query_failed {
                                    Err(anyhow!("fixture PackageManager query failed"))
                                } else {
                                    Ok(vec![
                                        "Codex.SandboxFixture_1.0.0.0_x64__8wekyb3d8bbwe".into(),
                                    ])
                                }
                            } else {
                                registered_packages(sid, family)
                            }
                        },
                    )
                    .unwrap_err();
                    let expected = if query_failed {
                        "fixture PackageManager query failed"
                    } else {
                        "sandbox account has a runtime registration not owned by this service"
                    };
                    assert_eq!(error.to_string(), expected);
                }
            }
        }
    }

    let logon = |account: SandboxRuntimeAccount| -> Result<OwnedHandle> {
        let mut token = std::ptr::null_mut();
        let succeeded = unsafe {
            LogonUserW(
                to_wide(account.username()).as_ptr(),
                to_wide(".").as_ptr(),
                to_wide("Codex-fixture-27967!").as_ptr(),
                LOGON32_LOGON_INTERACTIVE,
                LOGON32_PROVIDER_DEFAULT,
                &mut token,
            )
        };
        anyhow::ensure!(
            succeeded != 0,
            "fixture logon: {}",
            std::io::Error::last_os_error()
        );
        Ok(unsafe { OwnedHandle::from_raw_handle(token as _) })
    };
    if groups == "healthy" {
        assert!(!credentials_need_repair(
            &record,
            logon,
            registered_packages
        )?);
    }
    for successful in [Offline, Online] {
        let attempt = |registration: &InstallationRecord| {
            credentials_need_repair(
                registration,
                |account| {
                    if account == successful {
                        logon(account)
                    } else {
                        Err(anyhow!(SandboxAccountCredentialMismatch))
                    }
                },
                registered_packages,
            )
            .map_err(|error| error.to_string())
        };
        match groups.as_str() {
            "healthy" => {
                for records in [vec![], vec![Offline], vec![Online], vec![Offline, Online]] {
                    let mut migration = record.clone();
                    migration
                        .runtime_mut()?
                        .accounts
                        .retain(|entry| records.contains(&entry.account));
                    assert_eq!(
                        attempt(&migration),
                        Ok(true),
                        "migration {records:?}, successful {successful:?}"
                    );
                }
                for changed_account in [Offline, Online] {
                    let mut changed = record.clone();
                    changed
                        .runtime_mut()?
                        .accounts
                        .iter_mut()
                        .find(|entry| entry.account == changed_account)
                        .unwrap()
                        .user_sid = "S-1-5-21-0-0-0-999".into();
                    let expected = if changed_account == successful {
                        "managed runtime account SID changed"
                    } else {
                        "managed runtime account was replaced"
                    };
                    assert_eq!(attempt(&changed), Err(expected.into()));
                }
            }
            "administrator" => assert_eq!(
                attempt(&record),
                Err("managed runtime account is an administrator".into())
            ),
            "missing" => assert_eq!(
                attempt(&record),
                Err("runtime account is not in CodexSandboxUsers".into())
            ),
            _ => panic!("unknown fixture group state: {groups}"),
        }
    }
    Ok(())
}
