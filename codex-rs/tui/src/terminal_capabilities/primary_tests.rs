#![cfg(test)]

use super::wayland_primary_available;
use std::ffi::OsStr;
use wl_clipboard_rs::utils::PrimarySelectionCheckError::MissingProtocol;
use wl_clipboard_rs::utils::PrimarySelectionCheckError::NoSeats;

#[test]
fn missing_data_control_requires_the_compositors_own_x11_listener() {
    let available = |probe, display: Option<&str>| {
        wayland_primary_available(probe, display.map(OsStr::new), |number| number == 0)
    };
    assert!(available(Ok(true), None));
    assert!(available(Err(MissingProtocol), Some(":0.0")));
    assert!(!available(Err(MissingProtocol), Some(":99")));
    assert!(!available(Err(MissingProtocol), None));
    assert!(!available(Err(MissingProtocol), Some("localhost:0")));
    assert!(!available(Ok(true), Some("localhost:0")));
    assert!(!available(Ok(false), Some(":0")));
    assert!(!available(Err(NoSeats), Some(":0")));
}
