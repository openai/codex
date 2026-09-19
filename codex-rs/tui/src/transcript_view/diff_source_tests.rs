//! Diff copy text retains code indentation and signs while gutters and soft wraps stay visual.

use super::*;
use crate::diff_model::FileChange;
use crate::history_cell::HistoryCell;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

#[test]
fn wrapped_added_code_keeps_indentation_and_graphemes_when_copied_and_resized() {
    let code =
        "\tlet message = \"a long value with 界 e\u{301} and 👩‍💻 inside\";\n    return message;\n";
    let patch = crate::history_cell::new_patch_event(
        HashMap::from([(
            PathBuf::from("test.rs"),
            FileChange::Add {
                content: code.to_string(),
            },
        )]),
        Path::new("/project"),
    );
    let layout = TextLayout::new(
        patch.display_hyperlink_lines(/*width*/ 40),
        /*width*/ 40,
    );
    let expected = "• Added test.rs (+2 -0)\n+    let message = \"a long value with 界 e\u{301} and 👩‍💻 inside\";\n+    return message;";
    assert_eq!(layout.text(), expected);
    assert_eq!(layout.rewrap(/*width*/ 18).text(), expected);
    assert!(layout.row_count() > 3);
}

#[test]
fn annotated_diff_preserves_the_existing_hard_wrapped_visual_output() {
    let changes = HashMap::from([(
        PathBuf::from("test.rs"),
        FileChange::Add {
            content: "    let answer = calculate_the_result(first_argument, second_argument);\n"
                .to_string(),
        },
    )]);
    let lines = crate::diff_render::create_diff_summary_with_links(
        &changes,
        Path::new("/project"),
        /*wrap_cols*/ 40,
    );
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 8,
    );
    let mut expected = Buffer::empty(area);
    HyperlinkParagraph::new(&lines, Style::default()).render(area, &mut expected);
    let mut actual = Buffer::empty(area);
    TextLayout::new(lines, /*width*/ 40).render(area, &mut actual, /*start_row*/ 0);
    assert_eq!(actual, expected);
}

#[test]
fn updated_diff_copy_includes_signs_but_no_line_number_gutters() {
    let patch = crate::history_cell::new_patch_event(
        HashMap::from([(PathBuf::from("example.txt"), FileChange::Update {
            unified_diff: "--- a/example.txt\n+++ b/example.txt\n@@ -1,2 +1,2 @@\n unchanged\n-old long value with several words\n+new long value with several words\n".to_string(),
            move_path: None,
        })]),
        Path::new("/project"),
    );
    let layout = TextLayout::new(
        patch.display_hyperlink_lines(/*width*/ 20),
        /*width*/ 20,
    );
    let body = layout.text().split_once('\n').expect("header then diff").1;
    assert_eq!(
        body,
        " unchanged\n-old long value with several words\n+new long value with several words"
    );
}
