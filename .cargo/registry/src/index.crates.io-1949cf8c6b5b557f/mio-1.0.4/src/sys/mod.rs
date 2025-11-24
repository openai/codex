//! Module with system specific types.
//!
//! Required types:
//!
//! * `Event`: a type alias for the system specific event, e.g. `kevent` or
//!   `epoll_event`.
//! * `event`: a module with various helper functions for `Event`, see
//!   [`crate::event::Event`] for the required functions.
//! * `Events`: collection of `Event`s, see [`crate::Events`].
//! * `IoSourceState`: state for the `IoSource` type.
//! * `Selector`: selector used to register event sources and poll for events,
//!   see [`crate::Poll`] and [`crate::Registry`] for required methods.
//! * `tcp` and `udp` modules: see the [`crate::net`] module.
//! * `Waker`: see [`crate::Waker`].

cfg_os_poll! {
    macro_rules! debug_detail {
        (
            $type: ident ($event_type: ty), $test: path,
            $($(#[$target: meta])* $libc: ident :: $flag: ident),+ $(,)*
        ) => {
            struct $type($event_type);

            impl fmt::Debug for $type {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    let mut written_one = false;
                    $(
                        $(#[$target])*
                        #[allow(clippy::bad_bit_mask)] // Apparently some flags are zero.
                        {
                            // Windows doesn't use `libc` but the `afd` module.
                            if $test(&self.0, &$libc :: $flag) {
                                if !written_one {
                                    write!(f, "{}", stringify!($flag))?;
                                    written_one = true;
                                } else {
                                    write!(f, "|{}", stringify!($flag))?;
                                }
                            }
                        }
                    )+
                    if !written_one {
                        write!(f, "(empty)")
                    } else {
                        Ok(())
                    }
                }
            }
        };
    }
}

#[cfg(any(unix, target_os = "hermit"))]
cfg_os_poll! {
    mod unix;
    #[allow(unused_imports)]
    pub use self::unix::*;
}

#[cfg(windows)]
cfg_os_poll! {
    mod windows;
    pub use self::windows::*;
}

#[cfg(target_os = "wasi")]
cfg_os_poll! {
    mod wasi;
    pub(crate) use self::wasi::*;
}

cfg_not_os_poll! {
    mod shell;
    pub(crate) use self::shell::*;

    #[cfg(unix)]
    cfg_any_os_ext! {
        mod unix;
        #[cfg(feature = "os-ext")]
        pub use self::unix::SourceFd;
    }
}
