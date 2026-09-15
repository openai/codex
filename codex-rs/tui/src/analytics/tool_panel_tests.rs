//! Tool chart layout, independent selection, naming, and partial report states.

use super::*;
use crate::analytics::sections::Section;
use pretty_assertions::assert_eq;

#[test]
fn analytics_tools_reflow_and_keep_independent_days() {
    let mut view = fixture::view(models::AccountKind::Consumer);
    press(&mut view, KeyCode::Char('3'));
    let wide = screen(&mut view, /*width*/ 140, /*height*/ 48);
    assert!(wide.contains("186 calls"));
    press(&mut view, KeyCode::Char('4'));
    press(&mut view, KeyCode::Left);
    press(&mut view, KeyCode::Enter);
    assert_eq!(
        (
            view.sections[Section::Plugins].cursor,
            view.sections[Section::Skills].cursor,
            view.sections[Section::Skills].detail
        ),
        (6, 5, Some(5))
    );
    let narrow = screen(&mut view, /*width*/ 58, /*height*/ 32);
    assert!(narrow.contains("Sep 1 · 5 uses"));
    press(&mut view, KeyCode::Char('r'));
    assert_eq!(
        (
            view.sections[Section::Plugins].cursor,
            view.sections[Section::Skills].cursor,
            view.ranges
        ),
        (29, 28, [0, 0, 1])
    );
    press(&mut view, KeyCode::Char('r'));
    press(&mut view, KeyCode::Char('z'));
    press(&mut view, KeyCode::Char('z'));
    assert_eq!(
        (
            view.sections[Section::Plugins].cursor,
            view.sections[Section::Skills].cursor,
            view.sections[Section::Skills].detail
        ),
        (6, 5, Some(5))
    );
    insta::assert_snapshot!(format!("{wide}\n{narrow}"));
}

#[test]
fn analytics_tools_handle_empty_days_and_independent_failures() {
    let mut view = fixture::view(models::AccountKind::Consumer);
    let mut history = fixture::history(/*report*/ 4, /*range*/ 0, /*group*/ 0);
    history.data[6].total = 0.0;
    for value in &mut history.data[6].values {
        value.value = 0.0;
    }
    history.data.remove(/*index*/ 5);
    view.sections[Section::Skills].history = Load::Ready(history);
    press(&mut view, KeyCode::Char('4'));
    let empty = screen(&mut view, /*width*/ 140, /*height*/ 44);
    assert!(empty.contains("No uses reported for Sep 2."));
    assert!(!empty.contains("details"));
    press(&mut view, KeyCode::Enter);
    assert_eq!(view.sections[Section::Skills].detail, None);
    press(&mut view, KeyCode::Left);
    let missing = screen(&mut view, /*width*/ 140, /*height*/ 44);
    assert!(missing.contains("Data not reported for Sep 1."));
    assert_eq!(
        empty.lines().position(|line| line.contains('▲')),
        missing.lines().position(|line| line.contains('▲')),
    );
    view.sections[Section::Skills].history =
        Load::Error("Skills could not be loaded. Press R to retry.".into());
    let partial = screen(&mut view, /*width*/ 140, /*height*/ 44);
    assert!(partial.contains("Skills could not be loaded"));
    press(&mut view, KeyCode::Char('3'));
    assert!(screen(&mut view, /*width*/ 140, /*height*/ 44).contains("186 calls"));
    insta::assert_snapshot!(format!("{empty}\n{missing}\n{partial}"));
}

#[test]
fn analytics_tool_names_preserve_acronyms_and_distinct_namespaces() {
    let labels = [
        "Pr Reviewer",
        "Slack Cli",
        "Squatch: Squatch",
        "Openai Docs",
        "User Writing: Writing Style",
    ];
    assert_eq!(
        labels.map(tool_panel::tool_name),
        [
            "PR Reviewer",
            "Slack CLI",
            "Squatch",
            "OpenAI Docs",
            "User Writing: Writing Style"
        ]
        .map(str::to_owned),
    );
}
