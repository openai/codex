use super::*;
use pretty_assertions::assert_eq;

#[test]
fn wire_debug_redacts_thread_preview_and_raw_bytes() {
    let thread = wire::Thread {
        preview: Some("synthetic private preview".to_owned()),
        ..Default::default()
    };
    let response = wire::UpdateConfigResponse {
        thread: Some(Bytes::from(thread.encode_to_vec())),
        ..Default::default()
    };

    assert_eq!(format!("{thread:?}"), "Thread([redacted])");
    assert_eq!(format!("{response:?}"), "UpdateConfigResponse([redacted])");
}

#[test]
fn missing_history_mode_uses_legacy_compatibility_default() {
    assert_eq!(
        parse_history_mode(/*value*/ None).unwrap(),
        ThreadHistoryMode::Legacy
    );
}

#[test]
fn external_sandbox_network_access_defaults_only_when_omitted() {
    let decode = |network_access| {
        parse_sandbox(wire::SandboxPolicy {
            value: Some(wire::sandbox_policy::Value::ExternalSandbox(
                wire::ExternalSandbox { network_access },
            )),
        })
    };
    for (value, network_access) in [
        (None, NetworkAccess::Restricted),
        (Some(1), NetworkAccess::Restricted),
        (Some(2), NetworkAccess::Enabled),
    ] {
        assert_eq!(
            decode(value).unwrap(),
            SandboxPolicy::ExternalSandbox { network_access }
        );
    }
    for value in [0, 99] {
        assert!(matches!(
            decode(Some(value)),
            Err(Error::UnsupportedResponse("external network policy"))
        ));
    }
}
