//! Regression coverage for timing metadata at serialization boundaries.

use std::time::Duration;

use pretty_assertions::assert_eq;

use super::RuntimeResponse;
use crate::CellId;
use crate::FunctionCallOutputContentItem;
/// Runtime serialization distinguishes absent timing from a measured zero.
#[test]
fn code_mode_host_duration_survives_runtime_serialization() {
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
        }
    }
}
