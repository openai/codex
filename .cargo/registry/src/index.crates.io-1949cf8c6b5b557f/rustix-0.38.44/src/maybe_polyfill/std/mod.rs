//! Imports from `std` that would be polyfilled for `no_std` builds (see
//! `src/polyfill/no_std`).
//!
//! This implementation is used when `std` is available and just imports the
//! necessary items from `std`. For `no_std` builds, the file
//! `src/polyfill/no_std` is used instead, which doesn't depend on the standard
//! library.

#[cfg(not(windows))]
pub mod io {
    pub use std::io::{IoSlice, IoSliceMut};
}

#[cfg(not(any(target_os = "redox", target_os = "wasi")))]
#[cfg(feature = "net")]
pub mod net {
    pub use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
}

pub mod os {
    pub mod fd {
        // Change  to use `std::os::fd` when MSRV becomes 1.66 or higher.

        #[cfg(target_os = "wasi")]
        pub use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
        #[cfg(unix)]
        pub use std::os::unix::io::{
            AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd,
        };
    }

    #[cfg(windows)]
    pub mod windows {
        pub mod io {
            pub use std::os::windows::io::{
                AsRawSocket, AsSocket, BorrowedSocket, FromRawSocket, IntoRawSocket, OwnedSocket,
                RawSocket,
            };
        }
    }
}
