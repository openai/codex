//! Restored dashboard, visible controls, and responsive report regression coverage.

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

#[test]
fn tall_reports_use_available_height_and_wide_reports_put_details_beside_plot() {
    for (width, height) in [(80, 60), (180, 60)] {
        let mut view = fixture::view(models::AccountKind::Business);
        view.section = Section::Usage;
        let output = screen(&mut view, width, height);
        let plot_rows = output.lines().filter(|line| line.contains('█')).count();
        assert!(
            plot_rows > 14,
            "only {plot_rows} plot rows at {width} columns"
        );
        assert!(output.contains("731,000 tokens"));
        assert!(!output.contains("scroll"));
        if width == 180 {
            assert!(
                output
                    .lines()
                    .any(|line| line.contains("4.4M tokens") && line.contains('─'))
            );
            insta::assert_snapshot!(output);
        }
    }
}
