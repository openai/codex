//! Visible report controls and account-scoped dates.

use super::*;

#[test]
fn report_controls_show_dates_and_update_time() {
    let mut view = fixture::view(models::AccountKind::Business);
    view.section = Section::Usage;
    view.sections[Section::Usage].group = 6;
    fixture::seed_reports(&mut view);
    view.end_date = "2027-01-02".parse().unwrap();
    let output = screen(&mut view, /*width*/ 40, /*height*/ 16);
    assert!(output.contains("12/27/2026–01/02/2027"));
    assert!(output.contains("g Token type · m All models"));
    assert!(output.contains("Updated · Sep 2 16:00 UTC"));
}
