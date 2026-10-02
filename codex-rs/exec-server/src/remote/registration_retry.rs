//! Retry explicit registration conflicts and authentication outages before registration writes.
//! Ambiguous failures must not be replayed: a timed-out request can still replace a newer registration.
//! The enclosing remote-transport future owns cancellation; retries spawn no background work.

use http::StatusCode;
use tokio::time::sleep;
use tracing::warn;

use super::EnvironmentRegistryClient;
use crate::EnvironmentRegistryRegistrationResponse;
use crate::ExecServerError;
use crate::NoiseChannelPublicKey;
use crate::client::registry_recovery_retry_delay;

impl EnvironmentRegistryClient {
    pub(super) async fn register_environment_with_retry(
        &self,
        environment_id: &str,
        executor_public_key: &NoiseChannelPublicKey,
    ) -> Result<EnvironmentRegistryRegistrationResponse, ExecServerError> {
        // Competing executors for the same environment must not retry in lockstep.
        let retry_key = uuid::Uuid::new_v4().to_string();
        let mut attempt = 0_u32;
        loop {
            match self
                .register_environment(environment_id, executor_public_key)
                .await
            {
                Ok(response) => return Ok(response),
                Err(ExecServerError::EnvironmentRegistryHttp { status, code, .. })
                    if matches!(
                        (status, code.as_deref()),
                        (
                            StatusCode::SERVICE_UNAVAILABLE,
                            Some("registration_conflict")
                        ) | (
                            StatusCode::BAD_GATEWAY,
                            Some("authentication_service_unavailable")
                        )
                    ) =>
                {
                    let delay = registry_recovery_retry_delay(&retry_key, attempt);
                    attempt = attempt.saturating_add(1);
                    // Do not log response bodies or transport errors: they can contain credentials.
                    warn!(
                        noise_event = "registration",
                        noise_outcome = "retry",
                        retry_attempt = attempt,
                        retry_delay_ms = delay.as_millis() as u64,
                        status = status.as_u16(),
                        "Noise executor retrying uncommitted registry registration"
                    );
                    sleep(delay).await;
                }
                Err(error) => return Err(error),
            }
        }
    }
}

#[cfg(test)]
#[path = "registration_retry_tests.rs"]
mod tests;
