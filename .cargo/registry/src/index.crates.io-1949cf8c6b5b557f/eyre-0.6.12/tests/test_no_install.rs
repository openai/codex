#![cfg(not(feature = "auto-install"))]

use eyre::{eyre, set_hook, DefaultHandler, Report};

#[test]
fn test_no_hook_panic() {
    let panic_res = std::panic::catch_unwind(|| eyre!("this will never be displayed"));
    assert!(panic_res.is_err());

    let downcast_res = panic_res.unwrap_err().downcast::<String>();
    assert_eq!(
        *downcast_res.unwrap(),
        "a handler must always be installed if the `auto-install` feature is disabled"
    );

    assert!(set_hook(Box::new(DefaultHandler::default_with)).is_ok());
    let _error: Report = eyre!("this will be displayed if returned");
}
