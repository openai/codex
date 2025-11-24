#![allow(clippy::eq_op)]
mod common;

use self::common::*;
use eyre::{ensure, eyre, Result};
use std::cell::Cell;
use std::future::Future;
use std::pin::Pin;
use std::task::Poll;

#[test]
fn test_messages() {
    maybe_install_handler().unwrap();

    assert_eq!("oh no!", bail_literal().unwrap_err().to_string());
    assert_eq!("oh no!", bail_fmt().unwrap_err().to_string());
    assert_eq!("oh no!", bail_error().unwrap_err().to_string());
}

#[test]
fn test_ensure() {
    maybe_install_handler().unwrap();

    let f = || -> Result<()> {
        ensure!(1 + 1 == 2, "This is correct");
        Ok(())
    };
    assert!(f().is_ok());

    let v = 1;
    let f = || -> Result<()> {
        ensure!(v + v == 2, "This is correct, v: {}", v);
        Ok(())
    };
    assert!(f().is_ok());

    let f = || -> Result<()> {
        ensure!(v + v == 1, "This is not correct, v: {}", v);
        Ok(())
    };
    assert!(f().is_err());

    let f = || {
        ensure!(v + v == 1);
        Ok(())
    };
    assert_eq!(
        f().unwrap_err().to_string(),
        "Condition failed: `v + v == 1`",
    );
}

#[test]
fn test_temporaries() {
    struct Ready<T>(Option<T>);

    impl<T> Unpin for Ready<T> {}

    impl<T> Future for Ready<T> {
        type Output = T;

        fn poll(mut self: Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> Poll<T> {
            Poll::Ready(self.0.take().unwrap())
        }
    }

    fn require_send_sync(_: impl Send + Sync) {}

    require_send_sync(async {
        // If eyre hasn't dropped any temporary format_args it creates by the
        // time it's done evaluating, those will stick around until the
        // semicolon, which is on the other side of the await point, making the
        // enclosing future non-Send.
        let _ = Ready(Some(eyre!("..."))).await;
    });

    fn message(cell: Cell<&str>) -> &str {
        cell.get()
    }

    require_send_sync(async {
        let _ = Ready(Some(eyre!(message(Cell::new("..."))))).await;
    });
}

#[test]
#[cfg(not(eyre_no_fmt_args_capture))]
fn test_capture_format_args() {
    maybe_install_handler().unwrap();

    let var = 42;
    let err = eyre!("interpolate {var}");
    assert_eq!("interpolate 42", err.to_string());
}

#[test]
fn test_brace_escape() {
    maybe_install_handler().unwrap();

    let err = eyre!("unterminated ${{..}} expression");
    assert_eq!("unterminated ${..} expression", err.to_string());
}
