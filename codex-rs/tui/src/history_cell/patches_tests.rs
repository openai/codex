use super::*;
use pretty_assertions::assert_eq;

#[test]
fn compact_patches_retain_full_changes_and_failure_details() {
    let patch = new_patch_event(
        HashMap::from([
            (
                PathBuf::from("new.txt"),
                FileChange::Add {
                    content: "one\ntwo\nthree\nfour\nfive\n".into(),
                },
            ),
            (
                PathBuf::from("existing.txt"),
                FileChange::Update {
                    unified_diff: "@@ -1,3 +1,3 @@\n context\n-before\n+after\n tail\n".into(),
                    move_path: None,
                },
            ),
        ]),
        Path::new("."),
    );
    let failure = new_patch_apply_failure("first\nsecond\nthird\nfourth diagnostic".into());
    let missing = new_patch_apply_failure(String::new());
    let mut snapshots = Vec::new();
    for (label, cell) in [
        ("patch", &patch as &dyn HistoryCell),
        ("failure", &failure),
        ("missing diagnostics", &missing),
    ] {
        let compact = visible_lines(cell.compact_hyperlink_lines(/*width*/ 40));
        let full = visible_lines(cell.transcript_hyperlink_lines(/*width*/ 40));
        snapshots.push(format!(
            "{label}\nCompact\n{}\nFull\n{}",
            Text::from(compact),
            Text::from(full)
        ));
    }
    insta::assert_snapshot!(snapshots.join("\n\n"));
}

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

#[test]
fn failed_patch_keeps_diagnostics_beyond_the_legacy_preview() {
    let diagnostics = (1..=12)
        .map(|line| format!("diagnostic line {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    let cell = new_patch_apply_failure(diagnostics.clone());
    assert_eq!(
        cell.raw_lines(),
        std::iter::once(Line::from("Failed to apply patch"))
            .chain(diagnostics.lines().map(|line| Line::from(line.to_owned())))
            .collect::<Vec<_>>()
    );
}
