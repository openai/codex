//! Verifies local key lifecycle, signing, and cancellation without transport concerns.

use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

#[derive(Default)]
struct Provider {
    key: Mutex<Option<native::UserVerificationKeyInfo>>,
    signed: Mutex<Vec<Vec<u8>>>,
    deactivate_on_sign: Option<Arc<AtomicBool>>,
}

impl native::UserVerificationProvider for Provider {
    fn status(
        &self,
        _guard: &native::UserVerificationRequestGuard,
    ) -> Result<native::UserVerificationStatus, native::UserVerificationError> {
        let credential = self.key.lock().unwrap().clone();
        Ok(native::UserVerificationStatus {
            unavailable_reason: credential
                .is_none()
                .then_some(native::UserVerificationUnavailableReason::CredentialMissing),
            unavailable_message: credential.is_none().then(|| "No key".into()),
            credential,
        })
    }
    fn ensure_key(
        &self,
        guard: &native::UserVerificationRequestGuard,
    ) -> Result<native::UserVerificationKeyCreation, native::UserVerificationError> {
        guard.check()?;
        let mut key = self.key.lock().unwrap();
        let created = key.is_none();
        let credential = key
            .get_or_insert_with(|| native::UserVerificationKeyInfo {
                credential_id: "credential".into(),
                algorithm: "ecdsaP256Sha256X962".into(),
                public_key: "public-key".into(),
            })
            .clone();
        Ok(native::UserVerificationKeyCreation {
            created,
            credential,
        })
    }
    fn delete(
        &self,
        guard: &native::UserVerificationRequestGuard,
    ) -> Result<native::UserVerificationKeyDeletion, native::UserVerificationError> {
        guard.check()?;
        Ok(native::UserVerificationKeyDeletion {
            deleted_credential_id: self.key.lock().unwrap().take().map(|key| key.credential_id),
        })
    }
    fn verify(
        &self,
        request: &native::UserVerificationRequest,
        guard: &native::UserVerificationRequestGuard,
    ) -> Result<native::UserVerificationProof, native::UserVerificationError> {
        guard.check()?;
        self.signed.lock().unwrap().push(request.challenge.clone());
        if let Some(active) = &self.deactivate_on_sign {
            active.store(/*val*/ false, Ordering::Release);
        }
        Ok(native::UserVerificationProof {
            credential_id: "credential".into(),
            signature: "signature".into(),
        })
    }
}

#[tokio::test]
async fn local_enrollment_reuses_key_and_status_and_delete_have_no_signing_effects() {
    let provider = Arc::new(Provider::default());
    for _ in 0..2 {
        let response = run(
            NativeOperation::Enroll,
            provider.clone(),
            native::UserVerificationRequestGuard::default(),
            Arc::new(Semaphore::new(/*permits*/ 1)),
        )
        .await
        .unwrap();
        assert_eq!(
            serde_json::to_value(response.payload).unwrap(),
            json!({
                "credentialId": "credential",
                "algorithm": "ecdsaP256Sha256X962",
                "publicKey": "public-key"
            })
        );
    }
    let response = run(
        NativeOperation::Status,
        provider.clone(),
        native::UserVerificationRequestGuard::default(),
        Arc::new(Semaphore::new(/*permits*/ 1)),
    )
    .await
    .unwrap();
    assert_eq!(
        serde_json::to_value(response.payload).unwrap(),
        json!({"credentialId": "credential", "unavailableReason": null, "unavailableMessage": null})
    );
    for _ in 0..2 {
        let response = run(
            NativeOperation::Delete,
            provider.clone(),
            native::UserVerificationRequestGuard::default(),
            Arc::new(Semaphore::new(/*permits*/ 1)),
        )
        .await
        .unwrap();
        assert_eq!(serde_json::to_value(response.payload).unwrap(), json!({}));
    }
    assert_eq!(*provider.key.lock().unwrap(), None);
    assert!(provider.signed.lock().unwrap().is_empty());
}

#[tokio::test]
async fn signing_uses_decoded_bytes_without_a_pending_elicitation() {
    let provider = Arc::new(Provider::default());
    let request = validate(rpc::UserVerificationVerifyParams {
        challenge: "AQID".into(),
        title: "Approve".into(),
        description: "Action".into(),
    })
    .unwrap();
    let response = run(
        NativeOperation::Verify(request),
        provider.clone(),
        native::UserVerificationRequestGuard::default(),
        Arc::new(Semaphore::new(/*permits*/ 1)),
    )
    .await
    .unwrap();
    assert_eq!(
        serde_json::to_value(response.payload).unwrap(),
        json!({"proof": {"credentialId": "credential", "signature": "signature"}})
    );
    assert_eq!(*provider.signed.lock().unwrap(), vec![vec![1, 2, 3]]);
}

#[tokio::test]
async fn identity_change_during_native_prompt_discards_proof() {
    let active = Arc::new(AtomicBool::new(/*v*/ true));
    let provider = Arc::new(Provider {
        deactivate_on_sign: Some(active.clone()),
        ..Default::default()
    });
    let guard = native::UserVerificationRequestGuard::with_activity_check(move || {
        active.load(Ordering::Acquire)
    });
    let response = run(
        NativeOperation::Verify(native::UserVerificationRequest {
            challenge: vec![1],
            title: "Approve".into(),
            description: String::new(),
        }),
        provider,
        guard,
        Arc::new(Semaphore::new(/*permits*/ 1)),
    )
    .await
    .err()
    .unwrap();
    assert_eq!(
        response.data,
        Some(json!({"type": "cancelled", "reason": "interrupted"}))
    );
}

#[tokio::test]
async fn cancelled_operation_cannot_create_a_key() {
    let provider = Arc::new(Provider::default());
    let guard = native::UserVerificationRequestGuard::default();
    guard.cancel();
    let response = run(
        NativeOperation::Enroll,
        provider.clone(),
        guard,
        Arc::new(Semaphore::new(/*permits*/ 1)),
    )
    .await
    .err()
    .unwrap();
    assert_eq!(
        response.data,
        Some(json!({"type": "cancelled", "reason": "interrupted"}))
    );
    assert_eq!(*provider.key.lock().unwrap(), None);
}
