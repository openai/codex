use super::remote_control_processor::RemoteControlRequestProcessor;
use super::remote_control_processor::map_pairing_start_error;
use crate::error_code::INTERNAL_ERROR_CODE;
use crate::error_code::INVALID_REQUEST_ERROR_CODE;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::RemoteControlPairingStartParams;
use pretty_assertions::assert_eq;
use std::io;

#[tokio::test]
async fn pairing_start_returns_internal_error_when_remote_control_is_unavailable() {
    let err = RemoteControlRequestProcessor::new(/*remote_control_handle*/ None)
        .pairing_start(RemoteControlPairingStartParams::default())
        .await
        .expect_err("missing remote control should fail pairing");

    assert_eq!(
        err,
        JSONRPCErrorError {
            code: INTERNAL_ERROR_CODE,
            data: None,
            message: "remote control is unavailable for this app-server".to_string(),
        }
    );
}

#[test]
fn pairing_start_maps_invalid_input_to_invalid_request() {
    assert_eq!(
        map_pairing_start_error(io::Error::new(
            io::ErrorKind::InvalidInput,
            "remote control pairing is unavailable",
        )),
        JSONRPCErrorError {
            code: INVALID_REQUEST_ERROR_CODE,
            data: None,
            message: "remote control pairing is unavailable".to_string(),
        }
    );
}

#[test]
fn pairing_start_maps_backend_failures_to_internal_error() {
    assert_eq!(
        map_pairing_start_error(io::Error::other("remote control pairing failed")),
        JSONRPCErrorError {
            code: INTERNAL_ERROR_CODE,
            data: None,
            message: "remote control pairing failed".to_string(),
        }
    );
}
