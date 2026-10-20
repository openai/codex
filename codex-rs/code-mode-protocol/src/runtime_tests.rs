//! Regression coverage for timing metadata at serialization boundaries.

use std::time::Duration;

use pretty_assertions::assert_eq;

use super::MissingCodeModeHostDuration;
use super::RuntimeResponse;
use super::WaitOutcome;
use crate::CellId;
use crate::FunctionCallOutputContentItem;
use crate::host::WireRuntimeResponse;
use crate::host::WireWaitOutcome;

/// Raw runtime serialization preserves absent timing; stdio always carries a
/// measured duration, including zero, without changing the response payload.
#[test]
fn code_mode_host_duration_survives_runtime_and_stdio_serialization() {
    let content_items = vec![FunctionCallOutputContentItem::InputText {
        text: "output".to_string(),
    }];
    for response in [
        RuntimeResponse::Yielded {
            cell_id: CellId::new("yielded-cell".to_string()),
            content_items: content_items.clone(),
            code_mode_host_duration: None,
        },
        RuntimeResponse::Terminated {
            cell_id: CellId::new("terminated-cell".to_string()),
            content_items: content_items.clone(),
            code_mode_host_duration: None,
        },
        RuntimeResponse::Result {
            cell_id: CellId::new("completed-cell".to_string()),
            content_items,
            result_buffered_for: Duration::ZERO,
            error_text: Some("execution failed".to_string()),
            code_mode_host_duration: None,
        },
    ] {
        for duration in [
            None,
            Some(Duration::ZERO),
            Some(Duration::from_nanos(/*nanos*/ 1_234_567_890)),
            Some(Duration::from_nanos(u64::MAX)),
        ] {
            let mut expected = response.clone();
            match &mut expected {
                RuntimeResponse::Yielded {
                    code_mode_host_duration,
                    ..
                }
                | RuntimeResponse::Terminated {
                    code_mode_host_duration,
                    ..
                }
                | RuntimeResponse::Result {
                    code_mode_host_duration,
                    ..
                } => *code_mode_host_duration = duration,
            }

            let payload = serde_json::to_value(&expected).expect("serialize response");
            assert_eq!(
                serde_json::from_value::<RuntimeResponse>(payload).expect("deserialize response"),
                expected
            );

            if duration.is_some() {
                let wire_payload = serde_json::to_value(
                    WireRuntimeResponse::try_from(expected.clone()).expect("timed response"),
                )
                .expect("serialize response over stdio");
                assert_eq!(
                    RuntimeResponse::from(
                        serde_json::from_value::<WireRuntimeResponse>(wire_payload)
                            .expect("deserialize stdio response")
                    ),
                    expected
                );
            }
        }
    }
}

/// Encoding must not turn a missing request measurement into measured zero,
/// including when no live cell remains to supply output.
#[test]
fn stdio_encoding_rejects_untimed_runtime_output() {
    let response = RuntimeResponse::Terminated {
        cell_id: CellId::new("cell".to_string()),
        content_items: Vec::new(),
        code_mode_host_duration: None,
    };
    assert_eq!(
        WireRuntimeResponse::try_from(response.clone()),
        Err(MissingCodeModeHostDuration)
    );
    for outcome in [
        WaitOutcome::LiveCell(response.clone()),
        WaitOutcome::MissingCell(response),
    ] {
        assert_eq!(
            WireWaitOutcome::try_from(outcome),
            Err(MissingCodeModeHostDuration)
        );
    }
}
