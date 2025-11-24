#[cfg(feature = "track-caller")]
#[test]
fn disabled() {
    use color_eyre::eyre;
    use eyre::eyre;

    color_eyre::config::HookBuilder::default()
        .display_location_section(false)
        .install()
        .unwrap();

    let report = eyre!("error occured");

    let report = format!("{:?}", report);
    assert!(!report.contains("Location:"));
}
