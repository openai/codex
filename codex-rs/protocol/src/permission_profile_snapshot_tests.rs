//! Profile-root spelling changes must propagate through settings snapshots.

use super::*;

#[test]
fn executor_profile_root_case_changes_update_snapshot() {
    for (first, second) in [
        ("file:///C:/Work/Project", "file:///C:/work/project"),
        ("file://server/share/Project", "file://server/share/project"),
    ] {
        let snapshot = |root| {
            PermissionProfileSnapshot::active_with_profile_workspace_roots(
                PermissionProfile::read_only(),
                ActivePermissionProfile::new("executor"),
                vec![PathUri::parse(root).unwrap().into()],
            )
        };
        assert_ne!(snapshot(first), snapshot(second));
    }
}
