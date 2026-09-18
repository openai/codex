//! Tests owner admission, readiness receipts, and backward-compatible record decoding.

use super::*;
use crate::DesktopInstallation;
use pretty_assertions::assert_eq;
use std::path::PathBuf;

fn record() -> InstallationRecord {
    InstallationRecord {
        user_sid: "S-1-5-21-1-2-3-1000".into(),
        codex_home: PathBuf::from(r"C:\Users\owner\.codex"),
        session_id: 1,
        desktop_installation: Some(DesktopInstallation {
            created_codex_home: true,
            cache_home: PathBuf::from(r"C:\Users\owner\.cache"),
        }),
        runtime: Some(RuntimeRegistration {
            package_family: "OpenAI.Codex_publisher".into(),
            metadata_roots: Vec::new(),
            ready_package: None,
            accounts: vec![RuntimeAccountRegistration {
                cleanup_logon_pending: false,
                account: SandboxRuntimeAccount::Offline,
                user_sid: "S-1-5-21-1-2-3-1001".into(),
                alias_path: None,
            }],
            retiring: None,
        }),
    }
}

fn pending() -> InstallationRecord {
    let mut record = record();
    record.runtime_mut().unwrap().retiring = Some("one-cleanup-generation".into());
    record
}

const READY_PACKAGE: &str = "OpenAI.Codex_1.0.0.0_arm64__publisher";

fn ready_runtime() -> RuntimeRegistration {
    let mut runtime = record().runtime.unwrap();
    runtime.ready_package = Some(READY_PACKAGE.into());
    runtime.accounts[0].alias_path = Some(PathBuf::from(r"C:\aliases\offline.exe"));
    runtime.accounts.push(RuntimeAccountRegistration {
        cleanup_logon_pending: false,
        account: SandboxRuntimeAccount::Online,
        user_sid: "S-1-5-21-1-2-3-1002".into(),
        alias_path: Some(PathBuf::from(r"C:\aliases\online.exe")),
    });
    runtime
}

#[test]
fn readiness_requires_a_receipt_for_the_current_package_version() {
    let mut runtime = ready_runtime();
    assert!(runtime.ready_for_package(READY_PACKAGE));
    assert!(runtime.can_resume_registration());
    assert!(!runtime.ready_for_package("OpenAI.Codex_2.0.0.0_arm64__publisher"));

    runtime.ready_package = None;
    assert!(!runtime.ready_for_package(READY_PACKAGE));
    assert!(runtime.can_resume_registration());
}

#[test]
fn readiness_requires_both_distinct_account_receipts() {
    let mut missing_account = ready_runtime();
    missing_account.accounts.pop();
    let mut missing_alias = ready_runtime();
    missing_alias.accounts[1].alias_path = None;
    let mut duplicate_role = ready_runtime();
    duplicate_role.accounts[1].account = SandboxRuntimeAccount::Offline;
    let mut extra_account = ready_runtime();
    extra_account
        .accounts
        .push(extra_account.accounts[0].clone());

    for runtime in [
        missing_account,
        missing_alias,
        duplicate_role,
        extra_account,
    ] {
        assert!(!runtime.ready_for_package(READY_PACKAGE));
        assert!(!runtime.can_resume_registration());
    }
}

#[test]
fn readiness_is_revoked_by_the_retirement_fence() {
    let mut runtime = ready_runtime();
    runtime.retiring = Some("one-cleanup-generation".into());
    assert!(!runtime.ready_for_package(READY_PACKAGE));
    assert!(!runtime.can_resume_registration());
}

#[test]
fn admission_accepts_package_family_casing() {
    for stored in ["OpenAI.Codex_publisher", "openai.codex_PUBLISHER"] {
        let mut record = record();
        record.runtime_mut().unwrap().package_family = stored.into();
        let expected = record.clone();
        for requested in ["OpenAI.Codex_publisher", "OPENAI.CODEX_PUBLISHER"] {
            record.admit_owner(&record.clone(), requested).unwrap();
            assert_eq!(record, expected);
        }
    }
}

#[test]
fn admission_rejects_other_owner_home_and_family() {
    let record = record();
    let expected = record.clone();
    for (home, sid, family) in [
        (
            record.codex_home.as_path(),
            "S-1-5-21-1-2-3-2000",
            record.runtime().unwrap().package_family.as_str(),
        ),
        (
            std::path::Path::new(r"C:\Users\owner\other"),
            record.user_sid.as_str(),
            record.runtime().unwrap().package_family.as_str(),
        ),
        (
            record.codex_home.as_path(),
            record.user_sid.as_str(),
            "OpenAI.CodexBeta_publisher",
        ),
        (
            record.codex_home.as_path(),
            record.user_sid.as_str(),
            "OpenAI.Codex_otherpublisher",
        ),
    ] {
        let mut requested = record.clone();
        requested.codex_home = home.to_path_buf();
        requested.user_sid = sid.to_owned();
        assert!(record.admit_owner(&requested, family).is_err());
        assert_eq!(record, expected);
    }
}

#[test]
fn returned_owner_cannot_cancel_a_persisted_retirement_fence() {
    let expected = pending();
    let journal = serde_json::to_string(&expected).unwrap();
    let restored: InstallationRecord = serde_json::from_str(&journal).unwrap();
    let returning_owner = record();
    assert!(
        restored
            .admit_owner(
                &returning_owner,
                &returning_owner.runtime().unwrap().package_family
            )
            .is_err()
    );
    assert_eq!(restored, expected);
}

#[test]
fn journal_round_trip_retains_desktop_ownership_and_retirement_fence() {
    for record in [record(), pending()] {
        let json = serde_json::to_string(&record).unwrap();
        assert_eq!(
            serde_json::from_str::<InstallationRecord>(&json).unwrap(),
            record
        );
    }
}

#[test]
fn production_record_without_optional_core_or_desktop_fields_still_loads() {
    let mut expected = record();
    expected.runtime = None;
    expected.desktop_installation = None;
    let json = r#"{"user_sid":"S-1-5-21-1-2-3-1000","codex_home":"C:\\Users\\owner\\.codex","session_id":1}"#;
    assert_eq!(
        serde_json::from_str::<InstallationRecord>(json).unwrap(),
        expected
    );
}

#[test]
fn metadata_root_history_survives_record_round_trip() {
    let mut expected = record();
    expected.runtime_mut().unwrap().metadata_roots = vec![
        PathBuf::from(r"C:\Users\owner\AppData\Roaming"),
        PathBuf::from(r"D:\Redirected\Roaming"),
    ];
    for record in [expected.clone(), {
        let mut updated = expected;
        updated.runtime_mut().unwrap().ready_package = Some(READY_PACKAGE.into());
        updated
    }] {
        let json = serde_json::to_string(&record).unwrap();
        assert_eq!(
            serde_json::from_str::<InstallationRecord>(&json).unwrap(),
            record
        );
    }
}

#[test]
fn older_runtime_receipts_default_to_no_metadata_roots_or_cleanup_logon() {
    let expected = record();
    let mut json = serde_json::to_value(&expected).unwrap();
    json["runtime"]["accounts"][0]
        .as_object_mut()
        .unwrap()
        .remove("cleanup_logon_pending");
    assert!(json["runtime"].get("metadata_roots").is_none());
    assert_eq!(
        serde_json::from_value::<InstallationRecord>(json).unwrap(),
        expected
    );
}

#[test]
fn legacy_null_cleanup_alias_preserves_the_active_record() {
    let expected = record();
    let mut json = serde_json::to_value(&expected).unwrap();
    let runtime = json["runtime"].as_object_mut().unwrap();
    runtime.remove("retiring");
    runtime.insert("cleanup".into(), serde_json::Value::Null);

    assert_eq!(
        serde_json::from_value::<InstallationRecord>(json).unwrap(),
        expected
    );
}

#[test]
fn legacy_non_null_cleanup_journal_cannot_silently_drop_its_fence() {
    for stage in ["native_started", "native_complete"] {
        let mut json = serde_json::to_value(record()).unwrap();
        let runtime = json["runtime"].as_object_mut().unwrap();
        runtime.remove("retiring");
        let accounts = runtime["accounts"].clone();
        runtime.insert(
            "cleanup".into(),
            serde_json::json!({
                "id": "one-cleanup-generation",
                "stage": stage,
                "accounts": accounts,
            }),
        );

        assert!(serde_json::from_value::<InstallationRecord>(json).is_err());
    }
}

#[test]
fn interrupted_cleanup_logon_blocks_admission_after_record_reload() {
    let mut original = record();
    original.runtime = Some(ready_runtime());
    original.runtime_mut().unwrap().accounts[0].cleanup_logon_pending = true;
    let mut restored: InstallationRecord =
        serde_json::from_str(&serde_json::to_string(&original).unwrap()).unwrap();
    assert_eq!(restored, original);
    assert!(!restored.runtime().unwrap().ready_for_package(READY_PACKAGE));
    assert!(
        restored
            .admit_owner(&original, "OpenAI.Codex_publisher")
            .is_err()
    );

    restored.runtime_mut().unwrap().accounts[0].cleanup_logon_pending = false;
    assert!(restored.runtime().unwrap().ready_for_package(READY_PACKAGE));
    assert!(
        restored
            .admit_owner(&original, "OpenAI.Codex_publisher")
            .is_ok()
    );
}
