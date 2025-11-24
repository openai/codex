#![allow(unused_macros)]

/// Allows specifying arbitrary combinations of features and config flags,
/// which are also propagated to `docsrs` config.
///
/// Each contained item will have the annotations applied
///
/// ## Example usage:
/// ```no-compile
/// feature! {
/// #![any(
///     feature = "process",
///     feature = "sync",
///     feature = "rt",
///     tokio_unstable
/// )]
///     /// docs
///     pub struct MyStruct {};
///     /// docs
///     pub struct AnotherStruct {};
/// }
/// ```
///
macro_rules! feature {
    (
        #![$meta:meta]
        $($item:item)*
    ) => {
        $(
            #[cfg($meta)]
            #[cfg_attr(docsrs, doc(cfg($meta)))]
            $item
        )*
    }
}

/// Enables Windows-specific code.
/// Use this macro instead of `cfg(windows)` to generate docs properly.
macro_rules! cfg_windows {
    ($($item:item)*) => {
        $(
            #[cfg(any(all(doc, docsrs), windows))]
            #[cfg_attr(docsrs, doc(cfg(windows)))]
            $item
        )*
    }
}

/// Enables Unix-specific code.
/// Use this macro instead of `cfg(unix)` to generate docs properly.
macro_rules! cfg_unix {
    ($($item:item)*) => {
        $(
            #[cfg(any(all(doc, docsrs), unix))]
            #[cfg_attr(docsrs, doc(cfg(unix)))]
            $item
        )*
    }
}

/// Enables unstable Windows-specific code.
/// Use this macro instead of `cfg(windows)` to generate docs properly.
macro_rules! cfg_unstable_windows {
    ($($item:item)*) => {
        $(
            #[cfg(all(any(all(doc, docsrs), windows), tokio_unstable))]
            #[cfg_attr(docsrs, doc(cfg(all(windows, tokio_unstable))))]
            $item
        )*
    }
}

/// Enables `enter::block_on`.
macro_rules! cfg_block_on {
    ($($item:item)*) => {
        $(
            #[cfg(any(
                    feature = "fs",
                    feature = "net",
                    feature = "io-std",
                    feature = "rt",
                    ))]
            $item
        )*
    }
}

/// Enables internal `AtomicWaker` impl.
macro_rules! cfg_atomic_waker_impl {
    ($($item:item)*) => {
        $(
            #[cfg(any(
                feature = "net",
                feature = "process",
                feature = "rt",
                feature = "signal",
                feature = "time",
            ))]
            #[cfg(not(loom))]
            $item
        )*
    }
}

macro_rules! cfg_aio {
    ($($item:item)*) => {
        $(
            #[cfg(all(any(docsrs, target_os = "freebsd"), feature = "net"))]
            #[cfg_attr(docsrs,
                doc(cfg(all(target_os = "freebsd", feature = "net")))
            )]
            $item
        )*
    }
}

macro_rules! cfg_fs {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "fs")]
            #[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
            $item
        )*
    }
}

macro_rules! cfg_io_blocking {
    ($($item:item)*) => {
        $( #[cfg(any(
                feature = "io-std",
                feature = "fs",
                all(windows, feature = "process"),
        ))] $item )*
    }
}

macro_rules! cfg_io_driver {
    ($($item:item)*) => {
        $(
            #[cfg(any(
                feature = "net",
                all(unix, feature = "process"),
                all(unix, feature = "signal"),
                all(
                    tokio_uring,
                    feature = "rt",
                    feature = "fs",
                    target_os = "linux"
                )
            ))]
            #[cfg_attr(docsrs, doc(cfg(any(
                feature = "net",
                all(unix, feature = "process"),
                all(unix, feature = "signal"),
                all(
                    tokio_uring,
                    feature = "rt",
                    feature = "fs",
                    target_os = "linux"
                )
            ))))]
            $item
        )*
    }
}

macro_rules! cfg_io_driver_impl {
    ( $( $item:item )* ) => {
        $(
            #[cfg(any(
                feature = "net",
                all(unix, feature = "process"),
                all(unix, feature = "signal"),
                all(
                    tokio_uring,
                    feature = "rt",
                    feature = "fs",
                    target_os = "linux"
                )
            ))]
            $item
        )*
    }
}

macro_rules! cfg_not_io_driver {
    ($($item:item)*) => {
        $(
            #[cfg(not(any(
                feature = "net",
                all(unix, feature = "process"),
                all(unix, feature = "signal"),
                all(
                    tokio_uring,
                    feature = "rt",
                    feature = "fs",
                    target_os = "linux"
                )
            )))]
            $item
        )*
    }
}

macro_rules! cfg_io_readiness {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "net")]
            $item
        )*
    }
}

macro_rules! cfg_io_std {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "io-std")]
            #[cfg_attr(docsrs, doc(cfg(feature = "io-std")))]
            $item
        )*
    }
}

macro_rules! cfg_io_util {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "io-util")]
            #[cfg_attr(docsrs, doc(cfg(feature = "io-util")))]
            $item
        )*
    }
}

macro_rules! cfg_not_io_util {
    ($($item:item)*) => {
        $( #[cfg(not(feature = "io-util"))] $item )*
    }
}

macro_rules! cfg_loom {
    ($($item:item)*) => {
        $( #[cfg(loom)] $item )*
    }
}

macro_rules! cfg_not_loom {
    ($($item:item)*) => {
        $( #[cfg(not(loom))] $item )*
    }
}

macro_rules! cfg_macros {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "macros")]
            #[cfg_attr(docsrs, doc(cfg(feature = "macros")))]
            $item
        )*
    }
}

macro_rules! cfg_unstable_metrics {
    ($($item:item)*) => {
        $(
            #[cfg(tokio_unstable)]
            #[cfg_attr(docsrs, doc(cfg(tokio_unstable)))]
            $item
        )*
    }
}

/// Some metrics require 64-bit atomics.
macro_rules! cfg_64bit_metrics {
    ($($item:item)*) => {
        $(
            #[cfg(target_has_atomic = "64")]
            #[cfg_attr(docsrs, doc(cfg(target_has_atomic = "64")))]
            $item
        )*
    }
}

macro_rules! cfg_no_64bit_metrics {
    ($($item:item)*) => {
        $(
            #[cfg(not(target_has_atomic = "64"))]
            $item
        )*
    }
}

macro_rules! cfg_not_unstable_metrics {
    ($($item:item)*) => {
        $(
            #[cfg(not(tokio_unstable))]
            $item
        )*
    }
}

macro_rules! cfg_not_rt_and_metrics_and_net {
    ($($item:item)*) => {
        $( #[cfg(not(all(feature = "net", feature = "rt", tokio_unstable)))]$item )*
    }
}

macro_rules! cfg_net_or_process {
    ($($item:item)*) => {
        $(
            #[cfg(any(feature = "net", feature = "process"))]
            #[cfg_attr(docsrs, doc(cfg(any(feature = "net", feature = "process"))))]
            $item
        )*
    }
}

macro_rules! cfg_net {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "net")]
            #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
            $item
        )*
    }
}

macro_rules! cfg_net_or_uring {
    ($($item:item)*) => {
        $(
            #[cfg(any(
                feature = "net",
                all(
                    tokio_uring,
                    feature = "rt",
                    feature = "fs",
                    target_os = "linux",
                )
            ))]
            #[cfg_attr(
                docsrs,
                doc(cfg(any(
                    feature = "net",
                    all(
                        tokio_uring,
                        feature = "rt",
                        feature = "fs",
                        target_os = "linux",
                    )
                )))
            )]
            $item
        )*
    }
}

macro_rules! cfg_net_unix {
    ($($item:item)*) => {
        $(
            #[cfg(all(unix, feature = "net"))]
            #[cfg_attr(docsrs, doc(cfg(all(unix, feature = "net"))))]
            $item
        )*
    }
}

macro_rules! cfg_net_windows {
    ($($item:item)*) => {
        $(
            #[cfg(all(any(all(doc, docsrs), windows), feature = "net"))]
            #[cfg_attr(docsrs, doc(cfg(all(windows, feature = "net"))))]
            $item
        )*
    }
}

macro_rules! cfg_process {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "process")]
            #[cfg_attr(docsrs, doc(cfg(feature = "process")))]
            #[cfg(not(loom))]
            #[cfg(not(target_os = "wasi"))]
            $item
        )*
    }
}

macro_rules! cfg_process_driver {
    ($($item:item)*) => {
        #[cfg(unix)]
        #[cfg(not(loom))]
        cfg_process! { $($item)* }
    }
}

macro_rules! cfg_not_process_driver {
    ($($item:item)*) => {
        $(
            #[cfg(not(all(unix, not(loom), feature = "process")))]
            $item
        )*
    }
}

macro_rules! cfg_signal {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "signal")]
            #[cfg_attr(docsrs, doc(cfg(feature = "signal")))]
            #[cfg(not(loom))]
            #[cfg(not(target_os = "wasi"))]
            $item
        )*
    }
}

macro_rules! cfg_signal_internal {
    ($($item:item)*) => {
        $(
            #[cfg(any(feature = "signal", all(unix, feature = "process")))]
            #[cfg(not(loom))]
            $item
        )*
    }
}

macro_rules! cfg_signal_internal_and_unix {
    ($($item:item)*) => {
        #[cfg(unix)]
        cfg_signal_internal! { $($item)* }
    }
}

macro_rules! cfg_not_signal_internal {
    ($($item:item)*) => {
        $(
            #[cfg(any(loom, not(unix), not(any(feature = "signal", all(unix, feature = "process")))))]
            $item
        )*
    }
}

macro_rules! cfg_sync {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "sync")]
            #[cfg_attr(docsrs, doc(cfg(feature = "sync")))]
            $item
        )*
    }
}

macro_rules! cfg_not_sync {
    ($($item:item)*) => {
        $( #[cfg(not(feature = "sync"))] $item )*
    }
}

macro_rules! cfg_rt {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "rt")]
            #[cfg_attr(docsrs, doc(cfg(feature = "rt")))]
            $item
        )*
    }
}

macro_rules! cfg_not_rt {
    ($($item:item)*) => {
        $( #[cfg(not(feature = "rt"))] $item )*
    }
}

macro_rules! cfg_rt_multi_thread {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "rt-multi-thread")]
            #[cfg_attr(docsrs, doc(cfg(feature = "rt-multi-thread")))]
            $item
        )*
    }
}

macro_rules! cfg_not_rt_multi_thread {
    ($($item:item)*) => {
        $( #[cfg(not(feature = "rt-multi-thread"))] $item )*
    }
}

macro_rules! cfg_taskdump {
    ($($item:item)*) => {
        $(
            #[cfg(all(
                tokio_unstable,
                tokio_taskdump,
                feature = "rt",
                target_os = "linux",
                any(
                    target_arch = "aarch64",
                    target_arch = "x86",
                    target_arch = "x86_64"
                )
            ))]
            $item
        )*
    };
}

macro_rules! cfg_not_taskdump {
    ($($item:item)*) => {
        $(
            #[cfg(not(all(
                tokio_unstable,
                tokio_taskdump,
                feature = "rt",
                target_os = "linux",
                any(
                    target_arch = "aarch64",
                    target_arch = "x86",
                    target_arch = "x86_64"
                )
            )))]
            $item
        )*
    };
}

macro_rules! cfg_test_util {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "test-util")]
            #[cfg_attr(docsrs, doc(cfg(feature = "test-util")))]
            $item
        )*
    }
}

macro_rules! cfg_not_test_util {
    ($($item:item)*) => {
        $( #[cfg(not(feature = "test-util"))] $item )*
    }
}

macro_rules! cfg_time {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "time")]
            #[cfg_attr(docsrs, doc(cfg(feature = "time")))]
            $item
        )*
    }
}

macro_rules! cfg_not_time {
    ($($item:item)*) => {
        $( #[cfg(not(feature = "time"))] $item )*
    }
}

macro_rules! cfg_trace {
    ($($item:item)*) => {
        $(
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            #[cfg_attr(docsrs, doc(cfg(all(tokio_unstable, feature = "tracing"))))]
            $item
        )*
    };
}

macro_rules! cfg_unstable {
    ($($item:item)*) => {
        $(
            #[cfg(tokio_unstable)]
            #[cfg_attr(docsrs, doc(cfg(tokio_unstable)))]
            $item
        )*
    };
}

macro_rules! cfg_not_trace {
    ($($item:item)*) => {
        $(
            #[cfg(any(not(tokio_unstable), not(feature = "tracing")))]
            $item
        )*
    }
}

macro_rules! cfg_coop {
    ($($item:item)*) => {
        $(
            #[cfg(any(
                    feature = "fs",
                    feature = "io-std",
                    feature = "net",
                    feature = "process",
                    feature = "rt",
                    feature = "signal",
                    feature = "sync",
                    feature = "time",
                    ))]
            $item
        )*
    }
}

macro_rules! cfg_not_coop {
    ($($item:item)*) => {
        $(
            #[cfg(not(any(
                    feature = "fs",
                    feature = "io-std",
                    feature = "net",
                    feature = "process",
                    feature = "rt",
                    feature = "signal",
                    feature = "sync",
                    feature = "time",
                    )))]
            $item
        )*
    }
}

macro_rules! cfg_has_atomic_u64 {
    ($($item:item)*) => {
        $(
            #[cfg(target_has_atomic = "64")]
            $item
        )*
    }
}

macro_rules! cfg_not_has_atomic_u64 {
    ($($item:item)*) => {
        $(
            #[cfg(not(target_has_atomic = "64"))]
            $item
        )*
    }
}

macro_rules! cfg_has_const_mutex_new {
    ($($item:item)*) => {
        $(
            #[cfg(not(all(loom, test)))]
            $item
        )*
    }
}

macro_rules! cfg_not_has_const_mutex_new {
    ($($item:item)*) => {
        $(
            #[cfg(all(loom, test))]
            $item
        )*
    }
}

macro_rules! cfg_not_wasi {
    ($($item:item)*) => {
        $(
            #[cfg(not(target_os = "wasi"))]
            $item
        )*
    }
}

macro_rules! cfg_is_wasm_not_wasi {
    ($($item:item)*) => {
        $(
            #[cfg(all(target_family = "wasm", not(target_os = "wasi")))]
            $item
        )*
    }
}

/// Use this macro to provide two different implementations of the same API — one for stable
/// builds and one for unstable builds.
macro_rules! cfg_metrics_variant {
    (stable: {$($stable_code:tt)*}, unstable: {$($unstable_code:tt)*}) => {
        cfg_not_unstable_metrics! {
            $($stable_code)*
        }

        cfg_unstable_metrics! {
            $($unstable_code)*
        }
    }
}

macro_rules! cfg_tokio_uring {
    ($($item:item)*) => {
        $(
            #[cfg(all(
                tokio_uring,
                feature = "rt",
                feature = "fs",
                target_os = "linux",
            ))]
            $item
        )*
    };
}
