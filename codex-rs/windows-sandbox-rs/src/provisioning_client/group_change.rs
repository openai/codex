//! Retry only the service's authenticated refusal before dispatch.
//! A lost response may follow a mutation, so the retry's transport errors stay errors.

use std::time::Duration;
use std::time::Instant;

use crate::FramedProvisioningMessage;
use crate::SandboxProvisioningResponse;

pub(super) fn retry(
    request: &FramedProvisioningMessage,
    deadline: Instant,
) -> anyhow::Result<SandboxProvisioningResponse> {
    let mut pipe = loop {
        if let Some(pipe) = super::connect(deadline)? {
            break pipe;
        }
        anyhow::ensure!(
            Instant::now() < deadline,
            "refreshed sandbox service pipe is unavailable"
        );
        std::thread::sleep(Duration::from_millis(25));
    };
    super::exchange_request(&mut pipe, request, deadline)
}
