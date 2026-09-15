//! Regression coverage for reply classification and the shared restart deadline/budget.

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn authentication_failure_cannot_write_or_replay_the_request() -> anyhow::Result<()> {
    let pipe = tempfile::tempfile()?;
    let observation = pipe.try_clone()?;
    let request = FramedProvisioningMessage {
        version: crate::PROVISIONING_PROTOCOL_VERSION,
        message: crate::ProvisioningMessage::ProvisionSandboxRequest {
            payload: crate::SandboxProvisioningRequest {
                codex_home: r"C:\test-home".to_owned(),
                registered_core: true,
                refresh_only: true,
                settings: crate::WindowsSandboxProvisioningSettings {
                    proxy_ports: Vec::new(),
                    allow_local_binding: false,
                },
                listeners: crate::WindowsSandboxProxyListeners::default(),
            },
        },
    };
    let error = send(pipe, &request, Instant::now() + Duration::from_secs(60)).unwrap_err();
    assert_eq!(error.to_string(), "authenticate provisioning pipe server");
    assert_eq!(observation.metadata()?.len(), 0);
    Ok(())
}

#[test]
fn permits_two_response_disconnects_but_not_a_third() {
    let deadline = Instant::now() + Duration::from_secs(60);
    let response = Err(io::Error::from(io::ErrorKind::UnexpectedEof).into());
    let mut remaining = MAX_SERVICE_RESTARTS;
    let decisions = (0..3)
        .map(|_| take_restart(&response, &mut remaining, deadline))
        .collect::<Vec<_>>();
    assert_eq!((decisions, remaining), (vec![true, true, false], 0));
}

#[test]
fn accepts_only_response_pipe_disconnect_errors() {
    let deadline = Instant::now() + Duration::from_secs(60);
    for error in [
        io::Error::from(io::ErrorKind::UnexpectedEof),
        io::Error::from(io::ErrorKind::BrokenPipe),
        io::Error::from_raw_os_error(ERROR_BROKEN_PIPE as i32),
        io::Error::from_raw_os_error(ERROR_NO_DATA as i32),
        io::Error::from_raw_os_error(ERROR_PIPE_NOT_CONNECTED as i32),
    ] {
        let response = Err(anyhow::Error::from(error).context("read response"));
        let mut remaining = MAX_SERVICE_RESTARTS;
        assert_eq!(
            (take_restart(&response, &mut remaining, deadline), remaining),
            (true, 1)
        );
    }
}

#[test]
fn explicit_replies_never_consume_a_restart() {
    let deadline = Instant::now() + Duration::from_secs(60);
    for response in [
        SandboxProvisioningResponse::Ok,
        SandboxProvisioningResponse::Unavailable,
        SandboxProvisioningResponse::Error {
            message: "refused".to_owned(),
        },
        SandboxProvisioningResponse::Error {
            message: crate::SANDBOX_GROUP_CHANGED.to_owned(),
        },
    ] {
        let mut remaining = MAX_SERVICE_RESTARTS;
        assert_eq!(
            (
                take_restart(&Ok(response), &mut remaining, deadline),
                remaining
            ),
            (false, MAX_SERVICE_RESTARTS)
        );
    }
}

#[test]
fn timeout_permission_and_protocol_errors_do_not_consume_a_restart() {
    let deadline = Instant::now() + Duration::from_secs(60);
    for error in [
        io::Error::from(io::ErrorKind::TimedOut).into(),
        io::Error::from(io::ErrorKind::PermissionDenied).into(),
        io::Error::from(io::ErrorKind::InvalidData).into(),
        anyhow::anyhow!("unexpected sandbox provisioning response message"),
        serde_json::from_str::<FramedProvisioningMessage>("not JSON")
            .unwrap_err()
            .into(),
    ] {
        let mut remaining = MAX_SERVICE_RESTARTS;
        assert_eq!(
            (
                take_restart(&Err(error), &mut remaining, deadline),
                remaining
            ),
            (false, MAX_SERVICE_RESTARTS)
        );
    }
}

#[test]
fn disconnect_after_original_deadline_is_not_retried() {
    let deadline = Instant::now();
    let response = Err(io::Error::from(io::ErrorKind::UnexpectedEof).into());
    let mut remaining = MAX_SERVICE_RESTARTS;
    assert_eq!(
        (take_restart(&response, &mut remaining, deadline), remaining),
        (false, MAX_SERVICE_RESTARTS)
    );
    assert_eq!(
        remaining_time(deadline).unwrap_err().kind(),
        io::ErrorKind::TimedOut
    );
}
