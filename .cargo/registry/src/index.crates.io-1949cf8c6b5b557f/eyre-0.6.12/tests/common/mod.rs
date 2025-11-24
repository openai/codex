#![allow(dead_code)]

use eyre::{bail, set_hook, DefaultHandler, InstallError, Result};
use once_cell::sync::OnceCell;
use std::io;

pub fn bail_literal() -> Result<()> {
    bail!("oh no!");
}

pub fn bail_fmt() -> Result<()> {
    bail!("{} {}!", "oh", "no");
}

pub fn bail_error() -> Result<()> {
    bail!(io::Error::new(io::ErrorKind::Other, "oh no!"));
}

// Tests are multithreaded- use OnceCell to install hook once if auto-install
// feature is disabled.
pub fn maybe_install_handler() -> Result<(), InstallError> {
    static INSTALLER: OnceCell<Result<(), InstallError>> = OnceCell::new();

    if cfg!(not(feature = "auto-install")) {
        *INSTALLER.get_or_init(|| set_hook(Box::new(DefaultHandler::default_with)))
    } else {
        Ok(())
    }
}
