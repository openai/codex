use super::TurnAdmission;
use crate::error_code::INVALID_REQUEST_ERROR_CODE;
use codex_extension_api::TurnStartAdmission;
use pretty_assertions::assert_eq;

#[test]
fn drain_rejects_new_work_without_waiting_for_admitted_work() {
    let admission = TurnAdmission::default();
    let active = admission.subscribe_active();
    let in_flight = admission.admit().expect("initial request admitted");
    let automatic = admission
        .admit_turn_start()
        .expect("automatic start admitted");
    admission.begin_drain();
    assert_eq!(*active.borrow(), 2);
    assert!(admission.admit_turn_start().is_none());

    let error = admission.admit().err().expect("new request rejected");
    assert_eq!(error.code, INVALID_REQUEST_ERROR_CODE);
    assert_eq!(
        error.data,
        Some(serde_json::json!({ "reason": "serverShuttingDown" }))
    );
    drop(in_flight);
    assert_eq!(*active.borrow(), 1);
    drop(automatic);
    assert_eq!(*active.borrow(), 0);
}

#[tokio::test(start_paused = true)]
async fn workspace_pause_waits_for_admitted_work_and_preserves_shutdown_drain() {
    let admission = TurnAdmission::default();
    let mut resumed = admission.admission_updates().unwrap();
    let in_flight = admission.admit().unwrap();
    let pending_pause = admission.pause();
    tokio::pin!(pending_pause);
    tokio::select! {
        biased;
        _ = &mut pending_pause => panic!("pause completed before the existing request"),
        () = async {} => {}
    }
    assert!(admission.admit().is_err());
    assert!(admission.admit_turn_start().is_none());
    drop(in_flight);
    let pause = pending_pause.await.unwrap();
    drop(pause);
    resumed.changed().await.unwrap();
    drop(admission.admit().unwrap());

    // A timed-out switch must reopen admission and wake work blocked by its pause.
    let in_flight = admission.admit().unwrap();
    let pending_pause = admission.pause();
    tokio::pin!(pending_pause);
    tokio::select! {
        biased;
        _ = &mut pending_pause => panic!("pause completed before the existing request"),
        () = async {} => {}
    }
    assert!(admission.admit_turn_start().is_none());
    tokio::time::advance(std::time::Duration::from_secs(/*secs*/ 10)).await;
    assert!(pending_pause.await.is_err());
    resumed.changed().await.unwrap();
    drop(admission.admit_turn_start().unwrap());
    drop(in_flight);

    let pause = admission.pause().await.unwrap();
    admission.begin_drain();
    drop(pause);
    assert!(admission.admit().is_err());
    assert!(admission.admit_turn_start().is_none());
    assert!(!resumed.has_changed().unwrap());
}
