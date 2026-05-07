use super::AppServerAttestationStatus;
use super::app_server_attestation_header_value;
use pretty_assertions::assert_eq;

#[test]
fn app_server_attestation_header_value_wraps_opaque_client_payloads() {
    assert_eq!(
        app_server_attestation_header_value(
            AppServerAttestationStatus::Ok,
            Some("v1.opaque-client-payload"),
        ),
        r#"{"v":1,"s":0,"t":"v1.opaque-client-payload"}"#
    );
}

#[test]
fn app_server_attestation_header_value_reports_app_server_failures() {
    assert_eq!(
        app_server_attestation_header_value(AppServerAttestationStatus::Timeout, None),
        r#"{"v":1,"s":1}"#
    );
    assert_eq!(
        app_server_attestation_header_value(AppServerAttestationStatus::RequestFailed, None),
        r#"{"v":1,"s":2}"#
    );
    assert_eq!(
        app_server_attestation_header_value(AppServerAttestationStatus::RequestCanceled, None),
        r#"{"v":1,"s":3}"#
    );
    assert_eq!(
        app_server_attestation_header_value(AppServerAttestationStatus::MalformedResponse, None),
        r#"{"v":1,"s":4}"#
    );
}
