pub(crate) use self::inner::*;

#[cfg(not(any(
    feature = "tzdb-bundle-always",
    all(
        feature = "tzdb-bundle-platform",
        any(windows, target_family = "wasm"),
    ),
)))]
#[path = "disabled.rs"]
mod inner;
#[cfg(any(
    feature = "tzdb-bundle-always",
    all(
        feature = "tzdb-bundle-platform",
        any(windows, target_family = "wasm"),
    ),
))]
#[path = "enabled.rs"]
mod inner;
