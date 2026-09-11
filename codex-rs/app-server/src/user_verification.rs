//! Dispatches local verification onto one bounded native worker per service.
//! Request guards remain active through response delivery; registration is a later integration.

use crate::connection_rpc_gate::ConnectionRpcGate;
use crate::transport::ConnectionOrigin;
use codex_app_server_protocol as rpc;
use codex_login::AuthManager;
use codex_user_verification as native;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

#[path = "user_verification_adapter.rs"]
mod adapter;

use adapter::error;
use adapter::native_error;
pub(crate) use adapter::unavailable;
use adapter::unavailable_reason;
use adapter::validate;

pub(crate) enum Operation {
    Status,
    Enroll,
    Delete,
    Verify(rpc::UserVerificationVerifyParams),
}

/// Shared local provider dependencies and a bounded native worker slot.
/// The slot stays with the worker until it exits, including after RPC cancellation.
pub(crate) struct Service {
    auth_manager: Arc<AuthManager>,
    provider: Arc<
        dyn Fn(native::UserVerificationKeyNamespace) -> Arc<dyn native::UserVerificationProvider>
            + Send
            + Sync,
    >,
    platform_supported: bool,
    pub(crate) device_supported: fn() -> bool,
    worker: Arc<Semaphore>,
}

impl Service {
    pub(crate) fn new(auth_manager: Arc<AuthManager>) -> Self {
        Self {
            auth_manager,
            provider: Arc::new(native::platform_provider),
            platform_supported: native::platform_supported(),
            device_supported: native::device_supported,
            worker: Arc::new(Semaphore::new(/*permits*/ 1)),
        }
    }

    pub(crate) async fn handle(
        &self,
        operation: Operation,
        connection: Arc<ConnectionRpcGate>,
        cancellation: CancellationToken,
        origin: ConnectionOrigin,
    ) -> Result<GuardedResponse, rpc::JSONRPCErrorError> {
        // A network peer must sign on its own device. Stdio belongs to the local
        // parent process; in-process calls belong to the embedded application.
        if matches!(
            origin,
            ConnectionOrigin::WebSocket | ConnectionOrigin::RemoteControl
        ) && !matches!(operation, Operation::Status)
        {
            return Err(unavailable());
        }
        let operation = match operation {
            Operation::Verify(params) => NativeOperation::Verify(validate(params)?),
            Operation::Status => NativeOperation::Status,
            Operation::Enroll => NativeOperation::Enroll,
            Operation::Delete => NativeOperation::Delete,
        };
        let auth_manager = Arc::clone(&self.auth_manager);
        let auth_changes = auth_manager.auth_change_receiver();
        let auth_revision = *auth_changes.borrow();
        let Some(identity) = identity(&auth_manager).filter(|_| self.platform_supported) else {
            return match operation {
                NativeOperation::Status => Ok(GuardedResponse {
                    payload: rpc::UserVerificationStatusResponse {
                        credential_id: None,
                        unavailable_reason: Some(
                            rpc::UserVerificationUnavailableReason::ProviderUnavailable,
                        ),
                        unavailable_message: Some(
                            "User verification is not available in this build or account.".into(),
                        ),
                    }
                    .into(),
                    guard: CancelOnDrop(native::UserVerificationRequestGuard::default()),
                }),
                NativeOperation::Enroll | NativeOperation::Delete | NativeOperation::Verify(_) => {
                    Err(unavailable())
                }
            };
        };
        let provider = (self.provider)(native::UserVerificationKeyNamespace::new(&identity));
        let guard = native::UserVerificationRequestGuard::with_activity_check(move || {
            !connection.is_closed()
                && !cancellation.is_cancelled()
                && *auth_changes.borrow() == auth_revision
                && self::identity(&auth_manager).as_ref() == Some(&identity)
        });
        run(operation, provider, guard, Arc::clone(&self.worker)).await
    }
}

pub(crate) struct GuardedResponse {
    pub(crate) payload: rpc::ClientResponsePayload,
    guard: CancelOnDrop,
}

impl GuardedResponse {
    pub(crate) fn into_parts(
        self,
    ) -> (
        rpc::ClientResponsePayload,
        impl FnOnce() -> Result<(), rpc::JSONRPCErrorError>,
    ) {
        (self.payload, move || {
            self.guard.0.check().map_err(native_error)
        })
    }
}

fn identity(auth_manager: &AuthManager) -> Option<String> {
    auth_manager.auth_cached()?.get_chatgpt_account_user_id()
}

enum NativeOperation {
    Status,
    Enroll,
    Delete,
    Verify(native::UserVerificationRequest),
}

/// Cancels blocking work when its RPC future is dropped or times out.
struct CancelOnDrop(native::UserVerificationRequestGuard);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

async fn run(
    operation: NativeOperation,
    provider: Arc<dyn native::UserVerificationProvider>,
    guard: native::UserVerificationRequestGuard,
    worker: Arc<Semaphore>,
) -> Result<GuardedResponse, rpc::JSONRPCErrorError> {
    let cancel = CancelOnDrop(guard.clone());
    let permit = worker.try_acquire_owned().map_err(|_| {
        error(
            rpc::UserVerificationErrorDetails::Failed {
                reason: rpc::UserVerificationFailureReason::ProviderError,
            },
            "Another user verification operation is still running.",
        )
    })?;
    let (result_tx, work) = oneshot::channel();
    // Native APIs may block even after cancellation. They must not occupy Tokio's
    // blocking pool (which runtime shutdown joins), or create unbounded workers.
    std::thread::Builder::new()
        .name("user-verification".into())
        .spawn(move || {
            let result = (|| {
                guard.check().map_err(native_error)?;
                let result: rpc::ClientResponsePayload = match operation {
                    NativeOperation::Status => {
                        let status = provider.status(&guard).map_err(native_error)?;
                        rpc::UserVerificationStatusResponse {
                            credential_id: status.credential.map(|key| key.credential_id),
                            unavailable_reason: status.unavailable_reason.map(unavailable_reason),
                            unavailable_message: status.unavailable_message,
                        }
                        .into()
                    }
                    NativeOperation::Enroll => {
                        let key = provider.ensure_key(&guard).map_err(native_error)?;
                        // The trusted caller owns backend registration and can use verify
                        // to sign the enrollment challenge with this local credential.
                        rpc::UserVerificationEnrollResponse {
                            credential_id: key.credential.credential_id,
                            algorithm: Some(key.credential.algorithm),
                            public_key: Some(key.credential.public_key),
                        }
                        .into()
                    }
                    NativeOperation::Delete => {
                        // TODO: retain credential references and revoke server registrations when
                        // backend enrollment is connected. Current enroll only creates local keys.
                        provider.delete(&guard).map_err(native_error)?;
                        rpc::UserVerificationDeleteResponse {}.into()
                    }
                    NativeOperation::Verify(request) => {
                        let proof = provider.verify(&request, &guard).map_err(native_error)?;
                        rpc::UserVerificationVerifyResponse {
                            proof: rpc::UserVerificationProof {
                                credential_id: proof.credential_id,
                                signature: proof.signature,
                            },
                        }
                        .into()
                    }
                };
                guard.check().map_err(native_error)?;
                Ok(result)
            })();
            drop(permit);
            let _ = result_tx.send(result);
        })
        .map_err(|_| {
            error(
                rpc::UserVerificationErrorDetails::Failed {
                    reason: rpc::UserVerificationFailureReason::ProviderError,
                },
                "User verification could not start.",
            )
        })?;
    let result = match tokio::time::timeout(Duration::from_secs(/*secs*/ 120), work).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err(error(
            rpc::UserVerificationErrorDetails::Failed {
                reason: rpc::UserVerificationFailureReason::ProviderError,
            },
            "User verification could not complete.",
        )),
        Err(_) => Err(error(
            rpc::UserVerificationErrorDetails::Failed {
                reason: rpc::UserVerificationFailureReason::Timeout,
            },
            "User verification timed out.",
        )),
    };
    // The UI may disconnect or change accounts after the blocking worker completes
    // but before this task receives its result.
    if result.is_ok() {
        cancel.0.check().map_err(native_error)?;
    }
    result.map(|payload| GuardedResponse {
        payload,
        guard: cancel,
    })
}

#[cfg(test)]
#[path = "user_verification_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "user_verification_rpc_tests.rs"]
mod rpc_tests;

#[cfg(test)]
#[path = "user_verification_cancel_tests.rs"]
mod cancel_tests;

#[cfg(test)]
#[path = "user_verification_test_support.rs"]
mod test_support;

#[cfg(test)]
#[path = "user_verification_activation_tests.rs"]
mod activation_tests;

#[cfg(test)]
#[path = "user_verification_connection_tests.rs"]
mod connection_tests;
