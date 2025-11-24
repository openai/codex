#![warn(rust_2018_idioms)]
#![cfg(all(feature = "full", not(target_os = "wasi")))] // WASI does not support all fs operations
#![cfg(windows)]

use tokio::fs::OpenOptions;
use windows_sys::Win32::Storage::FileSystem;

#[tokio::test]
#[cfg(windows)]
async fn open_options_windows_access_mode() {
    // TESTING HACK: use Debug output to check the stored data
    assert!(format!("{:?}", OpenOptions::new().access_mode(0)).contains("access_mode: Some(0)"));
}

#[tokio::test]
#[cfg(windows)]
async fn open_options_windows_share_mode() {
    // TESTING HACK: use Debug output to check the stored data
    assert!(format!("{:?}", OpenOptions::new().share_mode(0)).contains("share_mode: 0,"));
}

#[tokio::test]
#[cfg(windows)]
async fn open_options_windows_custom_flags() {
    // TESTING HACK: use Debug output to check the stored data
    assert!(format!(
        "{:?}",
        OpenOptions::new().custom_flags(FileSystem::FILE_FLAG_DELETE_ON_CLOSE)
    )
    .contains("custom_flags: 67108864,"));
}

#[tokio::test]
#[cfg(windows)]
async fn open_options_windows_attributes() {
    assert!(format!(
        "{:?}",
        OpenOptions::new().attributes(FileSystem::FILE_ATTRIBUTE_HIDDEN)
    )
    .contains("attributes: 2,"));
}

#[tokio::test]
#[cfg(windows)]
async fn open_options_windows_security_qos_flags() {
    assert!(format!(
        "{:?}",
        OpenOptions::new().security_qos_flags(FileSystem::SECURITY_IDENTIFICATION)
    )
    .contains("security_qos_flags: 1114112,"));
}
