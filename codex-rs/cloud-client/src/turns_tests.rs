//! Independent wire fixtures for admission, addressed interruption and uncertain outcomes.
use super::*;
use pretty_assertions::assert_eq;

fn request() -> AddUserMessageRequest {
    AddUserMessageRequest {
        thread_id: "t".into(),
        client_user_message_id: "m".into(),
        text: "hello".into(),
        turn_trigger: None,
        mode: SubmissionMode::StartIfIdle,
        shared_access: true,
    }
}
fn admitted(outcome: u8) -> Vec<u8> {
    [
        message(/*tag*/ 1, b"\x22\x01u\x40\x04"),
        vec![0x10, outcome, 0x1a, 1, b'm'],
    ]
    .concat()
}
#[tokio::test]
async fn submission_preserves_admission_variants_and_wire_identity() {
    for (mode, mode_number, outcome_number) in [
        (SubmissionMode::StartIfIdle, 3, 1),
        (SubmissionMode::Steer, 1, 2),
        (SubmissionMode::Queue, 2, 3),
        (SubmissionMode::Queue, 2, 4),
    ] {
        let mut payload = admitted(outcome_number);
        if outcome_number == 4 {
            payload = [
                vec![0x10, 4, 0x1a, 1, b'm'],
                message(/*tag*/ 5, b"\x0a\x01q\x1a\x01m"),
            ]
            .concat();
        }
        payload.extend([0xa0, 0x06, 7]); // Unknown metadata survives.
        let mut fixture = fixture(response(vec![frame(&payload)], tonic::Status::ok(""))).await;
        let turn_trigger = (mode_number == 3).then(|| "safety_recourse_follow_up".to_owned());
        let result = fixture
            .client
            .add_user_message(&AddUserMessageRequest {
                turn_trigger,
                mode,
                ..request()
            })
            .await
            .unwrap();
        let turn = TurnSummary {
            id: "u".into(),
            status: TurnStatus::InProgress,
            error: None,
            started_at: None,
            completed_at: None,
            duration_ms: None,
        };
        let expected = match outcome_number {
            1 => AdmissionOutcome::Started(turn),
            2 => AdmissionOutcome::Steered(turn),
            3 => AdmissionOutcome::Duplicate {
                turn_id: Some("u".into()),
            },
            4 => AdmissionOutcome::Queued {
                submission_id: "q".into(),
            },
            _ => unreachable!(),
        };
        assert_eq!(
            (
                &result.thread_id,
                &result.client_user_message_id,
                &result.outcome,
                result.payload()
            ),
            (
                &"t".to_owned(),
                &"m".to_owned(),
                &expected,
                &Bytes::from(payload)
            )
        );
        assert_eq!(
            format!("{result:?} {:?}", result.outcome),
            "Admission([redacted]) AdmissionOutcome([redacted])"
        );
        let expected_request = [
            message(/*tag*/ 4, b"m"),
            message(
                /*tag*/ 11,
                &message(
                    /*tag*/ 1,
                    &message(/*tag*/ 1, &message(/*tag*/ 1, b"hello")),
                ),
            ),
            message(/*tag*/ 23, b"t"),
            if mode_number == 3 {
                message(/*tag*/ 25, b"safety_recourse_follow_up")
            } else {
                Vec::new()
            },
            vec![0xd0, 1, 1, 0xf0, 1, mode_number],
        ]
        .concat();
        assert_request(&mut fixture, "AddUserMessage", &expected_request).await;
    }
}
#[tokio::test]
async fn duplicate_can_omit_turn_but_malformed_admission_is_unknown() {
    let payloads = vec![
        vec![],
        admitted(/*outcome*/ 0),
        admitted(/*outcome*/ 99),
        b"\x10\x01\x1a\x01m".to_vec(),
        // Started with a turn ID but no turn status.
        b"\x0a\x03\x22\x01u\x10\x01\x1a\x01m".to_vec(),
        b"\x10\x04\x1a\x01m".to_vec(),
        b"\x10\x03\x1a\x01x".to_vec(),
        // A present duplicate turn must have a nonblank ID.
        b"\x0a\x00\x10\x03\x1a\x01m".to_vec(),
        b"\x0a\x02\x22\x00\x10\x03\x1a\x01m".to_vec(),
        b"\x0a\x03\x22\x01 \x10\x03\x1a\x01m".to_vec(),
        vec![0x80],
        [
            vec![0x10, 4, 0x1a, 1, b'm'],
            message(/*tag*/ 5, b"\x0a\x01q\x1a\x01x"),
        ]
        .concat(),
    ];
    for payload in payloads {
        let mut fixture = fixture(response(vec![frame(&payload)], tonic::Status::ok(""))).await;
        assert!(matches!(
            fixture.client.add_user_message(&request()).await,
            Err(MutationError::OutcomeUnknown(Error::InvalidResponse))
        ));
        fixture.requests.recv().await.unwrap();
        assert!(fixture.requests.try_recv().is_err());
    }
    // Duplicate identity is independent of absent or unknown lifecycle state.
    for turn in [&b"\x22\x01u"[..], &b"\x22\x01u\x40\x63"[..]] {
        let payload = [message(/*tag*/ 1, turn), b"\x10\x03\x1a\x01m".to_vec()].concat();
        let fixture = fixture(response(vec![frame(&payload)], tonic::Status::ok(""))).await;
        assert_eq!(
            fixture
                .client
                .add_user_message(&request())
                .await
                .unwrap()
                .outcome,
            AdmissionOutcome::Duplicate {
                turn_id: Some("u".into())
            }
        );
    }
    let fixture = fixture(response(
        vec![frame(b"\x10\x03\x1a\x01m")],
        tonic::Status::ok(""),
    ))
    .await;
    assert_eq!(
        fixture
            .client
            .add_user_message(&request())
            .await
            .unwrap()
            .outcome,
        AdmissionOutcome::Duplicate { turn_id: None }
    );
}
#[tokio::test]
async fn interrupt_addresses_original_turn_and_preserves_uncertain_errors() {
    for status in [
        tonic::Status::ok(""),
        tonic::Status::permission_denied("private"),
        tonic::Status::unavailable("private"),
    ] {
        let code = status.code() as i32;
        let mut fixture = fixture(response(vec![frame(&[])], status)).await;
        let result = fixture
            .client
            .interrupt_turn(&InterruptTurnRequest {
                thread_id: "t".into(),
                turn_id: "u".into(),
                shared_access: true,
            })
            .await;
        if code == 0 {
            result.unwrap();
        } else {
            let error = result.unwrap_err();
            assert!(!format!("{error:?} {error}").contains("private"));
            let MutationError::OutcomeUnknown(Error::Rpc(status)) = error else {
                panic!("expected unknown outcome")
            };
            assert_eq!((status.code, status.message), (code, "private".into()));
        }
        assert_request(&mut fixture, "InterruptTurn", b"\x0a\x01t\x12\x01u\x18\x01").await;
        assert!(fixture.requests.try_recv().is_err());
    }
}
#[tokio::test]
async fn invalid_input_never_sends_and_diagnostics_redact() {
    let mut fixture = fixture(response(vec![frame(&[])], tonic::Status::ok(""))).await;
    for input in [
        AddUserMessageRequest {
            text: " ".into(),
            ..request()
        },
        AddUserMessageRequest {
            client_user_message_id: "".into(),
            ..request()
        },
        AddUserMessageRequest {
            thread_id: "".into(),
            ..request()
        },
    ] {
        assert_eq!(format!("{input:?}"), "AddUserMessageRequest([redacted])");
        assert!(matches!(
            fixture.client.add_user_message(&input).await,
            Err(MutationError::InvalidRequest)
        ));
    }
    let input = InterruptTurnRequest {
        thread_id: "private".into(),
        turn_id: "".into(),
        shared_access: false,
    };
    assert_eq!(format!("{input:?}"), "InterruptTurnRequest([redacted])");
    assert!(matches!(
        fixture.client.interrupt_turn(&input).await,
        Err(MutationError::InvalidRequest)
    ));
    for thread_id in ["".to_owned(), " ".to_owned(), "t".repeat(MAX_MESSAGE_BYTES)] {
        assert!(matches!(
            fixture.client.activate(&thread_id).await,
            Err(MutationError::InvalidRequest)
        ));
    }
    assert!(fixture.requests.try_recv().is_err());
}
#[tokio::test]
async fn dropping_pending_submission_does_not_send_a_retry_or_interrupt() {
    let (sender, dropped) = tokio::sync::oneshot::channel::<()>();
    let stream = async_stream::stream! {
        let _sender = sender;
        yield Ok::<_, Infallible>(Frame::data(frame(&admitted(/*outcome*/ 1))));
        std::future::pending::<()>().await; // No final status yet: admission remains uncertain.
    };
    let mut fixture = fixture(
        http::Response::builder()
            .header("content-type", "application/grpc")
            .body(Body::new(StreamBody::new(stream)))
            .unwrap(),
    )
    .await;
    {
        let request = request();
        let call = fixture.client.add_user_message(&request);
        tokio::pin!(call);
        tokio::select! {
            result = &mut call => panic!("completed before trailers: {result:?}"),
            observed = fixture.requests.recv() => { observed.unwrap(); }
        }
    }
    assert!(
        tokio::time::timeout(Duration::from_secs(5), dropped)
            .await
            .unwrap()
            .is_err()
    );
    assert!(fixture.requests.try_recv().is_err());
}
#[tokio::test]
async fn server_rejection_and_transport_status_do_not_prove_no_admission() {
    for status in [
        tonic::Status::invalid_argument("private"),
        tonic::Status::deadline_exceeded("private"),
        tonic::Status::unavailable("private"),
    ] {
        let code = status.code() as i32;
        let mut fixture = fixture(response(vec![], status)).await;
        let error = fixture
            .client
            .add_user_message(&request())
            .await
            .unwrap_err();
        assert!(!format!("{error:?} {error}").contains("private"));
        let MutationError::OutcomeUnknown(Error::Rpc(status)) = error else {
            panic!("expected unknown outcome")
        };
        assert_eq!((status.code, status.message), (code, "private".into()));
        fixture.requests.recv().await.unwrap();
        assert!(fixture.requests.try_recv().is_err());
    }
}

#[tokio::test]
async fn mutations_require_explicit_successful_grpc_status() {
    let mut ok_trailers = http::HeaderMap::new();
    ok_trailers.insert("grpc-status", http::HeaderValue::from_static("0"));
    for (initial_status, trailers, include_message, expect_success) in [
        (None, None, true, false),
        (None, Some(http::HeaderMap::new()), true, false),
        (Some("0"), None, true, false),
        (Some("0"), None, false, false),
        (Some("0"), Some(ok_trailers.clone()), true, false),
        (None, Some(ok_trailers), true, true),
    ] {
        for method in ["AddUserMessage", "InterruptTurn", "UpdateConfig"] {
            let payload = match method {
                "AddUserMessage" => admitted(/*outcome*/ 1),
                "InterruptTurn" => vec![],
                "UpdateConfig" => [
                    // Approval, reviewer, cwd, model, provider and read-only sandbox.
                    b"\x12\x02\x08\x02\x18\x01\x2a\x01/\x52\x01m\x5a\x01p\x7a\x02\x12\x00".to_vec(),
                    message(
                        /*tag*/ 17,
                        b"\x22\x01v\x28\x01\x32\x01/\x48\x00\x72\x01t\x82\x01\x01p\xaa\x01\x00\xda\x01\x01s\xe2\x01\x02\x08\x01\xea\x01\x02\x12\x00\x80\x02\x02",
                    ),
                ].concat(),
                _ => unreachable!(),
            };
            let frames = include_message
                .then(|| Frame::data(frame(&payload)))
                .into_iter()
                .chain(trailers.clone().map(Frame::trailers));
            let mut builder = http::Response::builder().header("content-type", "application/grpc");
            if let Some(status) = initial_status {
                builder = builder.header("grpc-status", status);
            }
            let mut fixture = fixture(
                builder
                    .body(Body::new(StreamBody::new(futures::stream::iter(
                        frames.map(Ok::<_, Infallible>),
                    ))))
                    .unwrap(),
            )
            .await;
            let result = if method == "AddUserMessage" {
                fixture
                    .client
                    .add_user_message(&request())
                    .await
                    .map(|_| ())
            } else if method == "UpdateConfig" {
                fixture.client.activate("t").await.map(|_| ())
            } else {
                fixture
                    .client
                    .interrupt_turn(&InterruptTurnRequest {
                        thread_id: "t".into(),
                        turn_id: "u".into(),
                        shared_access: false,
                    })
                    .await
            };
            if expect_success {
                result.unwrap();
            } else {
                assert!(matches!(
                    result,
                    Err(MutationError::OutcomeUnknown(Error::InvalidResponse))
                ));
            }
            fixture.requests.recv().await.unwrap();
            assert!(fixture.requests.try_recv().is_err());
        }
    }
}
