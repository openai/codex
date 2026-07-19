use super::*;
use pretty_assertions::assert_eq;

#[test]
fn finalized_plan_reuses_lines_primed_by_transcript_height() {
    let cell = new_proposed_plan("1. Inspect **markdown**".to_string(), Path::new("/tmp"));
    let width = 48;

    assert_eq!(cell.desired_transcript_height(width), 8);
    cell.rendered_lines
        .cached
        .lock()
        .expect("render cache lock")
        .as_mut()
        .expect("render cache should be populated")
        .1 = vec![HyperlinkLine::from("cached")];

    assert_eq!(
        visible_lines(cell.transcript_hyperlink_lines(width)),
        vec![Line::from("cached")]
    );
}
