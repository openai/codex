//! The following is derived from Rust's
//! library/std/src/os/windows/raw.rs,
//! library/std/src/os/windows/io/raw.rs and
//! library/std/src/os/windows/io/socket.rs
//! at revision
//! 4f9b394c8a24803e57ba892fa00e539742ebafc0.
//!
//! All code in this file is licensed MIT or Apache 2.0 at your option.

mod raw {
    #[cfg(target_pointer_width = "32")]
    #[cfg_attr(staged_api, stable(feature = "raw_ext", since = "1.1.0"))]
    pub type SOCKET = u32;
    #[cfg(target_pointer_width = "64")]
    #[cfg_attr(staged_api, stable(feature = "raw_ext", since = "1.1.0"))]
    pub type SOCKET = u64;
}

pub mod io;
