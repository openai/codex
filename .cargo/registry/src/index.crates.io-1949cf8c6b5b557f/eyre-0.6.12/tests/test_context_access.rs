mod common;

use crate::common::maybe_install_handler;

#[test]
fn test_context() {
    use eyre::{eyre, Report};

    maybe_install_handler().unwrap();

    let error: Report = eyre!("oh no!");
    let _ = error.context();
}
