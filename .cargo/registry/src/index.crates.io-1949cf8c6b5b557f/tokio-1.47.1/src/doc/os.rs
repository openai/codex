//! See [`std::os`](https://doc.rust-lang.org/std/os/index.html).

/// Platform-specific extensions to `std` for Windows.
///
/// See [`std::os::windows`](https://doc.rust-lang.org/std/os/windows/index.html).
pub mod windows {
    /// Windows-specific extensions to general I/O primitives.
    ///
    /// See [`std::os::windows::io`](https://doc.rust-lang.org/std/os/windows/io/index.html).
    pub mod io {
        /// See [`std::os::windows::io::RawHandle`](https://doc.rust-lang.org/std/os/windows/io/type.RawHandle.html)
        pub type RawHandle = crate::doc::NotDefinedHere;

        /// See [`std::os::windows::io::OwnedHandle`](https://doc.rust-lang.org/std/os/windows/io/struct.OwnedHandle.html)
        pub type OwnedHandle = crate::doc::NotDefinedHere;

        /// See [`std::os::windows::io::AsRawHandle`](https://doc.rust-lang.org/std/os/windows/io/trait.AsRawHandle.html)
        pub trait AsRawHandle {
            /// See [`std::os::windows::io::AsRawHandle::as_raw_handle`](https://doc.rust-lang.org/std/os/windows/io/trait.AsRawHandle.html#tymethod.as_raw_handle)
            fn as_raw_handle(&self) -> RawHandle;
        }

        /// See [`std::os::windows::io::FromRawHandle`](https://doc.rust-lang.org/std/os/windows/io/trait.FromRawHandle.html)
        pub trait FromRawHandle {
            /// See [`std::os::windows::io::FromRawHandle::from_raw_handle`](https://doc.rust-lang.org/std/os/windows/io/trait.FromRawHandle.html#tymethod.from_raw_handle)
            unsafe fn from_raw_handle(handle: RawHandle) -> Self;
        }

        /// See [`std::os::windows::io::RawSocket`](https://doc.rust-lang.org/std/os/windows/io/type.RawSocket.html)
        pub type RawSocket = crate::doc::NotDefinedHere;

        /// See [`std::os::windows::io::AsRawSocket`](https://doc.rust-lang.org/std/os/windows/io/trait.AsRawSocket.html)
        pub trait AsRawSocket {
            /// See [`std::os::windows::io::AsRawSocket::as_raw_socket`](https://doc.rust-lang.org/std/os/windows/io/trait.AsRawSocket.html#tymethod.as_raw_socket)
            fn as_raw_socket(&self) -> RawSocket;
        }

        /// See [`std::os::windows::io::FromRawSocket`](https://doc.rust-lang.org/std/os/windows/io/trait.FromRawSocket.html)
        pub trait FromRawSocket {
            /// See [`std::os::windows::io::FromRawSocket::from_raw_socket`](https://doc.rust-lang.org/std/os/windows/io/trait.FromRawSocket.html#tymethod.from_raw_socket)
            unsafe fn from_raw_socket(sock: RawSocket) -> Self;
        }

        /// See [`std::os::windows::io::IntoRawSocket`](https://doc.rust-lang.org/std/os/windows/io/trait.IntoRawSocket.html)
        pub trait IntoRawSocket {
            /// See [`std::os::windows::io::IntoRawSocket::into_raw_socket`](https://doc.rust-lang.org/std/os/windows/io/trait.IntoRawSocket.html#tymethod.into_raw_socket)
            fn into_raw_socket(self) -> RawSocket;
        }

        /// See [`std::os::windows::io::BorrowedHandle`](https://doc.rust-lang.org/std/os/windows/io/struct.BorrowedHandle.html)
        pub type BorrowedHandle<'handle> = crate::doc::NotDefinedHere;

        /// See [`std::os::windows::io::AsHandle`](https://doc.rust-lang.org/std/os/windows/io/trait.AsHandle.html)
        pub trait AsHandle {
            /// See [`std::os::windows::io::AsHandle::as_handle`](https://doc.rust-lang.org/std/os/windows/io/trait.AsHandle.html#tymethod.as_handle)
            fn as_handle(&self) -> BorrowedHandle<'_>;
        }

        /// See [`std::os::windows::io::BorrowedSocket`](https://doc.rust-lang.org/std/os/windows/io/struct.BorrowedSocket.html)
        pub type BorrowedSocket<'socket> = crate::doc::NotDefinedHere;

        /// See [`std::os::windows::io::AsSocket`](https://doc.rust-lang.org/std/os/windows/io/trait.AsSocket.html)
        pub trait AsSocket {
            /// See [`std::os::windows::io::AsSocket::as_socket`](https://doc.rust-lang.org/std/os/windows/io/trait.AsSocket.html#tymethod.as_socket)
            fn as_socket(&self) -> BorrowedSocket<'_>;
        }
    }
}
