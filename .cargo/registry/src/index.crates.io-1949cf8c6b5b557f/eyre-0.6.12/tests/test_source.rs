mod common;

use self::common::maybe_install_handler;
use eyre::{eyre, Report};
use std::error::Error as StdError;
use std::fmt::{self, Display};
use std::io;

#[derive(Debug)]
enum TestError {
    Io(io::Error),
}

impl Display for TestError {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TestError::Io(e) => Display::fmt(e, formatter),
        }
    }
}

impl StdError for TestError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            TestError::Io(io) => Some(io),
        }
    }
}

#[test]
fn test_literal_source() {
    maybe_install_handler().unwrap();

    let error: Report = eyre!("oh no!");
    assert!(error.source().is_none());
}

#[test]
fn test_variable_source() {
    maybe_install_handler().unwrap();

    let msg = "oh no!";
    let error = eyre!(msg);
    assert!(error.source().is_none());

    let msg = msg.to_owned();
    let error: Report = eyre!(msg);
    assert!(error.source().is_none());
}

#[test]
fn test_fmt_source() {
    maybe_install_handler().unwrap();

    let error: Report = eyre!("{} {}!", "oh", "no");
    assert!(error.source().is_none());
}

#[test]
fn test_io_source() {
    maybe_install_handler().unwrap();

    let io = io::Error::new(io::ErrorKind::Other, "oh no!");
    let error: Report = eyre!(TestError::Io(io));
    assert_eq!("oh no!", error.source().unwrap().to_string());
}

#[test]
fn test_eyre_from_eyre() {
    maybe_install_handler().unwrap();

    let error: Report = eyre!("oh no!").wrap_err("context");
    let error = eyre!(error);
    assert_eq!("oh no!", error.source().unwrap().to_string());
}
