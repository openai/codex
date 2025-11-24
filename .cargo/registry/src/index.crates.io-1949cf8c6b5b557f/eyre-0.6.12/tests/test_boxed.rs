mod common;

use self::common::maybe_install_handler;
use eyre::{eyre, Report};
use std::error::Error as StdError;
use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
#[error("outer")]
struct MyError {
    source: io::Error,
}

#[test]
fn test_boxed_str() {
    maybe_install_handler().unwrap();

    let error = Box::<dyn StdError + Send + Sync>::from("oh no!");
    let error: Report = eyre!(error);
    assert_eq!("oh no!", error.to_string());
    assert_eq!(
        "oh no!",
        error
            .downcast_ref::<Box<dyn StdError + Send + Sync>>()
            .unwrap()
            .to_string()
    );
}

#[test]
fn test_boxed_thiserror() {
    maybe_install_handler().unwrap();

    let error = MyError {
        source: io::Error::new(io::ErrorKind::Other, "oh no!"),
    };
    let error: Report = eyre!(error);
    assert_eq!("oh no!", error.source().unwrap().to_string());
}

#[test]
fn test_boxed_eyre() {
    maybe_install_handler().unwrap();

    let error: Report = eyre!("oh no!").wrap_err("it failed");
    let error = eyre!(error);
    assert_eq!("oh no!", error.source().unwrap().to_string());
}

#[test]
fn test_boxed_sources() {
    maybe_install_handler().unwrap();

    let error = MyError {
        source: io::Error::new(io::ErrorKind::Other, "oh no!"),
    };
    let error = Box::<dyn StdError + Send + Sync>::from(error);
    let error: Report = eyre!(error).wrap_err("it failed");
    assert_eq!("it failed", error.to_string());
    assert_eq!("outer", error.source().unwrap().to_string());
    assert_eq!(
        "oh no!",
        error.source().unwrap().source().unwrap().to_string()
    );
}
