use super::reject_workload_identity;
use pretty_assertions::assert_eq;

#[test]
fn workload_identity_markers_are_rejected() {
    let error = reject_workload_identity(/*workload_identity_selected*/ true)
        .expect_err("mcp-server does not support workload identity");

    assert_eq!(
        error.to_string(),
        "workload identity is not supported by `codex mcp-server`"
    );
    reject_workload_identity(/*workload_identity_selected*/ false)
        .expect("mcp-server remains available without workload identity");
}
