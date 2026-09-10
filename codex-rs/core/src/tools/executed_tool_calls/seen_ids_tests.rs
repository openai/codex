use super::*;

#[test]
fn id_namespaces_are_separate_and_saturated_filters_reject_new_ids() {
    let mut tracker = SeenIds::default();
    assert!(tracker.observe_call_id("shared-id"));
    assert!(tracker.observe_runtime_cell_id(&CellId::new("shared-id".to_string())));
    assert!(!tracker.observe_call_id("shared-id"));
    assert!(!tracker.observe_runtime_cell_id(&CellId::new("shared-id".to_string())));

    tracker.bits.fill(u64::MAX);
    assert!(!tracker.observe_call_id("another-origin"));
    assert!(!tracker.observe_runtime_cell_id(&CellId::new("another-runtime".to_string())));
}
