//! This module contains the future directory structure. If possible, new definitions should
//! get added here.
//!
//! Eventually everything should be moved over, and we will move this directory to the top
//! level in `src`.

cfg_if! {
    if #[cfg(target_os = "linux")] {
        mod linux_uapi;
        pub use linux_uapi::*;
    } else if #[cfg(target_os = "android")] {
        mod bionic;
        pub use bionic::*;
    }
}
