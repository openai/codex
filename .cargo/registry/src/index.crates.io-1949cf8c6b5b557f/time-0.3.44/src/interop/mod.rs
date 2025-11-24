//! Comparison, arithmetic, and conversion between various types in `time` and the standard library.
//!
//! Currently, full interoperability is present between [`OffsetDateTime`](crate::OffsetDateTime),
//! [`UtcDateTime`](crate::UtcDateTime), and [`SystemTime`](std::time::SystemTime). Partial
//! interoperability is present with [`js_sys::Date`]. Note that
//! [`PrimitiveDateTime`](crate::PrimitiveDateTime) is not interoperable with any of these types due
//! to the lack of an associated UTC offset.

// Module names should have the two types sorted in alphabetical order. This avoids any question
// of which type should be the "primary" type in the module name.

#[cfg(all(
    target_family = "wasm",
    not(any(target_os = "emscripten", target_os = "wasi")),
    feature = "wasm-bindgen"
))]
mod js_sys_date_offsetdatetime;
#[cfg(all(
    target_family = "wasm",
    not(any(target_os = "emscripten", target_os = "wasi")),
    feature = "wasm-bindgen"
))]
mod js_sys_date_utcdatetime;
#[cfg(feature = "std")]
mod offsetdatetime_systemtime;
mod offsetdatetime_utcdatetime;
#[cfg(feature = "std")]
mod utcdatetime_systemtime;
