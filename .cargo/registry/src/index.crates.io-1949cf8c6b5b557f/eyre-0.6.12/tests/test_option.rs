mod common;

use self::common::maybe_install_handler;
use eyre::OptionExt;

#[test]
fn test_option_ok_or_eyre() {
    maybe_install_handler().unwrap();

    let option: Option<()> = None;

    let result = option.ok_or_eyre("static str error");

    assert_eq!(result.unwrap_err().to_string(), "static str error");
}
