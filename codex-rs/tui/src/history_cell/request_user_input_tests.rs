//! Result rendering preserves answer styles and wrapped unanswered questions.

use super::*;
use codex_app_server_protocol::ToolRequestUserInputOption;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[test]
fn completed_and_interrupted_results() {
    let question = ToolRequestUserInputQuestion {
        id: "choice".into(),
        header: "Approach".into(),
        question: "Which approach should we use for the next step?".into(),
        is_other: true,
        is_secret: false,
        options: Some(vec![ToolRequestUserInputOption {
            label: "Keep it small".into(),
            description: "Use the existing helper".into(),
        }]),
    };
    let mut cell = RequestUserInputResultCell {
        questions: vec![question],
        answers: HashMap::from([(
            "choice".into(),
            ToolRequestUserInputAnswer {
                answers: vec![
                    "Keep it small".into(),
                    "user_note: Preserve the behavior".into(),
                ],
            },
        )]),
        interrupted: false,
    };
    crate::terminal_palette::with_test_default_colors(
        crate::terminal_probe::DefaultColors {
            fg: (240, 240, 240),
            bg: (24, 24, 24),
        },
        || {
            let mut snapshots = Vec::new();
            for state in ["completed", "interrupted"] {
                let lines = cell.display_lines(/*width*/ 40);
                let area = Rect::new(
                    /*x*/ 0,
                    /*y*/ 0,
                    /*width*/ 40,
                    lines.len() as u16,
                );
                let mut buffer = Buffer::empty(area);
                Paragraph::new(lines).render(area, &mut buffer);
                snapshots.push(format!("{state}: {buffer:?}"));
                cell.answers.clear();
                cell.interrupted = true;
            }
            insta::assert_snapshot!(snapshots.join("\n"));
        },
    );
}
