//! Registration-specific setup serialization and legacy-materialization boundaries.

use super::Payload;
use super::tests::payload_json;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::fs;

#[test]
fn payload_runtime_wire_normalization_preserves_opt_in() {
    let mut payload: Payload = serde_json::from_value(payload_json()).unwrap();
    assert_eq!(payload.runtime, super::SetupRuntime::Legacy);
    let legacy = serde_json::to_value(&payload).unwrap();
    assert!(legacy.get("runtime").is_none());
    let mut explicit = legacy.clone();
    explicit["runtime"] = serde_json::json!("legacy");
    let decoded: Payload = serde_json::from_value(explicit).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), legacy);

    payload.runtime = super::SetupRuntime::Registered;
    let mut registered = legacy.clone();
    registered["runtime"] = serde_json::json!("registered");
    assert_eq!(serde_json::to_value(&payload).unwrap(), registered);
    let decoded: Payload = serde_json::from_value(registered.clone()).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), registered);

    for invalid in [
        serde_json::Value::Null,
        serde_json::json!(true),
        serde_json::json!("unverified"),
    ] {
        let mut malformed = legacy.clone();
        malformed["runtime"] = invalid;
        assert!(serde_json::from_value::<Payload>(malformed).is_err());
    }
}

#[test]
fn registered_payload_never_creates_or_locks_legacy_bin() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut json = payload_json();
    json["codex_home"] = json!(tmp.path());
    json["runtime"] = json!("registered");
    let payload: Payload = serde_json::from_value(json).expect("registered payload");
    // An empty SID would fail any ACL operation; Registered must not attempt one.
    super::lock_sandbox_bin_dir(&payload, &[]).expect("no legacy bin work");
    let bin = super::sandbox_bin_dir(tmp.path());
    assert!(!bin.exists());
    fs::create_dir(&bin).expect("existing legacy bin");
    let sentinel = bin.join("sentinel");
    fs::write(&sentinel, b"unchanged").expect("existing content");
    super::lock_sandbox_bin_dir(&payload, &[]).expect("leave existing bin alone");
    assert_eq!(fs::read(sentinel).expect("read sentinel"), b"unchanged");
}

#[test]
fn unverified_payload_cannot_create_legacy_bin() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut json = payload_json();
    json["codex_home"] = json!(tmp.path());
    json["runtime"] = json!("unverified");
    assert!(serde_json::from_value::<Payload>(json).is_err());
    assert!(!super::sandbox_bin_dir(tmp.path()).exists());
}

#[test]
fn helper_account_gate_covers_actual_provisioning_modes() {
    for (mode, expected) in [
        (super::SetupMode::Full, [true, false]),
        (super::SetupMode::InteractiveProvision, [true, true]),
        (super::SetupMode::ProvisionOnly, [true, true]),
        (super::SetupMode::ReadAclsOnly, [false, false]),
    ] {
        assert_eq!(
            [false, true].map(|refresh_only| mode.provisions_accounts(refresh_only)),
            expected
        );
    }
}

#[test]
fn parsed_provision_only_cannot_claim_the_private_service_entry() {
    let mut json = payload_json();
    json["mode"] = json!("provision-only");
    json["runtime"] = json!("registered");
    json["refresh_only"] = json!(true);
    json["service_admitted"] = json!(true);
    let payload: Payload = serde_json::from_value(json).expect("payload");
    assert!(payload.mode.provisions_accounts(payload.refresh_only));
    assert_ne!(payload.runtime, super::SetupRuntime::Legacy);
}
