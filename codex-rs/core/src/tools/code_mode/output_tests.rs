//! Checks code-mode timing headers, including absent and zero host measurements.

use super::*;
use codex_protocol::models::ImageReference;
use pretty_assertions::assert_eq;

#[test]
fn completed_timing_preserves_content_and_distinguishes_zero_from_unavailable() {
    let content = vec![
        FunctionCallOutputContentItem::InputText {
            text: "result".to_string(),
        },
        FunctionCallOutputContentItem::InputImage {
            image: ImageReference::Inline {
                image_url: "data:image/png;base64,image".to_string(),
            },
            detail: None,
        },
        FunctionCallOutputContentItem::InputAudio {
            audio_url: "data:audio/wav;base64,audio".to_string(),
        },
    ];
    for (host_duration, expected_timing) in [
        (
            Some(Duration::from_millis(/*millis*/ 750)),
            "Wall time 1.250 seconds (code-mode 0.750 seconds; overhead 0.500 seconds)",
        ),
        (
            Some(Duration::ZERO),
            "Wall time 1.250 seconds (code-mode 0.000 seconds; overhead 1.250 seconds)",
        ),
        (
            Some(Duration::from_micros(/*micros*/ 1_251_400)),
            "Wall time 1.250 seconds (code-mode 1.251 seconds; overhead -0.001 seconds)",
        ),
        (None, "Wall time 0.8 seconds"),
    ] {
        let mut output = CodeModeToolOutput::new(
            FunctionToolOutput::from_content(content.clone(), Some(false)),
            "Script failed".to_string(),
            Duration::from_millis(/*millis*/ 750),
            host_duration,
        );
        output.set_handler_duration_ms(/*handler_duration_ms*/ 1_250);
        let mut expected_content = vec![FunctionCallOutputContentItem::InputText {
            text: format!("Script failed\n{expected_timing}\nOutput:\n"),
        }];
        expected_content.extend(content.clone());
        let expected = FunctionToolOutput::from_content(expected_content, Some(false));
        for payload in [
            ToolPayload::Custom {
                input: "text('result')".to_string(),
            },
            ToolPayload::Function {
                arguments: r#"{"cell_id":"1"}"#.to_string(),
            },
        ] {
            let response = output.to_response_item("call", &payload);
            assert_eq!(response, expected.to_response_item("call", &payload));
            // Serialization must reuse the completed value, including on retries.
            assert_eq!(output.to_response_item("call", &payload), response);
        }
        assert_eq!(output.success_for_logging(), false);
    }
}
