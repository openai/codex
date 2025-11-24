#[macro_export]
#[deprecated = "
This macro has no effect on any version of Serde released in the past 2 years.
It was used long ago in crates that needed to support Rustc older than 1.26.0,
or Emscripten targets older than 1.40.0, which did not yet have 128-bit integer
support. These days Serde requires a Rust compiler newer than that so 128-bit
integers are always supported.
"]
#[doc(hidden)]
macro_rules! serde_if_integer128 {
    ($($tt:tt)*) => {
        $($tt)*
    };
}
