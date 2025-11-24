#![allow(clippy::assertions_on_result_states, clippy::wildcard_imports)]

mod common;
mod drop;

use self::common::*;
use self::drop::{DetectDrop, Flag};
use anyhow::Error;
use std::error::Error as StdError;
use std::fmt::{self, Display};
use std::io;

#[test]
fn test_downcast() {
    assert_eq!(
        "oh no!",
        bail_literal().unwrap_err().downcast::<&str>().unwrap(),
    );
    assert_eq!(
        "oh no!",
        bail_fmt().unwrap_err().downcast::<String>().unwrap(),
    );
    assert_eq!(
        "oh no!",
        bail_error()
            .unwrap_err()
            .downcast::<io::Error>()
            .unwrap()
            .to_string(),
    );
}

#[test]
fn test_downcast_ref() {
    assert_eq!(
        "oh no!",
        *bail_literal().unwrap_err().downcast_ref::<&str>().unwrap(),
    );
    assert_eq!(
        "oh no!",
        bail_fmt().unwrap_err().downcast_ref::<String>().unwrap(),
    );
    assert_eq!(
        "oh no!",
        bail_error()
            .unwrap_err()
            .downcast_ref::<io::Error>()
            .unwrap()
            .to_string(),
    );
}

#[test]
fn test_downcast_mut() {
    assert_eq!(
        "oh no!",
        *bail_literal().unwrap_err().downcast_mut::<&str>().unwrap(),
    );
    assert_eq!(
        "oh no!",
        bail_fmt().unwrap_err().downcast_mut::<String>().unwrap(),
    );
    assert_eq!(
        "oh no!",
        bail_error()
            .unwrap_err()
            .downcast_mut::<io::Error>()
            .unwrap()
            .to_string(),
    );

    let mut bailed = bail_fmt().unwrap_err();
    *bailed.downcast_mut::<String>().unwrap() = "clobber".to_string();
    assert_eq!(bailed.downcast_ref::<String>().unwrap(), "clobber");
    assert_eq!(bailed.downcast_mut::<String>().unwrap(), "clobber");
    assert_eq!(bailed.downcast::<String>().unwrap(), "clobber");
}

#[test]
fn test_drop() {
    let has_dropped = Flag::new();
    let error = Error::new(DetectDrop::new(&has_dropped));
    drop(error.downcast::<DetectDrop>().unwrap());
    assert!(has_dropped.get());
}

#[test]
fn test_as_ref() {
    let error = bail_error().unwrap_err();
    let ref_dyn: &dyn StdError = error.as_ref();
    assert_eq!("oh no!", ref_dyn.to_string());
    let ref_dyn_send_sync: &(dyn StdError + Send + Sync) = error.as_ref();
    assert_eq!("oh no!", ref_dyn_send_sync.to_string());
}

#[test]
fn test_large_alignment() {
    #[repr(align(64))]
    #[derive(Debug)]
    struct LargeAlignedError(&'static str);

    impl Display for LargeAlignedError {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str(self.0)
        }
    }

    impl StdError for LargeAlignedError {}

    let error = Error::new(LargeAlignedError("oh no!"));
    assert_eq!(
        "oh no!",
        error.downcast_ref::<LargeAlignedError>().unwrap().0
    );
}

#[test]
fn test_unsuccessful_downcast() {
    let mut error = bail_error().unwrap_err();
    assert!(error.downcast_ref::<&str>().is_none());
    assert!(error.downcast_mut::<&str>().is_none());
    assert!(error.downcast::<&str>().is_err());
}
