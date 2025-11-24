use color_eyre::install;

#[test]
fn double_install_should_not_panic() {
    install().unwrap();
    assert!(install().is_err());
}
