mod common;

use self::common::maybe_install_handler;
use eyre::{bail, Result, WrapErr};
use std::io;

fn f() -> Result<()> {
    bail!(io::Error::new(io::ErrorKind::PermissionDenied, "oh no!"));
}

fn g() -> Result<()> {
    f().wrap_err("f failed")
}

fn h() -> Result<()> {
    g().wrap_err("g failed")
}

const EXPECTED_ALTDISPLAY_F: &str = "oh no!";

const EXPECTED_ALTDISPLAY_G: &str = "f failed: oh no!";

const EXPECTED_ALTDISPLAY_H: &str = "g failed: f failed: oh no!";

const EXPECTED_DEBUG_F: &str = "oh no!";

const EXPECTED_DEBUG_G: &str = "\
f failed

Caused by:
    oh no!\
";

const EXPECTED_DEBUG_H: &str = "\
g failed

Caused by:
   0: f failed
   1: oh no!\
";

const EXPECTED_ALTDEBUG_F: &str = "\
Custom {
    kind: PermissionDenied,
    error: \"oh no!\",
}\
";

const EXPECTED_ALTDEBUG_G: &str = "\
Error {
    msg: \"f failed\",
    source: Custom {
        kind: PermissionDenied,
        error: \"oh no!\",
    },
}\
";

const EXPECTED_ALTDEBUG_H: &str = "\
Error {
    msg: \"g failed\",
    source: Error {
        msg: \"f failed\",
        source: Custom {
            kind: PermissionDenied,
            error: \"oh no!\",
        },
    },
}\
";

#[test]
fn test_display() {
    maybe_install_handler().unwrap();

    assert_eq!("g failed", h().unwrap_err().to_string());
}

#[test]
fn test_altdisplay() {
    maybe_install_handler().unwrap();

    assert_eq!(EXPECTED_ALTDISPLAY_F, format!("{:#}", f().unwrap_err()));
    assert_eq!(EXPECTED_ALTDISPLAY_G, format!("{:#}", g().unwrap_err()));
    assert_eq!(EXPECTED_ALTDISPLAY_H, format!("{:#}", h().unwrap_err()));
}

#[test]
#[cfg_attr(any(backtrace, track_caller), ignore)]
fn test_debug() {
    maybe_install_handler().unwrap();

    assert_eq!(EXPECTED_DEBUG_F, format!("{:?}", f().unwrap_err()));
    assert_eq!(EXPECTED_DEBUG_G, format!("{:?}", g().unwrap_err()));
    assert_eq!(EXPECTED_DEBUG_H, format!("{:?}", h().unwrap_err()));
}

#[test]
fn test_altdebug() {
    maybe_install_handler().unwrap();

    assert_eq!(EXPECTED_ALTDEBUG_F, format!("{:#?}", f().unwrap_err()));
    assert_eq!(EXPECTED_ALTDEBUG_G, format!("{:#?}", g().unwrap_err()));
    assert_eq!(EXPECTED_ALTDEBUG_H, format!("{:#?}", h().unwrap_err()));
}
