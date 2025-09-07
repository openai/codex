use codex_tui::metrics::{get_rejections, inc_rejections, Reason};

#[test]
fn counter_increments_on_enforcement_rejection_path() {
    let before = get_rejections(Reason::Enforcement);
    // Simulate enforcement rejection increment via central metrics API.
    inc_rejections(Reason::Enforcement);
    let after = get_rejections(Reason::Enforcement);
    assert_eq!(after, before + 1);
}
