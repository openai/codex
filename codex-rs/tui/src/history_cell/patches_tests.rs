use super::*;
use pretty_assertions::assert_eq;

#[test]
fn viewed_image_retains_original_path_in_details() {
    for path in [
        "/workspace/assets/example.png",
        r"C:\workspace\assets\example.png",
    ] {
        let cell = new_view_image_tool_call(LegacyAppPathString::from_string(path));
        assert_eq!(
            cell.display_lines(/*width*/ 80)
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            vec!["• Viewed image example.png"]
        );
        assert_eq!(
            cell.raw_lines(),
            vec![Line::from(format!("Viewed image {path}"))]
        );
        assert_eq!(
            cell.transcript_lines(/*width*/ 200)
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            vec![format!("• Viewed image {path}")]
        );
    }
}

#[test]
fn viewed_image_narrow_summary() {
    let cell = new_view_image_tool_call(LegacyAppPathString::from_string(
        "/workspace/very-long-screenshot-name.png",
    ));
    insta::assert_snapshot!(cell.display_lines(/*width*/ 32)[0].to_string());
}
