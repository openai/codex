use std::collections::HashMap;
use std::io::Cursor;

use codex_app_server_protocol::ToolRequestUserInputAnswer;
use codex_app_server_protocol::ToolRequestUserInputOption;
use codex_app_server_protocol::ToolRequestUserInputParams;
use codex_app_server_protocol::ToolRequestUserInputQuestion;
use codex_app_server_protocol::ToolRequestUserInputResponse;
use pretty_assertions::assert_eq;

use super::prompt_for_answers_with;

#[test]
fn collects_option_and_free_form_answers() {
    let params = ToolRequestUserInputParams {
        thread_id: "thread-1".to_string(),
        turn_id: "turn-1".to_string(),
        item_id: "item-1".to_string(),
        questions: vec![
            ToolRequestUserInputQuestion {
                id: "target".to_string(),
                header: "Target".to_string(),
                question: "Which target?".to_string(),
                is_other: true,
                is_secret: false,
                options: Some(vec![
                    ToolRequestUserInputOption {
                        label: "Core".to_string(),
                        description: "Inspect core".to_string(),
                    },
                    ToolRequestUserInputOption {
                        label: "TUI".to_string(),
                        description: "Inspect TUI".to_string(),
                    },
                ]),
            },
            ToolRequestUserInputQuestion {
                id: "details".to_string(),
                header: "Details".to_string(),
                question: "Anything else?".to_string(),
                is_other: true,
                is_secret: false,
                options: None,
            },
        ],
        auto_resolution_ms: Some(60_000),
    };
    let mut input = Cursor::new(b"2\ninclude snapshots\n");
    let mut output = Vec::new();

    let response = prompt_for_answers_with(&mut input, &mut output, &params).unwrap();

    assert_eq!(
        response,
        ToolRequestUserInputResponse {
            answers: HashMap::from([
                (
                    "target".to_string(),
                    ToolRequestUserInputAnswer {
                        answers: vec!["TUI".to_string()],
                    },
                ),
                (
                    "details".to_string(),
                    ToolRequestUserInputAnswer {
                        answers: vec!["user_note: include snapshots".to_string()],
                    },
                ),
            ]),
        }
    );
    assert_eq!(
        String::from_utf8(output).unwrap(),
        concat!(
            "\n[request_user_input for thread thread-1, turn turn-1]\n",
            "The app-server may auto-resolve this request after 60000 ms.\n",
            "\nTarget: Which target?\n",
            "  1. Core - Inspect core\n",
            "  2. TUI - Inspect TUI\n",
            "  o. Other (free-form)\n",
            "Choose 1-2 or o: ",
            "\nDetails: Anything else?\n",
            "Answer: ",
        )
    );
}

#[test]
fn retries_invalid_selection_and_collects_other_answer() {
    let params = ToolRequestUserInputParams {
        thread_id: "thread-1".to_string(),
        turn_id: "turn-1".to_string(),
        item_id: "item-1".to_string(),
        questions: vec![ToolRequestUserInputQuestion {
            id: "target".to_string(),
            header: "Target".to_string(),
            question: "Which target?".to_string(),
            is_other: true,
            is_secret: false,
            options: Some(vec![ToolRequestUserInputOption {
                label: "Core".to_string(),
                description: "Inspect core".to_string(),
            }]),
        }],
        auto_resolution_ms: None,
    };
    let mut input = Cursor::new(b"9\no\nSDK wrapper\n");
    let mut output = Vec::new();

    let response = prompt_for_answers_with(&mut input, &mut output, &params).unwrap();

    assert_eq!(
        response,
        ToolRequestUserInputResponse {
            answers: HashMap::from([(
                "target".to_string(),
                ToolRequestUserInputAnswer {
                    answers: vec!["user_note: SDK wrapper".to_string()],
                },
            )]),
        }
    );
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("Invalid selection; try again."));
}
