//! Completion diagnostics preserve native errors without depending on account mutation.

use super::*;
use pretty_assertions::assert_eq;

const ACCESS_DENIED: HRESULT = HRESULT(0x80070005_u32 as i32);
const CANCELED: HRESULT = HRESULT(0x800704C7_u32 as i32);
const ASYNC_FAILURE: HRESULT = HRESULT(0x80073D19_u32 as i32);
const DEPLOYMENT_FAILURE: HRESULT = HRESULT(0x80073D28_u32 as i32);

#[test]
fn retains_different_async_and_deployment_errors_with_activity() {
    let activity_id = GUID::from_u128(0x1b60fffa_59b3_0002_d8d5_611bb359dd01);
    let error = RegistrationResult {
        status: AsyncStatus::Error,
        async_error: Ok(ASYNC_FAILURE),
        result: Ok(DeploymentDetails {
            extended_error: Ok(DEPLOYMENT_FAILURE),
            activity_id: Ok(activity_id),
        }),
    }
    .check(|| false)
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        format!(
            "finish managed runtime package registration: status=3 async_error={ASYNC_FAILURE:?} get_results=0x00000000 activity_id={activity_id:?} phase=deployment extended_error={DEPLOYMENT_FAILURE:?}"
        )
    );
    assert_eq!(
        error.downcast_ref::<windows::core::Error>().unwrap().code(),
        DEPLOYMENT_FAILURE
    );
}

#[test]
fn get_results_errors_keep_original_hresult_and_phase() {
    for (status, code) in [
        (AsyncStatus::Error, ACCESS_DENIED),
        (AsyncStatus::Canceled, CANCELED),
    ] {
        let error = RegistrationResult {
            status,
            async_error: Ok(code),
            result: Err(windows::core::Error::from_hresult(code)),
        }
        .check(|| false)
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            format!(
                "finish managed runtime package registration: status={} async_error={code:?} phase=GetResults get_results={code:?}",
                status.0
            )
        );
        assert_eq!(
            error.downcast_ref::<windows::core::Error>().unwrap().code(),
            code
        );
    }
}

#[test]
fn diagnostic_getter_failures_do_not_mask_completion_or_leak_text() {
    let private_error = || windows::core::Error::new(ACCESS_DENIED, "private deployment path");
    for code in [HRESULT(0), CANCELED] {
        let result = RegistrationResult {
            status: AsyncStatus::Completed,
            async_error: Err(private_error()),
            result: Ok(DeploymentDetails {
                extended_error: code.ok().map(|()| code),
                activity_id: Err(private_error()),
            }),
        }
        .check(|| false);
        if code.is_err() {
            let error = result.unwrap_err();
            assert_eq!(
                error.to_string(),
                format!(
                    "finish managed runtime package registration: status=1 async_error=unavailable({ACCESS_DENIED:?}) get_results=0x00000000 activity_id=unavailable({ACCESS_DENIED:?}) phase=ExtendedErrorCode getter_error={CANCELED:?}"
                )
            );
            assert_eq!(
                error.downcast_ref::<windows::core::Error>().unwrap().code(),
                CANCELED
            );
            assert!(!format!("{error:#}").contains("private deployment path"));
        } else {
            assert_eq!(result.unwrap(), ());
        }
    }
}

#[test]
fn only_reconciles_a_terminal_service_privilege_error_for_the_registered_target() {
    for (status, code, registered, expected) in [
        (AsyncStatus::Error, DEPLOYMENT_FAILURE, true, true),
        (AsyncStatus::Error, DEPLOYMENT_FAILURE, false, false),
        (AsyncStatus::Canceled, DEPLOYMENT_FAILURE, true, false),
        (AsyncStatus::Error, ACCESS_DENIED, true, false),
    ] {
        let completion = RegistrationResult {
            status,
            async_error: Ok(ASYNC_FAILURE),
            result: Ok(DeploymentDetails {
                extended_error: Ok(code),
                activity_id: Ok(GUID::zeroed()),
            }),
        };
        let result = completion.check(|| {
            assert_eq!(status, AsyncStatus::Error);
            assert_eq!(code, DEPLOYMENT_FAILURE);
            registered
        });
        assert_eq!(result.is_ok(), expected);
    }
}
