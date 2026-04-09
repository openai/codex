use super::*;
use core_test_support::PathBufExt;
use pretty_assertions::assert_eq;

use tempfile::tempdir;

#[test]
fn convert_apply_patch_maps_add_variant() {
    let tmp = tempdir().expect("tmp");
    let p = tmp.path().join("a.txt").abs();
    // Create an action with a single Add change
    let action = ApplyPatchAction::new_add_for_test(&p, "hello".to_string());

    let got = convert_apply_patch_to_protocol(&action);

    assert_eq!(
        got.get(p.as_path()),
        Some(&FileChange::Add {
            content: "hello".to_string()
        })
    );
}
