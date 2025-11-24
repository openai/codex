//! Macros to ease conditional code based on enabled features.

// Depending on the features not all macros are used.
#![allow(unused_macros)]

/// The `os-poll` feature is enabled.
macro_rules! cfg_os_poll {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "os-poll")]
            #[cfg_attr(docsrs, doc(cfg(feature = "os-poll")))]
            $item
        )*
    }
}

/// The `os-poll` feature is disabled.
macro_rules! cfg_not_os_poll {
    ($($item:item)*) => {
        $(
            #[cfg(not(feature = "os-poll"))]
            $item
        )*
    }
}

/// The `os-ext` feature is enabled.
macro_rules! cfg_os_ext {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "os-ext")]
            #[cfg_attr(docsrs, doc(cfg(feature = "os-ext")))]
            $item
        )*
    }
}

/// The `net` feature is enabled.
macro_rules! cfg_net {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "net")]
            #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
            $item
        )*
    }
}

/// One of the features enabled that needs `IoSource`. That is `net` or `os-ext`
/// on Unix (for `pipe`).
macro_rules! cfg_io_source {
    ($($item:item)*) => {
        $(
            #[cfg(any(feature = "net", all(unix, feature = "os-ext")))]
            #[cfg_attr(docsrs, doc(cfg(any(feature = "net", all(unix, feature = "os-ext")))))]
            $item
        )*
    }
}

/// The `os-ext` feature is enabled, or one of the features that need `os-ext`.
macro_rules! cfg_any_os_ext {
    ($($item:item)*) => {
        $(
            #[cfg(any(feature = "os-ext", feature = "net"))]
            #[cfg_attr(docsrs, doc(cfg(any(feature = "os-ext", feature = "net"))))]
            $item
        )*
    }
}

macro_rules! trace {
    ($($t:tt)*) => {
        log!(trace, $($t)*)
    }
}

macro_rules! warn {
    ($($t:tt)*) => {
        log!(warn, $($t)*)
    }
}

macro_rules! error {
    ($($t:tt)*) => {
        log!(error, $($t)*)
    }
}

macro_rules! log {
    ($level: ident, $($t:tt)*) => {
        #[cfg(feature = "log")]
        { log::$level!($($t)*) }
        // Silence unused variables warnings.
        #[cfg(not(feature = "log"))]
        { if false { let _ = ( $($t)* ); } }
    }
}
