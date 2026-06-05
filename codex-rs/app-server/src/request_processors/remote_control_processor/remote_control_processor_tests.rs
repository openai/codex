use super::*;
use crate::error_code::INTERNAL_ERROR_CODE;
use crate::error_code::INVALID_REQUEST_ERROR_CODE;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn pairing_start_returns_internal_error_when_remote_control_is_unavailable() {
    let err = RemoteControlRequestProcessor::new(/*remote_control_handle*/ None)
        .pairing_start(
            RemoteControlPairingStartParams::default(),
            /*app_server_client_name*/ None,
        )
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

#[test]
fn client_management_maps_user_actionable_errors_to_invalid_request() {
    for kind in [
        io::ErrorKind::InvalidInput,
        io::ErrorKind::NotFound,
        io::ErrorKind::PermissionDenied,
        io::ErrorKind::WouldBlock,
    ] {
        assert_eq!(
            map_client_management_error(io::Error::new(kind, "client management unavailable")),
            JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                data: None,
                message: "client management unavailable".to_string(),
            }
        );
    }
}

#[test]
fn client_management_maps_backend_failures_to_internal_error() {
    assert_eq!(
        map_client_management_error(io::Error::other("client management failed")),
        JSONRPCErrorError {
            code: INTERNAL_ERROR_CODE,
            data: None,
            message: "client management failed".to_string(),
        }
    );
}
